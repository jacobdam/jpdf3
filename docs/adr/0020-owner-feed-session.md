# 20. One session routes every pending owner item; handled and routed stay text

Status: proposed (design issue #522, queued by the owner: filed by the owner's interactive
session and endorsed by the owner's untagged comment
https://github.com/jacobdam/jpdf3/issues/522#issuecomment-7238115763). A design ADR merges 24
hours after the latest of: its PR marked ready, its last `ai-review` success, the owner's last
`/release`, unless the owner comments `/hold` (ADR 0011 §7; ops/TICK.md §8 step 5). Amends
ADR 0011 §1 (a new session type) and §2 (precedence step 1), and ops/TICK.md §0's two replies.

## The problem class
Every owner item (an owner issue, a comment, an inline review comment, a review body) is its own
session, and most of those sessions only write a reply and a record.

The numbers, for 2026-11-28 up to 10:48 UTC (`git log origin/main`, `ops/journal/2026-11-28/`):
- 40 commits on main; 14 of them are `Records:` PRs. Of the 14: 8 post only the `done:`
  replies of one instruction after its merge (PRs #436, #443, #486, #488, #491, #492, #494,
  #496), 3 record a session that found the work already taken by the owner's interactive
  session (#502, #507, #521), 1 answers owner comments (#485), 1 closes an issue (#462), 1 is a
  design window (#432).
- 18 of the day's 41 journal files are owner picks (`Session: worker owner-…`), for 10 owner
  items. Eight instructions (#236, #433, #438, #463, #465, #466, #478, #493) each took two
  sessions: one that did the work and posted "picked up", and a later one picked only to post
  "done:" once the work PR had merged (ops/TICK.md §0: "done:" waits for the merge, which lands
  after the session exits).
- Each records PR runs the full CI (PR #521: 10:42:31 to 10:46:32 UTC, every judge job) and
  then a full `ci` run on main when it merges; two main runs were cancelled at 10:16 and 10:18
  UTC by the next merge.

The class: **an owner item's state lives in a session instead of on GitHub.** The scheduler can
see "pending" (no `done:` reply) and nothing finer, so it hands each item to a full Worker
session, and only that session can move it on. The owner's four ideas in #522 (one session for
the whole feed, reactions as acks, no journal when there is nothing to learn, a cheap path for
records PRs) are four cuts of the same cost.

## Why point fixes don't converge
- **Fewer replies alone don't help.** The "done:" reply is the handled marker that the protected
  `ops/driver/owner-issues.sh` reads (TICK §0, #173, #179). Dropping it leaves the item pending
  forever; keeping it and posting it sooner is false ("done" means on main).
- **A cheaper records PR alone doesn't help.** Records PRs already skip the review: their
  `ai-review` is posted as "records only" (PR #521's status). What they cost is the session that
  writes them, the CI run and the main run, and the session is the larger part.
- **Batching by prose fails.** Telling a session "also handle the other pending items" without a
  scheduler term repeats the self-selection that ADR 0011 §2 removed: the next pick still sees
  every item as pending, because nothing on GitHub says an item is under way.
- **Reactions as state fail.** The agent and the owner share one account, and anyone with a
  GitHub account can react on a public repo. A reaction carries no text, so the tag (CONSTITUTION
  1) can't mark it, and the owner's own 🚀 on a comment would read as "handled". Reading
  reactions would also need a protected change to the feed for every item.

The missing decision: **a pending owner item has two text states the scheduler can read,
*routed* and *handled*, and one session moves every unrouted item to one of them.**

## The invariant
1. **States.** Each owner item in the feed (`ops/driver/owner-issues.sh`, unchanged) is in one of
   three states, computed by `cargo xtask next-session` from tagged text and labels only:
   - **handled:** a tagged `jacobdam` reply `🤖 jpdf-agent done: <its URL>` exists (today's
     marker, read by the feed itself);
   - **routed:** not handled, and its work is carried by an open item the scheduler reads: an
     open PR or issue authored by `jacobdam`, with a tagged body holding a line
     `Owner instruction: <its URL>` (exact URL, then whitespace or the end), and for an issue
     the label `owner-work`; an untagged owner issue labeled `owner-work` routes itself;
   - **unrouted:** neither.
2. **One session moves the unrouted ones.** The **owner feed** session takes every unrouted item
   the scheduler lists, oldest first, and leaves each one handled or routed before it exits. It
   writes no code, spec, ADR, test or proposal. Its allowed actions:
   - a `done:` reply (TICK §0's form: what changed, then `waiting on owner: …`) after it carried
     the item out in place: labels, milestones, assignees, closing or reopening an issue, editing
     a tagged body, filing issues (without `ready`, so the Planner gates them, unless the owner
     asked for `ready`), a tagged comment elsewhere, a `/hold` or `/release` follow-up (ops/TICK.md
     §5: `owner-todo`, auto-merge after a `/release`), a "done:" for an item whose route closed;
   - a **question back**: a `done:` reply that asks one question and ends
     `waiting on owner: an answer to the question above`; the owner's answer is a new item;
   - a **route**: file a tagged issue with the `Owner instruction:` line and `owner-work` (plus
     `design` or `design-product` when the work is a design), or add `owner-work` to an untagged
     owner issue, or, for rework of an open agent PR, add the line to the PR's body, turn it back
     into a draft and post `ai-review` failure with a link to the item.
3. **Routed work keeps rule a's priority.** Precedence step 1 picks the routes before every
   other step: an open `owner-work` issue that no open PR links and that carries no skip label, as
   `worker owner-routed #N` (`design-technical owner-routed #N` with `design`,
   `design owner-routed #N` with `design-product`); an open routed PR that is failing (step 3's
   test, not held, not `needs-design`), as `worker owner-routed #N`. When the route closes or
   merges, the item is unrouted again, and the next owner feed session posts its `done:` (or
   routes it again when the PR closed unmerged).
4. **Reactions are display, never state.** The owner feed session (and any session that routes
   or handles an item) adds 👀 to a routed item and 🚀 to a handled one where GitHub allows a
   reaction (issues, issue comments, inline review comments; not review bodies). No script reads
   a reaction, and a reaction replaces no reply that the feed or the scheduler reads.

## Enforcement
1. **types:** not available: the states live on GitHub, written by sessions through `gh`.
2. **a lint or gate:** the scheduler is the gate (ADR 0011 §2). It computes the three states
   from `feed.jsonl` and two new inputs (the open issues labeled `owner-work` with author, body
   and labels; the bodies of open PRs, which `prs.json` already has), so an item counts as routed
   only when a `jacobdam` route with the exact line and label exists; a stranger's tagged issue,
   a closed route or a line in a comment route nothing. Tested offline with `--from` fixtures
   (task 1, task 2). The guard needs one protected change to allow the reaction writes
   (task 3), and the records fast path is a protected CI change (task 4); both go through
   `ops/proposals/`.
3. **a measurement:** the success check below, counted from `git log origin/main`,
   `ops/state/picks.tsv` (ADR 0018) and the journal.
4. **reviewer vigilance:** principle P10 (below) for the session's own judgement: whether an item
   was really carried out before its `done:`, and whether a route names the right work.

**The pick.** Precedence step 1 becomes, in this order:
1. the routes of invariant 3 (lowest number first);
2. `owner-feed` when at least one item is unrouted and either the newest unrouted item with a
   time (`created_at`) is at least 10 minutes old (a quiet period, so a burst of owner comments
   is one session), or the oldest is at least 60 minutes old (so a steady trickle still gets a
   session each hour), or an unrouted owner issue has no time in the feed (it is picked at once).
   The brief lists every unrouted item, oldest first, at most 25 (the rest wait for the next
   pick), and notes the items waiting for the quiet period when it doesn't pick;
3. the closed owner issues of today's `owner-done` term, read as unrouted items of (2): a closed
   owner issue of the last 14 days, labeled `owner-work` or with a legacy "picked up" reply, and
   no "done:" reply.

`worker owner-issue #N`, `worker owner-comment <url>` and `worker owner-done #N` are gone; an
`/approve` that step 3's design window counts stays out of step 1 (#372, unchanged).

**The journal rule.** An owner feed session writes a journal entry only when it has something a
later session needs: a non-empty `Ignored:` line (CONSTITUTION 1's trace of a non-owner request),
a lesson, or an item it left unrouted (the session died midway, a guard block). Otherwise it
writes none and opens no PR; its trace is the replies on GitHub and the scheduler's own line in
`ops/state/picks.tsv`. This is safe because the scheduler counts no owner feed record: they
are not Planner records (steps 8 and 11), and the work mix leaves owner sessions out (ADR 0011
§3; task 2 adds `owner-feed` to the left-out words). Every other session type writes its journal
as today.

**The records fast path.** A PR whose every changed path is a record path (ADR 0011 §3's record
commit, applied to the PR's whole diff against its base: each path added or modified under
`ops/journal/YYYY-MM-DD/<name>.md`, or added as `ops/lessons/pending/<name>.md` other than
`README.md`, each a regular file of mode 100644, no rename, delete or other mode) skips the
build, test and judge jobs, and `judge-verdict` passes it with the reason "records only". The
classifier runs from the base commit's copy, as the `policy` job reads pinned config from the base
(LESSONS, PR #444 R1.2), so a PR can't change the rule that grades it. Pushes to main keep the
full run (main red, rule b, reads them).

## Verification
- **Scheduler (tasks 1 and 2).** A fixture that selects each term and the nearest miss that
  doesn't: a route by a stranger with the tag; a route issue with the line and no `owner-work`;
  `owner-work` with the line naming another URL, or the URL followed by a digit (`…-1234` for
  `…-123`); a closed route (the item is unrouted again, so `owner-feed`); a routed PR that is
  green (no pick), failing (picked), held (no pick); the quiet period at 9:59 and 10:00 minutes,
  the trickle bound at 59:59 and 60:00, an issue item without a time; 25 and 26 items in the
  brief; a closed owner issue with `owner-work`, with a legacy "picked up", with neither.
  What broken implementation still passes: one that treats a route that a session filed for
  the wrong item (wrong URL by mistake, right shape) as correct. Only P10's reviewer line and
  the owner's view (👀 on the comment, the cross-reference GitHub shows on the route) catch it.
- **The journal rule.** The retro of the first full day after task 2 counts owner feed picks in
  `ops/state/picks.tsv` against the driver's `ops/logs/index.tsv` rows and the journal: each
  pick is a session with replies on GitHub or a journal entry. A session that did nothing and
  wrote nothing passes; picks.tsv is local and forgeable (ADR 0018), so the check is evidence,
  not proof.
- **Guard (task 3).** The proposal's tests (ADR 0014) allow exactly
  `gh api -X POST repos/jacobdam/jpdf3/issues/<n>/reactions`, `…/issues/comments/<n>/reactions`
  and `…/pulls/comments/<n>/reactions` with one field `content` of `eyes` or `rocket`, and block
  each near miss: another repo, another content value, a second field (`-f body=…`), a query
  string, a path with `..` or a trailing segment, `DELETE`, the same path under `pulls/<n>/`.
  What broken guard still passes: none of those shapes; a reaction on the wrong item is display
  only.
- **Records fast path (task 4).** The proposal's classifier tests: records-only diffs (a new
  journal file, an edited journal file, a new pending lesson) skip; each near miss runs the full
  CI: a journal file not ending `.md`, one level deeper, an undated directory, a renamed or
  deleted journal file, mode 100755 or 120000, a changed `ops/lessons/pending/README.md`, a
  modified pending lesson that is on the base, `ops/LESSONS.md`, any other path next to a
  record. A structural test checks that the workflow runs the base commit's classifier. What
  broken implementation still passes: a judge-relevant change hidden in Markdown under
  `ops/journal/` (nothing executes it, and the next session reads it as data; the same exposure
  as record commits today, ADR 0011 Consequences).

## Issues in the group
| Issue | Outcome | Why |
|---|---|---|
| #522 | kept, closed by this ADR's PR | the design issue |
| #238 | kept | the umbrella of session types; this ADR adds one (the owner feed), as #238's table invites |
| #263 | kept (M10, `owner-todo`) | its findings are about `owner-issues.sh`, which this ADR leaves unchanged; the handled marker it hardens is the one invariant 1 keeps |
| #217 | kept (M10) | the same feed's tag filter; unchanged here |
| #482 | kept, with a requirement (below) | ADR 0018 task 3 joins picks with journal records; an owner feed pick without a journal is a session, not a gap. The `design-window` session posts the requirement on #482 with the tasks |

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | `cargo xtask next-session` owner routes: the `owner-work` input, the *routed* state (invariant 1), the `owner-routed #N` picks for issues and failing PRs at the head of step 1 (invariant 3), items with a route left out of today's per-item picks; specs/tooling/next-session.md; `--from` fixtures for every term of Verification's first list that this task adds; ops/TICK.md §0: a session working an owner item that needs a PR may route it instead of doing it (the route's shape), and the route's PR body links the item as today. Create the `owner-work` label | #522 |
| 2 | `cargo xtask next-session` owner feed pick: `owner-feed` replaces `owner-issue`, `owner-comment` and `owner-done` (the pick terms above: quiet period, trickle bound, issues at once, 25 items, the closed-issue term); the work mix leaves `owner-feed` out; ops/TICK.md: a new section for the owner feed session (allowed actions, bound, the journal rule) and §2's pick list; AUTOMATION.md and ADR 0011 §1's table row by reference to this ADR | 1 |
| 3 | Proposal `ops/proposals/guard-reactions.md`: the guard allows the reaction writes of Verification (exact shapes) with its tests (ADR 0014); once the owner has applied it, the ops/TICK.md §0 text: 👀 and 🚀 per invariant 4, and "picked up" replies only for review bodies, which take no reaction. Until then sessions skip reactions | #522 |
| 4 | Proposal `ops/proposals/ci-records-fast-path.md`: the records-only classifier (run from the base commit) and the `ci.yml` and `judge-verdict` changes, with its tests (ADR 0014) | #522 |

Tasks are process work: by ADR 0019 clause 1 they go to `Ops`, unless a term holds when they are
filed or the owner picks a milestone for them. Task 3's TICK text after the owner's application
is part of task 3's issue, worked when the owner's merge is on main (it carries `owner-todo`
while the proposal waits, ops/TICK.md §0).

## What reviewers check against
New principle, in docs/PRINCIPLES.md:

> **P10.** Owner items move through text states; one session routes them; reactions are display
> - **Invariant:** an owner item (a line of `ops/driver/owner-issues.sh`) is *handled* only by a
>   tagged `jacobdam` reply `🤖 jpdf-agent done: <its URL>`, and *routed* only by an open PR or
>   issue authored by `jacobdam` whose tagged body has the line `Owner instruction: <its URL>`
>   (exact URL; an issue also labeled `owner-work`), or by the label `owner-work` on the untagged
>   owner issue itself; every other item is *unrouted*. The scheduler computes these states from
>   that text and those labels only, and nothing reads a reaction as state. The owner feed
>   session takes every unrouted item its brief lists and leaves each one handled (carried out,
>   or a question back) or routed; it writes no code, spec, ADR, test or proposal, and writes a
>   journal entry only for an `Ignored:` line, a lesson or an item left unrouted. Routes keep
>   rule a's priority (precedence step 1).
> - **Decided in:** ADR 0020 (amending ADR 0011 §1 and §2, ops/TICK.md §0).
> - **Enforced by:** `cargo xtask next-session`'s step 1 terms and their `--from` fixtures (ADR
>   0020 tasks 1 and 2); the guard's exact reaction shapes (task 3, an `ops/proposals/` change);
>   the rest is reviewer vigilance.
> - **Reviewers check:** a scheduler change reads a route only from a `jacobdam` author with a
>   tagged body, the exact `Owner instruction:` URL and (for an issue) `owner-work`, and treats a
>   closed route as unrouted; no rule or tool reads a reaction, or counts a reaction as a reply;
>   a TICK passage for the owner feed session adds no output beyond ADR 0020's allowed actions;
>   a session type that stops writing its journal shows that no scheduler term or metrics join
>   reads its records.
> - **Paths:** `xtask/src/next_session.rs`, `xtask/src/next_session/`, `ops/TICK.md`,
>   `AUTOMATION.md`, `ops/proposals/`.

## Requirements on future callers
- **ADR 0018 task 3 (#482)** and any later metrics join: a `picks.tsv` line whose first line
  starts `owner-feed`, with a driver session and no journal record, is an owner feed session,
  never an unrecorded or lost one.
- **A new owner command** (beyond `/hold`, `/release`, `/approve`) is an owner item like any
  other: the owner feed session carries it out or routes it, and a rule that reads it reads the
  untagged `jacobdam` text, not a reaction.
- **A new kind of route** (a label other than `owner-work`, a route in a comment) needs a
  scheduler term and fixture first; until then the item is unrouted, so the owner feed session
  takes it again.
- **A session type that skips its journal** (beyond the owner feed) first shows that no
  scheduler term and no metrics join reads its records (steps 8 and 11, the work mix, ADR 0018).

## Consequences
- **Fewer sessions.** A burst of owner comments is one owner feed session; the eight "done:"
  sessions of 2026-11-28 become the replies of whichever owner feed session runs after each
  merge, usually batched. With no journal for a plain run, those sessions open no PR.
- **One more step for work that needs a PR.** An owner item that needs code or an ADR is routed
  first and worked by the next pick, where today one session does both. The route keeps rule
  a's priority, so the delay is one session, and the routing is shared by the batch. Today's
  case, an endorsed design issue (#463, #493, this one), becomes: route (label the issue
  `design` and `owner-work`), then a Design: technical session picked by the route.
- **Slower first answer.** An owner comment waits up to its quiet period (10 minutes) or, in a
  steady trickle, an hour before its session; another session may run in between. An owner
  issue is picked at once.
- **Fewer comments, same trace.** "picked up" comments give way to 👀 (task 3) and the route's
  cross-reference; every `done:` still links what changed. A bare 🚀 without a `done:` reply is
  not offered: the feed reads only text, and changing that would make the shared account's
  reactions (the owner's included) decide what is handled.
- **Records PRs get cheap, not rare.** Sessions with a records PR (a Planner run with nothing
  to change, an owner feed session with a lesson) still open one; the fast path makes its CI
  short. The main run each merge starts is unchanged.
- **Not covered.** A session that routes an item to the wrong work, or posts a `done:` for
  something it didn't do, passes every check; P10 and the owner's own reading catch it. The
  sessions that found their work already taken by the owner's interactive session (#502, #507,
  #521) are rule c and Planner picks, not owner items; this ADR doesn't change them.

## Success check (the first retro whose period starts after tasks 1, 2 and 4 merge)
- `Records:` PRs on main: at most 4 per day on average over the period (14 on 2026-11-28).
- Owner-instruction sessions: the `owner-feed` and `owner-routed` picks in `ops/state/picks.tsv`
  are at most half the owner items handled in the period (18 owner picks for 10 items on
  2026-11-28).
- Every owner item from the period is handled or routed, and none waited more than 2 hours
  unrouted while sessions ran (feed timestamps against `picks.tsv`).
- Every `done:` reply that names a change links it (the retro samples 10).

If the first two fail, the retro reports where the sessions went (a quiet period too short, a
route kind the scheduler doesn't read); if the third fails, the quiet-period and trickle numbers
are a scheduler bug with a failing fixture.
