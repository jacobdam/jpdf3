#!/usr/bin/env python3
"""Reference renders + probe plan, produced ONLY by trusted tools. PROTECTED.

usage: reference.py MANIFEST CORPUS_DIR OUT_DIR [--probes PROBE_DIR] [--limit N]

OUT_DIR/reference.json, OUT_DIR/png/*, OUT_DIR/txt/*        (stay in the judge job)
PROBE_DIR/probe-plan.json + PROBE_DIR/inputs/*                (handed to the untrusted job;
                                                              contains no reference renders)
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import common as C  # noqa: E402

PROBE_COUNTS = {"annotate": 40, "rewrite": 40, "encrypt": 20, "decrypt": 21,
                "fill": 15, "redact": 30, "sign": 10, "verify": 20}
USER_PW, OWNER_PW, P12_PW = "jpdf-user", "jpdf-owner", "jpdf-p12"


def tool_versions() -> dict:
    def first_line(cmd):
        p = C.run_tool(cmd, 30)
        return (p.stdout or p.stderr).decode(errors="replace").strip().splitlines()[0:1]
    return {"mutool": first_line(["mutool", "-v"]), "pdftoppm": first_line(["pdftoppm", "-v"]),
            "qpdf": first_line(["qpdf", "--version"]), "pdfsig": first_line(["pdfsig", "-v"])}


def ref_file(blob: str, pdf: Path, out: Path) -> dict:
    rec: dict = {"qpdf_opens": False, "encrypted": False, "npages": None, "pages": {}}
    chk = C.run_tool(["qpdf", "--check", str(pdf)], 120)
    rec["qpdf_opens"] = chk.returncode in (0, 3)
    rec["encrypted"] = C.run_tool(["qpdf", "--is-encrypted", str(pdf)], 30).returncode == 0
    np_ = C.run_tool(["qpdf", "--show-npages", str(pdf)], 60)
    if np_.returncode in (0, 3):
        try:
            rec["npages"] = int(np_.stdout.decode().strip().splitlines()[0])
        except (ValueError, IndexError):
            pass
    if not rec["npages"]:
        return rec
    for p in range(1, min(C.PAGES_PER_FILE, rec["npages"]) + 1):
        mu = out / "png" / f"{blob}_{p}_mu.png"
        pp = out / "png" / f"{blob}_{p}_pp.png"
        tx = out / "txt" / f"{blob}_{p}.txt"
        t0 = time.monotonic()
        r1 = C.run_tool(["mutool", "draw", "-q", "-r", str(C.RENDER_DPI), "-o", str(mu), str(pdf), str(p)], 60)
        mu_ms = int((time.monotonic() - t0) * 1000)
        r2 = C.run_tool(["pdftoppm", "-q", "-r", str(C.RENDER_DPI), "-f", str(p), "-l", str(p),
                         "-png", "-singlefile", str(pdf), str(pp.with_suffix(""))], 60)
        r3 = C.run_tool(["pdftotext", "-q", "-enc", "UTF-8", "-f", str(p), "-l", str(p), str(pdf), str(tx)], 120)
        page: dict = {"mu_ms": mu_ms}
        page["mu"] = mu.name if r1.returncode == 0 and mu.exists() else None
        page["pp"] = pp.name if r2.returncode == 0 and pp.exists() else None
        page["text"] = tx.name if r3.returncode == 0 and tx.exists() else None
        if page["mu"] and page["pp"]:
            d = C.diff_fraction(mu, pp)
            page["mu_pp_diff"] = round(d, 5)
            page["consensus"] = d <= C.MATCH_MAX_FRACTION
        else:
            page["consensus"] = False
        rec["pages"][str(p)] = page
    return rec


# ------------------------------------------------------------------ probe plan
def eligible(ref: dict) -> list[str]:
    out = []
    for blob, r in sorted(ref.items()):
        p1 = r["pages"].get("1")
        if r["qpdf_opens"] and not r["encrypted"] and p1 and p1.get("consensus"):
            out.append(blob)
    return out


def page_size(png: Path) -> tuple[int, int]:
    from PIL import Image
    with Image.open(png) as im:
        return im.size


_WORD_RE = re.compile(r'<word xMin="([\d.]+)" yMin="([\d.]+)" xMax="([\d.]+)" yMax="([\d.]+)">([^<]*)</word>')


def pick_redaction(pdf: Path) -> dict | None:
    bb = C.run_tool(["pdftotext", "-q", "-bbox", "-f", "1", "-l", "1", str(pdf), "-"], 120)
    full = C.run_tool(["pdftotext", "-q", "-enc", "UTF-8", str(pdf), "-"], 120)
    if bb.returncode != 0 or full.returncode != 0:
        return None
    counts = C.words(full.stdout.decode("utf-8", "replace"))
    raw = C.raw_and_inflated(pdf.read_bytes())
    page_words = _WORD_RE.findall(bb.stdout.decode("utf-8", "replace"))
    best = None
    for x0, y0, x1, y1, w in page_words:
        if len(w) < 6 or not w.isalpha() or not w.isascii() or counts.get(w.lower(), 0) != 1:
            continue
        cand = {"word": w, "rect": [float(x0) - 1, float(y0) - 1, float(x1) + 1, float(y1) + 1],
                "raw_detectable": w.encode() in raw, "page_word_count": len(page_words)}
        if cand["raw_detectable"]:
            return cand
        best = best or cand
    return best


def text_fields(pdf: Path) -> list[dict]:
    p = C.run_tool(["qpdf", "--json=2", "--json-key=acroform", str(pdf)], 120)
    if p.returncode not in (0, 3):
        return []
    try:
        fields = json.loads(p.stdout)["acroform"]["fields"]
    except (KeyError, ValueError, TypeError):
        return []
    return [f for f in fields if f.get("fieldtype") == "/Tx" and f.get("fullname")]


def make_sign_material(d: Path) -> bool:
    d.mkdir(parents=True, exist_ok=True)
    steps = [
        ["openssl", "ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", str(d / "key.pem")],
        ["openssl", "req", "-new", "-x509", "-key", str(d / "key.pem"), "-out", str(d / "cert.pem"),
         "-days", "30", "-subj", "/CN=jpdf judge test signer"],
        ["openssl", "pkcs12", "-export", "-inkey", str(d / "key.pem"), "-in", str(d / "cert.pem"),
         "-out", str(d / "signer.p12"), "-passout", f"pass:{P12_PW}",
         "-keypbe", "AES-256-CBC", "-certpbe", "AES-256-CBC", "-macalg", "sha256"],
    ]
    return all(C.run_tool(s, 60).returncode == 0 for s in steps)


def pyhanko_sign(src: Path, dst: Path, keydir: Path) -> bool:
    try:
        from pyhanko.pdf_utils.incremental_writer import IncrementalPdfFileWriter
        from pyhanko.sign import signers
        signer = signers.SimpleSigner.load(str(keydir / "key.pem"), str(keydir / "cert.pem"))
        with src.open("rb") as f:
            w = IncrementalPdfFileWriter(f, strict=False)
            buf = signers.sign_pdf(w, signers.PdfSignatureMetadata(field_name="RefSig"), signer=signer)
            dst.write_bytes(buf.getbuffer())
        return True
    except Exception as e:  # reference unavailable for this file
        print(f"pyhanko failed on {src.name}: {e}", file=sys.stderr)
        return False


def tamper(src: Path, dst: Path) -> bool:
    data = bytearray(src.read_bytes())
    nl = data.find(b"\n")
    if nl < 0 or nl + 2 >= len(data):
        return False
    data[nl + 2] ^= 0x01  # flip a bit in the binary comment line: still parses, hash changes
    dst.write_bytes(bytes(data))
    return True


def build_probes(ref: dict, corpus: Path, refout: Path, pdir: Path) -> dict:
    inputs = pdir / "inputs"
    inputs.mkdir(parents=True, exist_ok=True)
    el = eligible(ref)
    plan: list[dict] = []

    def take(kind, start):
        n = PROBE_COUNTS[kind]
        return [el[(start + i * 7) % len(el)] for i in range(min(n, len(el)))] if el else []

    for blob in take("annotate", 0):
        w, h = page_size(refout / "png" / f"{blob}_1_mu.png")
        plan.append({"id": f"annotate-{blob}", "kind": "annotate", "input": f"corpus:{blob}",
                     "page": 1, "rect": [round(w * .3, 1), round(h * .4, 1), round(w * .7, 1), round(h * .5, 1)],
                     "color": [255, 230, 0]})
    for blob in take("rewrite", 1):
        plan.append({"id": f"rewrite-{blob}", "kind": "rewrite", "input": f"corpus:{blob}"})
    for blob in take("encrypt", 2):
        plan.append({"id": f"encrypt-{blob}", "kind": "encrypt", "input": f"corpus:{blob}",
                     "user_password": USER_PW, "owner_password": OWNER_PW})
    variants = [("r2", ["--allow-weak-crypto", "--bits=40"]), ("r4", ["--bits=128", "--use-aes=y"]), ("r6", ["--bits=256"])]
    for i, blob in enumerate(take("decrypt", 3)):
        name, args = variants[i % 3]
        dst = inputs / f"dec-{name}-{blob}.pdf"
        weak = [a for a in args if a == "--allow-weak-crypto"]
        enc = [a for a in args if a != "--allow-weak-crypto"]
        r = C.run_tool(["qpdf", *weak, "--encrypt", f"--user-password={USER_PW}", f"--owner-password={OWNER_PW}",
                        *enc, "--", str(corpus / f"{blob}.pdf"), str(dst)], 120)
        if r.returncode in (0, 3):
            plan.append({"id": f"decrypt-{name}-{blob}", "kind": "decrypt", "input": f"probe:{dst.name}",
                         "password": USER_PW, "expect_like": blob})
    n = 0
    for blob in el:
        if n >= PROBE_COUNTS["fill"]:
            break
        fs = text_fields(corpus / f"{blob}.pdf")
        if fs:
            plan.append({"id": f"fill-{blob}", "kind": "fill", "input": f"corpus:{blob}",
                         "field": fs[0]["fullname"], "value": "JPDF judge 4711"})
            n += 1
    n = 0
    for blob in el[5:] + el[:5]:
        if n >= PROBE_COUNTS["redact"]:
            break
        pick = pick_redaction(corpus / f"{blob}.pdf")
        if pick:
            plan.append({"id": f"redact-{blob}", "kind": "redact", "input": f"corpus:{blob}", "page": 1, **pick})
            n += 1
    keydir = pdir / "inputs" / "signer"
    if make_sign_material(keydir):
        for blob in take("sign", 4):
            plan.append({"id": f"sign-{blob}", "kind": "sign", "input": f"corpus:{blob}",
                         "p12": "probe:signer/signer.p12", "p12_password": P12_PW, "field": "JpdfSig"})
        for blob in take("verify", 5)[: PROBE_COUNTS["verify"] // 2]:
            good = inputs / f"signed-{blob}.pdf"
            bad = inputs / f"tampered-{blob}.pdf"
            if pyhanko_sign(corpus / f"{blob}.pdf", good, keydir) and tamper(good, bad):
                plan.append({"id": f"verify-good-{blob}", "kind": "verify", "input": f"probe:{good.name}", "expect": "valid"})
                plan.append({"id": f"verify-bad-{blob}", "kind": "verify", "input": f"probe:{bad.name}", "expect": "invalid"})
    (keydir / "key.pem").unlink(missing_ok=True)  # untrusted side only needs the p12
    return {"probes": plan}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("manifest", type=Path)
    ap.add_argument("corpus", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--probes", type=Path)
    ap.add_argument("--limit", type=int, default=0)
    a = ap.parse_args()
    entries = C.load_manifest(a.manifest)
    if a.limit:
        entries = entries[: a.limit]
    (a.out / "png").mkdir(parents=True, exist_ok=True)
    (a.out / "txt").mkdir(parents=True, exist_ok=True)
    missing = [e.blob for e in entries if not (a.corpus / f"{e.blob}.pdf").exists()]
    if missing:
        C.infra_fail(f"{len(missing)} corpus files missing locally")
    with ThreadPoolExecutor(min(os.cpu_count() or 4, 4)) as ex:
        recs = list(ex.map(lambda e: (e.blob, ref_file(e.blob, a.corpus / f"{e.blob}.pdf", a.out)), entries))
    ref = dict(recs)
    C.dump(a.out / "reference.json", {"tools": tool_versions(), "files": ref})
    if a.probes:
        C.dump(a.probes / "probe-plan.json", build_probes(ref, a.corpus, a.out, a.probes))
    pages = [p for r in ref.values() for p in r["pages"].values()]
    cons = sum(1 for p in pages if p.get("consensus"))
    print(f"reference: {len(ref)} files, {len(pages)} pages, {cons} consensus pages")


if __name__ == "__main__":
    main()
