# Proposal: check the ADR 0009 security gates in the gate tool, and rate policy-gate bypasses

Status: proposed by audit 0003 (issue #159, PR #160), 2026-11-22. Touches protected paths
(`ops/judge/gates.toml`, `ops/judge/gate.py`, `.github/workflows/`), so the owner applies it with
signed commits. Part 2 changes ADR 0009, an owner decision, so it is also left to the owner.

## 1. The gate tool doesn't check the security gates (audit finding AC.2)

### Problem
ADR 0009 and PLAN §8 say:
- M4 (owner preview) needs no open `sev:critical` or `sev:high` issue;
- M10 (release) needs no open `sev:medium` or higher.

AUTOMATION §9 says every milestone's exit criteria are in `ops/judge/gates.toml` and the gate tool
gives the verdict. But `[M4]` has no criteria ("journeys only"), `[M10]` lists only render, bench
and fuzz metrics, and `ops/judge/gate.py` reads no issues or labels. With a `sev:high` issue open,
M4 would still pass, and TICK rule j would tag `m4`.

### Proposed change
gate.py stays offline. The nightly workflow, which already has a token, counts open issues per
label and passes the counts in a file.

Workflow step, before the gate step (the job needs `issues: read`):
```sh
for s in critical high medium; do
  printf '"open_sev_%s": %s\n' "$s" "$(gh issue list -R "$GITHUB_REPOSITORY" --state open \
    --label security --label "sev:$s" --limit 1000 --json number --jq length)"
done | paste -sd, - | sed 's/^/{/; s/$/}/' > sev-counts.json
```
(and pass `--sev-counts sev-counts.json` to gate.py).

ops/judge/gate.py:
```diff
     ap.add_argument("--selftest", choices=["pass", "fail"], default="fail")
+    ap.add_argument("--sev-counts", type=Path)
     ap.add_argument("--out", type=Path, required=True)
@@
     m["journeys"] = smoke["metrics"].get("journeys", 0)
+    # ADR 0009 gates. A missing file leaves the metrics unset, so a gate that uses them fails.
+    if a.sev_counts:
+        sev = C.load(a.sev_counts, {})
+        for k in ("open_sev_critical", "open_sev_high", "open_sev_medium"):
+            if isinstance(sev.get(k), int):
+                m[k] = sev[k]
```

ops/judge/gates.toml:
```diff
-[M4]   # engine API + reader apps: journeys only
+[M4]   # engine API + reader apps; owner preview: no open sev:critical/high (ADR 0009)
+max = { open_sev_critical = 0, open_sev_high = 0 }
@@
 [M10]  # hardening + 1.0
 min = { smoke_render_match = 0.97, exam_render_match = 0.95, fuzz_clean_nights = 30 }
-max = { smoke_bench_ratio = 1.5 }
+max = { smoke_bench_ratio = 1.5, open_sev_medium = 0 }
```
Gates are cumulative, so M10 also inherits M4's critical/high criterion.

Test plan: run gate.py on a saved report with counts `{0,0,0}` (M4's own criteria pass), `{0,1,0}`
(M4 fails with `open_sev_high = 1 > 0`), no file (M4 fails with `= None`), and `{0,0,1}` (M4 passes,
M10 fails).

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
