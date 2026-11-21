# STATUS (rewritten by every session; ≤ 1 page)

Milestone: M0
Phase: bootstrap (pilot)

## In flight
- PR #98 (#50 items 1–2): the license check now splits production and test/build trees (auto-merge on). It
  includes journal, STATUS and LESSONS. Close #50 after it merges (owner: "close when items 1 and 2 are merged").

## Last session
- **#50 owner ruling** (rule a). `npm run licenses` holds the production tree to the strict list. That tree is every
  lockfile entry without `dev`, plus everything reachable from `dependencies` (installed manifests' and lockfile's
  edges).
  - Test/build tools may use MPL-2.0, BlueOak, CC0, 0BSD and NCSA. CC-BY-4.0 needs a named data package.
  - Shipped sources may import only `dependencies` or files inside `src/`.
  - Symlinks in `src/`, `#` subpaths, `public/` and Vite remap options are refused.
  - 33 node:test tests. Spec: specs/tooling/licenses.md. ADR 0002 item 5 and ADR 0005 item 2 amended.
  - Follow-ups: #99 (Rollup module-graph check), #100 (axe-core), #101 (Vite 8).

## Next step (suggested order, TICK letters)
1. **a: #60 owner ruling.** Write an ADR that supersedes ADR 0006, update specs/tooling/arlington.md, and choose
   between vendoring the raw TSVs and committing only the generated tables (smaller diff). Close #60 when it lands.
2. **#75 flaky** (rule e). It must be fixed within 5 sessions of audit 0001, and 3 have passed.
3. d (security) issues, lowest number first: **#77, #79, #80, #81, #82, #83**. Tighten the suites (#80, #81) and
   the contracts (#77, #79) **before** #16 admits any adapter.
4. **#76**: harness honesty. The judge relies on it.
5. Then the `ready` backlog (rule i), lowest number first: #21, #55, #61, #64, #66, #70, #84–#87, #91, #99–#101.
   - #14 closes after #55 and #70; #20 after #61; #22 after #64.
- **Next audit (rule g):** at the 10th merge after e82b6a6 (5 so far before #98), or by 2026-11-28.
- **Next retro (rule h):** by 2026-11-28. It checks the success measures in docs/retros/0001.md.

## Blockers / owner-todo
- #52: `sudo automationmodetool enable-automationmode-without-authentication` on the Mac mini.
- #21: the owner steps (stranger issue, /hold, HALT) go into a sub-issue when the drill is picked up.
- A local worktree `t/50-test-only-licenses` (`.claude/worktrees/license-amend`) predates PR #95, which merged the
  CONSTITUTION change. Agents leave it alone; the owner may delete it.
- Proposals for the owner: ops/proposals/pages-deploy-retry.md, ops/proposals/automation-priority-order.md (check
  whether #96/#97 made them obsolete).
- Stale remote `t/*` branches can't be deleted by agents (the guard blocks remote deletes).
