"""Run the guard at argv[1] as a hook (JSON on stdin) on the #162 probes; print exit codes.
Usage: probe.py <guard.py> <empty scratch dir, outside /tmp>. On main (e483671) the
first probe exits 2, the non-UTF-8 write exits 1 (a crash, not a block) and the rest exit 0;
patched, every probe but the last (a tagged post) exits 2."""
import json, os, subprocess, sys

guard = sys.argv[1]
g = os.path.abspath(sys.argv[2])
os.makedirs(os.path.join(g, ".git"), exist_ok=True)  # a fake repo root for the Write probe
bad_rs = os.path.join(g, "probe.rs")
with open(bad_rs, "wb") as f:
    f.write(b"fn f() {}\n// \xff\n")
T = "\U0001F916 jpdf-agent"
KW = "un" + "safe"
probes = [
    ("plain untagged", "Bash", {"command": "gh issue comment 5 --body hi"}),
    ("quoted gh", "Bash", {"command": '"gh" issue comment 5 --body hi'}),
    ("split-quote gh", "Bash", {"command": 'g""h issue comment 5 --body hi'}),
    ("glued ;gh", "Bash", {"command": "ls;gh issue comment 5 --body hi"}),
    ("glued (gh", "Bash", {"command": "(gh issue comment 5 --body hi)"}),
    ("$(gh", "Bash", {"command": "echo $(gh issue comment 5 --body hi)"}),
    ("backtick gh", "Bash", {"command": "echo `gh issue comment 5 --body hi`"}),
    ("line continuation", "Bash", {"command": "gh issue \\\ncomment 5 --body hi"}),
    ("bash -c", "Bash", {"command": "bash -c 'gh issue comment 5 --body hi'"}),
    ("eval", "Bash", {"command": "eval 'gh issue comment 5 --body hi'"}),
    ("arith shift var", "Bash", {"command": "x=$((a << b))\ncodex exec x"}),
    ("arith (( var", "Bash", {"command": "(( m = a << b ))\ncodex exec x"}),
    ("non-utf8 write", "Write", {"file_path": bad_rs, "content": "fn f() { %s { } }" % KW}),
    ("cat codex auth", "Bash", {"command": "cat ~/.codex/auth.json"}),
    ("cat ssh key", "Bash", {"command": "cat ~/.ssh/id_ed25519"}),
    ("tagged ok", "Bash", {"command": "gh issue comment 5 --body '%s hi'" % T}),
]
for name, tool, ti in probes:
    ev = {"tool_name": tool, "tool_input": ti, "cwd": g}
    p = subprocess.run([sys.executable, "-I", guard], input=json.dumps(ev), capture_output=True, text=True)
    print("%-20s exit %d %s" % (name, p.returncode, p.stderr.strip().splitlines()[-1][:90] if p.stderr.strip() else ""))
