# 9. Security issues carry a severity; only high+ block the owner preview

Status: accepted (owner issue #143, decided in the owner's interactive session, 2026-11-22).

## Context
The biggest product risk is never delivering. TICK rule d put every `security` issue ahead of the
backlog, and audits and reviews keep filing them: on 2026-11-22, 12 of M0's 38 open issues were
`security`, most of them test-suite strength and tooling hygiene that no hostile PDF can reach.
M0 kept growing instead of closing. The owner asked for a severity on every security issue and
for the release stage to decide which severities must be fixed.

## Decision
Every `security` issue carries exactly one severity label, set when it is filed:

| Label | Meaning |
|---|---|
| `sev:critical` | Escapes the trust boundary or leaks data: PDF JavaScript or a Launch action runs, data is sent out, the sandbox is escaped, a redaction leaks. |
| `sev:high` | One hostile PDF crashes, hangs or exceeds the 2 GiB cap on open or render, or a signature verifies wrongly. Counts even if the path doesn't exist yet but will exist by M4. |
| `sev:medium` | Needs an unusual setup (a local attacker, a non-default configuration, a future caller misusing an API), or wastes resources while staying within the limits. |
| `sev:low` | Defence in depth: test and conformance-suite strength, CI and tooling hygiene, hardening with no reachable failure. |

When unsure between two levels, pick the higher one and say why on the issue.

Gates (PLAN §8):
- **M4, the owner preview** (the first build the owner uses for feedback): no open `sev:critical`
  or `sev:high` issue.
- **M10, release**: no open `sev:medium` or higher. `sev:low` issues are optional.

Scheduling (TICK §3):
- Rule d covers `crash` issues and `sev:critical` / `sev:high` issues only.
- `sev:medium` and `sev:low` issues go to milestone M10 and are picked like any backlog issue
  there; they don't jump the queue before then.
- In a PR review, a medium or low security finding may be deferred to such an issue instead of
  fixed in the PR (triage `deferred: #N`), unless the fix is small. Blocker and major findings
  are still fixed in the PR.

Initial classification (2026-11-22): high #109, #146; medium #114, #116, #117, #141; low #83,
#122, #132, #134, #136, #137. #116 and #117 are suites (low by the table) raised to medium: their
audit proved broken AES-CBC and inflate backends pass them, and these suites admit third-party
crates (CONSTITUTION 11).

## Consequences
- Sessions spend their time on the M1–M4 path. Medium and low work waits in M10, visible and
  counted, instead of being dropped.
- The release gate depends on labels being honest; audits re-check the labels of open security
  issues and raise any that were under-rated.
- CONSTITUTION 4 is unchanged: a severity never justifies removing or raising a `Limits` check or
  weakening the trust boundary. It only decides when an open issue gets fixed.
