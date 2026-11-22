#!/usr/bin/env python3
"""PreToolUse guard for jpdf agent sessions. PROTECTED (CONSTITUTION rule 12).

First layer only: CI (the judge, incl. owner-signed protected changes) is the real enforcement.
Exit 0 = allow, exit 2 = block (reason on stderr). Any crash exits 1, which does NOT block,
so a guard bug can never lock a session.

Owner override: start Claude with JPDF_OWNER=1 in the environment. Agents can't set it,
because hooks inherit the Claude process environment, not a Bash command's environment.

Self-test: python3 -I .claude/hooks/guard.py --selftest
Compatible with Python 3.9 (macOS system python).
"""
import json
import os
import re
import shlex
import subprocess
import sys
import time

REPO = "jacobdam/jpdf3"
PROTECTED = (
    "CONSTITUTION.md", "LICENSE", ".github/", "ops/judge/", "ops/driver/", "ops/rulesets/",
    "deny.toml", "supply-chain/", "clippy.toml", ".cargo/", ".config/nextest.toml",
    ".claude/settings.json", ".claude/settings.local.json", ".claude/hooks/",
)
SECRET_READ = ("/.ssh/", "/.config/gh/", "/.codex/")
# Codex tripwire: does a Bash command run `codex` as a command? A small one-pass shell tokenizer
# instead of a regex, so there is no backtracking. It finds the command word of every simple command
# (after ; & | newline ( ) { $( ) and backticks, also inside double quotes), skipping VAR=value
# words, keywords, redirections, heredoc bodies and wrappers with their options (sudo -u x,
# timeout -s KILL 5, env -i, xargs -I {}, npx -y, pnpm dlx ...), and looks inside `bash -c '...'`,
# `eval ...` and after find's -exec. Quotes and backslashes are removed, so `co''dex` and `\codex`
# count. Recursion shares one character budget (2x the command + 4 KiB), so the total work stays
# linear; running out of budget, or nesting deeper than three levels, blocks. Not a sandbox: `c=codex; $c`, `echo codex | sh` and other
# launchers still pass (see ops/proposals/codex-second-reviewer.md, Risks).
CODEX_WRAPPERS = {  # wrapper -> its single-letter options that take a separate argument
    "sudo": "ughpcCDrtU", "doas": "uC", "env": "uCS", "exec": "a", "nohup": "", "command": "",
    "builtin": "", "time": "", "nice": "n", "timeout": "sk", "gtimeout": "sk", "caffeinate": "tw",
    "xargs": "ILnPsdEa", "npx": "p", "bunx": "", "npm": "", "yarn": "", "pnpm": "", "dlx": "",
    "coproc": "", "stdbuf": "ioe",
}
CODEX_SHELLS = ("bash", "sh", "zsh", "dash", "ksh")
CODEX_KEYWORDS = ("then", "do", "else", "elif", "if", "while", "until", "function", "!", "{", "}")
CODEX_MAX_WORDS = 64  # words looked at per command start; bounds the work per start
_CODEX_ASSIGN = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\+?=")


def _codex_commands(s):
    """Split shell text into simple commands: lists of words with quotes and backslashes removed.
    One pass; `$(`/backtick substitutions are nested commands (state saved on a stack), single
    quotes are literal, `#` at a word start begins a comment, redirection targets are dropped and
    heredoc bodies are skipped."""
    cmds, words, cur, started, skip = [], [], [], False, False
    mode, stack, heredocs = "n", [], []  # mode: n = unquoted, d = inside "...", s = inside '...'
    i, n = 0, len(s)

    def flush_word():
        nonlocal cur, started, skip
        if started and not skip:
            words.append("".join(cur))
        if started:
            skip = False
        cur, started = [], False

    def flush_cmd():
        nonlocal words, skip
        flush_word()
        if words:
            cmds.append(words)
        words = []
        skip = False

    def open_sub(closer):
        nonlocal words, cur, started, mode, skip
        stack.append((closer, mode, words, cur, started, skip))
        words, cur, started, mode, skip = [], [], False, "n", False

    def close_sub():
        nonlocal words, cur, started, mode, skip
        flush_cmd()
        _, mode, words, cur, _, skip = stack.pop()
        started = True

    while i < n:
        c = s[i]
        if mode == "s":
            if c == "'":
                mode = "n"
            else:
                cur.append(c)
            i += 1
            continue
        if c == "\\" and i + 1 < n:
            if s[i + 1] != "\n":
                cur.append(s[i + 1])
                started = True
            i += 2
            continue
        if c == "$" and s.startswith("$(", i):
            open_sub(")")
            i += 2
            continue
        if c == "`":
            if stack and stack[-1][0] == "`":
                close_sub()
            else:
                open_sub("`")
            i += 1
            continue
        if mode == "d":
            if c == '"':
                mode = "n"
            else:
                cur.append(c)
            i += 1
            continue
        if c in " \t":
            flush_word()
        elif c == "'":
            mode, started = "s", True
        elif c == '"':
            mode, started = "d", True
        elif c == "#" and not started:
            j = s.find("\n", i)
            i = n if j < 0 else j
            continue
        elif c == "$" and s.startswith("${", i):
            j = s.find("}", i)
            j = n if j < 0 else j + 1
            cur.append(s[i:j])
            started = True
            i = j
            continue
        elif c in "<>" or (c == "&" and s.startswith("&>", i)):
            if started and "".join(cur).isdigit():
                cur, started = [], False  # a file descriptor number: 2>&1
            flush_word()
            j = i
            while j < n and s[j] in "<>&|":
                j += 1
            op = s[i:j]
            if j < n and op.endswith("&") and (s[j].isdigit() or s[j] == "-"):
                while j < n and (s[j].isdigit() or s[j] == "-"):
                    j += 1
            elif op in ("<<", "<<-"):
                if j < n and s[j] == "-":
                    op, j = "<<-", j + 1
                while j < n and s[j] in " \t":
                    j += 1
                k = j
                while k < n and s[k] not in " \t\n;&|<>()":
                    k += 1
                delim = s[j:k].replace("'", "").replace('"', "").replace("\\", "")
                if delim[:1].isdigit() or delim[:1] in ("", "$", "("):
                    cur.append(op)  # arithmetic shift (1<<3, $((1 << n))), not a heredoc
                    started = True
                    i = j
                    continue
                heredocs.append((delim, op == "<<-"))
                j = k
            else:
                skip = True  # the next word is the redirection target
            i = j
            continue
        elif c in ";&|\n":
            flush_cmd()
            if c == "\n" and heredocs:
                j = i + 1
                for delim, tabs in heredocs:
                    while j < n:
                        e = s.find("\n", j)
                        e = n if e < 0 else e
                        line = s[j:e]
                        j = e + 1
                        if (line.lstrip("\t") if tabs else line) == delim:
                            break
                heredocs = []
                i = j
                continue
        elif c == "(":
            flush_cmd()
            stack.append(("g", mode, None, None, False, False))
        elif c == ")":
            if stack and stack[-1][0] == ")":
                close_sub()
            else:
                if stack and stack[-1][0] == "g":
                    stack.pop()
                flush_cmd()
        else:
            cur.append(c)
            started = True
        i += 1
    while stack:
        if stack[-1][0] == "g":
            stack.pop()
        else:
            close_sub()
    flush_cmd()
    return cmds


def _codex_runs(words, i, budget, depth, limit=CODEX_MAX_WORDS):
    """True if the simple command starting at words[i] runs codex (looking at most `limit` words)."""
    end = min(len(words), i + limit)
    while i < end:
        w = words[i]
        low = w.lower()
        base = low.rsplit("/", 1)[-1]
        if base.split("@", 1)[0] == "codex" and not base.startswith("@"):
            return True
        if low == "eval":
            return _codex_nested(" ".join(words[i + 1:end]), budget, depth)
        if low in CODEX_KEYWORDS or _CODEX_ASSIGN.match(w):
            i += 1
        elif base in CODEX_SHELLS:
            i, want_c = i + 1, False
            while i < end and words[i][:1] in "-+" and len(words[i]) > 1:
                opt = words[i]
                i += 1
                if opt == "--":
                    break
                if opt.startswith("--"):
                    continue
                want_c = want_c or "c" in opt
                if opt[-1] in "oO":
                    i += 1  # -o pipefail, -O extglob
            if want_c:
                return i < len(words) and _codex_nested(words[i], budget, depth)
        elif base in CODEX_WRAPPERS:
            argopts = CODEX_WRAPPERS[base]
            i += 1
            while i < end and words[i].startswith("-") and words[i] != "-":
                opt = words[i]
                i += 1
                if opt == "--":
                    break
                letters = opt[1:]
                if not opt.startswith("--") and letters[-1:] in tuple(argopts) and all(
                        ch not in argopts for ch in letters[:-1]):
                    i += 1  # -u x, -iu x
            if base in ("timeout", "gtimeout") and i < end and words[i][:1].isdigit():
                i += 1
        else:
            return False
    return False


def _codex_nested(text, budget, depth):
    """Check text run by `bash -c` or `eval`, charging the shared budget; out of budget blocks."""
    budget[0] -= len(text) + 1
    if budget[0] < 0 or depth >= 3:
        return True  # too deep or too much nested text: block
    return _codex_check(text, budget, depth + 1)


def _codex_check(text, budget, depth):
    for words in _codex_commands(text):
        if _codex_runs(words, 0, budget, depth):
            return True
        for j, w in enumerate(words):
            if j and w in ("-exec", "-execdir", "-ok", "{") and _codex_runs(words, j + 1, budget, depth, 8):
                return True
    return False


def codex_command(cmd):
    """True if the Bash command `cmd` runs codex. Linear: one tokenizer pass per text, and nested
    texts (bash -c, eval) draw on one budget of 2x the command plus 4 KiB."""
    flat = cmd.replace("\\\n", "").replace("'", "").replace('"', "").replace("\\", "").lower()
    if "codex" not in flat:
        return False
    return _codex_check(cmd, [2 * len(cmd) + 4096], 0)
# Every issue/PR/comment the agent writes starts with this tag. The agent shares the owner's
# GitHub account; ops/driver/owner-issues.sh treats only UNTAGGED owner text as instructions.
TAG = "\U0001F916 jpdf-agent"
DENIED_LINTS = ("unsafe_code", "unwrap_used", "expect_used", "panic", "indexing_slicing",
                "unimplemented", "todo", "unreachable", "dbg_macro", "exit", "mem_forget",
                "cast_possible_truncation", "cast_sign_loss", "cast_possible_wrap")
HOME = os.path.expanduser("~")


class Block(Exception):
    pass


# ---------------------------------------------------------------- paths
def repo_rel(path, cwd):
    """Path relative to the git working tree containing it (works for worktrees), or None."""
    p = os.path.normpath(os.path.join(cwd or os.getcwd(), os.path.expanduser(path)))
    d = p if os.path.isdir(p) else os.path.dirname(p)
    while True:
        if os.path.exists(os.path.join(d, ".git")):
            return os.path.relpath(p, d).replace(os.sep, "/")
        parent = os.path.dirname(d)
        if parent == d:
            return None
        d = parent


def is_protected_rel(rel):
    if rel is None or rel.startswith("../"):
        return False
    return any(rel == p.rstrip("/") or (p.endswith("/") and rel.startswith(p)) for p in PROTECTED)


def is_home_claude(path, cwd):
    """~/.claude config is off-limits, except per-project memory notes (~/.claude/projects/*/memory/)."""
    p = os.path.normpath(os.path.join(cwd or os.getcwd(), os.path.expanduser(path)))
    base = os.path.join(HOME, ".claude")
    if re.match(re.escape(os.path.join(base, "projects")) + r"/[^/]+/memory(/|$)", p):
        return False
    return p == base or p.startswith(base + os.sep)


def check_write_path(path, cwd):
    if is_home_claude(path, cwd):
        raise Block("writing under ~/.claude is not allowed for agents (CONSTITUTION 12)")
    rel = repo_rel(path, cwd)
    if is_protected_rel(rel):
        raise Block("'%s' is a protected path (CONSTITUTION 12). Write ops/proposals/<topic>.md instead." % rel)


# ---------------------------------------------------------------- rust content
_COMMENT = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)
_STRING = re.compile(r'r(#*)".*?"\1|b?"(?:\\.|[^"\\])*"', re.S)
_UNSAFE = re.compile(r"\bunsafe\b")
_IGNORE = re.compile(r"#\[ignore\b(?!\s*=\s*\"https://github\.com/jacobdam/jpdf3/issues/\d+\")")
_ALLOW = re.compile(r"\b(allow|expect|warn)\s*\(([^)]*)\)")


def strip_code(src):
    return _STRING.sub('""', _COMMENT.sub(" ", src))


def check_rust(new_text, old_text=""):
    code = strip_code(new_text)
    if _UNSAFE.search(code):
        raise Block("`unsafe` is forbidden in all first-party code (CONSTITUTION 3)")
    for m in _ALLOW.finditer(code):
        names = re.findall(r"[a-z_]+", m.group(2))
        bad = [n for n in names if n in DENIED_LINTS]
        if bad:
            raise Block("relaxing denied lint(s) %s is forbidden (CONSTITUTION 5)" % ", ".join(bad))
    if _IGNORE.search(new_text):
        raise Block('#[ignore] needs = "https://github.com/jacobdam/jpdf3/issues/<n>" (CONSTITUTION 14)')
    if "forbid(unsafe_code)" in old_text and "forbid(unsafe_code)" not in new_text:
        raise Block("removing #![forbid(unsafe_code)] is forbidden (CONSTITUTION 3)")


def check_cargo_toml(new_text):
    for lint in DENIED_LINTS:
        if re.search(r"\b%s\s*=\s*(\"(allow|warn)\"|\{[^}]*level\s*=\s*\"(allow|warn)\")" % lint, new_text):
            raise Block("relaxing lint '%s' in Cargo.toml is forbidden (CONSTITUTION 5)" % lint)
    if re.search(r"overflow-checks\s*=\s*false", new_text):
        raise Block("overflow-checks must stay on")


# ---------------------------------------------------------------- bash
_SEGMENT_SPLIT = re.compile(r"\|\||&&|;|\||\n")
WRITE_CMDS = {"tee", "sed", "perl", "cp", "mv", "rm", "truncate", "chmod", "chown", "ln",
              "install", "rsync", "touch", "dd", "patch"}


def protected_tokens(tokens, cwd):
    out = []
    for t in tokens:
        if t.startswith("-"):
            continue
        rel = repo_rel(t, cwd)
        if is_protected_rel(rel) or is_home_claude(t, cwd):
            out.append(t)
    return out


def check_bash(cmd, cwd):
    c = " ".join(cmd.split())
    low = c.lower()
    if "exam.tsv" in low:
        raise Block("the exam corpus is sealed (CONSTITUTION 13)")
    if "--dangerously-skip-permissions" in low or "settings.local.json" in low or "disableallhooks" in low:
        raise Block("changing Claude permissions/hooks is not allowed")
    if re.search(r"\bgh\s+auth\s+(token|refresh|login|logout|setup-git)\b|--show-token", c):
        raise Block("credential commands are not allowed")
    if re.search(r"(api|uploads)\.github\.com", low):
        raise Block("call GitHub only through `gh` (no direct API URLs)")
    post_text = check_gh_posts(cmd)
    inert_c = inert_view(cmd, post_text)
    # The cmux socket has no password (owner's choice; risk accepted in #130). This block is a
    # plain text match, not a barrier: quoting tricks (c''mux, c\mux) dodge it. Sessions are kept
    # off the owner's terminal tabs by CONSTITUTION and TICK, which forbid going around the guard.
    # The word may appear only as quoted, inert title/body text of a gh post; anywhere else
    # (incl. bash -c, $(...), grep) it is blocked.
    if re.search(r"\bcmux\b", inert_c.lower()):
        raise Block("cmux control is not allowed: it can type into other terminals, including the owner's")
    # The full command, not the inert view: quoted gh text is never a command word to the
    # tokenizer, and stripping it can be fooled (#130).
    if codex_command(cmd):
        raise Block("run Codex only through ops/driver/codex-review.sh (fixed sandbox flags)")
    if re.search(r"\bosascript\b", low) and re.search(r"keystroke|key code|do script|system events", low):
        raise Block("scripting keystrokes into other apps is not allowed")
    if re.search(r"\bsecurity\s+(find-|dump-keychain|export)", c):
        raise Block("keychain access is not allowed")
    if re.search(r"\bgit\s+config\b.*\bcredential", c):
        raise Block("changing git credentials is not allowed")
    for m in re.finditer(r"\bgit\s+push\b([^;&|]*)", c):
        args = m.group(1)
        if re.search(r"(\s|^)(-f|--force|--force-with-lease|--force-if-includes|--mirror|--delete|-d)(\s|=|$)", args) \
                or re.search(r"\s\+\S", args):
            raise Block("force/delete pushes are not allowed")
        if re.search(r"(\s|:)(main|master)(\s|$)", args):
            raise Block("pushing to main is not allowed; open a PR")
    if re.search(r"\bgh\s+pr\s+merge\b[^;&|]*--admin", c):
        raise Block("admin merges are not allowed")
    if re.search(r"\bgh\s+repo\s+(create|delete|fork|edit|rename|archive)\b", c):
        raise Block("repository management is not allowed")
    for m in re.finditer(r"\bgh\s[^;&|]*?(?:\s-R|--repo)[=\s]+(\S+)", c):
        if m.group(1).strip("'\"") != REPO:
            raise Block("only %s may be targeted (CONSTITUTION 2)" % REPO)
    for m in re.finditer(r"\bgh\s+api\b([^;&|]*)", c):
        args = m.group(1)
        write = re.search(r"(-X|--method)[=\s]*(POST|PUT|PATCH|DELETE)", args, re.I) or \
            re.search(r"\s(-f|-F|--field|--raw-field|--input)[\s=]", args)
        if "graphql" in args and "mutation" in low:
            raise Block("GraphQL mutations are not allowed; use REST on %s" % REPO)
        if write:
            paths = re.findall(r"(?:^|\s)/?(repos/\S+|orgs/\S+|user\S*|gists\S*|\S*/\S*)", args)
            target = next((p for p in paths if p.startswith(("repos/", "orgs/", "user", "gists"))), "")
            if not target.startswith("repos/%s/" % REPO):
                raise Block("GitHub writes are only allowed to repos/%s (CONSTITUTION 2)" % REPO)
            if re.search(r"/(rulesets|branches/[^/\s]+/protection|collaborators|keys|hooks|actions/secrets|"
                         r"actions/permissions|environments|invitations)", target):
                raise Block("changing repo security settings is not allowed")
            if re.search(r"/(contents|git)/", target + "/"):
                raise Block("committing through the API is not allowed; use git in a worktree")
            if re.search(r"/(issues|pulls)(/|$)|/comments(/|$)|/reviews(/|$)", target):
                raise Block("write issues/PRs/comments with `gh issue|pr ... --body \"%s ...\"`, not gh api" % TAG)
    if re.search(r"\bcargo\s+vet\s+(certify|trust|add-exemption|regenerate|prune|import|record-violation)", c):
        raise Block("cargo vet audits are imported only (CONSTITUTION 9)")
    if re.search(r"\bunsafe\b", inert_c) and re.search(r"\.rs\b", inert_c) and re.search(r"(>|\btee\b|sed\s+-i|perl\s+-[a-z]*i)", inert_c):
        raise Block("writing `unsafe` into Rust sources is forbidden (CONSTITUTION 3)")
    for seg in _SEGMENT_SPLIT.split(cmd):
        seg = seg.strip()
        if not seg:
            continue
        for m in re.finditer(r"\d?>>?\s*([^\s;&|]+)", seg):
            t = m.group(1)
            if t.startswith("&") or t.startswith("/dev/"):
                continue
            if protected_tokens([t], cwd):
                raise Block("redirecting into a protected path is not allowed (CONSTITUTION 12)")
        try:
            toks = shlex.split(seg)
        except ValueError:
            toks = seg.split()
        while toks and ("=" in toks[0] and not toks[0].startswith("-")) or (toks and toks[0] in ("sudo", "env", "command", "nohup", "time")):
            toks = toks[1:]
        if not toks:
            continue
        prog = os.path.basename(toks[0])
        args = toks[1:]
        if prog == "git" and args[:1] in (["apply"], ["am"]) and not any(a in ("--check", "--stat", "--numstat", "--summary") for a in args):
            raise Block("git apply/am can rewrite protected files unseen; edit files directly instead")
        if prog == "git" and args[:1] and args[0] in ("update-index", "commit-tree", "hash-object", "read-tree", "mktree",
                                                       "fast-import", "filter-branch", "filter-repo", "replace"):
            raise Block("git plumbing that builds commits/trees is not allowed (CONSTITUTION 12)")
        if prog == "patch":
            raise Block("patch can rewrite protected files unseen; edit files directly instead")
        if prog == "git" and args[:1] in (["rm"], ["mv"], ["checkout"], ["restore"]):
            if protected_tokens(args[1:], cwd):
                raise Block("modifying protected paths via git is not allowed (CONSTITUTION 12)")
        if prog == "git" and args[:1] in (["commit"], ["add"], ["stash"]):
            bad = [f for f in changed_files(cwd) if is_protected_rel(f)]
            if bad:
                raise Block("protected paths are modified in this worktree: %s. Revert them "
                            "(git checkout/restore) and use ops/proposals/ instead." % ", ".join(bad[:5]))
        if prog in WRITE_CMDS:
            if prog == "sed" and not any(a.startswith("-i") or a == "--in-place" for a in args):
                continue
            if prog == "perl" and not any(re.match(r"-[a-zA-Z]*i", a) for a in args):
                continue
            targets = args
            if prog == "cp":
                targets = [a for a in args if not a.startswith("-")][-1:]
            if protected_tokens(targets, cwd):
                raise Block("modifying protected paths is not allowed (CONSTITUTION 12)")


GH_POST_VERBS = {
    "issue": {"create", "comment", "edit", "close", "reopen"},
    "pr": {"create", "comment", "edit", "close", "reopen", "review"},
}
GH_BODY_FLAGS = {"-b", "--body", "-c", "--comment"}
GH_TEXT_FLAGS = {"-t", "--title"}
GH_NO_FLAGS = {"--body-file", "-F", "--fill", "-f", "--fill-first", "--fill-verbose", "--editor", "-e",
               "--web", "-w", "--template", "-T", "--recover"}


def check_gh_posts(cmd):
    """Agent text on GitHub must be tagged (see TAG), so it can never pass as the owner's."""
    if not re.search(r"\bgh\s+(issue|pr)\s+(%s)\b" % "|".join(sorted(GH_POST_VERBS["pr"] | GH_POST_VERBS["issue"])), cmd):
        return []
    texts = []
    try:
        toks = shlex.split(cmd)
    except ValueError:
        raise Block("could not parse this gh command; use one simple `gh ... --body \"%s ...\"`" % TAG)
    for i, t in enumerate(toks):
        if os.path.basename(t) != "gh" or i + 2 >= len(toks):
            continue
        group, verb = toks[i + 1], toks[i + 2]
        if verb not in GH_POST_VERBS.get(group, ()):
            continue
        if group == "pr" and verb == "review":
            raise Block("PR reviews are not allowed (the account is the owner's); post the ai-review status instead")
        bodies = []
        j = i + 3
        while j < len(toks) and not re.fullmatch(r"&&|\|\||;|\|", toks[j]):
            a = toks[j]
            flag = a.split("=", 1)[0]
            if flag in GH_NO_FLAGS:
                raise Block("gh %s %s: %s is not allowed; pass the text inline with --body \"%s ...\"" % (group, verb, flag, TAG))
            if flag in GH_BODY_FLAGS and (group, flag) != ("pr", "-c") or (flag in ("-c", "--comment") and verb in ("close", "reopen")):
                if "=" in a and a.startswith("--"):
                    bodies.append(a.split("=", 1)[1])
                elif j + 1 < len(toks):
                    bodies.append(toks[j + 1])
                    j += 1
            elif flag in GH_TEXT_FLAGS:
                if "=" in a and a.startswith("--"):
                    texts.append(a.split("=", 1)[1])
                elif j + 1 < len(toks):
                    texts.append(toks[j + 1])
                    j += 1
            j += 1
        if verb in ("create", "comment") and not bodies:
            raise Block("gh %s %s needs --body \"%s ...\" (CONSTITUTION 1: agent text is tagged)" % (group, verb, TAG))
        for b in bodies:
            if not b.lstrip().startswith(TAG):
                raise Block("agent text on GitHub must start with \"%s\" (owner-issues.sh relies on it)" % TAG)
        texts.extend(bodies)
    return texts


def strip_inert_text(cmd, texts):
    """Remove one quoted occurrence of each gh title/body that the shell passes through unexpanded:
    '...' (any content; a ' inside written as '\\''), or "..." with no $, `, \\ or ".
    Anything else stays and is checked."""
    for t in texts:
        if not t:
            continue
        forms = ["'%s'" % t.replace("'", "'\\''")]
        if not re.search(r'[$`\\"]', t):
            forms.append('"%s"' % t)
        for q in forms:
            if q in cmd:
                cmd = cmd.replace(q, "''", 1)
                break
    return cmd


_SINGLE_GH_POST = re.compile(
    r" *gh (issue|pr) (comment|create|edit)( +(?!=)([A-Za-z0-9_./:=@,+-]+|''|--[a-z-]+=''))* *")


def inert_view(cmd, texts):
    """The whitespace-collapsed command with its inert gh titles/bodies removed, but only if what
    remains is one plain gh post: spaces between words the shell reads literally and '' placeholders,
    nothing else (no newline, tab, quote, `#` comment, metacharacter or non-ASCII), so no second
    command can run a stripped copy of the text (bash -c, eval, a comment that ends inside a
    quoted body). Otherwise the full command, collapsed."""
    stripped = strip_inert_text(cmd, texts)
    if _SINGLE_GH_POST.fullmatch(stripped):
        return " ".join(stripped.split())
    return " ".join(cmd.split())


def changed_files(cwd):
    """Staged, unstaged and untracked paths (repo-relative) in the worktree at cwd."""
    try:
        p = subprocess.run(["git", "status", "--porcelain", "--untracked-files=all"], cwd=cwd or None,
                           capture_output=True, text=True, timeout=10)
        out = []
        for line in p.stdout.splitlines():
            path = line[3:].split(" -> ")[-1].strip().strip('"')
            if path:
                out.append(path)
        return out
    except Exception:
        return []


# ---------------------------------------------------------------- dispatch
def check(event):
    tool = event.get("tool_name", "")
    ti = event.get("tool_input") or {}
    cwd = event.get("cwd") or os.getcwd()
    if tool == "Read":
        fp = ti.get("file_path", "")
        if fp.endswith("exam.tsv") and "ops/judge/corpus" in fp:
            raise Block("the exam corpus is sealed (CONSTITUTION 13)")
        full = os.path.normpath(os.path.expanduser(fp))
        if any(s in full + "/" for s in SECRET_READ):
            raise Block("reading credentials is not allowed")
        return
    if tool in ("Edit", "Write", "MultiEdit", "NotebookEdit"):
        fp = ti.get("file_path") or ti.get("notebook_path") or ""
        check_write_path(fp, cwd)
        if tool == "Write":
            new, old = ti.get("content", ""), (open(fp).read() if os.path.isfile(fp) else "")
        elif tool == "Edit":
            new, old = ti.get("new_string", ""), ti.get("old_string", "")
        elif tool == "MultiEdit":
            edits = ti.get("edits") or []
            new = "\n".join(e.get("new_string", "") for e in edits)
            old = "\n".join(e.get("old_string", "") for e in edits)
        else:
            new, old = ti.get("new_source", ""), ""
        if fp.endswith(".rs"):
            check_rust(new, old)
        if os.path.basename(fp) == "Cargo.toml":
            check_cargo_toml(new)
        return
    if tool == "Bash":
        check_bash(ti.get("command", ""), cwd)


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--selftest":
        sys.exit(selftest())
    if os.environ.get("JPDF_OWNER") == "1":
        sys.exit(0)
    try:
        event = json.load(sys.stdin)
    except Exception:
        sys.exit(0)
    try:
        check(event)
    except Block as b:
        print("jpdf guard: BLOCKED: %s" % b, file=sys.stderr)
        sys.exit(2)
    sys.exit(0)


# ---------------------------------------------------------------- self-test
def selftest():
    root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    R = lambda p: os.path.join(root, p)  # noqa: E731
    cases = [
        # (tool, input, should_block)
        ("Bash", {"command": "codex exec -s danger-full-access 'x'"}, True),
        ("Bash", {"command": "/opt/homebrew/bin/codex exec x"}, True),
        ("Bash", {"command": "/opt/homebrew/Caskroom/codex/0.171.0/bin/codex exec x"}, True),
        ("Bash", {"command": "FOO=1 codex review"}, True),
        ("Bash", {"command": "FOO=\"a b\" codex exec"}, True),
        ("Bash", {"command": "FOO=" + "a" * 300 + " codex exec x"}, True),
        ("Bash", {"command": "env " + "A=1 " * 40 + "codex exec x"}, True),
        ("Bash", {"command": "bash -c 'codex exec x'"}, True),
        ("Bash", {"command": "bash -lc 'FOO=1 codex exec x'"}, True),
        ("Bash", {"command": "ls && codex exec"}, True),
        ("Bash", {"command": "x=1;codex exec"}, True),
        ("Bash", {"command": "a=b&&codex exec"}, True),
        ("Bash", {"command": "ls\ncodex exec x"}, True),
        ("Bash", {"command": "{ codex exec x; }"}, True),
        ("Bash", {"command": "(codex exec x)"}, True),
        ("Bash", {"command": "if true; then codex exec; fi"}, True),
        ("Bash", {"command": "case x in x) codex exec;; esac"}, True),
        ("Bash", {"command": "echo $(codex exec x)"}, True),
        ("Bash", {"command": "out=$(codex exec -s danger-full-access 'x')"}, True),
        ("Bash", {"command": "out=`codex exec x`"}, True),
        ("Bash", {"command": "echo \"result: $(codex exec x)\""}, True),
        ("Bash", {"command": "\\codex exec"}, True),
        ("Bash", {"command": "co''dex exec"}, True),
        ("Bash", {"command": "xargs -0 codex exec"}, True),
        ("Bash", {"command": "env -i codex exec"}, True),
        ("Bash", {"command": "env -u X codex exec"}, True),
        ("Bash", {"command": "nice -n 5" + " " * 300 + "codex exec"}, True),
        ("Bash", {"command": "sudo -u x codex exec"}, True),
        ("Bash", {"command": "timeout 5 codex exec"}, True),
        ("Bash", {"command": "timeout -s KILL -k 5 10 codex exec"}, True),
        ("Bash", {"command": "exec -a x codex exec"}, True),
        ("Bash", {"command": "coproc codex exec"}, True),
        ("Bash", {"command": "caffeinate -i codex exec"}, True),
        ("Bash", {"command": "/usr/bin/time codex exec"}, True),
        ("Bash", {"command": "find . -exec codex exec {} +"}, True),
        ("Bash", {"command": "npx @openai/codex exec"}, True),
        ("Bash", {"command": "pnpm dlx @openai/codex exec"}, True),
        ("Bash", {"command": "xargs -I {} codex exec {}"}, True),
        ("Bash", {"command": "xargs -P 4 codex exec"}, True),
        ("Bash", {"command": "env -C x codex exec"}, True),
        ("Bash", {"command": "sudo -u me -H codex exec"}, True),
        ("Bash", {"command": "sudo -iu me codex exec"}, True),
        ("Bash", {"command": "bash -euo pipefail -c 'codex exec'"}, True),
        ("Bash", {"command": "bash -c -- 'codex exec'"}, True),
        ("Bash", {"command": "eval 'codex exec'"}, True),
        ("Bash", {"command": "npx -y @openai/codex@0.171.0 exec"}, True),
        ("Bash", {"command": "npm exec @openai/codex"}, True),
        ("Bash", {"command": "yarn dlx @openai/codex"}, True),
        ("Bash", {"command": ">log codex exec"}, True),
        ("Bash", {"command": "2>&1 codex exec"}, True),
        ("Bash", {"command": "function f { codex exec; }"}, True),
        ("Bash", {"command": "coproc foo { codex exec; }"}, True),
        ("Bash", {"command": "c\\\nodex exec"}, True),
        ("Bash", {"command": "cat > target/scratch/b.md <<'EOF'\ndon't touch x\nEOF\ncodex exec - < target/scratch/b.md"}, True),
        ("Bash", {"command": "x=$((1<<3))\ncodex exec x"}, True),
        ("Bash", {"command": "(( m = 1 << 4 ))\ncodex exec"}, True),
        ("Bash", {"command": "eval eval eval eval codex exec"}, True),
        ("Read", {"file_path": os.path.join(HOME, ".codex/auth.json")}, True),
        ("Bash", {"command": "ops/driver/codex-review.sh target/scratch/review-1.md target/scratch/codex-1.json"}, False),
        ("Bash", {"command": "./ops/driver/codex-review.sh a b"}, False),
        ("Bash", {"command": "cat ops/proposals/codex-second-reviewer.md"}, False),
        ("Bash", {"command": "grep -rn codex ."}, False),
        ("Bash", {"command": "grep -rn 'codex' ."}, False),
        ("Bash", {"command": "echo \"codex is fine\""}, False),
        ("Bash", {"command": "echo a=b codex"}, False),
        ("Bash", {"command": "x=\"a=b codex\""}, False),
        ("Bash", {"command": "echo $(date) codex"}, False),
        ("Bash", {"command": "grep -c '(codex)' docs/audits/0003.md"}, False),
        ("Bash", {"command": "git commit -m 'journal: Codex ok; (codex) finding fixed'"}, False),
        ("Bash", {"command": "git commit -m \"journal\n\ncodex: ok\""}, False),
        ("Bash", {"command": "gh issue create -t x --body \"🤖 jpdf-agent finding (codex): `foo` overflows\""}, False),
        ("Bash", {"command": "git log --grep codex"}, False),
        ("Bash", {"command": "ls ~/.codex-review-notes ${HOME}/codex.txt"}, False),
        ("Bash", {"command": "jq . target/scratch/codex-12.json  # codex output"}, False),
        ("Bash", {"command": "git commit -m \"$(cat <<'EOF'\nfix x\n\nCodex found the overflow; Opus agreed.\nFound by Codex (codex) review.\nEOF\n)\""}, False),
        ("Bash", {"command": "cat <<EOF > notes.md\n(codex) finding\nfoo; codex bar\nEOF\nls"}, False),
        ("Bash", {"command": "python3 - <<'EOF'\ncodex = 1\nEOF"}, False),
        ("Bash", {"command": "cargo test 2>&1 | grep codex"}, False),
        ("Bash", {"command": "echo $((1<<2)) codex"}, False),
        ("Write", {"file_path": R("ops/judge/verdict.py"), "content": "x"}, True),
        ("Edit", {"file_path": R("CONSTITUTION.md"), "old_string": "a", "new_string": "b"}, True),
        ("Write", {"file_path": R(".claude/settings.local.json"), "content": "{}"}, True),
        ("Write", {"file_path": R(".github/workflows/x.yml"), "content": ""}, True),
        ("Write", {"file_path": os.path.join(HOME, ".claude/settings.json"), "content": "{}"}, True),
        ("Write", {"file_path": os.path.join(HOME, ".claude/projects/x/memory/note.md"), "content": "n"}, False),
        ("Write", {"file_path": os.path.join(HOME, ".claude/projects/x/settings.json"), "content": "n"}, True),
        ("Write", {"file_path": R("crates/jpdf-syntax/src/lexer.rs"), "content": "fn f(){ unsafe { } }"}, True),
        ("Write", {"file_path": R("crates/jpdf-syntax/src/lexer.rs"), "content": "// unsafe is banned\nfn f() { let s = \"unsafe\"; }"}, False),
        ("Edit", {"file_path": R("crates/jpdf-syntax/src/lib.rs"), "old_string": "#![forbid(unsafe_code)]\n", "new_string": ""}, True),
        ("Write", {"file_path": R("crates/a/src/x.rs"), "content": "#[allow(clippy::unwrap_used)]\nfn f(){}"}, True),
        ("Write", {"file_path": R("crates/a/src/x.rs"), "content": "#[allow(dead_code)]\nfn f(){}"}, False),
        ("Write", {"file_path": R("crates/a/src/x.rs"), "content": "#[test]\n#[ignore]\nfn t(){}"}, True),
        ("Write", {"file_path": R("crates/a/src/x.rs"), "content": "#[ignore = \"https://github.com/jacobdam/jpdf3/issues/12\"]\nfn t(){}"}, False),
        ("Write", {"file_path": R("Cargo.toml"), "content": "[workspace.lints.clippy]\nunwrap_used = \"allow\"\n"}, True),
        ("Write", {"file_path": R("PLAN.md"), "content": "ok"}, False),
        ("Write", {"file_path": R("rust-toolchain.toml"), "content": "[toolchain]\nchannel = \"1.101.0\"\n"}, False),
        ("Read", {"file_path": R("ops/judge/corpus/exam.tsv")}, True),
        ("Read", {"file_path": R("ops/judge/verdict.py")}, False),
        ("Read", {"file_path": "~/.ssh/id_ed25519"}, True),
        ("Bash", {"command": "cat ops/judge/corpus/exam.tsv | head"}, True),
        ("Bash", {"command": "git push --force origin t/12-x"}, True),
        ("Bash", {"command": "git push origin main"}, True),
        ("Bash", {"command": "git push origin HEAD:main"}, True),
        ("Bash", {"command": "git push -u origin t/12-domain-x"}, False),
        ("Bash", {"command": "gh pr merge 5 --squash --admin"}, True),
        ("Bash", {"command": "gh pr merge 5 --auto --squash"}, False),
        ("Bash", {"command": "gh auth token"}, True),
        ("Bash", {"command": "gh issue create -R someone/else -t hi -b x"}, True),
        ("Bash", {"command": "cp -R /tmp/a /tmp/b"}, False),
        ("Bash", {"command": "ls -R /tmp && gh pr list -R other/repo"}, True),
        ("Bash", {"command": "gh issue create -R jacobdam/jpdf3 -t hi -b x"}, True),
        ("Bash", {"command": "gh issue create -R jacobdam/jpdf3 -t hi -b '%s x'" % TAG}, False),
        ("Bash", {"command": "gh api -X PUT repos/jacobdam/jpdf3/rulesets/1 -f x=y"}, True),
        ("Bash", {"command": "gh api repos/other/repo/issues -f title=x"}, True),
        ("Bash", {"command": "gh api repos/jacobdam/jpdf3/statuses/abc -f state=success -f context=ai-review"}, False),
        ("Bash", {"command": "gh api repos/jacobdam/jpdf3/issues?state=open"}, False),
        ("Bash", {"command": "echo hi > ops/judge/x.py"}, True),
        ("Bash", {"command": "cat ops/judge/CONTRACT.md > $TMPDIR/c.md"}, False),
        ("Bash", {"command": "sed -i '' 's/a/b/' deny.toml"}, True),
        ("Bash", {"command": "sed -n 1,5p deny.toml"}, False),
        ("Bash", {"command": "cp /tmp/x .claude/hooks/guard.py"}, True),
        ("Bash", {"command": "cp ops/judge/README.md /tmp/x"}, False),
        ("Bash", {"command": "rm -rf supply-chain"}, True),
        ("Bash", {"command": "cargo vet certify foo 1.0"}, True),
        ("Bash", {"command": "cargo nextest run --workspace"}, False),
        ("Bash", {"command": "echo 'unsafe { }' >> crates/a/src/x.rs"}, True),
        ("Bash", {"command": "claude --dangerously-skip-permissions -p hi"}, True),
        ("Bash", {"command": "git checkout -- ops/judge/verdict.py"}, True),
        ("Bash", {"command": "git checkout -b t/1-x"}, False),
        ("Bash", {"command": "security find-generic-password -s x"}, True),
        ("Bash", {"command": "git apply /tmp/p.diff"}, True),
        ("Bash", {"command": "git apply --check /tmp/p.diff"}, False),
        ("Bash", {"command": "git am < x.mbox"}, True),
        ("Bash", {"command": "patch -p1 < x.diff"}, True),
        ("Bash", {"command": "git update-index --cacheinfo 100644,abc,ops/judge/x.py"}, True),
        ("Bash", {"command": "git hash-object -w f"}, True),
        ("Bash", {"command": "gh api -X PUT repos/jacobdam/jpdf3/contents/ops/judge/x.py -f message=m"}, True),
        ("Bash", {"command": "gh api repos/jacobdam/jpdf3/git/blobs -f content=x"}, True),
        ("Bash", {"command": "curl -H x https://api.github.com/repos/jacobdam/jpdf3/issues"}, True),
        ("Bash", {"command": "cmux send --surface surface:2 'rm -rf ~'"}, True),
        ("Bash", {"command": "bash -c 'cmux send x'"}, True),
        ("Bash", {"command": "grep -rn cmux ."}, True),
        ("Bash", {"command": "gh issue comment 5 --body '%s is the cmux socket password set?'" % TAG}, False),
        ("Bash", {"command": "gh issue comment 5 --body \"%s is the cmux socket password set?\"" % TAG}, False),
        ("Bash", {"command": "gh issue edit 5 -t 'cmux socket password'"}, False),
        ("Bash", {"command": "gh issue comment 5 --body \"%s $(cmux send x)\"" % TAG}, True),
        ("Bash", {"command": "gh issue comment 5 --body \"%s `cmux send x`\"" % TAG}, True),
        ("Bash", {"command": "gh issue comment 5 --body '%s cmux'; cmux send x" % TAG}, True),
        ("Bash", {"command": "cmux send x; gh issue edit 5 -t 'cmux send x'"}, True),
        ("Bash", {"command": "bash -c 'cmux send x'; gh issue edit 5 -t 'cmux send x'"}, True),
        ("Bash", {"command": "osascript -e 'tell application \"System Events\" to keystroke \"x\"'"}, True),
        ("Bash", {"command": "gh issue comment 5 --body 'done'"}, True),
        ("Bash", {"command": "gh issue comment 5 --body '%s done'" % TAG}, False),
        ("Bash", {"command": "gh pr comment 5 --body '%s a.rs: n > 1, no unsafe added'" % TAG}, False),
        ("Bash", {"command": "gh pr comment 5 --body '%s it'\\''s a.rs: n > 1, unsafe; cmux'" % TAG}, False),
        ("Bash", {"command": "gh pr comment 5 --body='%s a.rs > b, unsafe'" % TAG}, False),
        ("Bash", {"command": "gh pr comment 5 --body '%s x'; echo 'unsafe {}' > a.rs" % TAG}, True),
        ("Bash", {"command": "gh pr comment 5 --body '%s a.rs > b' && echo unsafe > a.rs" % TAG}, True),
        ("Bash", {"command": "gh pr comment 5 --body \"%s $(echo unsafe > a.rs)\"" % TAG}, True),
        ("Bash", {"command": "bash -c '%s; echo unsafe > a.rs'; gh pr comment 5 --body \"%s; echo un\"\"safe > a.rs\"" % (TAG, TAG)}, True),
        ("Bash", {"command": "eval '%s; echo unsafe > a.rs'; gh pr comment 5 --body \"%s; echo un\"\"safe > a.rs\"" % (TAG, TAG)}, True),
        ("Bash", {"command": "bash -c '%s; cmux send y'; gh issue comment 5 --body \"%s; cm\"\"ux send y\"" % (TAG, TAG)}, True),
        ("Bash", {"command": "gh pr edit 5 --title s'ed -i s/x/unsafe/ a.rs' --body '%s x'\nbash -c 'sed -i s/x/unsafe/ a.rs'" % TAG}, True),
        ("Bash", {"command": "gh pr edit 5 --title cm'ux send y' --body '%s x'\nbash -c 'cmux send y'" % TAG}, True),
        ("Bash", {"command": "gh pr comment 5 # --body '%s x\necho unsafe > a.rs #'" % TAG}, True),
        ("Bash", {"command": "gh pr edit 5 # --title 'x\ncmux send y #'"}, True),
        ("Bash", {"command": "gh pr comment 5 --body '%s x'\ncmux send y" % TAG}, True),
        ("Bash", {"command": "gh pr comment 5 --body '%s x'\rsed -i s/x/unsafe/ a.rs" % TAG}, True),
        ("Bash", {"command": "gh issue comment 5 --body-file /tmp/b.md"}, True),
        ("Bash", {"command": "gh issue create -t x -b \"%s\nline2; with semicolon\"" % TAG}, False),
        ("Bash", {"command": "gh issue create -t x -l human"}, True),
        ("Bash", {"command": "gh pr create --draft --fill"}, True),
        ("Bash", {"command": "gh pr create --draft -t x -B main -b \"%s body\"" % TAG}, False),
        ("Bash", {"command": "gh pr create --draft -t x -B main -b \"$(cat /tmp/b)\""}, True),
        ("Bash", {"command": "gh issue close 5 -c 'fixed'"}, True),
        ("Bash", {"command": "gh issue close 5"}, False),
        ("Bash", {"command": "gh issue edit 5 --add-label ready"}, False),
        ("Bash", {"command": "gh pr review 5 --approve"}, True),
        ("Bash", {"command": "gh api repos/jacobdam/jpdf3/issues/5/comments -f body=x"}, True),
        ("Bash", {"command": "gh pr merge 5 --auto --squash --body 'x'"}, False),
    ]
    failures = 0
    for tool, ti, should in cases:
        try:
            check({"tool_name": tool, "tool_input": ti, "cwd": root})
            blocked = False
        except Block:
            blocked = True
        if blocked != should:
            failures += 1
            print("FAIL: %s %s -> blocked=%s, expected %s" % (tool, json.dumps(ti)[:100], blocked, should))
    # inert_view keeps the full view unless the remainder is one plain gh post (each barrier alone).
    views = [
        ("gh pr comment 5 --body 'x y'", ["x y"], "gh pr comment 5 --body ''"),
        ("gh pr comment 5 --body 'x y'\nls", ["x y"], "gh pr comment 5 --body 'x y' ls"),
        ("gh pr comment 5 a'b' --body 'x y'", ["x y"], "gh pr comment 5 a'b' --body 'x y'"),
        ("gh pr comment 5 #a --body 'x y'", ["x y"], "gh pr comment 5 #a --body 'x y'"),
    ]
    # every character outside the word class, a glued placeholder, a widened --flag='', a word
    # starting with = (zsh EQUALS expansion), another verb, and text before gh: all keep the full view
    for bad in [";", "|", "&", "`", "$", ">", "<", "(", ")", "\\", "~", "*", "?", "{", "}", "[", "!",
                "\t", "\n", "\r", '"', "%", "^"]:
        views.append(("gh pr comment 5 a%sb --body 'x y'" % bad, ["x y"], None))
    views += [
        ("gh pr comment 5 a'x y'", ["x y"], None),
        ("gh pr edit 5 --x=a\n--y='x y'", ["x y"], None),
        ("gh pr edit 5 --x;y='x y'", ["x y"], None),
        ("gh pr comment 5 =ls --body 'x y'", ["x y"], None),
        ("gh pr merge 5 --body 'x y'", ["x y"], None),
        ("echo gh pr comment 5 --body 'x y'", ["x y"], None),
        ("gh issue create --title 'x y' --body=''", ["x y", ""], "gh issue create --title '' --body=''"),
    ]
    views = [(c, t, " ".join(c.split()) if w is None else w) for c, t, w in views]
    for cmd, texts, want in views:
        cases.append(("inert_view", {"command": cmd}, want))
        got = inert_view(cmd, texts)
        if got != want:
            failures += 1
            print("FAIL: inert_view(%r) -> %r, expected %r" % (cmd, got, want))
    # The Codex check stays linear: 300 kB adversarial inputs that contain "codex", so the fast path
    # doesn't skip them (the tokenizer reads the whole command before checking).
    pads = ("(" * 300000, "$(" * 150000, "`" * 300000, ";a=" * 100000, "a=b " * 75000,
            "/codex " * 43000, "((codexx" * 37500, "sudo -x " * 37500, "bash -c " * 37500,
            "'codex' " * 37500, "\\" * 300000, "${" * 150000, "-exec sudo " * 27000,
            "<<a\n" * 75000, "> " * 150000, ("{ eval '" + "`a`" * 5000 + "' ") * 20)
    nested = "a " * 50000 + "echo codex"  # bash -c behind four -exec at each of three levels
    for _ in range(3):
        nested = "sudo -exec sudo -exec sudo -exec sudo -exec bash -c " + shlex.quote(nested)
    pads += (nested,)
    for pad in pads:
        t0 = time.monotonic()
        codex_command("echo " + pad + " codex")
        if time.monotonic() - t0 > 2.0:
            failures += 1
            print("FAIL: codex_command took %.1fs on %d chars" % (time.monotonic() - t0, len(pad)))
    listed = os.path.join(root, "ops/judge/protected.txt")
    if os.path.isfile(listed):
        want = sorted(l.strip() for l in open(listed) if l.strip() and not l.startswith("#"))
        if want != sorted(PROTECTED):
            failures += 1
            print("FAIL: PROTECTED differs from ops/judge/protected.txt")
    print("guard selftest: %d/%d passed" % (len(cases) - failures, len(cases)))
    return 1 if failures else 0


if __name__ == "__main__":
    main()
