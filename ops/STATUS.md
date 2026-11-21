# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #103 (#75 flaky): the process-group kill test retries when the wrapper hadn't started. Auto-merge on; includes
  journal, STATUS and LESSONS. #75 closes when it merges.

## Last session
- **#75 flaky** (rule e). The test warms the script up, writes the pid atomically and retries at 3/6/12/24 s; the
  `sleep` is now a real grandchild, and the timeout has an upper bound. Hand-made mutants (no group kill, `pkill -P`)
  fail it. PR #102 (#60, Arlington vendoring) merged before this session.
- Arlington consequence for #61 and later `jpdf-cos` tests: use `jpdf-arlington` as a **dev-dependency from
  `tests/` only** (spec item 10).

## Next step (suggested order, TICK letters)
1. d (security) issues, lowest number first: **#77, #79, #80, #81, #82, #83**. Tighten the suites (#80, #81) and
   the contracts (#77, #79) **before** #16 admits any adapter.
2. **#76**: harness honesty. The judge relies on it.
3. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #84–#87, #91, #99–#101.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next audit (rule g):** at the 10th merge after e82b6a6 (8 with #102 and #103), or by 2026-11-28.
  - Audit housekeeping now includes the Arlington pin: bump it through `jpdf-arlington vendor` (spec item 8).
- **Next retro (rule h):** by 2026-11-28. It checks the success measures in docs/retros/0001.md.

## Blockers / owner-todo
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- No open proposals in ops/proposals/.
- Stale remote `t/*` branches can't be deleted by agents (the guard blocks remote deletes).
