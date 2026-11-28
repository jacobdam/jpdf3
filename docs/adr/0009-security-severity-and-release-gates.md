# 9. Security issues carry a severity; only high+ block the owner preview

Status: accepted (owner issue #143, decided in the owner's interactive session, 2026-11-22).
Amended 2026-11-28 (owner decision on #402 part 2, audit 0003 AS.56): rows for policy-check bypasses.
Amended 2026-11-28 (owner issue #466): where and when a latent high is worked (Latent highs, below).

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
| `sev:high` | One hostile PDF crashes, hangs or exceeds the 2 GiB cap on open or render, or a signature verifies wrongly. Counts even if the path doesn't exist yet but will exist by M4. A proven bypass of a CONSTITUTION policy check that lets agent text pass as the owner's. |
| `sev:medium` | Needs an unusual setup (a local attacker, a non-default configuration, a future caller misusing an API), or wastes resources while staying within the limits. A proven bypass of a CONSTITUTION policy check (the license check, a guard rule) that needs this project's own code or agent to trigger it, for example tool-only licensed material reaching a shipped artifact. |
| `sev:low` | Defence in depth: test and conformance-suite strength, CI and tooling hygiene (a proven policy-check bypass is medium or high, above), hardening with no reachable failure. |

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

### Latent highs (owner issue #466)
A **latent high** is a `sev:critical` or `sev:high` issue whose failure path doesn't exist on
main yet: it needs a later milestone's code, or a design that is still open. The table counts it
as high all the same; this section says where it lives and when it is worked.
1. **Milestone and deferral.** A latent high goes in the **milestone where its path appears**,
   never later than M4 (the owner-preview gate, above), and is labeled `deferred` with
   `Undefer when: milestone Mn is current`, or `Undefer when: #A closed` when one issue adds the
   path or the design it depends on (TICK §5, Deferring an issue). The comment that files,
   regrades or defers it names the missing path. A high whose path exists on main today is not
   latent: it stays in the current milestone without `deferred`, and rule d picks it at once.
2. **Who decides.** The session that files or regrades the issue says, in that comment, why the
   path is missing and where it appears. The Planner checks it on its next run (TICK §9 step 2's
   milestone moves). When unsure whether the path exists, choose the current milestone without
   `deferred`, as "when unsure, pick the higher" does for the severity.
3. **Rule d unchanged.** It still picks every high that is not `deferred` (or under another skip
   label), in any milestone. The `Undefer when:` line, which the Planner re-checks on every run
   (TICK §9 step 1a), is what brings a latent high back, so none is forgotten; once lifted it is
   rule d's whatever its milestone.
4. **One-time pass (2026-11-28).** The open `sev:critical`/`sev:high` issues when this section
   was written: only #146 (the parse heap high-water mark next to decode and raster buffers; its
   path appears when decode or raster first runs on the `jpdf-cli` path, M2 by PLAN §8). It is
   already `deferred` with `Undefer when: #474 closed` and `Depends on: #236`. The owner chose
   its milestone and condition on PR #465 (the memory ADR, still open): the milestone of that
   ADR's task 8, #474, which gates the measured peaks. That instruction overrides the move to M2
   that #466 proposed, so this pass doesn't move it. #231, which #466 also named, was closed as
   not planned before this pass.

Initial classification (2026-11-22): high #109 (closed by PR #142 the same day), #146; medium #114, #116, #117, #141; low #83,
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
