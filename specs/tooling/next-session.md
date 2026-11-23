# cargo xtask next-session (1/2)

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; docs/adr/0011-session-types-and-scheduler.md §2, ops/TICK.md §2)
Status: implemented

## Behavior
`cargo xtask next-session [--from DIR]` decides what the session does, so sessions stop picking
their own work from prose (ADR 0011 §2, task 1a, #242). This part decides precedence steps 1–5
(today's TICK rules a–f). When none of them applies it prints `legacy`, and the session uses
TICK rules g–j (task 1b, #243, moves those into the scheduler).

- **Output.** Exit 0 and, on stdout, a first line (the **pick**) and a brief:
  - first line: `<type> <step> <reference>`, or `legacy`. Types in this part: `worker` only.
    Steps and references:

    | Step | TICK rule | First line |
    |---|---|---|
    | 1 | a | `worker owner-issue #N`, `worker owner-comment <url>`, `worker owner-done #N` |
    | 2 | b | `worker main-red run <databaseId>` |
    | 3 | c | `worker own-pr #N`, `worker scheduler-bug #N` |
    | 4 | d | `worker crash-security #N` |
    | 5 | e, f | `worker flaky #N`, `worker leftover-worktree <branch>` |

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
  5. `prs.json`: `gh pr list --author @me --state all --limit 200 --json number,state,isDraft,body,headRefName,labels,statusCheckRollup`;
  6. `issues.json`: `gh issue list --state open --limit 1000 --json number,title,labels`;
  7. `runs.json`: `gh run list --workflow ci.yml --branch main --limit 20 --json databaseId,status,conclusion,event`;
  8. `worktrees.txt`: `git worktree list --porcelain`;
  9. `judge-alarm-since`: live, `ops/state/judge-alarm-since` (TICK rule b's local state file).
     It is the only optional input: a missing file means no alarm.

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
    session that died before review leaves it, or reviewed and failed).
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
  6. Otherwise: `legacy`.
- **Exit codes** (ADR 0011 §2 Failure):
  - **0**: a pick was printed.
  - **2**: unknown. A command could not start, failed or printed too much (gh or network
    error); the message goes to stderr and the session does no work.
  - **1**: the scheduler is broken: a usage error, an unreadable or missing `--from` file
    (other than `judge-alarm-since`), or an input that doesn't parse (not JSON, not the
    expected shape, no `FLOOR` line, a floor not in the form `YYYY-MM-DDTHH:MM:SSZ`). Every
    list is parsed in full before any step runs, so a renamed field fails whatever is picked. A panic exits 101; any exit other than 0 and 2 sends the
    session to TICK's legacy fallback and the `next-session: <first line>` bug issue.
- **Parsing.** `feed.jsonl`: one JSON object per non-empty line, each with a string `kind`;
  `issue` lines need a boolean `is_pr` and, when it is false, a number `number`; comment kinds a string `url` and `created_at`; other
  kinds are an error. The lists are JSON arrays of objects; the fields named above must have the
  right type (labels: `[{name}]`; comments: `[{author: {login}, body}]`, a null author counts as
  not the owner; `statusCheckRollup`: objects with `__typename` `CheckRun` (`name`,
  `conclusion`) or `StatusContext` (`context`, `state`), other entries are ignored). Timestamps
  are compared as ISO 8601 UTC text (GitHub writes one fixed form).

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
  - Arithmetic: only the 14-day subtraction from today (`i64` days since 1970, checked).
  - What broken implementation still passes? Every step has a fixture that selects it and a
    near miss for each filter (a done: reply for a longer URL, a pre-floor comment, a
    `pull_request` run, a PR with the skip label only on its issue, a `t/` branch with a
    closed PR, …), and a precedence test where every step applies and each one is removed in
    turn. A scheduler that ignored a filter, or swapped two steps, fails one of them.
  - What the tests can't see: the live `gh` field shapes (the fixtures are written from
    `gh --json` output seen on 2026-11-23); a field rename is a parse error, exit 1, which the
    failure rule turns into a bug issue.

## Accessibility
Not applicable (CLI).

## Platforms
macOS and Linux (`gh`, `git`, bash for the feed script).

## Performance
Seven `gh` calls plus the feed script (which makes its own); a few seconds.

## Tests
`xtask/tests/next_session.rs` runs the binary with `--from` on fixtures built from a base set
of inputs where no step applies (→ `legacy`):
- one test per step and sub-case that selects it, with the expected first line, and its near
  misses that must not select it;
- precedence: all steps apply, then each is removed in turn and the next one is picked;
- exit 1 for each malformed input and a missing `--from` file; exit 0 with a missing
  `judge-alarm-since`; exit 1 for a usage error;
- unit tests in `xtask/src/next_session.rs` for the `FLOOR` line and the done: URL match.
