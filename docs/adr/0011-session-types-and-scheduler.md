# 11. Session types, a scheduler, and bounded escalation to design

Status: proposed (owner issue #238, 2026-11-22). Needs the owner's approval before merge: the PR
carries `owner-todo` and no auto-merge. Absorbs #224 (the architect role becomes the
**design: technical** session type). Implementation tasks: §9, filed only after this ADR merges.

## Context
The loop discovers and fixes, one issue per session, and each session picks its own work from
ops/TICK.md §2 rules a–j. Nothing in it notices groups of related problems, designs them, sets
principles, controls how much work is in flight or tests independently. The owner found the
patterns by hand twice: #155 (review rounds, which became ADR 0010) and the memory-budget chain
#109 → #146 → #154 → #156 → #165 → #168 → #187 → #231 (which became #236). On 2026-11-22 the
project is still in M0, started 2026-11-19; the owner's count of the last 80 merged PRs (#238):
24 process/tooling, 18 journal-only, 7 audits, 3 retros, 8 memory/limits and about 20
product-ish ones, most of those hardening follow-ups.

Why the current loop can't fix this (5 whys, short):
1. **Self-selection.** Rules a–j are prose read by the session that then does the work. The
   picker has no counters, so a session can't tell that an area has produced its sixth
   follow-up, or that most recent sessions went to process work.
2. **One context does everything.** The session that writes the code also decides what to work
   on and how to grade it. The only fresh context in the loop is the reviewer subagent, and it
   sees one diff, never a group of issues.
3. **No design step.** A hard problem reaches review as a narrow diff. Review rounds then debate
   the design one finding at a time (PR #151: 15 rounds; PR #183, #186, #227, #232: the cap),
   and ADR 0010's only exit after the cap is the owner.
4. **Counters count the wrong thing.** Audit cadence counts every merge, including the
   journal-only record PRs (18 of the last 80), so the loop audits bookkeeping.

## Decision

### 1. Session types, not roles
A **session type** is defined by five things, and nothing else:
1. **Trigger:** a mechanical condition computed by a script from labels, git and GitHub, never
   a session's judgement.
2. **Context:** the files and issues it loads. The context window is the scarce resource, and a
   fresh context is the only real source of independence.
3. **Allowed outputs:** what it may change (for example ADRs only, no code; tests only, no
   product code). A PR outside its type's allowed outputs fails review as a blocker.
4. **Exit artifact:** the one thing the next session consumes.
5. **Bound:** every loop has a counter and a final state.

Work is split across two sessions only when the contexts don't fit together, when independence
is needed (no session grades its own work), or when outputs need a scope.

| Type | Trigger | Context | Allowed outputs | Exit artifact | Bound |
|---|---|---|---|---|---|
| **Worker** (reviewer subagents and Codex inside it, ops/TICK.md §4) | a `ready` issue the scheduler selects (§2) | the issue, its spec, `docs/PRINCIPLES.md` entries for its `area:` | code, tests, specs for that issue | one PR | ADR 0010's cap; over it → design (§5) |
| **Design: technical** | an area threshold (§4), a PR over the review cap (§5), an audit root-cause group (3+ findings, one cause), 3+ QC escapes with one cause | the issue group, its review ledgers, the code and ADRs of the area | one ADR, `docs/PRINCIPLES.md` entries, implementation issues sized for one session; **no code** | the ADR PR | one design per area per cycle (§5) |
| **Design: product** | `docs/PRODUCT.md` missing (v1), a journey-gap finding (user advocate, audit), an owner scope change | `docs/PRODUCT.md`, `specs/journeys/`, PLAN §4, the PDFgear inventory | `docs/PRODUCT.md`, journeys (append-only, CONSTITUTION 15), issues | the PRODUCT/journeys PR | one per trigger; v1 waits for the owner |
| **Design: UI** (brief names macOS or web) | a user-advocate visual finding; **before the M4 backlog opens** | `specs/ui*/`, Apple HIG or Material/MUI, dogfood screenshots, vision scores, current components | design system as checkable artifacts: tokens in code, a component catalog (SwiftUI previews, a web catalog page), screenshot baselines, UI ADRs; it must render them and look (Chrome DevTools, XCUITest) | the design-system PR | one per platform per trigger |
| **Audit** (read and question) | 10 **product merges** (§3) or 7 days since the last audit record | the merged range, industry practice (pdf.js, PDFium, MuPDF, OSS-Fuzz, HIG, OWASP) | `docs/audits/NNNN.md`, issues; findings **grouped by root cause**, questions sent to design | the audit PR | one per due date |
| **QC** (run and break) | a spec newly `Status: implemented`; 5 product merges in one area since its last QC; untriaged fuzz or differential crashes; a milestone gate (replaces rule j's gate check) | the spec and journeys, the built binaries and public APIs; **not** PR discussions or implementation reasoning (black-box) | tests, fixtures, corpus manifests, fuzz targets; **no product code** | failing tests as `#[ignore = "<issue URL>"]` plus `bug`/`crash` issues (CONSTITUTION 14), or "no escape found" in the journal; an escape line per bug (§6) | one area or one gate per session |
| **User advocate** | every 2nd audit slot, once a journey assigned to M4 or later passes on main | `docs/PRODUCT.md`, journeys, the running apps | screenshots (ours in `target/`, PDFgear's in `~/jpdf-research/`), `ux` issues, `docs/ux-scores.md` | findings sent to product or UI design | one per slot |
| **Retro** | 7 days since the last retro, or an incident (TICK rule h) | `cargo xtask metrics`, per-area ratios (§4) | `docs/retros/NNNN.md`, at most 3 measured process changes, `ops/LESSONS.md` curation | the retro PR | weekly |
| **Planner** | a milestone starts, 7 days since the last plan, the work-mix budget is breached (§3), or no `ready` issue has its dependencies met | the milestone's issues and dependencies, `ops/QUEUE.md`, metrics | `ready`/`deferred` labels, milestone moves, `ops/QUEUE.md`; **the only gate into `ready`** besides the owner | `ops/QUEUE.md` (ordered ready list, critical path, work-mix reading) | at most once a day unless the queue is empty |
| **Gardener** | daily | labels and issues | mostly `cargo xtask garden` (§2); a session only for its judgement items | label fixes, closed duplicates, a docs-drift list | once a day |

Mutation testing moves from the audit to QC. The dogfooding and completeness parts of the audit
(AUTOMATION.md §5.3, §5.4) move to the user advocate; until the user advocate's trigger first
holds, the audit keeps both.

### 2. The scheduler: `cargo xtask next-session`
TICK step 2 runs `cargo xtask next-session` instead of reading rules a–j. It reads labels, git,
GitHub and counters, and prints the session type plus a **brief**: the issue or PR, and exactly
which files and issues to load. Triggers, the precedence order, the work-mix budget and the
bounds are code with tests, not prompt text. The driver stays as it is: it still runs
`claude -p "$(cat ops/TICK.md)"`.

- **Precedence** (first match wins; it keeps rules a–f and the ADR 0009 security order):
  1. owner instructions without a "done:" reply (rule a), as the instruction asks;
  2. main red, judge alarm (rule b) → Worker;
  3. own open PR failing CI or review (rule c) → Worker;
  4. `crash`, `security` + `sev:critical`/`sev:high` (rule d) → Worker; in a `design-pending`
     area, a containment fix only (§4);
  5. ratchet or nightly regressions, `flaky` (rule e) → Worker; leftover branch (rule f);
  6. a PR over the review cap, then an area threshold → Design: technical (§4, §5);
  7. Audit due (or User advocate, on every 2nd slot) → then Retro due;
  8. QC due → Planner due → Design: product / UI due;
  9. Gardener due with judgement items;
  10. a `ready` issue in the current milestone with dependencies met, not `owner-todo`, not
      `design-pending`, lowest number first, respecting the work-mix budget (§3) → Worker;
  11. nothing else → Planner.
- **Read-only.** Like `cargo xtask metrics`, it runs only read commands and writes nothing to
  GitHub or git. Every GitHub write it wants (the Gardener's label fixes, a `design-pending`
  freeze) is printed as a list of single commands that the session runs one by one, so each still
  passes the guard and carries the tag. A script that wrote to GitHub itself would bypass the
  guard's tag check.
- **Testable offline.** `--from DIR` reads saved inputs, as `xtask metrics` does; every trigger
  and every precedence step gets a fixture that selects it and a near-miss fixture that doesn't.
- **Failure.** A `gh` or network error means "unknown": it exits 1 and the session does no
  work (the same rule as the driver's halt check). A crash or a wrong pick is a `bug` issue with
  `area:process`, picked first by the next session; the same scheduler error in 3 sessions in a
  row → `owner-todo`.
- **Transition.** Until task 1 (§9) merges, TICK rules a–j stay in force. Each task changes TICK
  and AUTOMATION.md for its own session type in its own PR.

### 3. Product merges and the work-mix budget
- **Product merge:** a merge on main whose PR closes an issue with a product `area:` label (§4).
  Process areas are the autonomy system itself: `area:process` (TICK, AUTOMATION, the
  scheduler, metrics, proposals) and `area:records`. Audit, QC and planner cadences count product
  merges only. The 7-day fallback still runs an audit in a week with none (all of M0 so far).
- **No record-only PRs.** The journal, STATUS and LESSONS update goes into the work PR before
  `ai-review` is posted (its `Result:` line says what was known then: "auto-merge on", "parked").
  A session whose work PR doesn't exist (a pure GitHub action) or is already merged writes them
  in the next work PR it opens. If a parked PR's STATUS change conflicts with main, the merge of
  origin/main takes main's STATUS and rewrites it.
- **Work-mix budget:** at least 70% of the last 20 sessions are on milestone work (a Worker,
  Design or QC session whose issue is in the current milestone and in a product area).
  Owner-instruction sessions (rule a) are not counted. Below 70%, the scheduler skips process
  issues in step 10 and the Planner runs. The Planner may move process issues to a later
  milestone or label them `deferred`, never close them. The session type is read from a
  `Session: <type>` line in each merged PR body and each journal entry.

### 4. Detection is mechanical: `area:` and `origin:` labels
- Every issue carries exactly one `area:<name>` label from `ops/areas.toml` (each area: a name,
  product or process, the paths it owns) and one `origin:` label: `owner`, `review`, `audit`,
  `qc`, `discovery` or `follow-up`. A follow-up issue's body has a line `Follow-up of: #N`. Sessions
  set both when filing; `cargo xtask issue-lint` reports open issues missing them, and the
  Gardener fixes them.
- `cargo xtask metrics` reports per area: open issues, issues opened per issue closed, and the
  deepest follow-up chain (the number of issues on the longest `Follow-up of:` path, root
  included).
- **Area threshold** (starting values, from #224): more than 4 open issues in the area, or a
  follow-up chain of 3 issues, or at least 5 issues opened in 14 days with more opened than closed.
  Task 4 checks the values against the memory chain: on its history the trigger must first fire
  when #154 is filed (#109 → #146 → #154, 3 issues), not earlier. If the values don't do that,
  task 4 changes them and says so in its PR.
- **Freeze while the design is pending.** When an area triggers, its open issues get
  `design-pending` and workers skip them. `crash` and `sev:high`+ issues in it get a minimal
  containment fix only (stop the crash or the leak; no redesign). The labels come off when the
  area's design ADR merges and its issues are closed, kept or folded into tasks, as the ADR says.

### 5. A PR over the review cap goes to design, not to the owner
ADR 0010 is unchanged up to its final check. Where TICK §4 step 6 now sends the PR to the owner
(draft, `blocked`, `owner-todo`), it gets `needs-design` instead, and the next Design: technical
session takes it with the full ledger. Its output is exactly one of:
1. **split:** a new PR with the part that converged, plus issues for the rest;
2. **ADR or principle:** write the missing ADR or principle; the issue becomes tasks;
3. **ruling:** the open findings are out of scope by a named existing principle or ADR; the
   ruling is posted on the PR and the PR is finished;
4. **owner question:** `owner-todo` with one specific question and the options.

A design ruling counts as the owner comment that ADR 0010 requires for any round K ≥ 6, and it
must say which review, if any, runs next. Outputs 1 and 3 run **one** final check (the ADR 0010
round-5 rules, on the diff since the last reviewed commit) and then post `ai-review`; anything
that check reports goes to output 4.

**Bound:** one design escalation per area per cycle (a cycle is the time between two retro
records). A second PR over the cap in an area already redesigned this cycle, or anything the
post-design final check reports, goes to the owner with both ledgers.

| State | Next step |
|---|---|
| final check clean (ADR 0010) | `ai-review` success, merge (unchanged) |
| final check reports a finding, area not yet escalated this cycle | `needs-design`; PR to draft; Design: technical |
| final check reports a finding, area already escalated this cycle | owner, both ledgers (`owner-todo`) |
| design output 1 or 3, its final check clean | `ai-review` success, merge |
| design output 1 or 3, its final check reports anything | output 4 |
| design output 2 | PR closed unmerged; tasks filed (`ready` only via the Planner) |
| design output 4 | `owner-todo`; no review round until an owner comment |
| a `/hold` at any point | unchanged (TICK §5): nothing moves until `/release` |

### 6. Rules for every session type
- **ADR template for design output** (`docs/adr/_TEMPLATE-design.md`, task 3): the problem
  class; why point fixes don't converge; the invariant; enforcement, preferred in this order:
  types, a lint or gate, a measurement, reviewer vigilance; verification; what happens to every
  issue in the group (superseded and closed with a link, kept, or folded into a task); the
  tasks; what reviewers check against; requirements on future callers. Findings about code that
  doesn't exist yet go into that last section, not into new issues.
- **Principles are read and enforced.** `docs/PRINCIPLES.md` (how the product is built; each
  entry an invariant linked to its ADR and enforcement) and `docs/PRODUCT.md` (what it is for)
  are loaded by the session types that need them and go into the reviewer brief for the touched
  area. `ops/LESSONS.md` stays about how the agent works.
- **QC escapes.** Each bug QC files names the gate that should have caught it (spec checklist,
  unit test, review, fuzz, judge). Three escapes with the same gate in one area → Design:
  technical.
- **Close the loop.** The retro tracks each area's opened-per-closed ratio after its design ADR
  merges. If the area keeps producing issues above the threshold for two retros, the ADR is
  reopened by a new design session, not patched.
- **Design ahead.** Before the Planner opens a milestone's backlog, it lists the cross-cutting
  concerns the milestone introduces (M2's image decoding: memory, untrusted input, threading,
  the CLI) and files a Design: technical issue for each one with no principle yet. A spec that
  touches a concern with no principle files such an issue instead of improvising.

### 7. Owner interface
- `docs/PRODUCT.md` v1 needs the owner's explicit approval (an owner comment), with no timeout.
  Later `PRODUCT.md` changes and design ADRs get `owner-todo` and merge after 24 hours unless the
  owner comments `/hold`: the first session that sees the PR ready for 24 hours or more, with
  `ai-review` success and no `/hold`, enables auto-merge and removes `owner-todo`.
- Only scope, taste or money questions go to the owner. Technical questions go to design.
- **Owner direction filed by an interactive session.** Such text is tagged (the guard requires
  it), so it is data under CONSTITUTION 1, and a `human` label on it proves nothing: the agent
  account can add labels too. #223 and #224 were lost this way. The interactive session adds
  `human` and asks the owner to post one untagged comment on the issue (any text). The
  scheduler picks it then, because `ops/driver/owner-issues.sh` already lists untagged owner
  comments without a "done:" reply. No protected change is needed.
- AUTOMATION.md principle 3 ("nothing waits for a human") gets two named exceptions: PRODUCT.md
  v1 and design output 4.

### 8. Protected files
None of this needs a protected change: the driver is unchanged, `owner-issues.sh` already feeds
rule a, labels and xtask commands are not protected, and the 24-hour design merge is a session
step, not a workflow. So there is no `ops/proposals/` file. A task that finds it needs one writes
the proposal then.

### 9. Implementation tasks (filed after this ADR merges; each one issue, one PR, ≤ 1500 lines)
Each task updates ops/TICK.md and AUTOMATION.md for what it adds.

| # | Task | Depends on |
|---|---|---|
| 1 | `cargo xtask next-session` with precedence steps 1–5, 7 and 10 as today's rules a–i and step 11 as rule j until task 2, product-merge counters (§3), `Session:` lines; TICK step 2 calls it; record-only PRs end | — |
| 2 | Planner session and the work-mix budget; `ops/QUEUE.md`; the Planner as the gate into `ready` | 1 |
| 3 | Design: technical: the design ADR template, `docs/PRINCIPLES.md`, `needs-design` (§5) with its state table in TICK §4 step 6, absorbing #224 | 1 |
| 4 | `ops/areas.toml`, `area:`/`origin:` labels on every open issue, `xtask issue-lint`, per-area metrics, the area threshold and `design-pending` (§4), checked against the memory chain; `xtask garden` and the Gardener | 1, 3 |
| 5 | QC session, escape lines, mutation testing moved from the audit | 1, 4 |
| 6 | Design: product and `docs/PRODUCT.md` v1 (waits for the owner's approval) | 1 |
| 7 | Design: UI, before the M4 backlog opens | 6 |
| 8 | User advocate, taking AUTOMATION.md §5.3/§5.4 from the audit | 4, 6 |

First design queue once task 3 works: #236 (memory architecture), #223 (security by
isolation), then a sweep of the other chains: trait contracts (#79, #106, #107), owner comments
(#173, #192), the e2e harness (#76, #200), and the stale `in-progress` labels (41 closed issues
still carried one on 2026-11-22).
#224 is closed into task 3.

## Consequences
- Sessions stop choosing their own work. A wrong pick is a scheduler bug with a failing
  fixture, not a judgement call buried in a journal.
- More sessions per problem: a hard problem costs a design session before the workers. The
  work-mix budget and the per-area bound keep that from crowding out milestone work.
- The scheduler is the new single point of failure. It is read-only and fails to "do nothing",
  so a broken scheduler stops progress but can't cause harm; the next session fixes it.
- Labels carry more meaning (`design-pending`, `needs-design`, `area:`), and the agent account
  can set any of them. As with `human` today, the guard can't check labels; the journal and the
  owner feed show who changed what.
- Until task 4 lands, there are no area labels, so the area threshold can't fire; escalation
  from the review cap (task 3) works without them.

## Success check (the first retro after tasks 1–4 merge, then the next)
- Work mix: at least 70% of sessions on milestone work, from the `Session:` lines.
- No PR reaches the owner from the review cap without a design session in between (except the
  second-in-cycle case of §5).
- Record-only PRs: 0 after task 1.
- The memory-chain replay (task 4) fires at #154, and in live use every area that triggers gets a
  design ADR within 7 days. The retro reports each designed area's opened-per-closed ratio; if
  it doesn't fall below 1 within two retros, the design is reopened (§6).
