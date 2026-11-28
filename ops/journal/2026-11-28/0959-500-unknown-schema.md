## 09:59 UTC #500 nightly: a previous regress.json of another schema counts as no previous
- Session: none (owner interactive session started this worker for #500; the issue was claimed for it at 09:59 UTC)
- Picked: the owner's request in the interactive session; #500 was `ready` in M0 (deferred from PR #499 R1.2) and already claimed with `in-progress`
- Did: PR #506: `cargo xtask nightly --previous` treats a file whose `schema` is an integer other than 1 as no previous (note `previous results <FILE> have schema <N>, this build reads schema 1`, no diff, no regression findings, the night goes on); not JSON, not an object, a non-integer schema (string, `null`, fraction, exponent, negative, over 2^64 - 1) or schema 1 with a bad member stays exit 1. `RESULTS_SCHEMA` shared by writer and reader; specs/tooling/nightly.md item 9 and self-check; tests changed and added (explained in the PR). Hand mutants: 5 of 6 caught, the digits-only check survives as the spec says. Live runs on macOS: schema 2 exit 0 with the note, own schema 1 file exit 0 with 0 changes, string schema exit 1. The pending lesson of #481 was left to PR #503, which already moves it.
- Result: strict lane; round 1: 1 minor, 1 nit fixed; round 2: 1 minor fixed; round 3: no findings; `ai-review` success posted on this commit, auto-merge on
- Cost: about 40% of the context; review rounds 3/4; regraded 0, overturned by the next reviewer 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` change)
- Lesson: a test that writes each bad case to one fixture path in a loop and then checks a later step on that path checks only the last case (R1.1); pending lesson file.
- Ignored: none
