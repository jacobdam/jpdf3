# Proposal: align AUTOMATION.md §4 with TICK's new priority order (retro 0001)

AUTOMATION.md isn't protected, but it isn't on the retro's allowed-change list (AUTOMATION §11)
either, so this asks the owner to apply the change.

## Problem
Retro 0001 (docs/retros/0001.md, root cause 1) moved due audits and retros ahead of the
backlog in ops/TICK.md §2. The old order starved them: the first audit ran after 26 merges
instead of 10. AUTOMATION.md §4 step 2 still shows the old order (audit = i and retro = j,
both after the backlog), so the two documents disagree.

## Proposed diff (AUTOMATION.md §4, step 2)
```diff
-   g. `ready` task in the current milestone (dependencies met, lowest number)
-   h. milestone backlog empty → run gate (§9) or generate next milestone's backlog
-   i. every 10th merge or 7 days → AUDIT session instead (§10)
-   j. retro due (7 days / gate passed / incident) → RETRO session instead (§11)
+   g. every 10th merge or 7 days → AUDIT session instead (§10)
+   h. retro due (7 days / gate passed / incident) → RETRO session instead (§11)
+   i. `ready` task in the current milestone (dependencies met, lowest number)
+   j. milestone backlog empty → run gate (§9) or generate next milestone's backlog
```

## Risk if not applied
A session that reads AUTOMATION instead of TICK picks the backlog over a due audit. TICK is
the per-session protocol and wins in practice, so the risk is low.
