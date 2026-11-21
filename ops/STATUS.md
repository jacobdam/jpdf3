# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #108 (#79 1/3): caller `Limits` clamped to the defaults in `Document::open`; new
  `max_retained_bytes_per_document`. Journal, STATUS and LESSONS are in the PR. #79 closes when it merges.

## Last session
- **#79 security** (rule d). Split by the sizing rule into #79 (1/3), #106 (decoder working memory and operations
  budget) and #107 (object resolve cache / charge each span once).
  - `Limits::clamped_to`; `Document::open` clamps first and keeps the clamped limits (`Document::limits()`).
  - ADR 0008: callers can only tighten limits. Spec: specs/core/limits-ceiling.md.
  - Review: 2 majors fixed. The default budgets sum to exactly 2 GiB with no headroom for uncharged memory: filed
    as **#109**.

## Next step (suggested order, TICK letters)
1. **Audit (rule g):** #108 is the 10th merge after e82b6a6, so the next session audits (unless main is red or an
   owner item appears). Audit housekeeping includes the Arlington pin bump through `jpdf-arlington vendor`.
2. d (security) issues, lowest number first: **#80, #81, #82, #83, #106, #107, #109**. Tighten the suites (#80,
   #81) and the contracts (#106) **before** #16 admits any adapter.
3. **#76**: harness honesty. The judge relies on it.
4. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #78, #84–#87, #91,
   #99–#101, #105.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next retro (rule h):** by 2026-11-28. It checks the success measures in docs/retros/0001.md.

## Blockers / owner-todo
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- macOS CI runs no Rust tests (`project-macos` runs xcodebuild/Playwright only), so macOS-only Rust code paths are
  checked locally only.
- No open proposals in ops/proposals/.
- Stale remote `t/*` branches can't be deleted by agents (the guard blocks remote deletes).
