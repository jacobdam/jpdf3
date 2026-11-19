#!/usr/bin/env python3
"""Evaluate milestone gates from nightly judge reports. PROTECTED.

usage: gate.py --smoke REPORT --exam REPORT --repo TREE [--prev GATES_JSON] [--fuzz clean|crash|none]
               [--selftest pass|fail] --out DIR
Writes DIR/gates.json and DIR/summary.md. Always exits 0 (a report, not a check),
unless inputs are missing (infra).
"""
from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import common as C  # noqa: E402

ORDER = [f"M{i}" for i in range(11)]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--smoke", type=Path, required=True)
    ap.add_argument("--exam", type=Path, required=True)
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--prev", type=Path)
    ap.add_argument("--fuzz", choices=["clean", "crash", "none"], default="none")
    ap.add_argument("--selftest", choices=["pass", "fail"], default="fail")
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    smoke, exam = C.load(a.smoke), C.load(a.exam)
    if smoke is None or exam is None:
        C.infra_fail("gate inputs missing")
    prev = C.load(a.prev, {}) if a.prev else {}
    nights = prev.get("fuzz_clean_nights", 0)
    nights = nights + 1 if a.fuzz == "clean" else (0 if a.fuzz == "crash" else nights)

    m = {f"smoke_{k}": v for k, v in smoke["metrics"].items()}
    m.update({f"exam_{k}": v for k, v in exam["metrics"].items()})
    m["fuzz_clean_nights"] = nights
    m["journeys"] = smoke["metrics"].get("journeys", 0)
    flags = {
        "binary_present": smoke["metrics"].get("binary") == "present",
        "ui_spec_present": any((a.repo / "specs" / "ui").glob("*.md")) if (a.repo / "specs" / "ui").exists() else False,
    }
    gates_cfg = tomllib.loads((Path(__file__).resolve().parent / "gates.toml").read_text())
    journeys = smoke.get("journeys", {})
    result, cumulative_ok = {}, True
    for g in ORDER:
        cfg = gates_cfg.get(g, {})
        fails = []
        if a.selftest != "pass":
            fails.append("judge self-test not green")
        if m.get("smoke_crashes", 1) or m.get("exam_crashes", 1):
            fails.append("crashes on smoke/exam corpus")
        for f in cfg.get("require", []):
            if not flags.get(f):
                fails.append(f"{f} is false")
        for k, v in cfg.get("min", {}).items():
            got = m.get(k)
            if got is None or got < v:
                fails.append(f"{k} = {got} < {v}")
        for k, v in cfg.get("max", {}).items():
            got = m.get(k)
            if got is None or got > v:
                fails.append(f"{k} = {got} > {v}")
        upto = ORDER.index(g)
        for jid, j in journeys.items():
            ms = j.get("milestone", "")
            if not j.get("superseded") and ms in ORDER and ORDER.index(ms) <= upto and not j.get("pass"):
                fails.append(f"journey {jid} ({ms}) not passing")
        own_ok = not fails
        cumulative_ok = cumulative_ok and own_ok
        result[g] = {"pass": cumulative_ok, "own_criteria_pass": own_ok, "failing": fails[:40]}
    current = next((g for g in ORDER if not result[g]["pass"]), "DONE")
    out = {"current_milestone": current, "gates": result, "metrics": m, "fuzz_clean_nights": nights}
    C.dump(a.out / "gates.json", out)
    lines = [f"## Milestone gates: current = {current}", "", "| gate | pass | first failing criteria |", "|---|---|---|"]
    for g in ORDER:
        r = result[g]
        lines.append(f"| {g} | {'✅' if r['pass'] else '❌'} | {'; '.join(r['failing'][:3])} |")
    (a.out / "summary.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
