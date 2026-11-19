#!/usr/bin/env python3
"""Run the agent-built jpdf-cli over a corpus and the probe plan. PROTECTED.

Runs in the UNTRUSTED job, which has no reference tools and no reference renders. It only
records what jpdf-cli produced. All grading happens later in the judge job.

usage: run_untrusted.py --bin BIN --manifest M --corpus DIR --out DIR [--probes DIR]
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import common as C  # noqa: E402


def rec(r: C.RunResult) -> dict:
    return {"status": r.status, "code": r.code, "ms": r.ms, "stderr": r.stderr[-600:].decode("utf-8", "replace")}


def run_file(bin_: str, blob: str, pdf: Path, out: Path) -> dict:
    res: dict = {"pages": {}}
    info = C.run_limited([bin_, "info", str(pdf)], C.TIMEOUT_INFO_S)
    res["info"] = rec(info)
    n = None
    if info.status == "ok":
        try:
            n = int(json.loads(info.stdout.decode("utf-8"))["pages"])
        except (ValueError, KeyError, TypeError, UnicodeDecodeError):
            res["info"]["status"] = "bad-output"
    res["npages"] = n
    if not n or n < 1:
        return res
    for p in range(1, min(C.PAGES_PER_FILE, n) + 1):
        png = out / "png" / f"{blob}_{p}.png"
        r = C.run_limited([bin_, "render", str(pdf), "--page", str(p), "--dpi", str(C.RENDER_DPI),
                           "--out", str(png)], C.TIMEOUT_RENDER_S)
        t = C.run_limited([bin_, "text", str(pdf), "--page", str(p)], C.TIMEOUT_RENDER_S)
        txt = out / "txt" / f"{blob}_{p}.txt"
        if t.status == "ok":
            txt.write_bytes(t.stdout)
        res["pages"][str(p)] = {"render": rec(r), "png": png.name if png.exists() else None,
                                "text": rec(t), "txt": txt.name if t.status == "ok" else None}
    return res


def resolve(ref: str, corpus: Path, probes: Path) -> Path:
    kind, _, name = ref.partition(":")
    return corpus / f"{name}.pdf" if kind == "corpus" else probes / "inputs" / name


def run_probe(bin_: str, pr: dict, corpus: Path, probes: Path, out: Path) -> dict:
    src = str(resolve(pr["input"], corpus, probes))
    dst = out / "probe" / f"{pr['id']}.pdf"
    k = pr["kind"]
    rect = ",".join(str(v) for v in pr.get("rect", []))
    if k == "annotate":
        cmd = ["annotate", src, "--out", str(dst), "--page", str(pr["page"]), "--kind", "highlight",
               "--rect", rect, "--color", ",".join(map(str, pr["color"])), "--save", "incremental"]
    elif k == "rewrite":
        cmd = ["rewrite", src, "--out", str(dst), "--mode", "full"]
    elif k == "encrypt":
        cmd = ["encrypt", src, "--out", str(dst), "--user-password", pr["user_password"],
               "--owner-password", pr["owner_password"]]
    elif k == "decrypt":
        dst = out / "probe" / f"{pr['id']}.png"
        cmd = ["render", src, "--page", "1", "--dpi", str(C.RENDER_DPI), "--out", str(dst), "--password", pr["password"]]
    elif k == "fill":
        cmd = ["fill", src, "--out", str(dst), "--field", pr["field"], "--value", pr["value"]]
    elif k == "redact":
        cmd = ["redact", src, "--out", str(dst), "--page", str(pr["page"]), "--rect", rect]
    elif k == "sign":
        cmd = ["sign", src, "--out", str(dst), "--p12", str(resolve(pr["p12"], corpus, probes)),
               "--p12-password", pr["p12_password"], "--field", pr["field"]]
    elif k == "verify":
        dst = None
        cmd = ["verify-signatures", src]
    else:
        return {"status": "unknown-probe"}
    r = C.run_limited([bin_, *cmd], C.TIMEOUT_PROBE_S)
    d = rec(r)
    d["out"] = dst.name if dst is not None and dst.exists() else None
    if k == "verify" and r.status == "ok":
        d["stdout"] = r.stdout.decode("utf-8", "replace")[:20000]
    return d


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--manifest", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--probes", type=Path)
    ap.add_argument("--limit", type=int, default=0)
    a = ap.parse_args()
    for d in ("png", "txt", "probe"):
        (a.out / d).mkdir(parents=True, exist_ok=True)
    entries = C.load_manifest(a.manifest)
    if a.limit:
        entries = entries[: a.limit]
    bin_ = str(Path(a.bin).resolve())
    present = Path(bin_).exists()
    workers = os.cpu_count() or 4
    with ThreadPoolExecutor(workers) as ex:
        files = dict(ex.map(lambda e: (e.blob, run_file(bin_, e.blob, a.corpus / f"{e.blob}.pdf", a.out)), entries))
    probes: dict = {}
    if a.probes and (a.probes / "probe-plan.json").exists():
        plan = C.load(a.probes / "probe-plan.json")["probes"]
        with ThreadPoolExecutor(workers) as ex:
            probes = dict(ex.map(lambda pr: (pr["id"], run_probe(bin_, pr, a.corpus, a.probes, a.out)), plan))
    # Written once, at the end, so nothing jpdf-cli did earlier can pre-seed the results.
    C.dump(a.out / "results.json", {"binary": "present" if present else "missing", "files": files, "probes": probes})
    crashes = sum(1 for f in files.values() for r in [f["info"], *[x for p in f["pages"].values() for x in (p["render"], p["text"])]]
                  if r["status"] in ("crash", "timeout"))
    print(f"untrusted run: binary {'present' if present else 'missing'}, {len(files)} files, {len(probes)} probes, {crashes} crash/timeouts")


if __name__ == "__main__":
    main()
