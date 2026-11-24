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
replaces the `legacy` exit, and reads the `Session:` records of ADR 0011 §3. Task 3 (#245)
adds step 6. Task 2 part 1 (#244) adds step 8's Planner triggers (a milestone with no plan, 7
days without a Planner run) and widens `records.txt` to 7 days.

- **Output.** Exit 0 and, on stdout, a first line (the **pick**) and a brief:
  - first line: `<type> <step> <reference>`, `audit due`, `retro due`, `planner due`,
    `planner` or `idle`.
    Its first word is the session type: `worker`, `audit`, `retro`, `planner`, or `idle` (no
    session: exit at once). Steps and references:

    | Step | TICK rule | First line |
    |---|---|---|
    | 1 | a | `worker owner-issue #N`, `worker owner-comment <url>`, `worker owner-done #N` |
    | 2 | b | `worker main-red run <databaseId>` |
    | 3 | c | `worker own-pr #N`, `worker scheduler-bug #N` |
    | 4 | d | `worker crash-security #N` |
    | 5 | e, f | `worker flaky #N`, `worker leftover-worktree <branch>` |
    | 7 | g, h | `audit due`, `retro due` (the reason is on the `Rule:` line) |
    | 8 | j | `planner due` (the reason is on the `Rule:` line) |
    | 10 | i | `worker ready #N` |
    | 11 | j | `planner`, `idle` |

  - then a blank line and the brief: a `Rule:` line (the TICK letter and why this item was
    picked), a `Load:` list (the commands and files to read, one per line, `- ` first), and a
    `Notes:` list when there is something to note (an active judge alarm; worktrees whose
    branch's PR is merged or closed, which can be removed; a due audit or retro that step 7
    didn't pick because its PR is open).
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
     → `planner due`. The work-mix trigger comes with part 2 (#288). QC, Design: product or UI
     (step 8) and 9. Gardener: not in this part (tasks 4–8).
  10. **A ready issue** (rule i): an open issue labeled `ready`, in the *current milestone* (the
      open milestone whose title is `M<digits>` with the smallest number), with no skip label
      and none of `in-progress` (claimed), `design`, `design-pending`, not in flight, and with its
      dependencies closed: every `#N` of the list right after a `Depends on:` in its body is not an
      open issue in `issues.json`. The list is `#N` items separated by commas, spaces or `and`,
      each optionally followed by one `(…)` note without nested parentheses; it ends at the
      first other text (a tab, a second or nested note included), so prose after the phrase
      names nothing (bold markers around the words allowed; case matters). Lowest
      number first. The work-mix ordering of ADR 0011 §3 comes with task 2 (#244).
  11. **Planner or idle** (rule j; ADR 0011 §1, the Planner's bound): nothing above applies.
      `idle` when `records.txt` has a line whose path is `ops/journal/<today>/<name>.md` (today:
      the UTC day of `now`; `<name>` without `/`) and whose text after `- Session: ` has
      `planner` as its first whitespace-separated word (case matters; `planner due` counts): the
      Planner ran today, so the session exits at once and writes nothing. Otherwise `planner`:
      a Planner session (TICK §9), whose record says `- Session: planner`. A Planner record on a closed unmerged PR is not on any listed ref,
      so it doesn't count (that run's labels may still stand; a second run that day is harmless).
- **Exit codes** (ADR 0011 §2 Failure):
  - **0**: a pick was printed.
  - **2**: unknown. A command could not start, failed or printed too much (gh or network
    error); the message goes to stderr and the session does no work.
  - **1**: the scheduler is broken: a usage error, an unreadable or missing `--from` file
    (other than `judge-alarm-since` and `queue.md`), or an input that doesn't parse (not JSON, not the
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
  needs `label: {name}`. `records.txt`: every non-empty line is `<path>\t<text>` with `<path>`
  `ops/journal/YYYY-MM-DD/<name>.md` (a real date, `<name>` without `/`) and `<text>` starting
  with `- Session: `; anything else is an error. `issues.json`: `body` a string, `milestone` null or an object with a
  string `title`. Unix seconds (`now.txt`, the alarm file after trimming whitespace, the log,
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
  then sees the reply and exits under rule a. No owner issue has 100 comments today.
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
  full week: about 15 a day, so about 100; on 2026-11-24 main had 8, since per-day directories
  started on 2026-11-23), each capped like any command. A Planner record older than 6 days is not
  read, which is what step 8's 7-day term wants.
- `queue.md`: one `git ls-tree` and at most one `git cat-file`, capped like any command. A record is found only in a file under its own day's directory, so an entry filed
  under the wrong day counts for the day its path says.
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
- **Read-only.** It runs `gh issue list`, `gh pr list`, `gh run list`, `git worktree list`,
  `git log`, `git for-each-ref`, `git ls-tree`, `git cat-file` and the protected feed script,
  which only reads. It writes no file.
- **Session records are self-reported** (ADR 0011 §3). Any session (or anyone who can push a
  branch with a tagged open PR) can write a `- Session: planner` line dated today and turn the
  rest of the day's fallback picks into `idle`, and turn step 8 off: both of its terms for that
  day, and the 7-day term for 7 days. The cost is bounded: `idle` happens only when steps 1–10
  found nothing, the milestone term comes back the next UTC day, and the 7-day term after 7
  days (a forged record delays a Planner run by at most that long; step 11 still runs it
  whenever nothing is ready). A branch name from `prs.json`
  goes to git only as `refs/remotes/origin/<name>`, so it can't be read as an option, and git's
  ref-name rules forbid `:` (no `<ref>:<path>` reads). Record lines are matched, never printed.
- Self-check:
  - Arithmetic: the 14-day and 7-day subtractions from today and the records window's first
    day (`i64` days since 1970, checked), step 8's `today - day` (only when `day <= today`;
    both are days of the years 0000–9999, so it can't overflow), the day of `now` for `records.txt` (`now / 86 400`, a checked conversion to `i64`), `now` minus a commit time (checked; both are bounded by the years 1970–9999),
    and `since + 1` (saturating).
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
    read the wrong refs would pass every test.
  - Labels and `Depends on:` lines are written by the agent account, so a session can move work
    up or down the queue by editing them (ADR 0011 Consequences); the journal shows the pick.

## Accessibility
Not applicable (CLI).

## Platforms
macOS and Linux (`gh`, `git`, bash for the feed script).

## Performance
Nine `gh` calls, one more per blocked issue updated in the last 7 days, three `git` commands
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
  bound (a `planner due` record today turns both terms off and step 11 into `idle`, and step
  10 picks again); step 8 after steps 1 to 7 and before step 10 (a second pass of the precedence
  test without the `idle` entry and with no `queue.md`); a `queue.md` that isn't UTF-8 is exit 1;
- unit tests in `xtask/src/next_session.rs` for the `FLOOR` line, the done: URL match, the
  ISO time form, the `Depends on:` parser, the journal listing over several days, the
  `ops/QUEUE.md` listing line and the `Milestone:` line.
