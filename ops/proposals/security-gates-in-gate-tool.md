# Proposal: check the ADR 0009 security gates in the gate tool, and rate policy-gate bypasses

Status: proposed by audit 0003 (issue #159, PR #160), 2026-11-22; revised after review round 1.
Touches protected paths (`ops/judge/gates.toml`, `ops/judge/gate.py`, a new `ops/judge/sev_counts.py`,
`.github/workflows/ci.yml`), so the owner applies it with signed commits. Part 2 changes ADR 0009,
an owner decision, so it is also left to the owner.

## 1. The gate tool doesn't check the security gates (audit finding AC.2)

### Problem
ADR 0009 and PLAN §8 say:
- M4 (owner preview) needs no open `sev:critical` or `sev:high` issue;
- M10 (release) needs no open `sev:medium` or higher.

AUTOMATION §9 says every milestone's exit criteria are in `ops/judge/gates.toml` and the gate tool
gives the verdict. But `[M4]` has no criteria ("journeys only"), `[M10]` lists only render, bench
and fuzz metrics, and `ops/judge/gate.py` reads no issues or labels. With a `sev:high` issue open,
the nightly gate report (`gates/gates.json` in `judge-report`, which TICK rule j reads) would show
M4 passing.

### Proposed change
gate.py stays offline. The nightly workflow, which already has a token, counts open issues per
label in its own step and passes the counts in a file. Design rules: a failed count must never stop
the existing exam-sealing loop, and a missing or malformed count must make the gates that use it
fail (fail closed), never pass.

`.github/workflows/ci.yml`, job `verdict`: add `issues: read` to its `permissions`, and add this step
right before "Grade exam (aggregate only) and evaluate gates (nightly)". Being a separate step with
`continue-on-error`, its failure can't skip the sealing loop in the grade step (which keeps
`if: always()`); the grade step then runs without a counts file and M4 fails closed.
```yaml
      - name: Count open security issues (nightly)
        if: always() && env.NIGHTLY == 'true'
        continue-on-error: true
        env:
          GH_TOKEN: ${{ github.token }}
          GH_REPO: ${{ github.repository }}
        run: |
          set -euo pipefail
          gh issue list --state open --limit 1000 --json number,labels > open-issues.json
          python3 judge/ops/judge/sev_counts.py open-issues.json > sev-counts.json
```
`GH_REPO` is required: the job checks out into `judge/` and `pr/`, so the workspace root where the
step runs is not a git repository (the job's other `gh` calls name the repository for the same
reason). Without it, `gh` fails every night and M4 fails closed forever.

New `ops/judge/sev_counts.py`. It counts by the `sev:*` label alone, so a `sev:high` issue missing
the `security` label still counts; it counts open `security` issues with no or several `sev:*`
labels separately; and it refuses a list of 1000, which may be truncated:
```python
import json, sys
issues = json.load(open(sys.argv[1]))
if not isinstance(issues, list) or len(issues) >= 1000:
    sys.exit("sev_counts: unexpected or possibly truncated issue list")
out = {"open_sev_critical": 0, "open_sev_high": 0, "open_sev_medium": 0, "open_sev_unlabeled": 0}
for i in issues:
    names = {l["name"] for l in i["labels"]}
    sev = [n for n in names if n.startswith("sev:")]
    for n in sev:
        k = "open_sev_" + n[4:]
        if k in out:
            out[k] += 1
    if "security" in names and len(sev) != 1:
        out["open_sev_unlabeled"] += 1
print(json.dumps(out))
```
Add `--sev-counts sev-counts.json` to the gate.py call in the grade step.

ops/judge/gate.py (a missing, malformed or partial file leaves the metrics unset, so M4 fails with
`open_sev_high = None > 0`; the existing `max` branch reports `None` because it checks
`got is None` first):
```diff
     ap.add_argument("--selftest", choices=["pass", "fail"], default="fail")
+    ap.add_argument("--sev-counts", type=Path)
     ap.add_argument("--out", type=Path, required=True)
@@
     m["journeys"] = smoke["metrics"].get("journeys", 0)
+    # ADR 0009 gates. Missing, malformed or partial counts leave the metrics unset (fail closed).
+    try:
+        sev = C.load(a.sev_counts, {}) if a.sev_counts else {}
+    except (OSError, ValueError):
+        sev = {}
+    keys = ("open_sev_critical", "open_sev_high", "open_sev_medium", "open_sev_unlabeled")
+    if isinstance(sev, dict) and all(type(sev.get(k)) is int and sev[k] >= 0 for k in keys):
+        m.update({k: sev[k] for k in keys})
```

ops/judge/gates.toml:
```diff
-[M4]   # engine API + reader apps: journeys only
+[M4]   # engine API + reader apps; owner preview: no open sev:critical/high (ADR 0009)
+max = { open_sev_critical = 0, open_sev_high = 0, open_sev_unlabeled = 0 }
@@
 [M10]  # hardening + 1.0
 min = { smoke_render_match = 0.97, exam_render_match = 0.95, fuzz_clean_nights = 30 }
-max = { smoke_bench_ratio = 1.5 }
+max = { smoke_bench_ratio = 1.5, open_sev_medium = 0 }
```
Gates are cumulative, so M10 also inherits M4's criteria.

### Test plan
A self-test next to the judge's others, on a **synthetic** smoke/exam report in which every metric
and journey passes, so the only failures are the new ones. Each case asserts the exact strings in
each gate's `failing` list, not just `pass`:
- counts all 0: no `open_sev_` string in any gate's `failing`; M4's and M10's own criteria pass.
- `open_sev_critical = 1`: M4 has `open_sev_critical = 1 > 0`.
- `open_sev_high = 1`: M4 has `open_sev_high = 1 > 0`.
- `open_sev_unlabeled = 1`: M4 has `open_sev_unlabeled = 1 > 0`.
- `open_sev_medium = 1`: M4 has no `open_sev_` string; M10 has `open_sev_medium = 1 > 0`.
- no file, an empty file, `{"open_sev_high": ""}`, a negative count, and a file missing one key:
  M4 has `open_sev_high = None > 0`, and gate.py exits 0 with gates.json written.
- sev_counts.py on a fixture list with a `sev:high` issue without `security` (counted high), a
  `security` issue with no `sev:*` (unlabeled 1), one with two `sev:*` labels (unlabeled 1, both
  counted), and a list of 1000 entries (exits non-zero, writes nothing).
- One dry nightly as applied (no deliberate failure): `gates/gates.json` has all four `open_sev_*`
  metrics set to integers that match `gh issue list` at that time. This catches a count step that
  can't reach the repository and a gate.py call missing `--sev-counts`.
- One dry nightly on a branch with a deliberately failing count step: the sealing loop still runs
  (the `results-exam` artifact is gone afterwards) and `gates-report` is uploaded.

### Risk if not applied
The M4 and M10 gates can pass with open high or medium security issues. Sessions would have to
remember to check by hand, which TICK rule j doesn't say.

### Limit
Labels are set by the agent. The gate counts them honestly, but it can't tell whether a label is
right; ADR 0009 already makes audits re-check them.

## 2. The rubric has no row for policy-gate bypasses (audit finding AS.56)

### Problem
ADR 0009's table rates by what a hostile PDF can do. A proven bypass of a policy gate (the
CONSTITUTION 10 license check that keeps tool-only packages out of shipped artifacts, the guard's
tag or protected-path rules) falls under "CI and tooling hygiene" = `sev:low`, which M10 makes
optional. Audit 0002 rated such a bypass (#112) high before ADR 0009. Audit 0003 rated the new
ones (#163, #162) medium and high by judgement, not by a row in the table.

### Proposed change (ADR 0009, Decision table)
Add to `sev:medium`: "a proven bypass of a CONSTITUTION policy check (license check, guard rule)
that needs this project's own code or agent to trigger it (for example, tool-only licensed
material reaching a shipped artifact)". Add to `sev:high`: "such a bypass that lets agent text
pass as the owner's".

### Risk if not applied
Policy-gate bypasses can be labeled `sev:low` and stay open through the release.
