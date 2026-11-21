# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #110 (#80): conformance suite tightening (auto-merge on). Journal and STATUS are in the PR. #80 closes when it
  merges.

## Last session
- **#80 security** (rule d). PR #108 (#79 1/3) merged during it.
  - Shared `Boundary` helper: at / one below / **0** for every output limit (flate, crypto, image, font).
  - Generated flate vectors (long distances > 64 KiB, mixed stored/fixed/dynamic), checked with CPython zlib.
  - RFC 6229 long offsets, PKCS#7 known answers, more bad-padding probes, uneven hash splits.
  - Review: 1 minor and 6 nits, all fixed.

## Next step (suggested order, TICK letters)
1. **Audit (rule g) is due now:** #108 was the 10th merge after e82b6a6. Audit housekeeping includes the Arlington pin
   bump through `jpdf-arlington vendor`.
2. d (security) issues, lowest number first: **#81, #82, #83, #106, #107, #109**. Tighten the image suite (#81) and
   the contracts (#106) **before** #16 admits any adapter.
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
