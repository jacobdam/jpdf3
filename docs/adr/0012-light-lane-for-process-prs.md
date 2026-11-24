# 12. A light lane for process PRs, and ADR 0011's not-yet-firing tasks moved out of M0

Status: proposed (owner interactive session 2026-11-24, issue #326); accepted when the owner
merges its PR. Amends ADR 0010 (rounds for light-lane PRs) and ADR 0011 (§1 triggers, §9 tasks).

## Context
M0 started 2026-11-19. Its judge gate passes (nightly 45179218903, 2026-11-23), but its open
count grows: 103 issues were filed in M0 from 2026-11-20 to 11-24 (24, 20, 31, 10, 18 a day)
and 76 closed (14, 19, 16, 15, 12), leaving 27 open. By origin (hand count from the titles and
labels, session 2026-11-24):

| Origin | Filed | Open |
|---|---|---|
| ADR 0011 tasks, their splits and scheduler follow-ups | 26 | 9 |
| Review and audit discoveries | 25 | 4 |
| Owner-todo, proposals, guard and Codex launcher | 19 | 4 |
| PLAN §8's M0 row | 19 | 4 |
| Splits of PLAN items (#300–#306) | 7 | 6 |
| Audits and retros | 7 | 0 |

ADR 0011 planned 8 tasks; they became 16 task issues plus 8 scheduler follow-ups, and 15 of the
38 merges since 2026-11-22. The work isn't hard; each piece is multiplied by rules made for the
PDF engine:
1. **Tests.** The template's self-check (retro 0002) asks, for every filter, gate, parser or
   rule, a should-reject and a near-miss case per term, a parse case per real line shape and a
   state table for each loop. One scheduler sentence (#314, a 24-hour window) needs about 10
   fixtures.
2. **Review.** Up to 4 rounds (ADR 0010); 47% of reviewed PRs needed 3 or more (retro 0002),
   most of them filters, gates and scheduler rules.
3. **Findings.** Every minor that isn't a quick fix becomes an issue in the current milestone
   (TICK §4 step 4, §5), so a process PR leaves issues behind in M0 (PR #321: #322, #323; PR
   #318: #319).
4. **Scope.** ADR 0011 §9 filed every task in M0, including session types whose triggers can't
   fire until there is product: QC (a spec `Status: implemented`), Design: UI ("before the M4
   backlog opens"), the user advocate (a journey assigned to M4 or later passing on main), and
   the mechanical area threshold, which needs product areas with history.

The process code is cheap to get wrong: `cargo xtask next-session` is read-only (ADR 0011 §2), a
wrong pick costs one session and the next one sees it, a broken scheduler falls back to the TICK
rules, and the protected judge and the guard don't depend on it. The PDF engine, the apps, the
guard and the owner-instruction rules are not cheap to get wrong.

## Decision

### 1. The light lane
A PR is in the **light lane** when every file it changes is owned (longest prefix, read from
**origin/main's** `ops/areas.toml`, so a PR can't widen its own lane) by the `process` or
`records` area (so any change to `xtask/src/main.rs`, which routes the judge's `ci` and
`nightly` commands, is strict), and it changes none of the **strict list**:
- the paths `.claude/`, `ops/driver/`, `ops/judge/`, `ops/rulesets/` (protected anyway),
  `ops/proposals/`, `ops/areas.toml`, `CONSTITUTION.md`, `LICENSE`, `CLAUDE.md`,
  `docs/adr/0010-review-rounds-converge.md` and this ADR;
- in any file: who counts as the owner or which text is an instruction (TICK §0's first bullets,
  §2 rule a, ADR 0011 §7, the owner-feed filters); the tag, guard or protected-path rules;
  security severity, rule d's order or the deferral of security findings (ADR 0009); the review
  rules (TICK §4) or this lane.

Every other PR is in the **strict lane**: today's size, test and review rules, unchanged (§2's
filing rule applies to both lanes). The lane belongs to the issue: an issue whose earlier PR ran
strict stays strict. The PR description's first line after the tag says `Lane: light (ADR 0012)`
or `Lane: strict`. The reviewer of each PR's first round, whatever its K, checks it and answers
`lane: light accepted` or `lane: strict, because <file or term>`; a strict answer moves the PR
to the strict lane for the rest of its rounds (K continues) and is final. A PR over 1500 added
lines that this moves to the strict lane is split before its next round or its `ai-review`,
whichever comes first (TICK §0).

In the light lane:
| Rule | Strict lane (unchanged) | Light lane |
|---|---|---|
| Size (TICK §0) | ≤ 1500 added lines | ≤ 3000 added lines, still one issue per PR |
| Tests (TICK §3 step 2, the template's self-check) | every term: a should-reject and a near-miss case; a case per line shape; state tables | each pick, trigger or rule it adds or changes: one test that selects it and one that doesn't (the nearest miss the author sees); one parse-error case per new input. The PR description answers one line instead of the filter/gate/parser items: "if this is wrong, what wrong pick follows, and which session notices" |
| Review rounds (TICK §4, ADR 0010) | 4, then step 6 | **2**: a delta round 2 runs if round 1's triage fixed, disputed or regraded a blocker, or disputed or regraded a major, or if the fixes since round 1 fail step 6's post-cap test (over 100 added lines, or a new file) or change any file outside this section's light-lane definition or touch the strict list (then the PR is strict from round 2 on). After round 2, §4 step 6 applies as it does after round 4 (post-cap diff, final check, design escalation), with round 2 in place of round 4: the final check is round 3, and K ≥ 4 is the design session's or the owner's call |
| Majors | fixed in the PR | fixed in the PR; round 2 doesn't run for a major's fix alone |
| Minors and nits | quick fix, else an issue (`deferred: #N`) | quick fix, else `dropped (light lane, ADR 0012)`: no issue |
| AUTOMATION.md | updated in the same PR | TICK.md is updated in the PR; each AUTOMATION.md line it makes false is listed in the PR description as `AUTOMATION.md now false: §<n> <quoted line>`, and the next retro rewrites them (TICK §7 step 4) |

Everything else stays: the fresh reviewer, the rubric, posting every review, the triage
comment, `ai-review`, record commits and auto-merge.

### 2. Where process issues go: the `Ops` milestone
A discovery, deferral or follow-up about process-area work (any lane) is filed in the
milestone **`Ops`**, not the current one, unless:
1. it blocks the current milestone's gate or its last step (for M0: the judge gate and #21), or
   the next milestone's first issues;
2. it is a wrong pick or a failure of `cargo xtask next-session` seen in live use (a session
   was sent to the wrong work, or the scheduler failed);
3. it is `crash` or `security` (ADR 0009 unchanged: `sev:critical`/`sev:high` in the current
   milestone for rule d, `sev:medium`/`sev:low` in M10);
4. it is about a strict-list subject (§1: the guard, the owner feed, `ops/proposals/`, the
   judge, the review rules).

Until #308's `area:` labels exist, "process-area" is judged by the files the issue would
change, against `ops/areas.toml`. A session that finds no `Ops` milestone creates it (the
command in §4) before filing; if that command fails, it files in the current milestone with the
body line `Move to: Ops (ADR 0012 §2)`, and the Planner moves it.

`Ops` is not an `M<n>` milestone, so step 10 never picks from it and it never holds a
milestone's tag. The Planner (TICK §9 step 2) moves an `Ops` issue into the current milestone
when term 1 or 2 becomes true or a retro asks for it, and keeps it `ready`-gated like any
other. Retros report the `Ops` count and its growth.

### 3. ADR 0011 amended
- **§1, triggers.** "Never a session's judgement" keeps holding for every trigger the scheduler
  computes, but the **Planner** may file a `design` issue labeled `ready` by judgement for a
  group of related issues it sees (a follow-up chain of 3 or more, or 5 or more open issues on
  one subsystem), listing the group in its body. The mechanical area threshold (§4: the
  threshold, both replays, `design-pending`, the per-area bound) and the Gardener wait (task 4
  part 3, #247): they are built when a retro finds a group the Planner missed for 7 days (TICK §7 step 4 counts
  them: a chain of 3 or more follow-ups, or 5 or more open issues on one subsystem, with no
  `design` issue filed within 7 days of the group forming).
- **§9, tasks.** Task 4 part 2 (#308: `area:`/`origin:` labels, the filing rule, per-area
  metrics) moves to **M1**, where product areas start to have issues. Task 4 part 3 (#247) moves
  to `Ops`, labeled `deferred`. Task 5, QC (#248), moves to **M1** and depends on #308 instead of
  #247. Task 7, Design: UI (#250), moves to **M3** (it must exist before M4's backlog opens).
  Task 8, the user advocate (#251), moves to **M4** and depends on #308 instead of #247. Until
  task 8 lands, the audit keeps AUTOMATION.md §5.3 and §5.4 (ADR 0011 §1 already says so).
- **§9, "each task updates TICK and AUTOMATION.md"**: a light-lane task follows §1's
  AUTOMATION.md row.
- ADR 0011's success check (the memory-chain replay) moves with #247.

### 4. One-time moves after the merge
The first Planner run after this ADR merges creates the milestone (`gh api
repos/jacobdam/jpdf3/milestones -f title=Ops -f description="Process work that no milestone
gate needs (ADR 0012 §2)"`), makes §3's moves with a comment on each, and applies §2 to the open
M0 issues. By §2 these leave M0 as of 2026-11-24: #313, #314 and #319 (process follow-ups,
none blocking the gate) and #328 (the lane-aware metrics; retros hand-count until one of them asks
the Planner to pull it in, §2) to `Ops`. These stay: #196 (`security`, `sev:high`, §2 term 3), #195 and #179 (about
`ops/proposals/` and the owner feed, §2 term 4) and #208 (#21 needs it). That takes M0 from 27
open issues (not counting #326, which this PR closes, or #328) to 19, before new inflow.

## Consequences
- Process PRs cost about one review round and leave no minor-finding issues; more process
  mistakes reach main. They show up as wrong picks, which §2 term 2 keeps in the current
  milestone so they are fixed first.
- The template's filter/gate/parser self-check stays for the strict lane: the guard, the
  owner-feed rules, product parsers and CI gates keep it.
- The work-mix budget (ADR 0011 §3) counts sessions on `Ops` issues as non-milestone work, so
  more process work makes the Planner add product issues sooner.
- `Ops` can grow without bound. Nothing picks from it, so its issues cost nothing until a
  Planner or a retro pulls one; a retro that finds it over 40 open issues closes the ones no
  longer true, with a comment.
- A PR can claim the light lane wrongly; the first-round reviewer's lane line catches it, and
  each audit checks every light-lane PR since the last audit against the strict list (TICK §6)
  and reports one that should have been strict as a finding.
- `cargo xtask metrics` doesn't know the lanes yet: it counts a final check only at round 5 and
  "at the cap" at 4 rounds. Until #328 teaches it the `Lane:` line, retros count light-lane PRs'
  rounds, caps and final checks by hand from the PR descriptions and review headers, and read the
  tool's cap and final-check shares as strict-lane numbers that light-lane PRs dilute.

## Success check (retro 0003, due by 2026-11-30, then retro 0004)
- M0's open count falls: at most 19 plus the M0 issues filed since this merge, and M0 issues
  filed per day below M0 issues closed per day over the period.
- Light-lane PRs (hand count from the `Lane:` lines, until #328): median review rounds ≤ 1, and
  no light-lane PR files a minor-finding issue.
- Wrong picks: at most 2 `next-session` wrong-pick or failure issues (§2 term 2) per 7 days.
- No audit finds a `sev:high` or worse problem introduced by a light-lane PR.

If the wrong-pick or audit checks fail at retro 0003, that retro reverts §1's table to the
strict lane (TICK §4) and keeps §2 and §3.
