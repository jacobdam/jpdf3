## 00:20 UTC #323 nightly: accept jpdf-cli only when the runner may exec it
- Session: worker ready #323
- Picked: rule i because #323 is the lowest-numbered ready M0 issue with its dependencies closed (no ready product issue; design issue #357 says the group's issues stay workable meanwhile)
- Did: PR #364: `is_executable_file` (xtask/src/nightly.rs) keeps the regular-file check and asks `/bin/sh -c 'test -x "$0"'` instead of `mode & 0o111`; new test `only_a_cli_the_runner_can_exec_is_accepted` (0o610/0o601/0o611 refused, 0o500/0o710/0o755 accepted, each checked against `/bin/sh` exec too; the old check fails it at 0o610); spec item 10 updated; STATUS's pending lesson (PR #362) moved into ops/LESSONS.md.
- Result: `ai-review` success posted on this commit, auto-merge on.
- Cost: about 25% of the context; review rounds 1/4; regraded 0, overturned by the next reviewer 0; deferred: none (2 minors and 2 nits fixed as wording)
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: a shell script fixture with mode 0o100 can't be exec'd (the interpreter needs read), so an exec oracle for permission modes must give the owner read (0o500), unlike a binary.
- Ignored: none
