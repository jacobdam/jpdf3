# cargo xtask next-session

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; docs/adr/0011-session-types-and-scheduler.md §2, ops/TICK.md §2)
Status: implemented

## Behavior
`cargo xtask next-session [--from DIR]` decides what the session does, so sessions stop picking
their own work from prose (ADR 0011 §2). Task 1a (#242) added precedence steps 1–5 (TICK rules
a–f); task 1b part 1 (#273) adds steps 7 (rules g, h) and 10 (rule i) and the interim
product-merge counter (ADR 0011 §3). When none of them applies it prints `legacy`, and the
session uses TICK rule j. Part 2 (#243) replaces that with step 11 (Planner or idle); task 3
(#245) adds step 6.

- **Output.** Exit 0 and, on stdout, a first line (the **pick**) and a brief:
  - first line: `<type> <step> <reference>`, `audit due`, `retro due`, or `legacy`. Types:
    `worker`, `audit`, `retro`. Steps and references:

    | Step | TICK rule | First line |
    |---|---|---|
    | 1 | a | `worker owner-issue #N`, `worker owner-comment <url>`, `worker owner-done #N` |
    | 2 | b | `worker main-red run <databaseId>` |
    | 3 | c | `worker own-pr #N`, `worker scheduler-bug #N` |
    | 4 | d | `worker crash-security #N` |
    | 5 | e, f | `worker flaky #N`, `worker leftover-worktree <branch>` |
    | 7 | g, h | `audit due`, `retro due` (the reason is on the `Rule:` line) |
    | 10 | i | `worker ready #N` |

  - then a blank line and the brief: a `Rule:` line (the TICK letter and why this item was
    picked), a `Load:` list (the commands and files to read, one per line, `- ` first), and a
    `Notes:` list when there is something to note (an active judge alarm; worktrees whose
    branch's PR is merged or closed, which can be removed).
- **Inputs.** It runs only read commands from the current directory (the main checkout). Each
  has a file name; `--from DIR` reads that file from `DIR` instead and runs no command, so every
  pick can be tested offline:
  1. `feed.jsonl`: the stdout of `ops/driver/owner-issues.sh` (JSON lines, the owner feed);
  2. `floor.txt`: the feed's `FLOOR` timestamp. Live, it is the value of the line
     `FLOOR="…"` in `ops/driver/owner-issues.sh` (one source; the file is protected);
  3. `owner-open.json`: `gh issue list --state open --author jacobdam --limit 1000 --json number,labels,comments`;
  4. `owner-closed.json`: `gh issue list --state closed --author jacobdam --search closed:>=<today − 14 days> --limit 1000 --json number,body,comments`
     (today is the UTC day from the system clock, as in `cargo xtask metrics`);
  5. `prs.json`: `gh pr list --author @me --state all --limit 200 --json number,state,isDraft,body,headRefName,labels,statusCheckRollup,mergeable`;
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
      as `[{"number":N,"events":[…]}, …]`.

  `issues.json` (input 6) also reads `body` and `milestone`:
  `gh issue list --state open --limit 1000 --json number,title,labels,body,milestone`.

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
       carries no skip label, and isn't linked by an open PR that isn't failing (that PR is
       already doing it; a failing one is picked here, so rule a outranks rule c for it).
       Lowest number first. `owner-open.json` supplies the replies and labels; an issue missing
       from it counts as pending with no labels;
     - an owner comment (feed kinds `comment`, `review_comment`, `review`) created at or after
       the floor (earlier ones need no reply, TICK §0), whose URL appears in no open PR body
       that isn't failing. Oldest `created_at` first, then by URL;
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
       carries a skip label. Lowest number first;
     - an open issue labeled `bug` whose title starts with `next-session: `, with no skip
       label and not in flight.
  4. **Crash and security** (rule d): an open issue labeled `crash`, or `security` with
     `sev:critical`, `sev:high` or no `sev:*` label (TICK: label it first), with no skip label
     and not in flight. Order: `sev:critical` (on a `crash` or `security` issue), then
     `security` with no `sev:*`, then the rest; lowest number first within each.

     Not in this part: ADR 0011 §2 step 3's own design or `PRODUCT.md` PR past its 24-hour
     window (task 3, #245, and task 6, #249) and step 4's `design-pending` containment rule
     (task 4, #246/#247).
  5. **Flaky and leftovers** (rules e, f): an open issue labeled `flaky` (same filters, lowest
     number first); then a worktree in `worktrees.txt` on a branch `t/…` (by name; the line
     `branch refs/heads/t/…`) for which `prs.json` has no PR of any state with that
     `headRefName` (a session died before its PR). The first such branch by name. Worktrees
     whose branch has only merged or closed PRs go into the brief's `Notes:` (they can be
     removed) and are never picked.

     Ratchet regressions show up as a red judge on main (step 2). The nightly that files
     findings as issues (#19) doesn't exist yet, so step 5 has no nightly term.
  6. Design: technical: not in this part (task 3, #245).
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
     Rule h's other incidents need no term of their own: main red for 3 sessions ends in a
     revert (rule b), and a gate that passes gets its milestone tag (rule j).
     The user-advocate slot (every 2nd audit slot) comes with task 8 (#251); until then every
     due slot is an audit.
  8. QC, Planner, Design: product or UI, and 9. Gardener: not in this part (tasks 2, 4–8).
  10. **A ready issue** (rule i): an open issue labeled `ready`, in the *current milestone* (the
      open milestone whose title is `M<digits>` with the smallest number), with no skip label
      and none of `in-progress` (claimed), `design`, `design-pending`, not in flight, and with its
      dependencies closed: every `#N` of the list right after a `Depends on:` in its body is not an
      open issue in `issues.json`. The list is `#N` items separated by commas, spaces or `and`,
      each optionally followed by a `(…)` note; it ends at the first other text, so prose after
      the phrase names nothing (bold markers around the words allowed; case matters). Lowest
      number first. The work-mix ordering of ADR 0011 §3 comes with task 2 (#244).
  11. Otherwise: `legacy` (TICK rule j; part 2, #243, replaces it with the Planner or idle).
- **Exit codes** (ADR 0011 §2 Failure):
  - **0**: a pick was printed.
  - **2**: unknown. A command could not start, failed or printed too much (gh or network
    error); the message goes to stderr and the session does no work.
  - **1**: the scheduler is broken: a usage error, an unreadable or missing `--from` file
    (other than `judge-alarm-since`), or an input that doesn't parse (not JSON, not the
    expected shape, no `FLOOR` line, a floor not in the form `YYYY-MM-DDTHH:MM:SSZ`). Every
    input is parsed in full whatever is picked, so a renamed field fails every run. A panic exits 101; any exit other than 0 and 2 sends the
    session to TICK's legacy fallback and the `next-session: <first line>` bug issue.
- **Parsing.** `feed.jsonl`: one JSON object per non-empty line, each with a string `kind`;
  `issue` lines need a boolean `is_pr` and, when it is false, a number `number`; comment kinds a string `url` and `created_at`; other
  kinds are an error. The lists are JSON arrays of objects; the fields named above must have the
  right type (labels: `[{name}]`; comments: `[{author: {login}, body}]`, a null author counts as
  not the owner; `mergeable`: a string, one of `MERGEABLE`, `CONFLICTING`, `UNKNOWN`;
  `statusCheckRollup`: objects with `__typename` `CheckRun` (`name`,
  `conclusion`) or `StatusContext` (`context`, `state`), other entries are ignored). Timestamps
  are compared as ISO 8601 UTC text (GitHub writes one fixed form).
  `main-log.txt`: non-empty lines are either `commit <40 hex digits> <Unix seconds>[ <subject>]`
  or `<status>\t<path>` with status `A`, `M`, `D` or `T` after a commit line; anything else (a
  rename line, a file line before the first commit) is an error, and so is a log with no commit.
  `tags.txt`: `<name> <Unix seconds>` per non-empty line. `milestones.json`: objects with a
  string `title`. `blocked.json`: objects with a number `number` and an array `events` of
  objects with strings `event` and `created_at` (`YYYY-MM-DDTHH:MM:SSZ`); a `labeled` event also
  needs `label: {name}`. `issues.json`: `body` a string, `milestone` null or an object with a
  string `title`. Unix seconds (`now.txt`, the alarm file after trimming whitespace, the log,
  the tags) are digits only, within the years 1970–9999 (`cargo xtask metrics`' rule).

## Persistence
None. It writes nothing; the pick goes to the session's journal entry once task 1b adds
`Session:` records.

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
  then sees the reply and exits under rule a. No owner issue has 100 comments today.
- Blocked issues: only issues that still carry `blocked` when the scheduler runs are listed
  (`--label blocked`), so an issue labeled and unlabeled between two retros is missed; listing
  every issue updated in the week would cost one events call each. At most 100 issues updated in the last 7 days, and one page of 100 events each
  (`gh api` returns the oldest first, so an issue with more than 100 events hides its newest
  ones: a missed incident, and the retro still comes by age within 7 days).
- `main-log.txt` holds the whole first-parent history, read live through the same 16 MiB cap;
  it grows by about 250 bytes per merge.
- A `Depends on:` number that `issues.json` doesn't list counts as closed, so a list cut off at
  1000 issues could release an issue early.
- No loop runs on a count from the input except over the parsed lists themselves.

## Security
- **Who gives instructions.** The feed is the protected `owner-issues.sh`; the scheduler only
  narrows it (floor, done: replies, in-flight PRs). A done: reply counts only from the owner
  account and only with the exact URL followed by whitespace or the end, so a reply for
  `…/issues/1234` never handles `…/issues/123`. A stranger can't hide an instruction: their
  comments are not owner replies, and a PR counts as in flight only when it is a tagged PR
  of the shared account (`--author @me`; the guard forces the tag on agent PRs, and the owner's
  own untagged PRs are ignored). The skip labels can be set by the agent account
  (ADR 0011 Consequences: the guard can't check labels).
- **What it outputs.** Issue and PR numbers, run ids, branch names and owner-comment URLs from
  the feed. No titles or bodies are printed, so issue text (data, CONSTITUTION 1) never appears
  in the pick or brief as if it were an instruction. Branch names come from the local worktree
  list.
- **Read-only.** It runs `gh issue list`, `gh pr list`, `gh run list`, `git worktree list` and
  the protected feed script, which only reads. It writes no file.
- Self-check:
  - Arithmetic: the 14-day and 7-day subtractions from today (`i64` days since 1970,
    checked), `now` minus a commit time (checked; both are bounded by the years 1970–9999),
    and `since + 1` (saturating).
  - What broken implementation still passes? Every step has a fixture that selects it and a
    near miss for each filter (a done: reply for a longer URL, a pre-floor comment, a
    `pull_request` run, a PR with the skip label only on its issue, a `t/` branch with a
    closed PR, a green PR whose `mergeable` is `UNKNOWN` or `MERGEABLE`, …), and a precedence test where every step applies and each one is removed in
    turn. A scheduler that ignored a filter, or swapped two steps, fails one of them.
  - What the tests can't see: the live `gh` field shapes (the fixtures are written from
    `gh --json` output seen on 2026-11-23; `mergeable` on PRs #265 and #271); a field rename is a parse error, exit 1, which the
    failure rule turns into a bug issue. The live assembly of `blocked.json` and the git
    command lines (which paths `--name-status` prints) are not run by the tests; they were
    checked once by hand on 2026-11-23 (the pick was `retro due`, from #162, #168 and #187).
  - Labels and `Depends on:` lines are written by the agent account, so a session can move work
    up or down the queue by editing them (ADR 0011 Consequences); the journal shows the pick.

## Accessibility
Not applicable (CLI).

## Platforms
macOS and Linux (`gh`, `git`, bash for the feed script).

## Performance
Nine `gh` calls, one more per blocked issue updated in the last 7 days, two `git` reads and the
feed script (which makes its own); a few seconds.

## Tests
`xtask/tests/next_session.rs` runs the binary with `--from` on fixtures built from a base set
of inputs where no step applies (→ `legacy`):
- one test per step and sub-case that selects it, with the expected first line, and its near
  misses that must not select it;
- precedence: all steps apply, then each is removed in turn and the next one is picked;
- exit 1 for each malformed input and a missing `--from` file; exit 0 with a missing
  `judge-alarm-since`; exit 1 for a usage error;
- steps 7 and 10: each record kind, product path and incident with its near misses (a changed
  record, a 3-digit name, a path that only starts like a product path, a revert before the
  retro record, a tag at the retro record's second, 2 blocked issues, an `unlabeled` event,
  each skip label, an open dependency, a non-current milestone) and each 7-day boundary at
  the second it is reached and one second before;
- unit tests in `xtask/src/next_session.rs` for the `FLOOR` line, the done: URL match, the
  ISO time form and the `Depends on:` parser.
