# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #102 (#60): the Arlington TSVs are vendored, and their git ids are checked offline (auto-merge on). It includes
  journal, STATUS and LESSONS. #60 closes when it merges.

## Last session
- **#60 owner ruling** (rule a).
  - ADR 0007 supersedes ADR 0006. The raw TSVs (493 KB) are vendored in `tools/arlington/data/` with upstream
    `LICENSE` and `NOTICE.txt`. The generated tables (2.4 MB) stay in `target/`.
  - Loading the data recomputes the git tree and blob ids (own SHA-1) and compares them with the pins. So
    `cargo xtask arlington` and `ci linux` are now offline.
  - `jpdf-arlington vendor --out DIR` re-vendors when the pin is bumped (audit time).
  - A test enforces "tests and tools only": `cargo tree -i` over every target and feature, plus a scan of shipped
    sources.
  - Review: 1 major and 6 minors in the first round, all fixed. The delta review's 2 new minors and 5 nits were
    also fixed (no third review round).
  - Consequence for #61 and later `jpdf-cos` tests: use `jpdf-arlington` as a **dev-dependency from `tests/`
    only**. `#[cfg(test)]` modules in `src/` and any mention of Arlington in engine sources fail the guard (spec
    item 10).

## Next step (suggested order, TICK letters)
1. **#75 flaky** (rule e). It must be fixed within 5 sessions of audit 0001, and 4 have passed. **Do it next.**
2. d (security) issues, lowest number first: **#77, #79, #80, #81, #82, #83**. Tighten the suites (#80, #81) and
   the contracts (#77, #79) **before** #16 admits any adapter.
3. **#76**: harness honesty. The judge relies on it.
4. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #84–#87, #91, #99–#101.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next audit (rule g):** at the 10th merge after e82b6a6 (6 so far before #102), or by 2026-11-28.
  - Audit housekeeping now includes the Arlington pin: bump it through `jpdf-arlington vendor` (spec item 8).
- **Next retro (rule h):** by 2026-11-28. It checks the success measures in docs/retros/0001.md.

## Blockers / owner-todo
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- No open proposals in ops/proposals/.
- Stale remote `t/*` branches can't be deleted by agents (the guard blocks remote deletes).
