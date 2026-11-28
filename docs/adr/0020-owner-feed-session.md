# 20. One session routes every pending owner item; handled and routed stay text

Status: proposed (design issue #529, `Designed in: #522`; escalated from #522 / PR #524, whose
design issue #522 was queued by the owner: filed by the owner's interactive session and
endorsed by the owner's untagged comment
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
- Each records PR runs the PR CI (PR #521: 10:42:31 to 10:46:32 UTC, the Linux judge jobs
  `reference`, `policy`, `untrusted`, `project-linux` and `judge-verdict`; `project-macos` and
  `project-nightly` skip on PRs) and then a full `ci` run on main when it merges. Main runs queue
  behind each other: the runs for b23aba1 (#504) and f134fe4 (#506) were cancelled at 10:18:44
  and 10:18:54 UTC while waiting behind the run of records PR #507's merge (10:13:51 to
  10:19:12).

The class: **an owner item's state lives in a session instead of on GitHub.** The scheduler can
see "pending" (no `done:` reply) and nothing finer, so it hands each item to a full Worker
session, and only that session can move it on. The owner's four ideas in #522 (one session for
the whole feed, reactions as acks, no journal when there is nothing to learn, a cheap path for
records PRs) are four cuts of the same cost.

The first draft of this ADR (PR #524) reached the review cap (rounds 1 to 4 and a round 5 final
check; ledger on PR #524 and in #522's `escalated to design: PR #524` comment). Round 1's four
majors were fixed and stayed fixed. Every later major came from one seam: R2.1, R3.1, R4.1 and
R5.1 (and R4.2, which the final check found still open) are each about routing a review body,
or about a route that names a PR competing with rule c.

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
- **Routing review bodies fails (PR #524 rounds 2 to 5).** A review body has a different lifetime
  from every other item: the feed lists it only while its PR is open (a limit
  `ops/driver/owner-issues.sh` states), and GitHub's REST API has no reactions endpoint for it
  (so task 3's guard shapes can't ack it). Each fix of the draft added a term for that lifetime,
  and each term opened the next gap:
  - R2.1: a route stopped counting when the review's PR closed, so the review lost its priority;
    the fix read the PR's reviews in the scheduler.
  - R3.1: once the route also closed, nothing listed the review to post its `done:`; the fix
    posted `done:` at routing, before the work.
  - R4.1: an early `done:` let a green PR merge the approach the review rejected; the fix added
    a `Rework: #<n>` route that drafts the PR.
  - R4.2: with `done:` already posted, a route closed unfinished told nobody.
  - R5.1: rule c still picked the PR that a parked `Rework:` route names.

  The missing decision is not another term: a review body is the one item kind whose lifetime
  doesn't fit a route, so it isn't routed. It keeps today's per-item pick, whose behavior is
  known. The other route that names a PR (rework asked in a comment, routed by the PR's own
  body) is picked by rule c's own failing-PR test, so a skip label parks both picks at once.

The missing decision: **a pending owner item, other than a review body, has two text states the
scheduler can read, *routed* and *handled*, and one session moves every unrouted item to one of
them; a routed PR is picked by the same test as rule c.**

## The invariant
1. **States.** An *owner item* is a line of the feed (`ops/driver/owner-issues.sh`, unchanged)
   that today's precedence step 1 already considers, other than a review body, and only those:
   an owner issue (kind `issue`, `is_pr` false) without a skip label (`owner-todo`, `blocked`,
   `deferred`: it is parked, the owner's turn, and comes back when the label goes, as today); an
   owner comment or inline review comment created at or after the feed's `FLOOR` (earlier ones
   need no reply, TICK §0, and the feed ignores `done:` replies for them, so they are never
   items); not an `/approve` that step 3's design window counts (#372). A review body (URL
   `…/pull/<n>#pullrequestreview-<id>`) is no owner item: invariant 5 keeps it as today. Each
   owner item is in one of three states, computed by `cargo xtask next-session` from tagged text
   and labels only:
   - **handled:** a tagged `jacobdam` reply `🤖 jpdf-agent done: <its URL>` exists (today's
     marker, read by the feed itself);
   - **routed:** not handled, and its work is carried by an open item the scheduler reads: an
     open PR or issue authored by `jacobdam`, with a tagged body holding a line
     `Owner instruction: <its URL>` (exact URL, then whitespace or the end), and for an issue
     the label `owner-work`; an untagged owner issue labeled `owner-work` routes itself, and is
     routed only that way (a closed owner issue's `done:` is found by that label, term 4 below);
   - **unrouted:** neither.

   A *route* is such an open PR or issue, and it is a route only while the URL on its line is a
   pending owner item (not handled): a line naming a stranger's comment, a pre-floor comment, a
   handled item, a review body, an owner issue or any other URL makes no route, so it gets no
   rule a priority (its issue is worked, if at all, by the normal rules). An untagged owner issue
   labeled `owner-work` is a route too while it is a pending owner item; a *route issue* below
   means either kind of issue route, tagged with the line or the owner's own with the label.
2. **One session moves the unrouted ones.** The **owner feed** session takes every unrouted item
   the scheduler lists, oldest first, and leaves each one handled or routed before it exits. It
   writes no code, spec, ADR, test or proposal, and it takes no review body (its brief lists
   none). Its allowed actions:
   - a `done:` reply (TICK §0's form: what changed, then `waiting on owner: …`) after it carried
     the item out in place: labels, milestones, assignees, closing or reopening an issue, editing
     a tagged body, filing issues (without `ready`: the Planner gates them, P6; work the owner
     wants now is a route, below), a tagged comment elsewhere, a `/hold` or `/release`
     follow-up (ops/TICK.md §5: `owner-todo`, auto-merge after a `/release`), a "done:" for an
     item whose route closed;
   - a **question back**: a `done:` reply that asks one question and ends
     `waiting on owner: an answer to the question above`; the owner's answer is a new item;
   - a **route**: file a tagged issue with the `Owner instruction:` line and `owner-work` (plus
     `design` or `design-product` when the work is a design), or add `owner-work` to an untagged
     owner issue, or, for rework of an open agent PR asked in a comment or an inline review
     comment on it or on the route issue it links (`Closes #N`), add the line to the PR's body
     (no new route issue: the PR is already the route issue's work), disable its auto-merge and
     post `ai-review` failure with a link to the item. It doesn't turn the PR into a draft: the
     failing required check already stops the merge, and a draft would change which branch of
     step 3's test applies (a design PR's window `owner-todo` parks a draft, below).
3. **Routed work keeps rule a's priority, by rule c's own test for PRs.** Precedence step 1 picks
   the routes after the review bodies (invariant 5) and before every other step: an open
   `owner-work` issue that no open PR links and that carries no skip label, as `worker
   owner-routed #N` (`design-technical owner-routed #N` with `design`, `design owner-routed #N`
   with `design-product`); an open routed PR, or an open PR that links (`Closes #N`) an open
   route issue of either kind, that step 3's failing-own-PR term would pick, as `worker
   owner-routed #<PR>`. That test is shared, not copied (task 1 extracts it from the closure in
   `choose` into one function that both steps call): failing, not held, not on an `owner/`
   branch, not `needs-design`, and no skip label on the PR or on the issue it closes, except that
   the issue's `owner-todo` doesn't park a design PR that is not a draft (its 24-hour window's
   own label, #372). So one skip label parks a routed PR for step 1 and step 3 alike, and
   removing it brings back one pick, the route; a design PR reworked in its window stays a route
   that is picked, since the rework route doesn't draft it (invariant 2). While a route is parked
   or green its item stays routed (no owner feed pick for it); it moves again when the label goes
   or the PR fails. A route is not picked while an unrouted owner item sits on it (a comment or
   an inline review comment on the route issue or PR, or on an open PR that links the route
   issue; the scheduler reads where an item sits from the `/issues/<n>` or `/pull/<n>` segment of
   its URL, matched as a whole number): that item may change the work, so the owner feed session
   reads it first, and it is picked without the quiet period (below). A review body on a route
   waits behind an older unrouted item on the same PR (invariant 5). When the route closes or
   merges, the item is unrouted again, and the next owner feed session posts its `done:` (or
   routes it again when the PR closed unmerged).
4. **Reactions are display, never state.** The owner feed session (and any session that routes
   or handles an item) adds 👀 to a routed item and 🚀 to a handled one where GitHub allows a
   reaction (issues, issue comments, inline review comments). No script reads a reaction, and a
   reaction replaces no reply that the feed or the scheduler reads.
5. **Review bodies keep today's per-item pick.** A review body from `FLOOR` on with no `done:`
   reply, not named by the body of an open agent PR that is not failing (or is parked or
   `needs-design`; today's in-flight test), is picked at once as `worker owner-comment <url>`,
   first in step 1, unless an older unrouted owner item sits on the same PR `<n>` (a comment or
   inline review comment whose URL has `/pull/<n>`, or one on the issue that PR closes): then
   that older item goes first, as an `owner-feed` pick at once (term 3), so the review's session
   never works before an older correction is read. This keeps today's oldest-first order among
   the items on one PR. Across different PRs and issues the order changes: today owner issues and
   older comments anywhere come first, and under this ADR a review body comes before the owner
   feed session (which takes the others in one batch). That session gives it ops/TICK.md §0's two
   replies and does its work as today, including the rework of the PR it sits on. Its known limit
   is unchanged: the feed drops a review body when its PR closes, so a `done:` not posted by then
   is never asked for (`ops/driver/owner-issues.sh` states it).

## Enforcement
1. **types:** not available: the states live on GitHub, written by sessions through `gh`.
2. **a lint or gate:** the scheduler is the gate (ADR 0011 §2). It computes the three states from
   `feed.jsonl`, one new input (the open issues labeled `owner-work`, with author, body and
   labels), one new field (`labels` on `owner-closed.json`, today `number,body,comments`, for
   term 4) and the bodies of open PRs, which `prs.json` already has, so an item counts as routed
   only when a `jacobdam` route with the exact line and label exists; a stranger's tagged issue,
   a closed route, a line naming a review body or a line in a comment route nothing. A routed PR
   is picked through one function that step 3 also uses for failing own PRs (task 1 extracts it
   from step 3's inline closure), so the two terms can't drift. Tested offline with `--from`
   fixtures (task 1, task 2). The guard needs one protected change to allow the reaction writes
   (task 3), and the records fast path is a protected CI change (task 4); both go through
   `ops/proposals/`.
3. **a measurement:** the success check below, counted from `git log origin/main`,
   `ops/state/picks.tsv` (ADR 0018) and the journal.
4. **reviewer vigilance:** principle P10 (below) for the session's own judgement: whether an item
   was really carried out before its `done:`, and whether a route names the right work.

**The pick.** Precedence step 1 becomes, in this order:
1. the review bodies of invariant 5 (today's `worker owner-comment` term, for review bodies
   only), oldest first;
2. the routes of invariant 3 (lowest number first);
3. `owner-feed` when at least one item is unrouted and either the newest unrouted item with a
   time (`created_at`) is at least 10 minutes old (a quiet period, so a burst of owner comments
   is one session), or the oldest is at least 60 minutes old (so a steady trickle still gets a
   session each hour), or an unrouted owner issue has no time in the feed, or an unrouted item
   suspends a pick (invariant 3: on a route that step 1.2 would pick without it; invariant 5: an
   older item on the PR of a pending review body); the last two are picked at once. An item on a
   route that step 1.2 wouldn't pick even without it waits for the quiet period like any other:
   a green or parked routed PR, a parked route issue that no open PR links, or a route issue
   whose linking PR is green or parked (by the shared test, so a design PR in its window is not
   parked by its issue's `owner-todo`).
   The brief lists every unrouted item, oldest first, at most 25 (the rest wait for the next
   pick), and notes the items waiting for the quiet period when it doesn't pick;
4. the closed owner issues of today's `owner-done` term, read as unrouted items of (3): a closed
   owner issue of the last 14 days, labeled `owner-work` or with a legacy "picked up" reply, and
   no "done:" reply (owner issues are routed only by their own label, invariant 1, so this finds
   every routed one).

`worker owner-issue #N` and `worker owner-done #N` are gone, and `worker owner-comment <url>`
remains for review bodies only; an `/approve` that step 3's design window counts stays out of
step 1 (#372, unchanged).

**The journal rule.** An owner feed session writes a journal entry only when it has something a
later session needs: a non-empty `Ignored:` line (CONSTITUTION 1's trace of a non-owner request),
a lesson, or an item it left unrouted (the session died midway, a guard block). Otherwise it
writes none and opens no PR; its trace is the replies on GitHub and the scheduler's own line in
`ops/state/picks.tsv`. This is safe because the scheduler counts no owner feed record: they
are not Planner records (steps 8 and 11), and the work mix leaves owner sessions out (ADR 0011
§3; task 2 adds `owner-feed` to the left-out words). Every other session type, the review-body
pick and the `owner-routed` picks included, writes its journal as today.

**The records fast path.** A PR whose every changed path is a record path (a stricter subset of
ADR 0011 §3's record commit, applied to the PR's whole diff against its base: each path added
or modified under `ops/journal/YYYY-MM-DD/<name>.md`, or added as `ops/lessons/pending/<name>.md`
other than `README.md`, each a regular file of mode 100644, no rename, delete or other mode)
skips the build, test and judge jobs, and `judge-verdict` passes it with the reason "records
only". The legacy `ops/journal/YYYY-MM-DD.md` files, which §3 also accepts, take the full run.
The classifier runs from the base commit's copy, as the `policy` job reads pinned config from
the base (LESSONS, PR #444 R1.2), so a PR can't change the rule that grades it. Pushes to main
keep the full run (main red, rule b, reads them).

## Verification
- **Scheduler (tasks 1 and 2).** A fixture that selects each term and the nearest miss that
  doesn't:
  - routes: a route by a stranger with the tag; a route issue with the line and no
    `owner-work`; `owner-work` with the line naming another URL, or the URL followed by a digit
    (`…-1234` for `…-123`); a route whose line names a stranger's comment, one whose item is
    already handled, one naming an owner review body (the review is still picked by step 1.1),
    and one naming an owner issue's URL (no route in all four; the owner issue is still an
    unrouted item); a closed route (the item is unrouted again, so `owner-feed`);
  - routed PRs, by the shared test: every fixture of step 3's failing-own-PR term rerun with the
    PR as a route picks `worker owner-routed #N` exactly when step 3 picks the PR. Named among
    them: green (no pick), failing (picked), held (no pick), `needs-design` (no pick), on an
    `owner/` branch (no pick); its closed issue labeled `owner-todo`, `blocked` or `deferred` (no
    pick by step 1 or step 3), and the same with the label removed (one pick, the route); a route
    issue labeled `owner-todo` that a failing PR links (no pick of either); a failing PR that
    links an open route issue while main is red (picked as `worker owner-routed #<PR>`, ahead of
    main red), the same with an untagged owner issue labeled `owner-work` as the route issue
    (picked the same way), and the same PR green (no pick); a routed design PR in its window, not
    a draft, failing after the rework route, its issue with the window's `owner-todo` (picked),
    and the same PR as a draft (no pick by step 1 or step 3);
  - items: an owner issue labeled `owner-todo`, `blocked` or `deferred` (no item; one pick after
    the label goes); a pre-floor owner comment updated within the last 14 days, so the feed
    still shows it (never an item);
  - suspension: a route with an unrouted owner comment on it, an inline review comment, and a
    comment on the PR that links it (no route pick, `owner-feed` at once); the same comment once
    handled (the route is picked again); a comment on a route issue whose linking PR is failing
    (`owner-feed` at once, the PR not picked by step 1 or step 3 meanwhile; once the session
    routes that comment into the PR's body, the PR is picked as `worker owner-routed #<PR>` and
    no new route issue exists) and on one whose linking PR is green (it waits for the quiet
    period); a comment on a route issue labeled `owner-todo` whose linking design PR is in its
    window, not a draft and failing (`owner-feed` at once), and on a route issue labeled
    `owner-todo` that no open PR links (it waits for the quiet period); a comment on a green
    routed PR (it waits for the quiet period); a comment on issue #5 while the route is #52, and
    on #520 (no suspension);
  - review bodies: an owner review body on an agent PR (picked at once as `worker owner-comment`,
    and never in the `owner-feed` brief); the same with a tagged `jacobdam` `done:` naming it
    (no pick); named by the body of an open agent PR that is green (no pick) or failing (picked);
    a review body on a failing routed PR (the review is picked, not the route); a stranger's, a
    tagged or a pre-floor review (no pick); an owner comment on PR #N older than a review body on
    #N (`owner-feed` at once, the review not picked), the same comment once handled or routed
    (the review picked), and an older owner comment on another PR (the review picked first);
  - the pick: the quiet period at 9:59 and 10:00 minutes, the trickle bound at 59:59 and 60:00,
    an issue item without a time; 25 and 26 items in the brief; a closed owner issue with
    `owner-work`, with a legacy "picked up", with neither, and with `owner-work` and a `done:`
    (no pick), read from `owner-closed.json`'s new `labels` field.

  What broken implementation still passes: one that treats a route that a session filed for
  the wrong item (wrong URL by mistake, right shape) as correct. Only P10's reviewer line and
  the owner's view (👀 on the comment, the cross-reference GitHub shows on the route) catch it.
  A routed-PR test copied from step 3 instead of shared would pass the named fixtures today and
  drift later; the shared-test fixture set (every step 3 fixture rerun as a route) is what
  catches a copy that differs.
- **The journal rule.** The retro of the first full day after task 2 counts owner feed picks in
  `ops/state/picks.tsv` against the driver's `ops/logs/index.tsv` rows and the journal: each
  pick is a session with replies on GitHub or a journal entry. A session that did nothing and
  wrote nothing passes; picks.tsv is local and forgeable (ADR 0018), so the check is evidence,
  not proof.
- **Guard (task 3).** The proposal's tests (ADR 0014) allow exactly
  `gh api -X POST repos/jacobdam/jpdf3/issues/<n>/reactions`, `…/issues/comments/<n>/reactions`
  and `…/pulls/comments/<n>/reactions` with one field `content` of `eyes` or `rocket`, and block
  each near miss: another repo, another content value, a second field (`-f body=…`), the field
  given by `-F`, `--field` or `--raw-field` with another name, `--input <file>` or `--input -`,
  `--method=PATCH` or `DELETE`, an extra `-H` header, `--hostname`, a query string, a path with
  `..` or a trailing segment, the same path under `pulls/<n>/`. Each allowed form is tested with
  `-X POST` and `--method=POST`.
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
| #529 | kept, closed by this ADR's PR | the design issue for the escalated draft |
| #522 | closed as folded into #529 | the first design issue; its draft PR #524 reached the cap, and its owner comment's `done:` follows this ADR's merge (this PR's body links it) |
| #238 | kept | the umbrella of session types; this ADR adds one (the owner feed), as #238's table invites |
| #263 | kept (M10, `owner-todo`) | its findings are about `owner-issues.sh`, which this ADR leaves unchanged; the handled marker it hardens is the one invariant 1 keeps |
| #217 | kept (M10) | the same feed's tag filter; unchanged here |
| #236, #433, #438, #463, #465, #466, #478, #493 | not changed (evidence) | their two-session instructions are the cost counted above; each is closed or proceeds under its own ADR |
| #482 | kept, with a requirement (below) | ADR 0018 task 3 joins picks with journal records; an owner feed pick without a journal is a session, not a gap. The `design-window` session posts the requirement on #482 with the tasks |

PR #524 (the draft) is closed unmerged; its ledger is this ADR's input. It had no tasks filed.

## Tasks
| # | Task (one issue, one PR, ≤ 1500 added lines, at most one new trait family, tool or crate) | Depends on |
|---|---|---|
| 1 | `cargo xtask next-session` owner routes: the `owner-work` input, the *routed* state (invariant 1), the `owner-routed` picks for issues and for failing routed PRs and PRs of route issues through one failing-own-PR function extracted from step 3 and shared with it (invariant 3), review bodies kept first in step 1 under today's term except behind an older unrouted item on the same PR (invariant 5), items with a route left out of today's per-item picks; specs/tooling/next-session.md; `--from` fixtures for every term of Verification's first list that this task adds; ops/TICK.md §0: a session working an owner item that needs a PR may route it instead of doing it (the route's shape), and the route's PR body links the item as today. Create the `owner-work` label | #529 |
| 2 | `cargo xtask next-session` owner feed pick: `owner-feed` replaces `owner-issue`, `owner-done` and the `owner-comment` pick for comments and inline review comments (the pick terms above: quiet period, trickle bound, issues at once, suspension at once, 25 items, the closed-issue term with the `labels` field on `owner-closed.json`); the work mix leaves `owner-feed` out; ops/TICK.md: a new section for the owner feed session (allowed actions, bound, the journal rule) and §2's pick list; AUTOMATION.md and ADR 0011 §1's table row by reference to this ADR | 1 |
| 3 | Proposal `ops/proposals/guard-reactions.md`: the guard allows the reaction writes of Verification (exact shapes) with its tests (ADR 0014); once the owner has applied it, the ops/TICK.md §0 text: 👀 and 🚀 per invariant 4, and "picked up" replies only for review bodies, which take no reaction. Until then sessions skip reactions | #529 |
| 4 | Proposal `ops/proposals/ci-records-fast-path.md`: the records-only classifier (run from the base commit) and the `ci.yml` and `judge-verdict` changes, with its tests (ADR 0014) | #529 |

Tasks are process work: by ADR 0019 clause 1 they go to `Ops`, unless a term holds when they are
filed or the owner picks a milestone for them. Task 3's TICK text after the owner's application
is part of task 3's issue, worked when the owner's merge is on main (it carries `owner-todo`
while the proposal waits, ops/TICK.md §0).

## What reviewers check against
New principle, in docs/PRINCIPLES.md:

> **P10.** Owner items move through text states; one session routes them; reactions are display
> - **Invariant:** an owner item (a line of `ops/driver/owner-issues.sh` that precedence step 1
>   considers, other than a review body: an owner issue without `owner-todo`, `blocked` or
>   `deferred`, or an owner comment or inline review comment from the feed's `FLOOR` on, not an
>   `/approve` that the design window counts, #372) is *handled* only by a tagged `jacobdam`
>   reply `🤖 jpdf-agent done: <its URL>`, and *routed* only by an open PR or issue authored by
>   `jacobdam` whose tagged body has the line `Owner instruction: <its URL>` (exact URL; an issue
>   also labeled `owner-work`), or by the label `owner-work` on the untagged owner issue itself,
>   and only while that URL is a pending owner item; every other item is *unrouted*. The
>   scheduler computes these states from that text and those labels only, and nothing reads a
>   reaction as state. An owner issue is routed only by its own label. A routed PR, or the PR of
>   a route issue, is picked only through the failing-own-PR function step 3 also uses, so a
>   skip label on it or its issue parks both picks. A review body is never routed: it keeps
>   today's per-item pick, first in step 1 unless an older unrouted owner item sits on its PR,
>   which the owner feed session reads first. The owner feed session takes every unrouted item its
>   brief lists and leaves each one handled (carried out, or a question back) or routed; it
>   writes no code, spec, ADR, test or proposal, and writes a journal entry only for an
>   `Ignored:` line, a lesson or an item left unrouted. Routes keep rule a's priority
>   (precedence step 1), except while an unrouted owner item sits on the route, which the owner
>   feed session reads first.
> - **Decided in:** ADR 0020 (amending ADR 0011 §1 and §2, ops/TICK.md §0).
> - **Enforced by:** `cargo xtask next-session`'s step 1 terms and their `--from` fixtures (ADR
>   0020 tasks 1 and 2); the guard's exact reaction shapes (task 3, an `ops/proposals/` change);
>   the rest is reviewer vigilance.
> - **Reviewers check:** a scheduler change reads a route only from a `jacobdam` author with a
>   tagged body, the exact `Owner instruction:` URL of a pending owner item and (for an issue)
>   `owner-work`, treats a closed route as unrouted, keeps the floor and skip-label terms of an
>   owner item, keeps review bodies out of routes and the owner feed brief and behind an older
>   item on their PR, and picks a routed PR or a route issue's PR through the same function as
>   step 3, not a copy of it; no rule or tool reads a reaction, or counts a reaction as a reply;
>   a TICK passage for the owner feed session adds no output beyond ADR 0020's allowed actions;
>   a session type that stops writing its journal shows that no scheduler term or metrics join
>   reads its records.
> - **Paths:** `xtask/src/next_session.rs`, `xtask/src/next_session/`, `ops/TICK.md`,
>   `xtask/tests/next_session.rs`, `specs/tooling/next-session.md`, `AUTOMATION.md`,
>   `ops/proposals/`.

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
- **A new owner item kind** (a feed line the scheduler doesn't read yet) first shows its
  lifetime against a route's: listed until handled, on what object, and whether that object can
  close first. If it can, it stays a per-item pick like a review body (invariant 5).
- **A new term that picks a PR** for rule a's priority picks through step 3's failing-own-PR
  test, so a skip label parks it with rule c.
- **A session type that skips its journal** (beyond the owner feed) first shows that no
  scheduler term and no metrics join reads its records (steps 8 and 11, the work mix, ADR 0018).

## Consequences
- **Fewer sessions.** A burst of owner comments is one owner feed session; the eight "done:"
  sessions of 2026-11-28 become the replies of whichever owner feed session runs after each
  merge, usually batched. With no journal for a plain run, those sessions open no PR.
- **One more step for work that needs a PR.** An owner item that needs code or an ADR is routed
  first and worked by the next pick, where today one session does both. The route keeps rule
  a's priority, so the delay is one session, and the routing is shared by the batch. Today's
  case, an endorsed design issue (#463, #493, #522), becomes: route (label the issue `design`
  and `owner-work`), then a Design: technical session picked by the route.
- **Slower first answer.** An owner comment waits up to its quiet period (10 minutes) or, in a
  steady trickle, an hour before its session; another session may run in between. An owner
  issue, a review body and a comment that suspends a route are picked at once.
- **A comment is not a stop.** On a green agent PR with auto-merge on, an owner comment waits
  for the quiet period, and the PR can merge first. The owner's immediate stop is `/hold`, which
  a workflow applies at once (ops/TICK.md §5). Today a comment also waits for the next pick, so
  this is a longer wait, not a new gap.
- **Review bodies keep their cost** (owner idea 1 not fully adopted). #522 asked the owner feed
  session to take "PR reviews or approvals" too. A review body keeps one session of its own and
  today's two replies, because its lifetime doesn't fit a route (Why point fixes don't converge).
  The feed listed none at the time of writing (2026-11-28 11:40 UTC); an approving review with
  no text is not in the feed, and an `/approve` is a comment, so approvals are owner items as
  the owner asked.
- **Fewer comments, same trace.** "picked up" comments give way to 👀 (task 3) and the route's
  cross-reference; every `done:` still links what changed. A bare 🚀 without a `done:` reply is
  not offered (owner idea 2 not fully adopted): the feed reads only text, and changing that would
  make the shared account's reactions (the owner's included) decide what is handled.
- **Records PRs get cheap, not rare.** Sessions with a records PR (a Planner run with nothing
  to change, an owner feed session with a lesson) still open one; the fast path makes its CI
  short. The main run each merge starts is unchanged.
- **Not covered.** A session that routes an item to the wrong work, or posts a `done:` for
  something it didn't do, passes every check; P10 and the owner's own reading catch it. A review
  body whose PR closes before its `done:` is never asked for again (invariant 5, today's limit).
  The sessions that found their work already taken by the owner's interactive session (#502,
  #507, #521) are rule c and Planner picks, not owner items; this ADR doesn't change them.

## Success check (the first retro whose period starts after tasks 1, 2 and 4 merge)
- `Records:` PRs on main: at most 4 per day on average over the period (14 on 2026-11-28).
- Owner-instruction sessions: the `owner-feed`, `owner-routed` and review-body `owner-comment`
  picks in `ops/state/picks.tsv` are at most half the owner items and review bodies handled in
  the period (18 owner picks for 10 items on 2026-11-28).
- Every owner item from the period is handled or routed, and none waited more than 2 hours
  unrouted while sessions ran (feed timestamps against `picks.tsv`).
- Every `done:` reply that names a change links it (the retro samples 10).

If the first two fail, the retro reports where the sessions went (a quiet period too short, a
route kind the scheduler doesn't read); if the third fails, the quiet-period and trickle numbers
are a scheduler bug with a failing fixture.
