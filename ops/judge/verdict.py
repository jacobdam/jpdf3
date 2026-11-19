#!/usr/bin/env python3
"""Grade an untrusted run against trusted references and the ratchet baseline. PROTECTED.

usage:
  verdict.py --tier smoke --ref REFDIR --results RESDIR --probes PROBEDIR --corpus CORPUS
             --baseline-status found|none-ever|missing [--baseline FILE]
             --journeys DIR [--base-journeys DIR] [--e2e FILE ...] [--junit FILE]
             --out OUTDIR
  verdict.py --tier exam --ref REFDIR --results RESDIR --corpus CORPUS --out OUTDIR   (aggregates only)

Writes OUTDIR/report.json, OUTDIR/summary.md and, for smoke, OUTDIR/baseline.json
(the merged best-ever baseline, uploaded only by CI on main). Exit 0 = pass, 1 = fail.
"""
from __future__ import annotations

import argparse
import json
import math
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import common as C  # noqa: E402

REWRITE_MAX_DIFF = 0.005


# ------------------------------------------------------------------ corpus grading
def grade_corpus(ref: dict, res: dict, refdir: Path, resdir: Path) -> dict:
    files, pages, crashes, ratios = {}, {}, [], []
    for blob, rf in ref["files"].items():
        rr = res["files"].get(blob)
        if rr is None:
            files[blob] = {"opened": False}
            continue
        for what, r in [("info", rr["info"])] + [(f"p{p}.{k}", v[k]) for p, v in rr["pages"].items() for k in ("render", "text")]:
            if r["status"] in ("crash", "timeout"):
                crashes.append({"file": blob, "op": what, "status": r["status"], "code": r["code"], "stderr": r.get("stderr", "")[-300:]})
        files[blob] = {"opened": rr["info"]["status"] == "ok" and bool(rr.get("npages"))}
        for p, rp in rf["pages"].items():
            key = f"{blob}#{p}"
            ours = rr["pages"].get(p)
            item: dict = {"consensus": bool(rp.get("consensus"))}
            if rp.get("consensus") and ours and ours.get("png") and ours["render"]["status"] == "ok":
                d = C.diff_fraction(refdir / "png" / rp["mu"], resdir / "png" / ours["png"])
                item["match"] = d <= C.MATCH_MAX_FRACTION
                item["diff"] = round(d, 5)
                if item["match"] and rp.get("mu_ms") and ours["render"]["ms"]:
                    ratios.append(max(ours["render"]["ms"], 1) / max(rp["mu_ms"], 1))
            else:
                item["match"] = False
            if rp.get("text"):
                ref_txt = (refdir / "txt" / rp["text"]).read_text("utf-8", "replace")
                our_txt = (resdir / "txt" / ours["txt"]).read_text("utf-8", "replace") if ours and ours.get("txt") else ""
                item["text_f1"] = round(C.text_f1(our_txt, ref_txt), 4) if (ours and ours.get("txt")) else 0.0
            pages[key] = item
    qpdf_ok = [b for b, r in ref["files"].items() if r["qpdf_opens"]]
    cons = [k for k, v in pages.items() if v["consensus"]]
    tf = [v["text_f1"] for v in pages.values() if "text_f1" in v]
    metrics = {
        "files": len(files),
        "open_rate": ratio(sum(files[b]["opened"] for b in qpdf_ok), len(qpdf_ok)),
        "consensus_pages": len(cons),
        "render_match": ratio(sum(pages[k]["match"] for k in cons), len(cons)),
        "text_f1": round(sum(tf) / len(tf), 4) if tf else 0.0,
        "crashes": len(crashes),
        "bench_ratio": round(math.exp(sum(math.log(r) for r in ratios) / len(ratios)), 3) if ratios else None,
        "bench_samples": len(ratios),
    }
    return {"files": files, "pages": pages, "crashes": crashes, "metrics": metrics}


def ratio(a: int, b: int) -> float:
    return round(a / b, 4) if b else 0.0


# ------------------------------------------------------------------ probes
def qpdf_ok(path: Path, pw: str | None = None) -> bool:
    cmd = ["qpdf", "--check", *( [f"--password={pw}"] if pw else []), str(path)]
    return C.run_tool(cmd, 120).returncode in (0, 3)


def mu_render(pdf: Path, out: Path, pw: str | None = None) -> bool:
    cmd = ["mutool", "draw", "-q", "-r", str(C.RENDER_DPI), *(["-p", pw] if pw else []), "-o", str(out), str(pdf), "1"]
    return C.run_tool(cmd, 60).returncode == 0 and out.exists()


def qdf_text(pdf: Path) -> bytes:
    return C.run_tool(["qpdf", "--qdf", "--object-streams=disable", str(pdf), "-"], 120).stdout


def pdftotext_words(pdf: Path, page: int | None = None) -> dict:
    rng = ["-f", str(page), "-l", str(page)] if page else []
    return C.words(C.run_tool(["pdftotext", "-q", "-enc", "UTF-8", *rng, str(pdf), "-"], 120).stdout.decode("utf-8", "replace"))


def check_probe(pr: dict, r: dict, ctx: dict) -> tuple[bool, str]:
    if r.get("status") != "ok":
        return False, r.get("status", "missing")
    k = pr["kind"]
    corpus, probes, resdir, refdir, ref, tmp = (ctx[x] for x in ("corpus", "probes", "resdir", "refdir", "ref", "tmp"))
    src = corpus / f"{pr['input'].split(':', 1)[1]}.pdf" if pr["input"].startswith("corpus:") else probes / "inputs" / pr["input"].split(":", 1)[1]
    out = resdir / "probe" / r["out"] if r.get("out") else None
    if k != "verify" and (out is None or not out.exists()):
        return False, "no output"
    blob = pr["input"].split(":", 1)[1]
    ref_png = refdir / "png" / f"{blob}_1_mu.png"
    if k == "annotate":
        data, orig = out.read_bytes(), src.read_bytes()
        if not data.startswith(orig):
            return False, "not an incremental update"
        if not qpdf_ok(out):
            return False, "qpdf --check failed"
        if not re.search(rb"/Subtype\s*/Highlight", qdf_text(out)):
            return False, "no Highlight annotation"
        png = tmp / f"{pr['id']}.png"
        if not mu_render(out, png):
            return False, "mutool cannot render output"
        inside, outside = C.region_change(ref_png, png, tuple(pr["rect"]))
        if inside < 0.05:
            return False, f"highlight not visible (changed {inside:.3f} inside rect)"
        if outside > 0.01:
            return False, f"page changed outside rect ({outside:.3f})"
        return True, ""
    if k == "rewrite":
        data = out.read_bytes()
        if data == src.read_bytes():
            return False, "output identical to input (not a rewrite)"
        if data.count(b"%%EOF") != 1:
            return False, "full rewrite must not contain incremental history"
        if not qpdf_ok(out):
            return False, "qpdf --check failed"
        png = tmp / f"{pr['id']}.png"
        if not mu_render(out, png) or C.diff_fraction(ref_png, png) > REWRITE_MAX_DIFF:
            return False, "rendering changed after rewrite"
        return True, ""
    if k == "encrypt":
        show = C.run_tool(["qpdf", "--show-encryption", f"--password={pr['user_password']}", str(out)], 60).stdout.decode()
        if "R = 6" not in show:
            return False, "not AES-256 R6"
        if C.run_tool(["qpdf", "--check", str(out)], 60).returncode not in (2,):
            return False, "opens without the password"
        if not qpdf_ok(out, pr["user_password"]):
            return False, "qpdf --check with password failed"
        png = tmp / f"{pr['id']}.png"
        if not mu_render(out, png, pr["user_password"]) or C.diff_fraction(ref_png, png) > C.MATCH_MAX_FRACTION:
            return False, "decrypted rendering differs"
        return True, ""
    if k == "decrypt":
        like = refdir / "png" / f"{pr['expect_like']}_1_mu.png"
        d = C.diff_fraction(like, out)
        return (d <= C.MATCH_MAX_FRACTION), f"diff {d:.3f}"
    if k == "fill":
        js = C.run_tool(["qpdf", "--json=2", "--json-key=acroform", str(out)], 60)
        try:
            af = json.loads(js.stdout)["acroform"]
        except (ValueError, KeyError):
            return False, "cannot read AcroForm"
        if af.get("needappearances"):
            return False, "NeedAppearances is true (appearance not generated)"
        fld = next((f for f in af.get("fields", []) if f.get("fullname") == pr["field"]), None)
        if not fld or pr["value"] not in json.dumps(fld.get("value", "")):
            return False, "field value not set"
        if C.raw_and_inflated(out.read_bytes()).count(pr["value"].encode()) < 2:
            return False, "value missing from appearance stream"
        return True, ""
    if k == "redact":
        if not qpdf_ok(out):
            return False, "qpdf --check failed"
        w = pr["word"]
        if pdftotext_words(out).get(w.lower(), 0):
            return False, "redacted word still extractable"
        if pr.get("raw_detectable") and w.encode() in C.raw_and_inflated(out.read_bytes()):
            return False, "redacted word still in file bytes (incl. old revisions/streams)"
        kept = sum(pdftotext_words(out, pr["page"]).values())
        if kept < 0.7 * max(pr["page_word_count"] - 1, 0):
            return False, f"too much content removed ({kept}/{pr['page_word_count']})"
        png = tmp / f"{pr['id']}.png"
        if not mu_render(out, png):
            return False, "mutool cannot render output"
        import numpy as np
        a = C.load_rgb(png)
        x0, y0, x1, y1 = (int(round(v)) for v in pr["rect"])
        region = a[max(y0, 0):max(y1, 0), max(x0, 0):max(x1, 0)]
        if region.size == 0 or float(np.mean(region)) > 80:
            return False, "redaction area not painted"
        return True, ""
    if k == "sign":
        if not out.read_bytes().startswith(src.read_bytes()):
            return False, "signature not added incrementally"
        s = C.run_tool(["pdfsig", str(out)], 60).stdout.decode("utf-8", "replace")
        if "Signature is Valid" not in s:
            return False, "pdfsig: signature not valid"
        if "Total document signed" not in s:
            return False, "pdfsig: does not cover the whole document"
        return True, ""
    if k == "verify":
        try:
            sigs = json.loads(r.get("stdout", ""))["signatures"]
            got = sigs[0]["integrity"]
        except (ValueError, KeyError, IndexError, TypeError):
            return False, "bad output"
        return got == pr["expect"], f"got {got}, expected {pr['expect']}"
    return False, "unknown probe"


def grade_probes(plan: list[dict], res: dict, ctx: dict) -> tuple[dict, list]:
    out, crashes = {}, []
    for pr in plan:
        r = res["probes"].get(pr["id"], {"status": "missing"})
        if r.get("status") in ("crash", "timeout"):
            crashes.append({"probe": pr["id"], "status": r["status"], "code": r.get("code"), "stderr": r.get("stderr", "")[-300:]})
        try:
            ok, why = check_probe(pr, r, ctx)
        except Exception as e:  # a grader error is a judge bug: fail the item, report it
            ok, why = False, f"grader error: {e!r}"
        out[pr["id"]] = {"kind": pr["kind"], "ok": ok, "why": why}
    return out, crashes


def probe_metrics(probes: dict) -> dict:
    kinds: dict = {}
    for v in probes.values():
        t = kinds.setdefault(v["kind"], [0, 0])
        t[0] += v["ok"]
        t[1] += 1
    m = {f"probe_{k}": ratio(a, b) for k, (a, b) in kinds.items()}
    m["redact_leaks"] = sum(1 for v in probes.values() if v["kind"] == "redact" and "still" in v["why"])
    return m


# ------------------------------------------------------------------ journeys
J_HEAD = re.compile(r"^##\s+(J-[A-Z0-9]+-\d+)\b", re.M)


def parse_journeys(d: Path | None) -> dict:
    out: dict = {}
    if not d or not d.exists():
        return out
    for f in sorted(d.glob("*.md")):
        txt = f.read_text("utf-8", "replace")
        heads = list(J_HEAD.finditer(txt))
        for i, m in enumerate(heads):
            body = txt[m.end(): heads[i + 1].start() if i + 1 < len(heads) else len(txt)]

            def field(name):
                mm = re.search(rf"^-\s*{name}:\s*(.+)$", body, re.M)
                return mm.group(1).strip() if mm else ""
            out[m.group(1)] = {"milestone": field("milestone").upper(), "platform": field("platform"),
                               "tests": [t.strip() for t in field("test").split(",") if t.strip()],
                               "superseded_by": field("superseded_by") or None}
    return out


def grade_journeys(cur: dict, base: dict, e2e: dict) -> tuple[dict, list]:
    problems = []
    for jid in base:
        if jid not in cur:
            problems.append(f"journey {jid} was deleted (append-only, CONSTITUTION 15)")
    for jid, j in cur.items():
        if j["superseded_by"] and j["superseded_by"] not in cur:
            problems.append(f"journey {jid} superseded by unknown {j['superseded_by']}")
        if not j["tests"] and not j["superseded_by"]:
            problems.append(f"journey {jid} has no test ids")
    status = {}
    for jid, j in cur.items():
        status[jid] = {"milestone": j["milestone"], "superseded": bool(j["superseded_by"]),
                       "pass": bool(j["tests"]) and all(e2e.get(t) == "pass" for t in j["tests"])}
    return status, problems


# ------------------------------------------------------------------ tests
def parse_junit(path: Path | None) -> dict:
    if not path or not path.exists():
        return {"passed": 0, "failed": 0, "flaky": 0, "present": False}
    root = ET.parse(path).getroot()
    passed = failed = flaky = 0
    for tc in root.iter("testcase"):
        tags = {c.tag for c in tc}
        if tags & {"failure", "error"}:
            failed += 1
        elif "skipped" in tags:
            continue
        else:
            passed += 1
            if tags & {"flakyFailure", "flakyError", "rerunFailure"}:
                flaky += 1
    return {"passed": passed, "failed": failed, "flaky": flaky, "present": True}


# ------------------------------------------------------------------ ratchet
def ratchet(cur: dict, base: dict) -> list[str]:
    reg = []
    for b, v in base.get("files", {}).items():
        if v and not cur["files"].get(b, False):
            reg.append(f"file no longer opens: {b}")
    for k, v in base.get("pages", {}).items():
        c = cur["pages"].get(k)
        if v.get("match") and not (c and c.get("match")):
            reg.append(f"page no longer matches consensus: {k}")
        if "text_f1" in v and c is not None and c.get("text_f1", 0.0) < v["text_f1"] - C.TEXT_DROP_TOLERANCE:
            reg.append(f"text score dropped on {k}: {v['text_f1']} -> {c.get('text_f1')}")
    for k, v in base.get("probes", {}).items():
        if v and not cur["probes"].get(k, False):
            reg.append(f"probe regressed: {k}")
    for k, v in base.get("journeys", {}).items():
        if v and not cur["journeys"].get(k, False):
            reg.append(f"journey regressed: {k}")
    if cur["tests_passed"] < base.get("tests_passed", 0):
        reg.append(f"passing test count dropped: {base['tests_passed']} -> {cur['tests_passed']}")
    return reg


def snapshot(corpus: dict, probes: dict, journeys: dict, tests: dict) -> dict:
    return {
        "files": {b: v["opened"] for b, v in corpus["files"].items()},
        "pages": {k: {kk: v[kk] for kk in ("match", "text_f1") if kk in v} for k, v in corpus["pages"].items()},
        "probes": {k: v["ok"] for k, v in probes.items()},
        "journeys": {k: v["pass"] for k, v in journeys.items()},
        "tests_passed": tests["passed"],
    }


def merge_best(base: dict, cur: dict) -> dict:
    out = json.loads(json.dumps(cur))
    for sect in ("files", "probes", "journeys"):
        for k, v in base.get(sect, {}).items():
            out[sect][k] = bool(v) or bool(out[sect].get(k))
    for k, v in base.get("pages", {}).items():
        o = out["pages"].setdefault(k, {})
        if v.get("match"):
            o["match"] = True
        if "text_f1" in v:
            o["text_f1"] = max(v["text_f1"], o.get("text_f1", 0.0))
    out["tests_passed"] = max(base.get("tests_passed", 0), cur["tests_passed"])
    out["fuzz_clean_nights"] = base.get("fuzz_clean_nights", 0)
    return out


# ------------------------------------------------------------------ main
def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tier", choices=["smoke", "exam"], required=True)
    ap.add_argument("--ref", type=Path, required=True)
    ap.add_argument("--results", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--probes", type=Path)
    ap.add_argument("--baseline", type=Path)
    ap.add_argument("--baseline-status", choices=["found", "none-ever", "missing"], default="none-ever")
    ap.add_argument("--journeys", type=Path)
    ap.add_argument("--base-journeys", type=Path)
    ap.add_argument("--e2e", type=Path, nargs="*", default=[])
    ap.add_argument("--junit", type=Path)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    ref = C.load(a.ref / "reference.json")
    res = C.load(a.results / "results.json")
    if ref is None:
        C.infra_fail("reference.json missing")
    if res is None:
        res = {"binary": "missing", "files": {}, "probes": {}}
    corpus = grade_corpus(ref, res, a.ref, a.results)

    if a.tier == "exam":  # aggregates only: never per-file detail (CONSTITUTION 13)
        report = {"tier": "exam", "metrics": corpus["metrics"]}
        C.dump(a.out / "report.json", report)
        (a.out / "summary.md").write_text("## Exam corpus (aggregate only)\n\n" + table(corpus["metrics"]))
        print(json.dumps(report["metrics"]))
        sys.exit(1 if corpus["metrics"]["crashes"] else 0)

    tmp = a.out / "tmp"
    tmp.mkdir(exist_ok=True)
    plan = C.load(a.probes / "probe-plan.json", {"probes": []})["probes"] if a.probes else []
    ctx = {"corpus": a.corpus, "probes": a.probes, "resdir": a.results, "refdir": a.ref, "ref": ref, "tmp": tmp}
    probes, pcrash = grade_probes(plan, res, ctx)
    e2e: dict = {}
    for f in a.e2e:
        d = C.load(f, {}) or {}
        for k, v in (d.get("results") or {}).items():
            e2e[k] = v
    journeys, jproblems = grade_journeys(parse_journeys(a.journeys), parse_journeys(a.base_journeys), e2e)
    tests = parse_junit(a.junit)
    cur = snapshot(corpus, probes, journeys, tests)

    failures: list[str] = []
    crashes = corpus["crashes"] + pcrash
    if crashes:
        failures.append(f"{len(crashes)} crash/timeout(s) on hostile input")
    if tests["failed"]:
        failures.append(f"{tests['failed']} failing test(s)")
    failures += jproblems
    base = None
    if a.baseline_status == "missing":
        failures.append("ratchet baseline missing although main has successful runs (judge alarm)")
    elif a.baseline_status == "found":
        base = C.load(a.baseline) if a.baseline else None
        if base is None:
            failures.append("ratchet baseline unreadable (judge alarm)")
    regressions = ratchet(cur, base) if base else []
    if regressions:
        failures.append(f"{len(regressions)} ratchet regression(s)")

    metrics = {**corpus["metrics"], **probe_metrics(probes),
               "tests_passed": tests["passed"], "tests_failed": tests["failed"], "tests_flaky": tests["flaky"],
               "journeys": len(journeys), "journeys_passing": sum(j["pass"] for j in journeys.values()),
               "binary": res.get("binary", "missing")}
    report = {"tier": "smoke", "pass": not failures, "failures": failures, "metrics": metrics,
              "regressions": regressions, "crashes": crashes,
              "probes_failed": {k: v["why"] for k, v in probes.items() if not v["ok"]},
              "journeys": journeys}
    C.dump(a.out / "report.json", report)
    C.dump(a.out / "baseline.json", merge_best(base or {}, cur))
    lines = ["## Judge verdict: " + ("PASS" if not failures else "FAIL"), ""]
    lines += [f"- {f}" for f in failures] + ["", table(metrics)]
    if regressions:
        lines += ["", "### Regressions (first 50)"] + [f"- {r}" for r in regressions[:50]]
    if crashes:
        lines += ["", "### Crashes (first 20)"] + [f"- `{json.dumps(c)[:300]}`" for c in crashes[:20]]
    (a.out / "summary.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines[:40]))
    sys.exit(0 if not failures else 1)


def table(m: dict) -> str:
    return "| metric | value |\n|---|---|\n" + "\n".join(f"| {k} | {v} |" for k, v in sorted(m.items())) + "\n"


if __name__ == "__main__":
    main()
