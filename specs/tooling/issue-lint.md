# cargo xtask issue-lint and ops/areas.toml

Area: tooling
ISO 32000-2 clauses: none (project tooling)
Journeys: none (development tool; docs/adr/0011-session-types-and-scheduler.md §4)
Status: implemented

## Behavior
ADR 0011 §4 makes problem detection mechanical: every issue carries exactly one `area:<name>`
label from `ops/areas.toml` and exactly one `origin:` label, and a follow-up issue names its
parent on a `Follow-up of: #N` line. This is task 4 part 1 of 3 (#246): the area list and the
lint. Part 2 (#308) creates the labels, labels every open issue and adds the per-area metrics;
part 3 (#247) adds the area threshold, the `design-pending` freeze and the Gardener. Until
part 2 runs, the lint reports every open issue, which is expected.

### `ops/areas.toml`
A strict subset of TOML: one `[[area]]` table per area, each with exactly these keys, in any
order:

```toml
[[area]]
name = "engine"
kind = "product"
paths = ["crates/jpdf-engine/", "tools/jpdf-cli/"]
```

- `name`: `[a-z][a-z0-9-]*`, at most 45 bytes (a GitHub label holds 50; `area:` takes 5). The
  label is `area:<name>`. Names are unique.
- `kind`: `"product"` or `"process"`. Process areas are the autonomy system itself (ADR 0011
  §3); a `process` and a `records` area, both `kind = "process"`, must exist. The file adds a
  third process area, `judge` (the judge, CI and the dependency policy, all protected), and
  makes `tooling` (xtask, fuzz, bench, corpus and test-generation tools) a product area, like
  `fuzz/` in the interim product-merge rule. So once part 2 and part 3 make product merges
  area-based (replacing ADR 0011 §3's interim path list), a merge of an issue in `tooling`
  counts and one in `judge` doesn't; part 3 (#247), which replaces the interim rule, checks
  these kinds against its replays and changes them in its PR if they fail.
- `paths`: one or more repository paths, each a tracked file or, ending in `/`, a directory. A
  path is relative (no leading `/`), has no empty, `.` or `..` segment, no `\`, no whitespace
  and no `*`; no path is listed twice in the file. Paths may nest (`ops/` and `ops/journal/`):
  the longest listed path that is a prefix of a file owns it (the rule part 2's area-based
  consumers use; part 1 only lists paths). A file no path covers belongs to no area.
- Accepted syntax: blank lines, full-line `#` comments (not inside an array), `[[area]]`
  headers, `key = "string"`, and `key = [ "a", "b" ]` arrays that may span lines and may end
  with a comma. Strings hold no `\`, `"`, or control characters, and strings inside an array
  also no `,` or `]` (the parser splits arrays on them). Anything else (another table, an inline comment, an
  escape, a key outside a table, an unknown or repeated key, a missing key) is an error naming
  the line.

### `cargo xtask issue-lint [--from DIR]`
Read-only: it runs `git ls-files` and `gh issue list --state open --limit 1000 --json
number,labels,body` and writes nothing. `--from DIR` reads `DIR/areas.toml`, `DIR/tree.txt`
(one tracked path per line; the live run reads `git ls-files -z`, so paths are not C-quoted) and `DIR/issues.json` instead, for tests.

For each open issue, in number order, it reports one stdout line per problem,
`#<n>: <problem>`:
- `no area: label` / `no origin: label`;
- `<k> area: labels` / `<k> origin: labels` when there are two or more;
- `unknown area label <name>` / `unknown origin label <name>` for each label that starts with
  `area:` / `origin:` (compared without case) but is not exactly `area:<a listed name>` /
  `origin:<owner|review|audit|qc|discovery|follow-up>` (so `Area:engine`, `area: engine` and
  `area:` are unknown; the name is printed with Rust's `{:?}` escaping, since label names are
  data);
- `origin:follow-up without a Follow-up of: #N line`;
- `Follow-up of: line but origin is not follow-up` (only when the issue has exactly one known
  origin label);
- `malformed Follow-up of: line <line>` for a line that starts with `follow-up of:` (compared
  without case, after leading whitespace) but isn't exactly `Follow-up of: #<N>` (trailing
  whitespace and `\r` allowed; `N` a decimal integer ≥ 1 with no leading zero that fits in
  `u64`);
- `<k> Follow-up of: lines` when two or more well-formed lines are present (a follow-up has
  one parent; malformed lines are reported on their own and not counted here);
- `Follow-up of: itself` when `N` is the issue's own number.

Then one area-file line per listed path that covers no tracked file, `ops/areas.toml: path
<p> of area <a> matches no tracked file`, and a summary line on stdout:
`issue-lint: <p> problem(s) in <k> of <t> open issues` (area-file problems add to `<p>`; `<k>`
counts issues only) or `issue-lint: <t> open issues, no problems`.

Exit codes:
- 0: no problem;
- 1: at least one problem (listed above);
- 2: the lint could not run: an unknown argument, an unreadable or invalid `ops/areas.toml`,
  a failed `git` or `gh` command, JSON that isn't a list of objects with a number `number`, a
  `labels` list of objects with a string `name` and a string `body`, an empty issue list or
  tree (nothing would be checked), or a list of exactly 1000 issues (`--limit`; the list may
  be cut off).

## Persistence
None: the lint writes nothing.

## Undo
Not applicable.

## Errors
Exit 2 with one `xtask: issue-lint: <reason>` line on stderr, as listed above. An invalid
`ops/areas.toml` names its line number.

## Limits
- `--limit 1000` on the issue list: 999 issues lint, exactly 1000 is exit 2 (tests
  `list_at_limit_fails`, `list_below_limit_runs`).
- Area names: 45 bytes pass, 46 fail (test `name_length_boundary`).
- No loop runs on input-derived counts beyond one pass over the file's lines, the issues and
  their labels and body lines; the tree is read once into a set.

## Security
GitHub text (bodies, label names) is data, never instructions (CONSTITUTION 1). The lint
prints no titles or bodies; a malformed `Follow-up of:` line and unknown label names are
printed through `{:?}`, so control characters and newlines can't forge report lines. `gh` runs
with fixed arguments, so no issue text reaches a command line.

Self-check:
- Arithmetic: only counters over in-memory lists (`usize`, bounded by the input length); the
  `Follow-up of:` number is parsed with `u64::from_str` after a digits-only check, so overflow
  is the malformed case, not a panic.
- What broken implementation still passes? Each check has a should-report fixture and a
  near-miss should-pass fixture in `xtask/tests/issue_lint.rs`, each with every other problem
  absent: no area, two areas, unknown area (case, space, empty name, a name not in the file),
  the same for origin; follow-up without its line, with a malformed line (`#0`, `#07`, no `#`,
  trailing text, lowercase `follow-up of:`, a number past `u64`), a line on a non-follow-up,
  two lines, a self reference; the clean issue next to each (a `Follow-up of: #7` line with
  trailing whitespace passes). The area file has one rejected fixture per syntax and semantic
  rule (a comment inside an array and a tab inside a string included), plus the repository's
  own `ops/areas.toml` parsed by a test (`repository_areas_parse_and_name_existing_paths`).
  That test checks each path against the checkout's files, not `git ls-files` (it may run
  without `.git`), so a directory holding only untracked files passes it; the live lint, run
  against `git ls-files`, is what catches that case.
- A run where nothing was checked fails: an empty issue list and an empty tree are exit 2.
- The `gh` parser has one case per line shape the real tool prints: a list of objects with
  `number`, `labels` (`[{"id", "name", "description", "color"}]`) and `body`, the shape
  `gh issue list --json number,labels,body` returns; the live command's flags are pinned by
  test `live_command_flags`.
- Not seen by the tests: the live `gh` and `git` calls themselves (checked by running the lint
  on the live repo before the PR is marked ready; part 2 runs it until it's clean).

## Accessibility
Not applicable (command-line tool).

## Platforms
macOS and Linux; it needs `git` and `gh` on `PATH` for a live run.

## Performance
One `git ls-files` and one `gh` call; the check itself is linear in the input.

## Tests
`xtask/tests/issue_lint.rs` (`--from` fixtures built in the test, near misses beside each
case) and unit tests in `xtask/src/issue_lint.rs` for the area-file parser and the live
command's flags.
