# Milestone Mn: pre-close checklist

<!-- Written by the Planner session that runs ops/TICK.md §9 step 4's checklist, in its
Planner PR. Every item links its evidence (a run, a file or directory at the judged commit, a
command, an issue or PR). Write only what is true when the PR merges: the tag is pushed later,
by the close stage, so say where it goes, not that it exists. -->

- Checklist run: <YYYY-MM-DD HH:MM UTC>, session <journal entry>
- Judged commit: <sha> (the tag `mN` goes on it when the close stage runs)

## (a) Fresh gate
- Run: <URL of the `ci.yml` schedule or workflow_dispatch run>, head <sha>, concluded success
- `gates.<Mn>.pass`: true
- Commits on origin/main after the judged commit: none / only record commits: <shas>

## (b) Deliverables (PLAN.md §8, the Mn row)
| Deliverable | Evidence on main |
|---|---|
| <deliverable as PLAN names it> | <path, command, test or closed issue and its PR> |

## (c) Issues that left Mn
| Issue | Now | How it left | Reason (link) | On the exit path? |
|---|---|---|---|---|
| #<n> | <milestone, or closed> | <moved / closed not planned / duplicate / closed with no merged PR> | <comment link> | no |

## (d) Exit criteria not measured by the gate
| Criterion (PLAN.md §8) | Evidence |
|---|---|
| <criterion> | <file, run or closed issue> |
