# 11. Session types, a scheduler, and bounded escalation to design

Status: accepted (owner issue #238, 2026-11-22; the owner approved PR #239, merged in 662bde6).
Absorbs #224 (the architect role becomes the **design: technical** session type; #224 is closed
into #245). Implementation tasks: §9, filed as #242–#251 (task 4 split in two: #246, #247).
Amended by ADR 0012 (light-lane PRs: §1 triggers and §9 tasks).

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
| **Design: technical** | a PR over the review cap (§5), a `ready` issue labeled `design` (filed by an audit root-cause group, 3+ QC escapes with one cause, the Planner's design-ahead list, §6, or the owner), an area threshold (§4) | the issue group, its review ledgers, the code and ADRs of the area | one ADR, `docs/PRINCIPLES.md` entries, implementation issues sized for one session; for output 1 of §5 only, a split PR holding nothing but lines already reviewed in the original PR; **no new code** | the ADR PR | one design per escalated issue (§5); one per area per cycle once areas exist (§4) |
| **Design: product** | `docs/PRODUCT.md` missing (v1), a journey-gap finding (user advocate, audit), an owner scope change | `docs/PRODUCT.md`, `specs/journeys/`, PLAN §4, the PDFgear inventory | `docs/PRODUCT.md`, journeys (append-only, CONSTITUTION 15), issues | the PRODUCT/journeys PR | one per trigger; v1 waits for the owner |
| **Design: UI** (brief names macOS or web) | a user-advocate visual finding; **before the M4 backlog opens** | `specs/ui*/`, Apple HIG or Material/MUI, dogfood screenshots, vision scores, current components | design system as checkable artifacts: tokens in code, a component catalog (SwiftUI previews, a web catalog page), screenshot baselines, UI ADRs; it must render them and look (Chrome DevTools, XCUITest) | the design-system PR | one per platform per trigger |
| **Audit** (read and question) | 10 **product merges** (§3) or 7 days since the last audit slot (an audit or user-advocate record) | the merged range, industry practice (pdf.js, PDFium, MuPDF, OSS-Fuzz, HIG, OWASP) | `docs/audits/NNNN.md`; findings **grouped by root cause**, each group one `design` issue with its questions; only `crash`, `sev:critical` and `sev:high` findings become plain issues for the queue (`sev:medium`/`sev:low` ones also keep their M10 issue, ADR 0009) | the audit PR | one per slot |
| **QC** (run and break) | a spec newly `Status: implemented`; 5 product merges in one area since its last QC; untriaged fuzz or differential crashes; a milestone gate (replaces rule j's gate check) | the spec and journeys, the built binaries and public APIs; **not** PR discussions or implementation reasoning (black-box) | tests, fixtures, corpus manifests, fuzz targets; **no product code** | failing tests as `#[ignore = "<issue URL>"]` plus `bug`/`crash` issues (CONSTITUTION 14), or "no escape found" in the journal; an escape line per bug (§6) | one area or one gate per session |
| **User advocate** | every 2nd audit slot (slots are numbered by the count of `docs/audits/NNNN.md` plus `docs/advocate/NNNN.md` records; an even next number is the advocate's), once a journey assigned to M4 or later passes on main | `docs/PRODUCT.md`, journeys, the running apps | `docs/advocate/NNNN.md`, screenshots (ours in `target/`, PDFgear's in `~/jpdf-research/`), `ux` issues, `docs/ux-scores.md` | findings sent to product or UI design | one per slot |
| **Retro** | 7 days since the last retro, or an incident (TICK rule h) | `cargo xtask metrics`, per-area ratios (§4) | `docs/retros/NNNN.md`, at most 3 measured process changes, `ops/LESSONS.md` curation | the retro PR | weekly |
| **Planner** | a milestone starts, 7 days since the last plan, the work-mix budget is breached (§3), or no `ready` issue has its dependencies met | the milestone's issues and dependencies, `ops/QUEUE.md`, PLAN §8, metrics | `ready`/`deferred` labels, milestone moves, new milestone issues from PLAN §8 and the journeys, `ops/QUEUE.md`; **the only gate into `ready`** besides the owner | `ops/QUEUE.md` (ordered ready list, critical path, work-mix reading) | at most once a day: it ran today if a `Session: planner` record dated today (UTC) is on origin/main or on the head of an open agent PR (every Planner session writes one, in its `ops/QUEUE.md` PR or a records PR); if it ran today and nothing is ready, the scheduler prints "idle" and the session exits at once with no record |
| **Gardener** | daily | labels and issues | mostly `cargo xtask garden` (§2); a session only for its judgement items | label fixes, closed duplicates, a docs-drift list | once a day, read from its `Session:` record (§3) |

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
  3. own open PR failing CI or review (rule c) → Worker; a scheduler `bug` issue (see Failure
     below) → Worker; an own design or `PRODUCT.md` PR whose 24-hour window has passed (§7) →
     Worker, to enable auto-merge;
  4. `crash`, `security` + `sev:critical`/`sev:high` (rule d) → Worker; in a `design-pending`
     area, a containment fix only (§4);
  5. ratchet or nightly regressions, `flaky` (rule e) → Worker; leftover branch (rule f);
  6. Design: technical, in this order: a PR labeled `needs-design` (§5), a `ready` issue labeled
     `design`, an area over its threshold (§4);
  7. Audit due (or User advocate, on its slot) → then Retro due;
  8. QC due → Planner due → Design: product / UI due;
  9. Gardener due with judgement items;
  10. a `ready` issue in the current milestone with dependencies met, not `owner-todo`, not
      `design`, not `design-pending`, lowest number first, ordered by the work-mix budget (§3) →
      Worker;
  11. nothing else → Planner, or "idle" when the Planner already ran today (its table row).
- **Read-only.** Like `cargo xtask metrics`, it runs only read commands and writes nothing to
  GitHub or git. Every GitHub write it wants (the Gardener's label fixes, a `design-pending`
  freeze) is printed as a list of single commands that the session runs one by one, so each still
  passes the guard and carries the tag. A script that wrote to GitHub itself would bypass the
  guard's tag check.
- **Testable offline.** `--from DIR` reads saved inputs, as `xtask metrics` does; every trigger
  and every precedence step gets a fixture that selects it and a near-miss fixture that doesn't.
- **Failure.** Exit 2 means "unknown" (a `gh` or network error): the session does no work, as
  with the driver's halt check. Any other non-zero exit (a panic, a parse error) means the
  scheduler is broken: the session falls back to the legacy rules that TICK keeps for this case
  (today's rules b and c), then finds or files the `bug` issue titled
  `next-session: <first line of the error>` and fixes it as a Worker. Each failing session adds a
  comment `failed again in session <time>` to that issue; the issue is the persistent counter.
  While an open PR links the issue, sessions add no comment and open no second PR: they work that
  PR under rule c if it fails CI or review or has no `ai-review` result on its head (a session
  that died before review), else only the legacy rules a, b and f. At 3 such comments, the issue
  gets `owner-todo`, and from then on sessions work only the legacy rules a, b, c and f (rule a
  carries any owner comment on the issue) and exit when none applies, until the owner's part is
  done.
- **Transition.** Until task 1a (§9) merges, TICK rules a–j stay in force. From 1a until 1b,
  the scheduler decides steps 1–5 and prints `legacy` when none applies; the session then uses
  TICK rules g–j. Task 3 (which needs only 1a) adds step 6 ahead of that `legacy` exit. Each
  task changes TICK and AUTOMATION.md for its own part in its own PR.

### 3. Product merges, records and the work-mix budget
- **Product merge:** a merge on main whose PR closes an issue with a product `area:` label (§4).
  Process areas are the autonomy system itself: `area:process` (TICK, AUTOMATION, the
  scheduler, metrics, proposals) and `area:records`. **Until task 4 adds area labels**, a product
  merge is a merge that changes at least one file under `crates/`, `bindings/`, `apps/`,
  `fuzz/`, `tools/jpdf-cli/` or `tools/fsio/` (the code that ships or is graded; `jpdf-fsio`
  is compiled into `jpdf-cli`, audit 0006 AC.1), whatever else it changes.
  Audit, QC and planner cadences count product merges only. The 7-day fallback still runs an
  audit in a week with none.
- **Records go into the work PR.** The journal and STATUS update is committed to the work PR
  before `ai-review` is posted, on the head commit that includes it (its `Result:` line
  says what was known then: "auto-merge on", "parked"). A commit that changes only files under
  `ops/journal/` and `ops/STATUS.md` is a **record commit**: it doesn't count as "changed after
  the last reviewed commit" for ADR 0010's final check. Neither does a merge of origin/main whose
  `git show --remerge-diff` is empty or touches only those two paths; any other merge resolution
  counts, as TICK §4 step 6 says today. The check is by path: a commit touching any other file
  counts, `ops/LESSONS.md` included (every session reads it, so a change to it is reviewed). So
  a LESSONS change goes into the PR before its last review round runs. A lesson found after that
  goes into a `Pending lessons` list in `ops/STATUS.md`; the next session that opens a work PR
  moves each one into `ops/LESSONS.md` before that PR's round 1 and deletes it from the list. A
  merge of origin/main that conflicts in `ops/STATUS.md` takes main's copy and rewrites it.
- **Sessions without a work PR** (a Gardener run, a "done:" reply, a Planner run with nothing
  to change in `ops/QUEUE.md`) still record themselves: one
  records PR with only record files, which is never a product merge. Record PRs exist only for
  those sessions. An "idle" pick and a scheduler exit 2 write no record: the session exits at
  once (the driver starts the next one after its pause, so an idle loop costs a short session
  per pause and leaves nothing on GitHub).
- **Session records.** Every journal entry has a `Session: <type> <scheduler pick>` line, the
  type and the first line the scheduler printed. The scheduler reads them from origin/main and
  from the heads of open agent PRs, so a record PR still waiting for CI is seen. The lines are
  self-reported: the scheduler saves no inputs, so nothing can replay its pick. The retro checks
  each line only against what GitHub shows (the issue or PR the entry names, and its labels and
  milestone) and reports mismatches; a session that lies consistently is not caught.
- **Work-mix budget:** at least 70% of the last 20 sessions (by their `Session:` records) are
  on milestone work: a Worker, Design or QC session whose issue is in the current milestone and
  is a product issue. A product issue has a product `area:` label. Before task 4, a Worker or QC
  session on an issue in the current milestone counts if its PR changes a path of the interim
  product-merge rule above (whether the PR is open, merged or closed), and Design sessions are
  left out of the window (neither counted nor in the 20). Owner-instruction sessions (rule a)
  are not counted. The budget **orders**, it never blocks: below 70%, step 10 picks the
  lowest-numbered product issue first and a process issue only when no product issue is ready,
  and the Planner becomes due (once a day) to add product issues from PLAN §8. The Planner may move process
  issues to a later milestone or label them `deferred`, never close them.

### 4. Detection is mechanical: `area:` and `origin:` labels
- Every issue carries exactly one `area:<name>` label from `ops/areas.toml` (each area: a name,
  product or process, the paths it owns) and one `origin:` label: `owner`, `review`, `audit`,
  `qc`, `discovery` or `follow-up`. A follow-up issue's body has a line `Follow-up of: #N`. Sessions
  set both when filing; `cargo xtask issue-lint` reports open issues missing them, and the
  Gardener fixes them.
- `cargo xtask metrics` reports per area: open issues, issues opened per issue closed, and the
  deepest follow-up chain (the number of issues on the longest `Follow-up of:` path, root
  included).
- **Area threshold** (starting values, from #224): 5 or more open issues in the area, or a
  follow-up chain of 3 or more issues, or 5 or more issues opened in the last 14 days with more
  opened than closed in that window. Issues that a merged design ADR kept or filed as its tasks
  don't count toward their area's open-issue term, so an area doesn't re-freeze on its own
  design. Task 4 replays the memory chain: the trigger must first fire when #154 is filed
  (#109 → #146 → #154, 3 issues), not earlier. It also replays an ordinary backlog area as a
  near miss that must not fire. If the values fail either replay, task 4 changes them and says so
  in its PR.
- **Freeze while the design is pending.** When an area triggers, its open issues get
  `design-pending` and workers skip them. `crash` and `sev:high`+ issues in it get a minimal
  containment fix only (stop the crash or the leak; no redesign). The labels come off when the
  area's design ADR merges and its issues are closed, kept or folded into tasks, as the ADR says.

### 5. A PR over the review cap goes to design, not to the owner
ADR 0010 is unchanged up to its final check. Where TICK §4 step 6 now sends the PR to the owner
(draft, `blocked`, `owner-todo`, `ai-review` failure), the PR goes to draft with `ai-review`
failure and gets `needs-design` instead, and the next Design: technical session takes it with
the full ledger. This applies to both of step 6's owner exits: the final check reported anything
(ADR 0010's "anything": a finding of any grade, a rejected dispute or regrade), or the fix diff
was over 100 lines or added a file, so no final check ran. (A Planner PR closes no issue, so it
has no escalated issue to design: at the cap it is closed unmerged and its open findings filed
as an issue, ops/TICK.md §9 step 6; #244.) Its output is exactly one of:
1. **split:** a new PR holding only lines already reviewed in the original PR (the part that
   converged), plus issues for the rest. If the split needs any new line, it is a task issue for
   a Worker instead;
2. **ADR or principle:** write the missing ADR or principle; the issue becomes tasks;
3. **ruling:** an open major, minor or nit is out of scope by a named existing principle or ADR.
   No ruling may cover a blocker, a CONSTITUTION finding, a security finding that is `sev:high`
   or above by the ADR 0009 table, or a finding whose dispute or regrade a reviewer already
   rejected (ADR 0010 makes that final);
4. **owner question:** `owner-todo` with one specific question and the options.

A design ruling counts as the owner comment that ADR 0010 requires for any round K ≥ 6, and it
must say which review, if any, runs next. Outputs 1 and 3 run **one** final check (the ADR 0010
round-5 rules, on the diff since the last reviewed commit, record commits excepted), and only
if that diff adds ≤ 100 lines and no new file; otherwise the design goes to output 4. Its brief
also holds every ruling, and it gives each one line, `<ID>: ruling accepted` or
`<ID>: ruling rejected` (a missing line counts as rejected), as ADR 0010 does for disputes. It
then posts `ai-review`; anything that check reports, a rejected ruling included, goes to output 4.

**Bound:** one design escalation per escalated issue. Every task issue a design files carries a
`Designed in: #<escalated issue>` line, and so does the split PR's issue. A PR over the cap
whose issue is already escalated or carries that line goes to the owner with both ledgers; so
does anything the post-design final check reports. Once task 4 adds areas, a second bound
applies too: one design escalation per area per cycle (a cycle is the time between two retro
records).

| State | Next step |
|---|---|
| final check clean (ADR 0010) | `ai-review` success, merge (unchanged) |
| final check reports anything, or no final check ran (fix diff over 100 lines or a new file); issue not yet escalated (and, once areas exist, area not escalated this cycle) | `needs-design`; PR to draft; `ai-review` failure; Design: technical |
| the same, issue already escalated or carrying `Designed in:` (or area already escalated this cycle) | owner, both ledgers (`owner-todo`) |
| design output 1 or 3, its final check clean and every ruling accepted | `ai-review` success, merge |
| design output 1 or 3, its final check reports anything or rejects a ruling | output 4 |
| design output 1 or 3, the diff since the last reviewed commit over 100 lines or with a new file | output 4 (no check runs) |
| design output 2 | PR closed unmerged; tasks filed with `Designed in:` (`ready` only via the Planner) |
| design output 4 | `owner-todo`; no review round until an owner comment |
| a record commit, or a merge of origin/main whose remerge-diff is empty or record-only | no new review (§3) |
| a merge of origin/main with any other resolution | counts as an author change (TICK §4 step 6) |
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
  unit test, review, fuzz, judge). Three escapes with the same gate in one area → a `design`
  issue.
- **Close the loop.** The retro tracks each area's opened-per-closed ratio (over the retro
  period) after its design ADR merges. If the ratio is 1 or more at two retros in a row, a new
  design session reopens the ADR; it isn't patched.
- **Design ahead.** Before the Planner opens a milestone's backlog, it lists the cross-cutting
  concerns the milestone introduces (M2's image decoding: memory, untrusted input, threading,
  the CLI) and files a `design` issue for each one with no principle yet. A spec that touches a
  concern with no principle files such an issue instead of improvising.

### 7. Owner interface
- `docs/PRODUCT.md` v1 needs the owner's explicit approval (an owner comment), with no timeout.
  Later `PRODUCT.md` changes and design ADRs get `owner-todo` and merge after 24 hours unless the
  owner comments `/hold`. The 24 hours start at the latest of: the PR marked ready, its last
  `ai-review` success, and the owner's last `/release`. That success counts from the triage
  comment it links, so one re-posted after a record-only update (record commits, merges of
  origin/main whose remerge-diff is empty or touches only record paths) keeps the start (#373). The first session that sees the window
  passed with `ai-review` success on the head and no `/hold` after the last `/release` enables
  auto-merge and removes `owner-todo` (precedence step 3).
- Only scope, taste or money questions go to the owner. Technical questions go to design.
- **Owner direction filed by an interactive session.** Such text is tagged (the guard requires
  it), so it is data under CONSTITUTION 1, and a `human` label on it proves nothing: the agent
  account can add labels too. #223 and #224 were lost this way. The interactive session adds
  `human` (as #238 asks; harmless now that rule a ignores it on tagged text) and asks the owner
  to post one untagged comment on the issue (any text). The scheduler picks it then,
  because `ops/driver/owner-issues.sh` already lists untagged owner comments without a "done:"
  reply. TICK rule a says the same from this PR on: a `human` label on tagged text is not an
  instruction. No protected change is needed.
- AUTOMATION.md principle 3 ("nothing waits for a human") names the owner waits: the existing
  ones (proposals to apply, `/hold`, ADR 0010 past its final check) plus PRODUCT.md v1 and design
  output 4.

### 8. Protected files
None of this needs a protected change: the driver is unchanged, `owner-issues.sh` already feeds
rule a, labels and xtask commands are not protected, and the 24-hour design merge is a session
step, not a workflow. So there is no `ops/proposals/` file. A task that finds it needs one writes
the proposal then.

### 9. Implementation tasks (filed after this ADR merges; each one issue, one PR, ≤ 1500 lines)
Each task updates ops/TICK.md and AUTOMATION.md for what it adds. Task 1 is split in two by the
sizing rule.

| # | Task | Depends on |
|---|---|---|
| 1a | `cargo xtask next-session` 1/2: precedence steps 1–5 as today's rules a–f, the failure rule (§2) with the legacy fallback kept in TICK, `--from` fixtures; TICK step 2 calls it | — |
| 1b | `next-session` 2/2: steps 7, 10 and 11 as today's rules g–j, the interim product-merge counter (§3), `Session:` records, records in the work PR | 1a |
| 2 | Planner session, `ops/QUEUE.md`, the Planner as the gate into `ready`, the work-mix budget (interim product definition until task 4) | 1b |
| 3 | Design: technical: the design ADR template, `docs/PRINCIPLES.md`, the `design` and `needs-design` labels, §5 with its state table in TICK §4 step 6, the per-issue bound, absorbing #224 | 1a |
| 4 | `ops/areas.toml`, `area:`/`origin:` labels on every open issue, `xtask issue-lint`, per-area metrics, the area threshold and `design-pending` (§4) with both replays, the per-area bound, the area-based product merge; `xtask garden` and the Gardener | 1b, 3 |
| 5 | QC session, escape lines, mutation testing moved from the audit | 1b, 4 |
| 6 | Design: product and `docs/PRODUCT.md` v1 (waits for the owner's approval) | 1b |
| 7 | Design: UI, before the M4 backlog opens | 6 |
| 8 | User advocate, `docs/advocate/`, taking AUTOMATION.md §5.3/§5.4 from the audit | 4, 6 |

First design queue once task 3 works: #236 (memory architecture), #223 (security by
isolation), then a sweep of the other chains: trait contracts (#79, #106, #107), owner comments
(#173, #192), the e2e harness (#76, #200), and the stale `in-progress` labels (41 closed issues
still carried one on 2026-11-22). Each gets the `design` label when it is queued.
#224 is closed into task 3.

## Consequences
- Sessions stop choosing their own work. A wrong pick is a scheduler bug with a failing
  fixture, not a judgement call buried in a journal.
- More sessions per problem: a hard problem costs a design session before the workers. The
  work-mix budget and the per-issue and per-area bounds keep that from crowding out milestone
  work.
- The scheduler is the new single point of failure. It is read-only; an "unknown" exit stops
  one session, and a broken scheduler sends the session to the legacy rules b and c and to the
  scheduler's own bug issue (§2), so the loop repairs it or reaches the owner after 3 sessions.
- Record commits skip the final check by path only, so a record commit can't carry code. The
  journal and STATUS text after the last review is no longer reviewed (today record PRs are:
  PR #233's round 1 found a false tally in a journal entry). A false line there is caught only
  by the owner, the next session or an audit.
- Labels carry more meaning (`design`, `design-pending`, `needs-design`, `area:`), and the agent
  account can set any of them. As with `human` today, the guard can't check labels; the journal
  and the owner feed show who changed what.
- Until task 4 lands, product merges and the work mix use the interim path and label
  definitions of §3, and the area threshold can't fire; escalation from the review cap (task 3)
  works without areas.

## Success check (the first retro after tasks 1–4 merge, then the next)
- Work mix: at least 70% of sessions on milestone work, from the `Session:` lines, and every
  mismatch the retro's cross-check finds explained.
- No PR reaches the owner from the review cap without a design session in between (except the
  already-escalated cases of §5).
- Record-only PRs only for sessions without a work PR (§3).
- The memory-chain replay (task 4) fires at #154 and the near-miss replay doesn't; in live use
  every area that triggers gets a design ADR within 7 days. A designed area whose
  opened-per-closed ratio is 1 or more at two retros in a row is reopened (§6).
