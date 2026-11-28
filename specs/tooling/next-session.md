# cargo xtask next-session

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; docs/adr/0011-session-types-and-scheduler.md §2, ops/TICK.md §2)
Status: implemented

## Behavior
`cargo xtask next-session [--from DIR]` decides what the session does, so sessions stop picking
their own work from prose (ADR 0011 §2). Task 1a (#242) added precedence steps 1–5 (TICK rules
a–f); task 1b part 1 (#273) added steps 7 (rules g, h) and 10 (rule i) and the interim
product-merge counter (ADR 0011 §3); part 2 (#243) adds step 11 (Planner or idle), which
replaces the `legacy` exit, and reads the `Session:` records of ADR 0011 §3. Task 3 part 2
(#275) adds step 6 (Design: technical) and takes `needs-design` PRs out of step 3. Task 2 part 1 (#244) adds step 8's Planner triggers (a milestone with no plan, 7
days without a Planner run) and widens `records.txt` to 7 days. Task 2 part 2 (#288) adds the
work-mix budget of ADR 0011 §3: step 8's third term and step 10's product-first order. Task 6
part 1 (#249) adds step 8's Design: product terms (ops/TICK.md §10) after the Planner's. Task 3
part 3 (#314) adds step 3's third term: an own design PR past its 24-hour window (ADR 0011 §7),
which TICK §1 checked by hand before. #373 counts that term's `ai-review` success from the
triage comment it links, so a record-only update doesn't restart the window. #372 adds the
owner's `/approve`, which ends that window early, and lets step 3's first term pick a design
PR that fails in its window. ADR 0015 task 1 (#440) adds step 11's rerun: on a day the Planner
already ran, `planner new-issues` for an issue filed after its newest run, at most 3 Planner
runs a UTC day.

- **Output.** Exit 0 and, on stdout, a first line (the **pick**) and a brief:
  - first line: `<type> <step> <reference>`, `audit due`, `retro due`, `planner due`,
    `planner`, `planner new-issues` or `idle`.
    Its first word is the session type: `worker`, `design-technical`, `audit`, `retro`,
    `planner`, `design` (Design: product, TICK §10), or `idle` (no session: exit at once).
    Steps and references:

    | Step | TICK rule | First line |
    |---|---|---|
    | 1 | a | `worker owner-issue #N`, `worker owner-comment <url>`, `worker owner-done #N` |
    | 2 | b | `worker main-red run <databaseId>` |
    | 3 | c | `worker own-pr #N`, `worker scheduler-bug #N`, `worker design-window #N` |
    | 4 | d | `worker crash-security #N` |
    | 5 | e, f | `worker flaky #N`, `worker leftover-worktree <branch>` |
    | 6 | (§2 step 6, TICK §8) | `design-technical needs-design #N` (a PR), `design-technical design-issue #N` |
    | 7 | g, h | `audit due`, `retro due` (the reason is on the `Rule:` line) |
    | 8 | j, k | `planner due` (the reason is on the `Rule:` line), then `design product v1`, `design product v1 #N`, `design product #N` |
    | 10 | i | `worker ready #N` |
    | 11 | j | `planner`, `planner new-issues` (ADR 0015), `idle` |

  - then a blank line and the brief: a `Rule:` line (the TICK letter, or `step 6`, which has
    none, and why this item was picked), a `Load:` list (the commands and files to read, one per line, `- ` first), and a
    `Notes:` list when there is something to note (each open agent PR on the owner's `/hold`
    and each one on an `owner/` branch, which step 3 doesn't pick; an active judge alarm;
    worktrees whose branch's PR is merged or closed, which can be removed; a due audit or
    retro that step 7 didn't pick because its PR is open, and a due Design: product term that
    step 8 didn't pick for the same reason; on a `planner`, `planner due` or `planner new-issues`
    pick, last, the work-mix reading, which the Planner copies into `ops/QUEUE.md`, TICK §9).
- **Inputs.** It runs only read commands from the current directory (the main checkout). Each
  has a file name; `--from DIR` reads that file from `DIR` instead and runs no command, so every
  pick can be tested offline:
  1. `feed.jsonl`: the stdout of `ops/driver/owner-issues.sh` (JSON lines, the owner feed);
  2. `floor.txt`: the feed's `FLOOR` timestamp. Live, it is the value of the line
     `FLOOR="…"` in `ops/driver/owner-issues.sh` (one source; the file is protected);
  3. `owner-open.json`: `gh issue list --state open --author jacobdam --limit 1000 --json number,labels,comments`;
  4. `owner-closed.json`: `gh issue list --state closed --author jacobdam --search closed:>=<today − 14 days> --limit 1000 --json number,body,comments`
     (today is the UTC day from the system clock, as in `cargo xtask metrics`);
  5. `prs.json`: `gh pr list --author @me --state all --limit 200 --json number,state,isDraft,autoMergeRequest,createdAt,body,headRefName,labels,statusCheckRollup,mergeable`;
  6. `issues.json`: `gh issue list --state open --limit 1000 --json number,title,labels`;
  7. `runs.json`: `gh run list --workflow ci.yml --branch main --limit 20 --json databaseId,status,conclusion,event`;
  8. `worktrees.txt`: `git worktree list --porcelain`;
  9. `judge-alarm-since`: live, `ops/state/judge-alarm-since` (TICK rule b's local state file,
     the Unix time the alarm was raised). It is the only optional input: a missing file means
     no alarm;
  10. `now.txt`: Unix seconds; live, the system clock;
  11. `main-log.txt`: `git log --first-parent --no-renames --name-status --format=commit %H %ct %s origin/main`
      (the whole first-parent history; about 70 KB on 2026-11-23);
  12. `tags.txt`: `git for-each-ref --format=%(refname:short) %(creatordate:unix) refs/tags`;
  13. `milestones.json`: `gh api repos/jacobdam/jpdf3/milestones?state=open&per_page=100`;
  14. `blocked.json`: live, built from read commands: for each number in
      `gh issue list --label blocked --state all --search updated:>=<UTC day 7 days before now> --limit 100 --json number`,
      the array that `gh api repos/jacobdam/jpdf3/issues/<n>/events?per_page=100` prints, written
      as `[{"number":N,"events":[…]}, …]`;
  15. `records.txt`: session records (ADR 0011 §3), one line `<path>\t<line>` per line
      starting `- Session: ` of a journal file. Live: for `refs/remotes/origin/main` and, for
      each open agent PR in `prs.json`, `refs/remotes/origin/<headRefName>`,
      `git -c core.quotePath=true ls-tree -r --full-tree <ref> -- ops/journal/<day>/ …` with one
      `<day>` for today and each of the 6 days before it (today: the UTC day of the same clock
      reading as `now.txt`; `--full-tree` so the path is from the repository root wherever the
      command runs), then `git cat-file blob <id>` for each regular
      file (mode `100644`) listed whose path is `ops/journal/<day>/<name>.md` for one of those days, with a plain
      `<name>` (no `/`, not C-quoted: git quotes any unusual byte), each blob id and path once,
      so everything listed parses below. A blob that can't be read as text (not UTF-8, over the
      output cap, a failed `cat-file`) is skipped: its records are missed, which costs at most one
      more Planner run that day, never a stopped scheduler. The refs are what the
      session's last `git fetch` brought (TICK §1); a ref that doesn't exist (a PR branch pushed
      after that fetch) is a failed command, exit 2.
  16. `queue.md`: `ops/QUEUE.md` on origin/main (the Planner's plan, TICK §9). Live:
      `git ls-tree --full-tree refs/remotes/origin/main -- ops/QUEUE.md`; a failed command is
      exit 2. When it lists a regular file (`100644 blob <id>\tops/QUEUE.md`),
      `git cat-file blob <id>`. Optional, like `judge-alarm-since`: no such file on main, or a
      blob that can't be read as text (a failed `cat-file`, not UTF-8, over the output cap),
      means no plan, never a stopped scheduler (the Planner rewrites the file). With `--from`,
      a missing file means no plan; a file that isn't UTF-8 is exit 1 (a broken fixture).
  17. `holds.json`: `gh pr list --author @me --state open --limit 200 --json number,comments`
      (the conversation comments of each open PR, for the owner's `/hold`, #295).
  18. `pr-files.json`: `gh pr list --author @me --state all --limit 200 --json number,files`
      (the changed files of each PR, for the work mix, #288; about 96 KB and 9 s on 2026-11-24);
  19. `all-issues.json`: `gh issue list --state all --limit 1000 --json number,milestone` (the
      milestone of every issue, closed ones included, for the work mix);
  20. `areas.toml`: live, `ops/areas.toml` in the current directory (the main checkout), as
      `cargo xtask issue-lint` reads it: which `area:` labels are product (step 10).
  21. `design-ready.json`: live, built from read commands: for each open agent PR in `prs.json`
      labeled `design`, the `ready_for_review` events of
      `gh api --paginate --slurp repos/jacobdam/jpdf3/issues/<n>/timeline?per_page=100` (every
      page; other events are dropped, and each kept one only as `event` and `created_at`, which
      must be `YYYY-MM-DDTHH:MM:SSZ`), written as `[{"number":N,"events":[…]}, …]` (#314). With
      no such PR it is `[]` and no command runs.

  `issues.json` (input 6) also reads `body`, `milestone` and `createdAt` (step 11's rerun,
  ADR 0015):
  `gh issue list --state open --limit 1000 --json number,title,labels,body,milestone,createdAt`.

  Nothing is written anywhere: not to GitHub, git or the file system.
- **Common terms.**
  - *Tagged:* text whose body starts, after leading whitespace, with `🤖 jpdf-agent` (agent
    text; CONSTITUTION 1). An owner reply is a comment by `jacobdam` (the shared account).
  - *Skip labels:* `owner-todo` (the owner's turn), `blocked`, `deferred`.
  - *Agent PRs:* the entries of `prs.json` whose body is tagged. `--author @me` is the shared
    account, so untagged PRs there are the owner's and are ignored everywhere below.
  - *Linked issue* of a PR: the first `Closes #N` in its body with digits after the `#` (a
    quoted `Closes #<issue>` is skipped; TICK §3 step 4).
  - *In flight:* an issue is in flight when an open agent PR links it.
  - *Needs design:* an open agent PR labeled `needs-design` (exact name): escalated to a
    design session (TICK §4 step 6). It is a draft with `ai-review` failure on purpose, so it is
    failing too; step 6 picks it, never step 3.
  - *Work mix* (ADR 0011 §3, #288; the interim rule until task 4 adds `area:` labels): the
    `records.txt` lines, ordered by path (a journal file's name starts with its UTC time; lines
    of one file keep their order), each (path, text) pair once (the same file on two refs). The
    words of the text after `- Session: ` (split on whitespace) classify each record:
    - *left out:* a first word starting `design-` (a Design session; before task 4 neither
      counted nor in the window), or `worker` with a second word starting `owner-` (an owner
      instruction, rule a);
    - *on an issue:* exactly three words, the first `worker` or `qc`, the last `#<digits>`; the
      issue is that number, except for the steps `own-pr` and `design-window`, where it is
      that PR;
    - *other:* everything else (audit, retro, Planner, `main-red`, `leftover-worktree`, a
      `fallback exit` line, any other text): in the window, never milestone work.

    The *window* is the newest 20 records not left out. Fewer than 20 (in the 7 days that
    `records.txt` covers), or no current milestone (step 10's): *not measured*, never below the
    budget. Otherwise a record is *milestone work* when it is on issue N, N's milestone in
    `all-issues.json` is the current milestone (exact title), and an agent PR in `prs.json` of
    any state links N (*Linked issue*, its first `Closes #N` only) and changes a *product path* (a `pr-files.json` path
    starting with one of step 7's prefixes, no unquoting: `gh` writes paths plain); for `own-pr
    #N`, when PR N is an agent PR that changes a product path and links an issue in the current
    milestone. A record names no PR, so on issue N it counts when *any* agent PR linking N
    changes a product path, not only the one that session opened: a process-only follow-up
    session on an issue whose earlier PR changed `crates/` counts (an approximation of ADR 0011
    §3's "its PR"). The work mix is *below the budget* when fewer than 70% of the window
    (`counted × 100 < 70 × 20`, so 13 or fewer of 20) is milestone work. The *reading* is a
    `Notes:` line: `work mix: <c> of the last 20 sessions on milestone work (<c × 5>%; budget
    70%)`, `work mix: not measured (<n> of the 20 session records it needs)` or `work mix: not
    measured (no current milestone)`.
  - *Product issue* (step 10): an issue with a label `area:<name>` (exact) where `<name>` is an
    area of kind `product` in `areas.toml`.
  - *Failing* open PR: a head check run whose conclusion is `FAILURE`, `TIMED_OUT`,
    `STARTUP_FAILURE`, `ACTION_REQUIRED` or `CANCELLED`, or a status context in state `FAILURE`
    or `ERROR`, or no `ai-review` status context in state `SUCCESS` on the head (unreviewed, as a
    session that died before review leaves it, or reviewed and failed), or `mergeable`
    `CONFLICTING` (a merge conflict with its base: auto-merge can never land it, #270).
    `MERGEABLE` and `UNKNOWN` (GitHub computes mergeability lazily, so a fresh push or base
    change reads `UNKNOWN` for a while) don't count; a conflict that `UNKNOWN` hides is seen by
    a later run.
- **Precedence** (first match wins; within a step, the order given):
  1. **Owner instructions** (rule a, §0 of TICK):
     - an owner issue (a feed line of kind `issue` with `is_pr` false; an owner PR is the
       owner's own work, and owner comments on it come as comment lines) that has no owner reply starting with
       `🤖 jpdf-agent done: <the issue's URL>` (exact URL, followed by whitespace or the end),
       carries no skip label, and isn't linked by an open PR that isn't failing, is held or
       needs design (step 3; that PR is already doing it, or waits for the owner's `/release` or
       a design session, step 6; a failing one is picked here, so rule a outranks rule c for it).
       Lowest number first. `owner-open.json` supplies the replies and labels; an issue missing
       from it counts as pending with no labels;
     - an owner comment (feed kinds `comment`, `review_comment`, `review`) created at or after
       the floor (earlier ones need no reply, TICK §0), whose URL appears in no open PR body
       that isn't failing, is held or needs design, and that isn't the `url` of an owner
       `/approve` step 3's window term picks its PR by (#372, PR #376 R1.1: else rule a would
       pick it first every run; the design-window session posts both replies). Oldest
       `created_at` first, then by URL;
     - a closed owner issue in `owner-closed.json`, untagged, with an owner reply starting
       `🤖 jpdf-agent picked up <its URL>` and none starting `🤖 jpdf-agent done: <its URL>`
       (both exact, as above; a reply for a comment on the issue doesn't count)
       → `owner-done` (post the missing reply). Lowest number first.
  2. **Main red** (rule b): of the runs in `runs.json` with `status` `completed` and an event
     other than `pull_request`, the one with the largest `databaseId` (the newest; list order
     isn't trusted) has conclusion `failure`, `timed_out` or `startup_failure`. A newer run
     still in progress doesn't hide it. Skipped while the judge-alarm file exists (rule b: the alarm is already
     raised; the session continues with other work); the brief notes the alarm instead.
  3. **Own PRs and the scheduler** (rule c, and ADR 0011 §2 Failure):
     - an open PR in `prs.json` that is failing, where neither the PR nor its linked issue
       carries a skip label, that doesn't need design (step 6's), and that is not *held*. For a
       PR labeled `design` (exact) that isn't a draft, the linked issue's `owner-todo` doesn't
       count (only `blocked` and `deferred` do, as in the window term below): the issue carries
       it while the window runs, so without this a design PR that conflicts or loses its
       review in its window was picked by no step (#372, the #347/#359 deadlock). A draft
       design PR keeps the full rule (a design question or an escalation is the owner's turn). Lowest number first. A PR is held when,
       of its comments in `holds.json` that are an owner command, the newest (largest
       `createdAt`; on a tie, the later in the list) is a hold. A command is a comment that
       `.github/workflows/owner-commands.yml` acts on: author `jacobdam`, a body that starts
       with `/hold` or `/release` and doesn't contain the tag, both ignoring case (Actions'
       `startsWith` and `contains` do; ASCII case here). The workflow then holds when the body
       starts with `/hold` exactly and releases otherwise, so `/Hold` or `/RELEASE` is a
       release. TICK §5: nothing moves until
       `/release`, so a session picked for it could do nothing, and every later session would
       get the same pick (#295). The brief notes each held open agent PR
       (`PR #N is on the owner's /hold (no /release after it): step 3 doesn't pick it`), lowest
       number first. A held PR still links its issue (in flight), and step 1 treats it like a
       PR that isn't failing: its owner issue or comment waits for the `/release` too. The
       same goes for an open PR whose head branch starts with `owner/` (case and position
       exact): AUTOMATION.md §6.1's branch for the owner's own protected change. The owner
       signs its commits, posts its `ai-review` and merges it, so a session can't push to it
       and has nothing to do; its body is tagged when the owner's interactive session wrote
       it, so it is in `prs.json` (#298: PRs #290 and #291 link their issues with `Refs`, so
       the linked-issue skip missed them). The brief notes each one (`PR #N is the owner's
       own change (an owner/ branch, AUTOMATION.md §6.1): step 3 doesn't pick it`, without
       the branch name), lowest number first, after the held ones; a PR that is both gets
       both notes;
     - an open issue labeled `bug` whose title starts with `next-session: `, with no skip
       label and not in flight;
     - an open agent PR labeled `design` (exact name: a design ADR PR, TICK §8 step 5, or a
       later `docs/PRODUCT.md` change, TICK §10 step 6; v1 never carries it) whose 24-hour
       window has passed (ADR 0011 §7, #314): not a draft (`isDraft` false), auto-merge off
       (`autoMergeRequest` null), not failing (so `ai-review` success is on its head and it has
       no conflict), not held, not on an `owner/` branch, and with no skip label on the PR. Its
       linked issue's `owner-todo` doesn't stop it (the issue carries it while the window runs,
       and this session removes it), but `blocked` or `deferred` there does (the safe side:
       a session or the owner parked the work). The window *has passed* when every one of these
       times is at or before `now − 86 400` (so at exactly 24 hours it has passed): the PR marked
       ready (the newest `created_at` of its `ready_for_review` events in `design-ready.json`,
       or its `createdAt` when it has none: it was created ready); the newest `ai-review` status
       context in state `SUCCESS` in its `statusCheckRollup` (by `startedAt`), counted from the
       triage comment its `targetUrl` names when that is one of this PR's comments in
       `holds.json` with author `jacobdam` and a body starting `🤖 jpdf-agent **AI review
       triage` (that comment's `createdAt`, #373), else from its own `startedAt`; and, when the
       newest of its owner commands in `holds.json` (the hold rule above, unchanged) is a release,
       that command's `createdAt`. The window also ends early by the owner's `/approve`
       (#372): a comment in `holds.json` by `jacobdam` without the tag (ASCII case, as the hold
       rule) whose body starts with `/approve` in any ASCII case followed by ASCII whitespace
       or the end (so `/approved`, `ok /approve` and ` /approve` are not). It counts when it is
       the newest of the PR's owner commands counted together with the approvals (largest
       `createdAt`, on a tie the later in the list; so a `/release` or `/hold` after it ends
       it) and its `createdAt` is at or after the time the `ai-review` success counts from
       (the linked triage comment's, else the status's): the owner saw that review, and a
       success re-posted after a record-only update links the same triage, while a new round
       after any other push links a later one. It doesn't change the hold rule, so an
       approval after a `/hold` leaves the PR held. A PR with no entry in `design-ready.json` (only a broken
       fixture, live) is never picked, approved or not. Lowest number first → `worker design-window #N`. The
       `Rule:` line says `c (own design PR past its 24-hour window, ADR 0011 §7: enable
       auto-merge, remove owner-todo from its issue)`, or for an approval that counts `c (own
       design PR approved by the owner's /approve, ADR 0011 §7: enable auto-merge, remove
       owner-todo from its issue)`; the brief loads `gh pr view N --comments`,
       the linked issue (when there is a `Closes` link) and `ops/TICK.md §8 step 5`.

       The order follows ADR 0011 §2 step 3: a failing PR first (a design PR whose CI fails is
       fixed there when its issue has no skip label), then the scheduler's own bug (a broken
       scheduler can mis-pick everything else), then the window, which is the least urgent: the
       owner already had the 24 hours, and a later session merges it as well. It outranks step
       4 because ADR 0011 §2 puts it in step 3.
  4. **Crash and security** (rule d): an open issue labeled `crash`, or `security` with
     `sev:critical`, `sev:high` or no `sev:*` label (TICK: label it first), with no skip label
     and not in flight. Order: `sev:critical` (on a `crash` or `security` issue), then
     `security` with no `sev:*`, then the rest; lowest number first within each.

     Not in this part: step 4's `design-pending` containment rule (task 4, #246/#247).
  5. **Flaky and leftovers** (rules e, f): an open issue labeled `flaky` (same filters, lowest
     number first); then a worktree in `worktrees.txt` on a branch `t/…` (by name; the line
     `branch refs/heads/t/…`) for which `prs.json` has no PR of any state with that
     `headRefName` (a session died before its PR). The first such branch by name. Worktrees
     whose branch has only merged or closed PRs go into the brief's `Notes:` (they can be
     removed) and are never picked.

     Ratchet regressions show up as a red judge on main (step 2). The nightly that files
     findings as issues (#19) doesn't exist yet, so step 5 has no nightly term.
  6. **Design: technical** (ADR 0011 §2 step 6, TICK §8; #275), in this order:
     - a PR that needs design, where neither the PR nor its linked issue carries a skip label,
       not held and not on an `owner/` branch (as in step 3: nothing moves until `/release`,
       TICK §5). Lowest number first → `design-technical needs-design #N`. The brief loads the
       PR's comments (its ledger), its linked issue and the search for earlier PRs closing it
       (their ledgers, TICK §8 step 2), then the design files below; a PR without a `Closes`
       link gets no issue lines;
     - an open issue labeled `design` and `ready` (exact names) with no skip label and not in
       flight. Any milestone, none included (design queue items are filed by audits, the owner or
       other sessions, ADR 0011 §1), and `in-progress` doesn't stop it (TICK §8 step 1 resumes
       a design session that died). Lowest number first → `design-technical design-issue #N`.
       The brief loads the issue, then the design files.

     The design files, in order: `ops/TICK.md §8`, `specs/tooling/design-technical.md`,
     `docs/PRINCIPLES.md`, `docs/adr/_TEMPLATE-design.md`,
     `docs/adr/0010-review-rounds-converge.md`, `docs/adr/0011-session-types-and-scheduler.md §5`.
     Not in this part: the third term of ADR 0011 §2 step 6 (an area over its threshold, task 4,
     #247).
  7. **Audit, then retro** (rules g, h). From `main-log.txt` (newest first): a *record commit*
     adds (status `A`) a file `docs/audits/NNNN.md` or `docs/advocate/NNNN.md` (an *audit slot*,
     ADR 0011 §1), or `docs/retros/NNNN.md` (a retro), with exactly four digits and nothing else
     in the name. A *product merge* is a first-parent commit that changes (any status) at least one
     path starting with `crates/`, `bindings/`, `apps/`, `fuzz/`, `tools/jpdf-cli/` or
     `tools/fsio/` (ADR 0011 §3, until task 4 adds `area:` labels; git C-quotes unusual paths,
     and the opening `"` is ignored).
     - **Audit due** when there is no audit-slot record, or 10 or more product merges are newer
       than the newest one (its own commit not counted), or `now` is at least 7 days (604 800 s)
       after that commit's time → `audit due`.
     - **Retro due**, with *since* the time of the newest retro record (or of the first commit,
       the last line of the log, when there is none), when `now` is at least 7 days after
       *since*, or there is an incident after *since*:
       - a revert: a first-parent commit newer than the retro record (any commit when there is
         none) whose subject starts with `Revert "` (GitHub's revert PR title);
       - a judge alarm raised after *since* (the alarm file's time);
       - a milestone tag `m<digits>` created after *since*;
       - 3 or more issues in `blocked.json` with a `labeled` event for the label `blocked`
         created after *since* (rule h's "3 issues blocked this week": a retro record older than
         7 days makes the retro due by age anyway, so "since the retro" is within the week).
       → `retro due`.
     - **Slot in flight** (#282): a due audit (or retro) is not picked while an open agent PR
       links an open issue (one in `issues.json`) labeled `audit` (or `retro`); the session
       doing it is already in CI or review, and picking it again writes a second record. The
       PR's checks don't matter: a failing one is step 3's pick, and one whose issue carries a
       skip label is parked (`owner-todo`: the owner's turn; `blocked`, `deferred`: set aside),
       not a reason for a new audit. The hold has no time limit: a parked audit or retro PR holds
       its term until it merges or closes, and each run's brief notes it
       (`audit due (<why>) but PR #N is open for it: not picked`, lowest PR number), and the
       scheduler goes on: a retro due after an audit in flight is picked. The label is the
       signal, so the audit or retro issue must carry it (TICK §6, §7); one without it is not
       seen, and the term is picked as before.
     Rule h's other incidents need no term of their own: main red for 3 sessions ends in a
     revert (rule b), and a gate that passes gets its milestone tag (rule j).
     The user-advocate slot (every 2nd audit slot) comes with task 8 (#251); until then every
     due slot is an audit.
  8. **Planner due** (rule j; ADR 0011 §1): skipped when the Planner ran today (step 11's
     test). Otherwise due, with each reason that holds on the `Rule:` line, when:
     - *the current milestone has no plan:* there is a current milestone (step 10's) and the
       plan's milestone differs from its title, or there is no plan. The plan's milestone is
       the text after `Milestone: ` on the first line of `queue.md` that starts so (no leading
       space; case matters), trimmed; a missing file or no such line is no plan. The file's
       text is never printed;
     - *no plan in 7 days:* no `records.txt` line whose first word after `- Session: ` is
       `planner` (so `planner due` counts) has a path day from today − 6 to today (UTC days of
       `now`; a later day doesn't count).
     - *the work mix is below the budget* (#288).

     → `planner due`. The reasons, in this order, on the `Rule:` line: `ops/QUEUE.md is missing
     on origin/main or unreadable` or `ops/QUEUE.md on origin/main doesn't plan the current
     milestone <M>`, `no Session: planner record is dated in the last 7 UTC days`, `the work mix
     is below 70%: <c> of the last 20 sessions on milestone work`.

     **Design: product** (rule k; ADR 0011 §1, TICK §10), when the Planner term didn't pick:
     - *v1:* `docs/PRODUCT.md` is not on origin/main. v1's issues are the open ones labeled
       `design-product` and not `ready` (ready ones are gap issues for after v1, TICK §10 step
       3). None → `design product v1` (the session files one). Else, when any of them has a
       skip label or an open dependency (step 10's `Depends on:` test; an escalated v1 waiting
       on its design ADR, TICK §8 output 2) → nothing, and the brief notes `Design: product due
       (<why>) but an open design-product issue without ready has a skip label or an open
       dependency: not picked` (unless a product PR is open: then the slot rule's note below);
       else `design product v1 #N`, the lowest. Whether the file is there comes from `main-log.txt`
       (newest first): the newest commit with a file line whose path is exactly
       `docs/PRODUCT.md` decides; status `D` means deleted, `A`, `M` or `T` present, and no
       such line means never added. `--no-renames` turns a rename into `D` plus `A`, so a
       rename away from the path reads as a delete. The file's text is never read;
     - *a product issue:* the file is on origin/main, and an open issue is labeled
       `design-product` and `ready`, with no skip label and its dependencies closed (step 10's
       `Depends on:` test), in any milestone or none (product decisions aren't milestone work;
       such issues are filed `ready`, TICK §9's opening, so no Planner run is needed) →
       `design product #N`, lowest number first. An in-flight one needs no filter of its own:
       the PR linking it trips the slot rule below. While the file is missing, only v1 is
       picked;
     - *one product PR at a time:* neither is picked while an open agent PR links an open issue
       labeled `design-product` (step 7's slot rule, #282: any checks, any other labels; a
       failing one is step 3's). The brief notes the term instead (`Design: product due (<why>)
       but PR #N is open for it: not picked`, lowest PR number), also when an earlier step
       won, as step 7's notes do. So the v1 PR, which waits for the owner's approval with no
       timeout (ADR 0011 §7), holds both terms until it merges or closes.
     QC and Design: UI (step 8) and 9. Gardener: not in this part (tasks 4, 5, 7, 8).
  10. **A ready issue** (rule i): an open issue labeled `ready`, in the *current milestone* (the
      open milestone whose title is `M<digits>` with the smallest number), with no skip label
      and none of `in-progress` (claimed), `design`, `design-pending`, `design-product` (step 8's), not in flight, and with its
      dependencies closed: every `#N` of the list right after `Depends on:` on a dependency line
      of its body is not an open issue in `issues.json`. A dependency line is one whose text,
      after leading whitespace and an optional `**` (no space after it), starts with `Depends on:`; the phrase
      anywhere else in a line (``Its `Depends on: #14` is met by``, `**Refs:** … Depends on:
      #14`) names nothing (#412: #303's body named #14 that way, and the loop idled for about
      70 sessions). The list is `#N` items separated by commas, spaces or `and`,
      each optionally followed by one `(…)` note without nested parentheses; it ends at the
      first other text (a tab, a second or nested note included), so prose after the phrase
      names nothing (bold markers around the words allowed; case matters). Lowest
      number first; while the work mix is below the budget, the lowest-numbered *product issue*
      among them first, and the lowest-numbered one when none is a product issue (the budget
      orders, it never blocks: ADR 0011 §3). Below the budget the `Rule:` line says so: `i (the
      work mix is below 70%: the lowest-numbered ready product issue in the current milestone
      with its dependencies closed, this one)`, or `…, none, so the lowest-numbered ready
      issue)`.
  11. **Planner or idle** (rule j; ADR 0011 §1, the Planner's bound, as ADR 0015 amends it):
      nothing above applies. A *Planner file of today* is a path `ops/journal/<today>/<name>.md`
      (today: the UTC day of `now`; `<name>` without `/`) with a `records.txt` line whose text
      after `- Session: ` has `planner` as its first whitespace-separated word (case matters;
      `planner due` and `planner new-issues` count, `planners` and `Planner` don't). Each path
      counts once, however many lines or refs list it. With none: `planner`, a Planner session
      (TICK §9), whose record says `- Session: planner`. With one or more (the Planner ran
      today):
      - **`planner new-issues`** when fewer than 3 such files exist and at least one issue is
        *new*: in `issues.json` (open), its milestone the current milestone (step 10's, exact
        title; none means none is new), without `ready` and without a skip label, `in-progress`,
        `design`, `design-pending` or `design-product` (exact names), not in flight, and with a
        `createdAt` at or after the *rerun time* (text comparison, both in GitHub's form). The
        rerun time is the end of the minute in the newest file's name: of the files' names, each
        read as the minute `HH × 60 + MM` of its first four characters when they are ASCII
        digits followed by `-` with `HH` < 24 and `MM` < 60, the largest, plus one minute, on
        today's date (`2359-…` gives 00:00:00 of tomorrow). A name without such a time reads as
        the end of its day: when any of today's files has one, no issue is new. The `Rule:`
        line is `j (nothing applies, the Planner ran <n> of at most 3 times today, and these
        issues are new since its newest run: #A, #B): the Planner again, ADR 0015`, lowest
        number first, numbers only; the brief loads what `planner` loads. Its record says
        `- Session: planner new-issues`, which moves the rerun time to its own minute;
      - otherwise **`idle`**: the session exits at once and writes nothing.

      A Planner record on a closed unmerged PR is not on any listed ref, so it doesn't count
      (that run's labels may still stand; one more run that day is harmless). The rerun sits at
      step 11, after step 10: the Planner orders work, it doesn't interrupt it (ADR 0015
      Consequences). Step 8 is unchanged: once any Planner file of today exists, its terms are
      off for the day.
- **Exit codes** (ADR 0011 §2 Failure):
  - **0**: a pick was printed.
  - **2**: unknown. A command could not start, failed or printed too much (gh or network
    error); the message goes to stderr and the session does no work.
  - **1**: the scheduler is broken: a usage error, an unreadable or missing `--from` file
    (other than `judge-alarm-since` and `queue.md`; live, a missing `ops/areas.toml` too), or an
    input that doesn't parse (not JSON, not the
    expected shape, no `FLOOR` line, a floor not in the form `YYYY-MM-DDTHH:MM:SSZ`). Every
    input is parsed in full whatever is picked, so a renamed field fails every run. A panic exits 101; any exit other than 0 and 2 sends the
    session to TICK's legacy fallback and the `next-session: <first line>` bug issue.
- **Parsing.** `feed.jsonl`: one JSON object per non-empty line, each with a string `kind`;
  `issue` lines need a boolean `is_pr` and, when it is false, a number `number`; comment kinds a string `url` and `created_at`; other
  kinds are an error. The lists are JSON arrays of objects; the fields named above must have the
  right type (labels: `[{name}]`; comments: `[{author: {login}, body}]`, a null author counts as
  not the owner; `mergeable`: a string, one of `MERGEABLE`, `CONFLICTING`, `UNKNOWN`;
  `statusCheckRollup`: objects with `__typename` `CheckRun` (`name`,
  `conclusion`) or `StatusContext` (`context`, `state`), other entries are ignored; on an open PR
  labeled `design`, an `ai-review` status in state `SUCCESS` also needs `startedAt` in the form
  `YYYY-MM-DDTHH:MM:SSZ`, and its `targetUrl`, if present, is a string or null). In
  `holds.json`, a comment by `jacobdam` whose body starts `🤖 jpdf-agent **AI review triage`
  also needs a string `url` and a `createdAt` in that form (#373); an owner `/approve` needs
  that `createdAt` and a string `url` too (#372); other comments need neither
  `url` nor, unless they are an owner command, `createdAt`. Every PR in `prs.json` needs `isDraft` (a boolean),
  `autoMergeRequest` (null or an object) and `createdAt` (`YYYY-MM-DDTHH:MM:SSZ`).
  `design-ready.json`: objects with a number `number` (each at most once) and an array `events`
  of objects with a string `event`; a `ready_for_review` event also needs `created_at` in that
  form, other events need nothing more. Timestamps are compared as ISO 8601 UTC text (GitHub
  writes one fixed form).
  `main-log.txt`: non-empty lines are either `commit <40 hex digits> <Unix seconds>[ <subject>]`
  or `<status>\t<path>` with status `A`, `M`, `D` or `T` after a commit line; anything else (a
  rename line, a file line before the first commit) is an error, and so is a log with no commit.
  `tags.txt`: `<name> <Unix seconds>` per non-empty line. `milestones.json`: objects with a
  string `title`. `blocked.json`: objects with a number `number` and an array `events` of
  objects with strings `event` and `created_at` (`YYYY-MM-DDTHH:MM:SSZ`); a `labeled` event also
  needs `label: {name}`. `records.txt`: every non-empty line is `<path>\t<text>` with `<path>`
  `ops/journal/YYYY-MM-DD/<name>.md` (a real date, `<name>` without `/`) and `<text>` starting
  with `- Session: `; anything else is an error. `pr-files.json`: objects with a number
  `number` and an array `files` of objects with a string `path`. `all-issues.json`: objects with
  a number `number` and `milestone` null or an object with a string `title`. `areas.toml`:
  `cargo xtask issue-lint`'s parser and rules (specs/tooling/issue-lint.md; its `process` and
  `records` areas included). `issues.json`: `body` a string, `milestone` null or an object with a
  string `title`, `createdAt` a string in the form `YYYY-MM-DDTHH:MM:SSZ` (on every issue). Unix seconds (`now.txt`, the alarm file after trimming whitespace, the log,
  the tags) are digits only, within the years 1970–9999 (`cargo xtask metrics`' rule).

## Persistence
None. It writes nothing. The session copies the first line into its journal entry as
`- Session: <first line>` (TICK §5), which step 11 reads back on later runs.

## Undo
Not applicable.

## Errors
Every error goes to stderr as `xtask: next-session: <message>` with the exit code above. With
`--from`, a missing input file other than `judge-alarm-since` is exit 1 (the fixture is broken),
never 2 (no command ran).

## Limits
- Command output: `macos::capture` caps each pipe at `MAX_TOOL_OUTPUT`; over it is exit 2.
  `--from` files are read whole (test fixtures and saved inputs, written by us).
- JSON nesting: `jpdf_testgen::json` caps depth at 128.
- `gh` lists: `--limit 1000` (PRs: 200, newest first; runs: 20). A cut-off list can hide an old
  item. For owner issues and open issues 1000 is far above today's counts (under 100). For PRs
  only the open ones (a handful) and the branches of local worktrees matter. A `gh issue list`
  `comments` field holds at most the first 100 comments of an issue (gh's GraphQL page), so a
  done: reply after the 100th comment is missed and the issue is picked again; the session
  then sees the reply and exits under rule a. No owner issue has 100 comments today. The same
  cap applies to `holds.json`: a `/hold` or `/release` after a PR's 100th comment is missed,
  and step 3 judges the PR by the earlier commands (a missed `/hold` picks it as before #295;
  the session then sees the hold and does nothing, TICK §5). No open PR has 100 comments
  today.
- The design window (#314): `design-ready.json` reads every timeline page of each open agent
  design PR (one `gh api` call each; one or two such PRs at a time), so no ready event is
  missed. `holds.json`'s 100-comment cap applies to `/release` too: a release after a PR's
  100th comment is missed, and the window would start at an earlier time, so the PR could be
  picked before 24 hours after that release (the owner can `/hold` again; no open PR has 100
  comments today). The `ai-review` time is the status's `startedAt` as `statusCheckRollup`
  shows it (the newest status of each context on the head; checked on PR #327 on 2026-11-24),
  or the `createdAt` of the triage comment its `targetUrl` names (#373; `targetUrl` checked on
  PR #359 on 2026-11-27). A triage comment after a PR's 100th comment is missed, so the status
  time counts and the window restarts (the safe side). An `/approve` after a PR's 100th
  comment is missed (the window runs its 24 hours: the safe side), but a `/hold` or
  `/release` after the 100th that follows an earlier approval is missed too, so that approval
  still counts (#372); the owner's `/hold` itself still drafts the PR through the workflow, so
  GitHub can't merge it, and the session sees the hold (TICK §5). No open PR has 100 comments
  today.
- Blocked issues: only issues that still carry `blocked` when the scheduler runs are listed
  (`--label blocked`), so an issue labeled and unlabeled between two retros is missed; listing
  every issue updated in the week would cost one events call each. At most 100 issues updated
  in the last 7 days, and one page of 100 events each
  (`gh api` returns the oldest first, so an issue with more than 100 events hides its newest
  ones: a missed incident, and the retro still comes by age within 7 days).
- `main-log.txt` holds the whole first-parent history, read live through the same 16 MiB cap;
  it grows by about 250 bytes per merge.
- A `Depends on:` number that `issues.json` doesn't list counts as closed, so a list cut off at
  1000 issues could release an issue early.
- `records.txt`: one `git ls-tree` per open agent PR plus main (7 directory paths each), and
  one `git cat-file` per journal file of those 7 days on any of those refs (an estimate for a
  full week: about 15 a day, so about 100; at 48f63d9 main had 8, since per-day directories
  started on 2026-11-23), each capped like any command. A Planner record older than 6 days is not
  read, which is what step 8's 7-day term wants.
- `docs/PRODUCT.md`: no command of its own; step 8 reads `main-log.txt`, so a log cut off by
  the cap is exit 2 as before. A history that reached the file only through a non-first-parent
  merge would show the merge's first-parent diff, which names it (git's `--first-parent`
  implies `--diff-merges=first-parent`).
- `queue.md`: one `git ls-tree` and at most one `git cat-file`, capped like any command. A record is found only in a file under its own day's directory, so an entry filed
  under the wrong day counts for the day its path says.
- The work mix: `pr-files.json` lists the 200 newest PRs of the account and at most 100 files
  of each (gh's GraphQL page): a product path after a PR's 100th file is missed, and a record
  naming a PR older than the 200 doesn't count (the window's records are at most 7 days old, and
  about 40 PRs open a week). `all-issues.json`: the 1000 newest issues (about 200 today). The
  window covers only `records.txt`'s 7 days: with fewer than 20 records in them the budget is
  not measured, so a slow week can't breach it. Until #308 gives issues `area:` labels, no
  issue is a product issue, so step 10's order doesn't change; step 8's term still applies.
- Step 11's rerun (ADR 0015): at most 3 Planner sessions a UTC day, counted over the files
  `records.txt` lists. A record that leaves that list (a Planner records PR closed unmerged, a
  blob that can't be read, a session that dies before it pushes) allows one run beyond the cap,
  and if it was the newest, the issues it judged count as new again: the exposure the once-a-day
  bound had (one more Planner run that day). Issues the Planner files during its own run, after
  its record's minute, are new for the next run; that costs at most one rerun, inside the cap.
- No loop runs on a count from the input except over the parsed lists themselves.

## Security
- **Who gives instructions.** The feed is the protected `owner-issues.sh`; the scheduler only
  narrows it (floor, done: replies, in-flight PRs). A done: reply counts only from the owner
  account and only with the exact URL followed by whitespace or the end, so a reply for
  `…/issues/1234` never handles `…/issues/123`. A stranger can't hide an instruction: their
  comments are not owner replies, and a PR counts as in flight only when it is a tagged PR
  of the shared account (`--author @me`; the guard forces the tag on agent PRs, and the owner's
  own untagged PRs are ignored). The skip labels can be set by the agent account
  (ADR 0011 Consequences: the guard can't check labels). A hold counts only from an untagged
  owner-account comment, as the workflow applies it, so a stranger's `/hold` or `/release`
  changes nothing; an agent comment can't release a hold (it carries the tag). The owner's
  commands only ever hold back step 3: a held PR is skipped, never picked. The `owner/`
  branch skip only holds back step 3 too: any session can name a branch that way (the
  account is shared), but that only stops its own PR from being picked, and whatever the PR
  links then waits on it, as it would on a held PR: an owner issue or comment it names (step
  1), and any other issue it closes, a `crash` or `security` one included (in flight, steps 4
  to 10). An open PR with a skip label already does the same, so nothing new is exposed.
- **Design: product.** Its terms read the agent account's labels and main's history, so a
  session can hold them off (an open PR linking a `design-product` issue) or call one up (the
  label): the same exposure as step 7's slot rule. Nothing here is a mechanical merge gate:
  under TICK §10 step 6, v1 merges only after an untagged owner comment approves it, and a
  later `docs/PRODUCT.md` change merges after a 24-hour window unless the owner `/hold`s it,
  so a forged `design product #N` pick can lead to a merged change the owner didn't stop in
  time, as a design ADR PR can (ADR 0011 §7).
- **The design window** (#314). The pick only tells a session to enable auto-merge on its own
  design PR; it never merges anything and never skips the owner's command: a held PR (the
  owner's newest command is a `/hold`) is never picked, and an owner `/release` starts the
  window again, both by step 3's unchanged command rule (untagged, from `jacobdam`, as the
  workflow acts on it). A stranger's or an agent comment can't restart or end the window. The
  times come from GitHub (the timeline, the status, the PR) and from owner comments; the
  `ai-review` status is posted by the agent account, so a session could post one early and
  start its own window early, as it could under TICK §1's by-hand check: the owner still gets
  24 hours from the PR marked ready and from any `/release`. Since #373 the success counts from
  the triage comment it links, so a session that re-posts a success after a non-record push
  while linking the old triage comment (TICK §4 allows the old link only after a record-only
  update) would keep the old start; the PR then merges content the owner's window didn't
  cover. Only the agent account can post the status or a tagged `jacobdam` comment, a stranger's
  comment never counts (author check), and a link that names no triage comment of this PR falls
  back to the status time (a restart). The owner's `/approve` (#372) counts only from an
  untagged `jacobdam` comment, so a stranger or an agent comment can't end the window; it can
  only make a PR merge sooner, never hold one back or release a `/hold`. It counts from the
  review time above, so the same exposure applies: a session that re-posts a success with an
  old link after a non-record push would keep an approval the owner gave on other content.
  Step 3's first term now picks a non-draft design PR whose issue has `owner-todo`; that only
  sends a session to fix its own failing PR (TICK rule c), which merges nothing by itself. A wrong pick costs a design ADR
  merged before the owner's 24 hours, which is why this term is in the strict lane (ADR 0012
  §1, ADR 0011 §7). The cutoff `now − 86 400` saturates and is floored at the epoch (`now` is
  in the years 1970–9999).
- **What it outputs.** Issue and PR numbers, run ids, branch names and owner-comment URLs from
  the feed. No titles or bodies are printed, so issue text (data, CONSTITUTION 1) never appears
  in the pick or brief as if it were an instruction. Branch names come from the local worktree
  list.
- **Read-only.** It runs `gh issue list`, `gh pr list`, `gh run list`, `gh api` reads (GET
  only: milestones, issue events, PR timelines), `git worktree list`,
  `git log`, `git for-each-ref`, `git ls-tree`, `git cat-file` and the protected feed script,
  which only reads. It writes no file.
- **Session records are self-reported** (ADR 0011 §3). Any session (or anyone who can push a
  branch with a tagged open PR) can write a `- Session: planner` line dated today and turn the
  rest of the day's fallback picks into `idle` or `planner new-issues` (step 11), and turn step 8 off: both of its terms for that
  day, and the 7-day term for 7 days. The cost is bounded: `idle` happens only when steps 1–10
  found nothing, the milestone term comes back the next UTC day, and the 7-day term after 7
  days (a forged record delays a Planner run by at most that long; step 11 still runs it
  whenever nothing is ready). Forged records also move the work-mix reading: they can make the
  Planner due once a day or reorder step 10 between ready issues, never block or hide one (the
  budget only orders). Step 11's rerun (ADR 0015): a forged record can't add a run, since a
  visible record only raises the day's count or moves the rerun time later. A record named
  `2359-…` (or with no valid time), or 3 forged files dated today, stop reruns for the rest of
  the day: the once-a-day bound, so at most until 00:00 UTC. An issue is new only in the current
  milestone, and setting a milestone needs triage access (only the shared account has it), so
  an outsider's issue never triggers a rerun; an agent session that files issues on every run
  can trigger at most 2 reruns a day. The brief prints new issues' numbers, never their titles. An `area:` label is set by the agent account too (ADR 0011
  Consequences), so a session can mark an issue product; the journal shows the pick. A branch name from `prs.json`
  goes to git only as `refs/remotes/origin/<name>`, so it can't be read as an option, and git's
  ref-name rules forbid `:` (no `<ref>:<path>` reads). Record lines are matched, never printed.
- Self-check:
  - Arithmetic: the 14-day and 7-day subtractions from today and the records window's first
    day (`i64` days since 1970, checked), step 8's `today - day` (only when `day <= today`;
    both are days of the years 0000–9999, so it can't overflow), the day of `now` for `records.txt` (`now / 86 400`, a checked conversion to `i64`), `now` minus a commit time (checked; both are bounded by the years 1970–9999),
    and `since + 1` (saturating). Step 11's rerun time: `today × 86 400 + (minute + 1) × 60`
    (`checked_mul` and `checked_add`; `today` is a day of the years 1970–9999 and the minute is
    below 1 440, so neither fails; a failure would make no issue new); the name's digits are
    read from a fixed-length slice pattern, never indexed. The work mix: `counted × 100` and `70 × 20` (a count of at
    most 20 records and constants), `counted × 100 / 20`, and the window's first index
    `len − 20`, taken only when `len ≥ 20`.
  - What broken implementation still passes? The records window's day list is
    `record_dirs`, unit tested on a known day; only its use in the live command is not run. Every step has a fixture that selects it and a
    near miss for each filter (a done: reply for a longer URL, a pre-floor comment, a
    `pull_request` run, a PR with the skip label only on its issue, a `t/` branch with a
    closed PR, a green PR whose `mergeable` is `UNKNOWN` or `MERGEABLE`, …), and a precedence test where every step applies and each one is removed in
    turn. A scheduler that ignored a filter, or swapped two steps, fails one of them.
  - What the tests can't see: the live `gh` field shapes (the fixtures are written from
    `gh --json` output seen on 2026-11-23; `mergeable` on PRs #265 and #271); a field rename is a parse error, exit 1, which the
    failure rule turns into a bug issue. The live assembly of `blocked.json` and the git
    command lines (which paths `--name-status` prints) are not run by the tests; they were
    checked once by hand on 2026-11-23 (the pick was `retro due`, from #162, #168 and #187).
    For `records.txt`, the listing filter, the dedupe and the skip of unreadable blobs are unit
    tested (`session_lines` in `xtask/src/next_session.rs`, on a fake listing and reader); the
    ref list, the two git command lines and the clock-to-day step are not; nor are the two
    `ops/QUEUE.md` command lines (checked by hand on 2026-11-24: the live run from the worktree
    exits 0, and `ls-tree` on origin/main without the file prints nothing). They were checked by
    hand on 2026-11-23: the `ls-tree` line run from `xtask/` lists main's three journal files of
    that day with root-relative paths, and the scheduler ran live with exit 0. A producer that
    read the wrong refs would pass every test. The live assembly of `design-ready.json` (#314)
    is not run by the tests either: on 2026-11-24 the `--paginate --slurp` timeline of PR #327
    printed one page, an array of event objects with its `ready_for_review` event and its
    `created_at`, and the scheduler ran live with exit 0 (no open design PR, so no call).
  - The work mix: what broken implementation still passes? Each filter has a fixture whose
    removal changes the pick: the window's order by path (records written newest first), the
    newest 20 (older milestone records that would lift the share), the (path, text) dedupe
    (each line twice), each left-out kind (newer left-out records that would push milestone
    ones out) and its near misses, each milestone-work condition (another milestone, none, not
    listed; no product path, paths that only start like one; an untagged PR, a PR closing
    another issue; `own-pr` on an issue number and the reverse), the 14/13-of-20 boundary, 19
    records, no current milestone, and step 10's product label (process areas, near-miss
    names, areas.toml's kind). Hand mutants of the sort, the dedupe, the left-out filter, the
    `<` and the Planner-only note each fail a test. What the tests can't see: the two live `gh`
    command lines (checked by hand on 2026-11-24: `files` holds `path`, the scheduler ran live
    with exit 0).
  - Step 11's rerun: what broken implementation still passes? Each is a hand mutant that fails
    a test: comparing the creation day instead of the minute (an issue at 00:04:59 after a
    record at 00:04), the oldest record's minute (two records bracketing the issue), counting
    lines instead of files (one file listed twice, one file with two records), `<=` 3, `>`
    instead of `>=` (00:05:00 exactly), dropping the `+ 1` minute, each filter of *new* (each
    label, the milestone, in flight), skipping a name without a time instead of reading it as
    the end of the day, a dropped `-` or digit check, `MM` = 60 accepted, any word
    `planner`, any day, and an unchecked `createdAt`. Equivalent ones: taking the last file
    instead of the newest (the files are a set sorted by path, and a path's day directory is
    fixed, so the last is the newest minute; a last-*listed* rule is caught by the bracketing
    fixture listed newer first), and `HH` = 24 accepted (it reads past the day's end, so no
    issue created by `now` is new, as for a name without a time). What the tests can't see:
    the live `createdAt` field (checked by hand on the live command, see the PR), and whether
    the Planner readies the issue (TICK §9 step 2).
  - Labels and `Depends on:` lines are written by the agent account, so a session can move work
    up or down the queue by editing them (ADR 0011 Consequences); the journal shows the pick.

## Accessibility
Not applicable (CLI).

## Platforms
macOS and Linux (`gh`, `git`, bash for the feed script).

## Performance
Ten `gh` calls (`pr-files.json` takes about 9 s), one more per blocked issue updated in the last 7 days and one (all pages) per open agent design PR, three `git` commands
plus one `git ls-tree` per open agent PR and one `git cat-file` per journal file of the last 7
days, two more for `ops/QUEUE.md`, and the feed script (which makes its own); a few seconds.

## Tests
`xtask/tests/next_session.rs` runs the binary with `--from` on fixtures built from a base set
of inputs where no step before 11 applies (→ `planner`):
- one test per step and sub-case that selects it, with the expected first line, and its near
  misses that must not select it;
- precedence: all steps apply, then each is removed in turn and the next one is picked;
- exit 1 for each malformed input and a missing `--from` file; exit 0 with a missing
  `judge-alarm-since`; exit 1 for a usage error;
- step 11's rerun (ADR 0015): with a record `0004-planner.md`, an M0 issue created at
  `00:04:59` is `idle` and at `00:05:00` is `planner new-issues` (also later that day; not one
  of yesterday), with the whole brief (two new issues, lowest first, no title); a Planner record
  only yesterday is `planner`; `planner due` counts (at `03:00:59` and `03:01:00`); each of
  `ready` (with an open dependency, so step 10 doesn't take it), the skip labels,
  `in-progress`, `design`, `design-pending` and `design-product` stops it, and other labels
  (`bug`, `discovery`, `readyx`, `Design`, `design-products`) don't; an `Ops` issue, an `M1`
  issue and one with no milestone; in flight (an open PR) and not (a merged one); a ready issue
  and an owner issue outrank it; 2 Planner files today rerun and 3 are `idle` (the base's
  `planners` and `Planner` files don't count), one file listed twice or holding two records
  counts once, yesterday's file doesn't count, and `planner due` and `planner new-issues` are
  among the 3; two records bracketing the issue, in both list orders (the newer decides); ten
  names without a valid time (`planner.md`, `2460-`, `0060-`, `2400-`, no `-`, three or five
  digits, `0500.md`, a letter, a leading `-`), alone and next to a valid one, are the end of the
  day, and `0000-`, `0500-`, `0558-`, `0559-` and `2359-` are read; a `createdAt` missing,
  null, a number, empty, with a space or without the `Z` exits 1, also when step 1 picks; the
  precedence test has a `planner new-issues` entry between step 10 and `idle`; a unit test
  reads the minute and the day's count;
- step 11: `idle` from a Planner record dated today in any line order, `planner` without one;
  near misses: a Planner record dated yesterday or tomorrow, `planners`, `Planner`, `planner`
  as a later word, a worker record, an empty file (which is `planner due` since step 8: no Planner record in 7 days); the day boundary at midnight UTC and one
  second before; an earlier step outranks `idle`; malformed lines (no tab, text not starting
  with `- Session: `, a malformed line after a Planner record dated today, a path not `ops/journal/YYYY-MM-DD/<name>.md` in six ways) exit 1 whatever is picked;
- steps 7 and 10: each record kind, product path and incident with its near misses (a changed
  record, a 3-digit name, a path that only starts like a product path, a revert before the
  retro record, a tag at the retro record's second, 2 blocked issues, an `unlabeled` event,
  each skip label, an open dependency, a non-current milestone) and each 7-day boundary at
  the second it is reached and one second before;
- step 3's hold (#295): a failing PR whose newest owner command is `/hold` is not picked and
  the brief notes it; near misses: `/release` after the `/hold` (by `createdAt`, whatever the
  list order), a tagged `/hold`, a `/hold` by another author, a body that only contains
  `/hold` after other text, a hold on another PR number; an equal `createdAt` goes by list
  order; a `/holdx` body counts as a hold (the workflow's prefix match); `/Release` or `/Hold`
  after a `/hold` releases it, and a body with the tag in other case is no command; step 1:
  an owner issue linked by, or an owner comment named in, a held failing PR is not picked,
  and two held PRs are noted lowest first; a malformed
  `holds.json` (no `comments`, no `createdAt` on an owner command, an owner command's `createdAt`
  not in GitHub's `YYYY-MM-DDTHH:MM:SSZ` form, a PR listed twice: #314) exits 1;
- step 6 (#275): a `needs-design` PR is picked and not step 3's, lowest number first, after a
  failing PR without the label (step 3); near misses: `needs-designx`, `design`, `Needs-design`
  (step 3 picks them), each skip label on the PR and on its issue, a merged or closed PR, an
  `owner/` branch, an untagged PR, a held PR (none picked); a PR without a `Closes` link has no
  issue lines; the whole brief. Step 1: an owner issue it closes and an owner comment its body
  names are not picked, and are without the label. A `design` and `ready` issue in any
  milestone, none, or claimed is picked, lowest first, after a `needs-design` PR; near misses:
  `design` alone, `ready` alone, `design-pending`, each skip label, linked by an open PR (picked
  when the PR is merged or closed); the whole brief. The precedence test has a step 6 entry
  between steps 5 and 7 (its issue term), and a `needs-design` PR loses to main red, the
  scheduler bug, a crash, a `sev:high` security issue, a flaky test and a leftover worktree and
  wins over a due audit; step 10's `design` near miss is now step 6's pick (step 10's own
  `design` skip is defensive only: step 6 takes any such issue first);
- step 3's design window (#314): an agent design PR whose issue carries `owner-todo` is picked
  with its whole brief (and without a `Closes` link, without the issue line); each start time
  being the latest (the newest of two ready events in either list order, `createdAt` with no
  ready event, the newest of two `ai-review` successes, the owner's last `/release`, a `/Hold`
  release) at exactly 24 hours (picked) and one second less (not); a ready event wins over a
  later `createdAt`; near misses: a non-owner, tagged or prefixed `/release`, a release on
  another PR, other timeline events, a pending `ai-review` or another context's later status
  (all still picked); a draft, auto-merge on, held (noted), no `ai-review` success, an
  `ai-review` failure, a failed check, a conflict, no label, `designx`, `Design`,
  `needs-design`, each skip label on the PR, untagged, merged, closed, an `owner/` branch, no
  `design-ready.json` entry (none picked); a failing design PR whose issue has no skip label
  is `own-pr`; lowest number first, and a waiting lower PR doesn't hide a higher one; a failing
  PR, the scheduler bug and main red win over it, and it wins over a crash; malformed
  `design-ready.json` (7 shapes, also with no design PR open) and `prs.json` (`isDraft`,
  `autoMergeRequest`, `createdAt` wrong or missing; an open design PR's `ai-review` success
  without or with a malformed `startedAt`, which a PR without the label doesn't need) exit 1;
  `design-window #N` is a PR for the work mix; and (#373) the `ai-review` success counts from
  the triage comment its `targetUrl` names: a success re-posted after the window passed that
  links a triage at exactly 24 hours is picked and one a second later isn't, also with the
  status earlier than the comment; with two triage comments, the linked one counts in either list order (the new one linked: a restart, as after a non-record push and a new round; the old one linked: kept, as after a record-only re-post); an owner `/release` after the triage still restarts it; near
  misses (the status time counts): no such comment, a null or absent `targetUrl`, a link to
  another comment, another author, a review comment (not a triage), an untagged or indented
  header, the triage listed under another PR; a triage comment without `url` or with a
  malformed `createdAt`, and a `targetUrl` that isn't a string or null, exit 1. Hand mutants of
  the PR-number key, the author check and the header check each fail a test. Hand mutants of every filter (draft, auto-merge,
  failing, parked, skip label, `<=` to `<`, `all` to `any`, the release, the review time, the
  newest ready event, the missing entry, the event filter, the review state, context and
  newest, `createdAt`, the duplicate check, the record word) each fail a test, except two that
  are equivalent: the release lookup ignoring a newest `/hold` (that PR is held, so never
  picked) and a looser label test in `design_label` (the review time is read only for the exact
  label, so such a PR is never picked);
- step 3's failing design PR (#372): a design PR that conflicts or has no `ai-review`
  success, whose issue has `owner-todo`, is `worker own-pr #N` (the existing window test's
  failing cases changed from `planner` to that pick); near misses: `blocked` or `deferred` on
  the issue (with or without `owner-todo`), a draft (with `owner-todo`: not picked; without:
  picked), no label, `designx`, `Design`, each skip label on the PR, held;
- step 3's `/approve` (#372): a design PR an hour into its window is picked with an approval
  at the review's second and after it, not one second before; the whole brief with the
  approval's `Rule:` line; `/Approve`, `/APPROVE`, text after a space, a second line, a tab
  count; near misses: another author, a null author, the tag (any case), `/approved`,
  `/approvex`, `ok /approve`, ` /approve`, `approve`, an approval on another PR; a `/hold`
  after it and an approval after a `/hold` (held, noted); a `/release` after it; an approval
  after a `/release` in either list order; equal times by list order; other comments after it;
  a record-only re-post linking the triage the owner saw keeps it, an unlinked later status
  and a new round's later triage end it, an unreviewed push is `own-pr`; a draft, auto-merge
  on, a skip label on the PR, no label, an `owner/` branch, `blocked` or `deferred` on the
  issue, no `design-ready.json` entry (none picked); an approval with an empty or malformed
  `createdAt` or no `url` exits 1. Step 1 (R1.1): with the approval in the feed, one that
  counts is step 3's pick, while one before the review, after a `/hold`, followed by a
  `/release`, on a draft or conflicting PR (which, green again on the same review, is then
  step 3's by it), or older than the review of a PR picked by its window is
  `worker owner-comment <url>`, and another owner comment on the PR stays rule a's, also next
  to a newer approval that counts. Hand mutants of the review-time comparison (`>` and dropped), the
  newest-command rule, the draft exemption, the issue-label rule, the word boundary, the tag,
  author and case checks, the hold/release counting, step 1's skip and its approval-only
  filter each fail a test;
- step 3's `owner/` branch (#298): a failing PR on `owner/<x>` or the bare `owner/` is not
  picked, its linked issue stays in flight, and the brief notes it once without the branch
  name; near misses: `t/301-y`, `owner`, `owners/x`, `Owner/x`, `t/owner/x`, ` owner/x`,
  `x-owner/y` (all picked); a merged or closed owner PR is not noted and doesn't hold its
  issue; a green owner PR is noted; a held owner PR gets both notes; step 1: an owner issue
  the owner PR closes and an owner comment its body names are not picked;
- step 7's slot in flight: an open PR linking an `audit` or `retro` issue holds that term only
  (failing: step 3 picks it; with `owner-todo` on its issue: nothing), with near misses (the
  other label, a longer label, a merged, closed or untagged PR, no `Closes` link, a closed
  issue, the label on the PR instead of the issue), and the notes' text;
- step 8: `planner due` with no `queue.md`, and for each near miss of the `Milestone:` line
  (another milestone, `M01`, `M0x`, lower case, a leading space or `- `, no space after the
  colon, an empty file, a later matching line after a non-matching one), `planner` with the
  line trimmed or without a newline; the reason text for each term and both together, without
  the file's text; the current milestone moving to M1; no current milestone; the 7-day term at
  6 and 7 days and a record dated tomorrow, `planner due` as a Planner record and the other
  kinds as near misses, the window at the last second of a day and at midnight; the once-a-day
  bound (a `planner due` record today turns both terms off and step 11 into `idle` (no issue new since it), and step
  10 picks again); step 8 after steps 1 to 7 and before step 10 (a second pass of the precedence
  test without the `idle` entry and with no `queue.md`); a `queue.md` that isn't UTF-8 is exit 1;
- step 8's Design: product (#249): `design product v1` with the base log minus its
  `docs/PRODUCT.md` line, and its full brief; the newest commit decides (`D` on top is v1;
  `M`, `T`, `A` on top are present, also over an older `D`; `D` over an older `A` is v1); near
  misses of the path (`docs/PRODUCT.md.bak`, `docs/product.md`, `PRODUCT.md`,
  `x/docs/PRODUCT.md`, a quoted path) are v1; v1 with open `design-product` issues: the lowest
  one without `ready` with its brief, a ready one alone → plain v1, any waiting one (each skip
  label, an open dependency, also beside a workable one or a ready gap issue) → nothing with
  the note, a waiting ready gap issue (each skip label, an open dependency) beside a workable
  v1 issue → that v1 issue (#313), an issue with another label → plain v1; with the file present, the lowest ready
  `design-product` issue in any milestone or none, its brief, and that it wins over a ready
  Worker issue; a closed dependency; near misses (not `ready`, each skip label, an open
  dependency, `design`, `design-products`); a ready `design-product` issue in M0 is never step 10's; while the file
  is missing a ready product issue is never picked; the slot rule (an open PR linking a
  `design-product` issue with `owner-todo` holds both terms with the note; a failing one is
  step 3's; near misses: the `design` label, another issue, no `Closes`, merged, closed,
  untagged, the label on the PR); `planner due` wins, and the note still shows; the
  precedence test's step 8 entry between `retro due` and step 10, and its second pass, where
  `planner due` wins over it;
- the work mix (#288): 14 of 20 meets the budget, 13 of 20 is below it (`planner due`, its
  reason and the note), 20/20, 0/20, 19 records (not measured), all three step 8 reasons
  together, the note only on Planner picks, no current milestone, a Planner record dated today
  (idle); the newest 20 by path whatever the line order; a duplicated line counted once and a
  different line in the same file counted; each left-out kind and 13 near misses; each Worker
  step and QC on an issue, `own-pr`, open and closed PRs, each milestone and file near miss,
  each product prefix, an untagged PR, a PR closing another issue, a second PR for the issue,
  the current milestone moving to M1; step 10 below the budget: the lowest product issue first,
  the rule text with and without one, label near misses (`area:records`, `area:enginex`,
  `area:Engine`, `engine`, `area: engine`, `area:`), a product issue each step 10 filter drops,
  and areas.toml deciding the kind; malformed `pr-files.json`, `all-issues.json` and
  `areas.toml` exit 1, also when step 1 picks;
- unit tests in `xtask/src/next_session.rs` for the `FLOOR` line, the done: URL match, the
  ISO time form, the `Depends on:` parser, the journal listing over several days, the
  `ops/QUEUE.md` listing line, the `Milestone:` line and the record words of the work mix
  (`design-window #N` among them).
