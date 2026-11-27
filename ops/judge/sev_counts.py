#!/usr/bin/env python3
"""Count open security issues per sev:* label for the ADR 0009 gates. PROTECTED.

usage: sev_counts.py OPEN_ISSUES_JSON   (from `gh issue list --state open --limit 1000 --json number,labels`)
Prints {"open_sev_critical", "open_sev_high", "open_sev_medium", "open_sev_unlabeled"} as JSON.
Counts by the sev:* label alone; an open `security` issue with no or several sev:* labels also
counts as unlabeled. Refuses a list of 1000 or more (possibly truncated): exits non-zero and
prints nothing, so gate.py fails the gates closed.
"""
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
