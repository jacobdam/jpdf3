"""Hand mutants of the patched guard: each must make --selftest fail.
Usage: mutants.py <patched guard.py> <empty scratch dir, outside /tmp>."""
import os, shutil, subprocess, sys

src_path = sys.argv[1]
src = open(src_path, encoding="utf-8").read()
MUTANTS = [
    ("arith skips heredoc", "if arith or sq or no_skip or not delim:", "if sq or no_skip or not delim:"),
    ("no arith frames", 'kind = "a" if i and s[i - 1] == "(" else "g"', 'kind = "g"'),
    ("arith never closes", 'in ("g", "a"):\n                    arith -= stack.pop()[0] == "a"\n                flush_cmd()',
     'in ("g", "a"):\n                    stack.pop()\n                flush_cmd()'),
    ("no $' quoting", "elif c == \"$\" and i + 1 < n and s[i + 1] in", "elif False and s[i + 1] in"),
    ("raw prefilter", "if not _GH_POST.search(flat):", "if not _GH_POST.search(cmd):"),
    ("no tokenizer post check", "                _check_post(words, j)", "                pass"),
    ("tokenizer word 0 only", "        for j in range(len(words)):\n            if _gh_post_at(words, j):",
     "        for j in range(1):\n            if _gh_post_at(words, j):"),
    ("no shlex post check", "            texts.extend(_check_post(toks, i))", "            pass"),
    ("$ kept in prefilter", "flat = re.sub(r\"\\$(?=['\\\"])\", \"\", cmd.replace(", "flat = (cmd.replace("),
    ("no nested check", "    if _codex_check(cmd, [2 * len(cmd) + 4096], 0, _gh_post_nested):", "    if False:"),
    ("nested hit off", "return words[i] == AMBIGUOUS or depth > 0 and _gh_post_at(words, i)", "return words[i] == AMBIGUOUS"),
    ("nested hit at depth 0", "return words[i] == AMBIGUOUS or depth > 0 and _gh_post_at(words, i)",
     "return words[i] == AMBIGUOUS or _gh_post_at(words, i)"),
    ("fail open", "        return 2\n    return 0", "        return 0\n    return 0"),
    ("strict decode", 'encoding="utf-8", errors="replace"', 'encoding="utf-8"'),
    ("no secret rule", "    if secret_word(cmd):", "    if False:"),
    ("secret rule on prose", 'for w in words if w and not re.search(r"\\s", w))', "for w in words if w)"),
    ("secret prefix narrowed", 'r"(?:^|[/~=:])(?:', 'r"(?:^|[/~])(?:'),
    ("secret no normpath", "_SECRET_WORD.search(posixpath.normpath(w))", "_SECRET_WORD.search(w)"),
    ("secret targets dropped", "_codex_commands(cmd, True)", "_codex_commands(cmd)"),
    ("secret raw prefilter", 'if not re.search(r"\\.(?:ssh|codex|config)", flat):',
     'if not re.search(r"\\.(?:ssh|codex|config)", cmd):'),
    ("${ swallowed", 'cur.append("${")  # scan on', 'cur.append(s[i:(s.find("}", i) + 1) or n]); i = (s.find("}", i) + 1 or n) - 2  # scan on'),
    ("heredoc delim quotes off", """elif ch in "'\\"" or (ch == "$" and""", """elif False and ch in "'\\"" or (ch == "$" and"""),
    ("heredoc delim drops all", 'delim = "".join(d)',
     """delim = s[j:k].replace("'", "").replace('"', "").replace("\\\\", "")"""),
    ("brace literal off", 'if brace and c in " \\t\\n;&|<>()}":', 'if False and c in " \\t\\n;&|<>()}":'),
    ("brace never closes", 'brace -= c == "}"', "pass"),
    ("brace kept in sub", 'words, cur, started, mode, skip, brace = [], [], False, "n", False, 0',
     'words, cur, started, mode, skip = [], [], False, "n", False'),
    ("backtick ignores ticks", "            if ticks:\n", "            if stack and stack[-1][0] == \"`\":\n"),
    ("$[ not arithmetic", "if arith or sq or no_skip or not delim:", "if arith or no_skip or not delim:"),
    ("delim $ quote off", """elif ch in "'\\"" or (ch == "$" and""", """elif ch in "'\\"" or (False and"""),
    ("delim keeps continuation", 'd.append(s[k].strip("\\n"))  # \\ + newline', 'd.append(s[k])  # \\ + newline'),
    ("ambiguous ignored", "return cmds + [[AMBIGUOUS]] if ambiguous else cmds", "return cmds"),
    ("open quote not ambiguous", 'ambiguous = no_skip or mode != "n" or', 'ambiguous = no_skip or'),
    ("secret substring", '_SECRET_WORD = re.compile(r"(?:^|[/~=:])(?:\\.ssh|\\.codex|\\.config/gh)(?:/|$)")',
     '_SECRET_WORD = re.compile(r"(?:\\.ssh|\\.codex|\\.config/gh)")'),
]
work = os.path.join(os.path.abspath(sys.argv[2]), "m", ".git")  # fake repo root: m/x/hooks/guard.py
os.makedirs(work, exist_ok=True)
hooks = os.path.join(os.path.dirname(work), "x", "hooks")
os.makedirs(hooks, exist_ok=True)
target = os.path.join(hooks, "guard.py")
bad = 0
for name, old, new in MUTANTS:
    if src.count(old) != 1:
        print("NOT APPLIED (%d matches): %s" % (src.count(old), name))
        bad += 1
        continue
    open(target, "w", encoding="utf-8").write(src.replace(old, new, 1))
    try:
        p = subprocess.run([sys.executable, "-I", target, "--selftest"], capture_output=True, text=True, timeout=60)
    except subprocess.TimeoutExpired:
        print("%-24s killed (selftest over 60 s)" % name, flush=True)
        continue
    fails = [l for l in p.stdout.splitlines() if l.startswith("FAIL")]
    status = "killed" if p.returncode != 0 else "SURVIVED"
    bad += p.returncode == 0
    print("%-24s %s %s" % (name, status, fails[0][:110] if fails else ""), flush=True)
shutil.rmtree(os.path.dirname(work))
print("mutants: %d/%d killed" % (len(MUTANTS) - bad, len(MUTANTS)))
sys.exit(1 if bad else 0)
