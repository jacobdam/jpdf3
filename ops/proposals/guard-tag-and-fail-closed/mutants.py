"""Hand mutants of the patched guard: each must make --selftest fail.
Usage: mutants.py <patched guard.py> <empty scratch dir, outside /tmp>."""
import os, shutil, subprocess, sys

src_path = sys.argv[1]
src = open(src_path, encoding="utf-8").read()
MUTANTS = [
    ("arith skips heredoc", "if arith or delim[:1]", "if delim[:1]"),
    ("no arith frames", 'kind = "a" if i and s[i - 1] == "(" else "g"', 'kind = "g"'),
    ("arith never closes", 'arith -= stack.pop()[0] == "a"', "stack.pop()"),
    ("no $' quoting", "elif c == \"$\" and i + 1 < n and s[i + 1] in", "elif False and s[i + 1] in"),
    ("raw prefilter", "if not _GH_POST.search(flat):", "if not _GH_POST.search(cmd):"),
    ("no count check", "if shell_posts > checked or", "if False or"),
    ("no nested check", "or _codex_check(cmd, [2 * len(cmd) + 4096], 0, _gh_post_nested):", ":"),
    ("nested hit off", "return depth > 0 and _gh_post_at(words, i)", "return False"),
    ("nested hit at depth 0", "return depth > 0 and _gh_post_at(words, i)", "return _gh_post_at(words, i)"),
    ("fail open", "        return 2\n    return 0", "        return 0\n    return 0"),
    ("strict decode", 'encoding="utf-8", errors="replace"', 'encoding="utf-8"'),
    ("no secret rule", "if any(_SECRET_WORD.search(t) for t in toks if not re.search(r\"\\s\", t)):", "if False:"),
    ("secret rule on prose", "for t in toks if not re.search(r\"\\s\", t)):", "for t in toks):"),
    ("secret substring", '_SECRET_WORD = re.compile(r"(?:^|[/~<>=:])(?:\\.ssh|\\.codex|\\.config/gh)(?:/|$)")',
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
