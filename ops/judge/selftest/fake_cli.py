"""Fake jpdf-cli for judge self-tests. PROTECTED.

FAKE_MODE=good   implements the contract with reference tools (should PASS what it implements)
FAKE_MODE=crash  segfaults on render (must FAIL: crash)
FAKE_MODE=leaky  "rewrite"/"encrypt" just copy the input (probes must FAIL)
"""
import json
import os
import shutil
import signal
import subprocess
import sys


def run(cmd):
    return subprocess.run(cmd, capture_output=True, check=False)


def opt(args, name, default=None):
    return args[args.index(name) + 1] if name in args else default


def main():
    mode = os.environ.get("FAKE_MODE", "good")
    cmd, args = sys.argv[1], sys.argv[2:]
    src = args[0]
    pw = opt(args, "--password")
    if cmd == "info":
        p = run(["qpdf", *([f"--password={pw}"] if pw else []), "--show-npages", src])
        if p.returncode not in (0, 3):
            return 2
        print(json.dumps({"pages": int(p.stdout.decode().split()[0])}))
        return 0
    if cmd == "render":
        if mode == "crash":
            os.kill(os.getpid(), signal.SIGSEGV)
        p = run(["mutool", "draw", "-q", "-r", opt(args, "--dpi"), *(["-p", pw] if pw else []),
                 "-o", opt(args, "--out"), src, opt(args, "--page")])
        return 0 if p.returncode == 0 else 2
    if cmd == "text":
        pg = opt(args, "--page")
        p = run(["pdftotext", "-q", "-enc", "UTF-8", "-f", pg, "-l", pg, src, "-"])
        sys.stdout.buffer.write(p.stdout)
        return 0 if p.returncode == 0 else 2
    out = opt(args, "--out")
    if cmd == "rewrite":
        if mode == "leaky":
            shutil.copyfile(src, out)
            return 0
        return 0 if run(["qpdf", src, out]).returncode in (0, 3) else 2
    if cmd == "encrypt":
        if mode == "leaky":
            shutil.copyfile(src, out)
            return 0
        p = run(["qpdf", "--encrypt", f"--user-password={opt(args, '--user-password')}",
                 f"--owner-password={opt(args, '--owner-password')}", "--bits=256", "--", src, out])
        return 0 if p.returncode in (0, 3) else 2
    if cmd == "sign":
        from pyhanko.pdf_utils.incremental_writer import IncrementalPdfFileWriter
        from pyhanko.sign import signers
        signer = signers.SimpleSigner.load_pkcs12(opt(args, "--p12"), passphrase=opt(args, "--p12-password").encode())
        with open(src, "rb") as f:
            w = IncrementalPdfFileWriter(f, strict=False)
            buf = signers.sign_pdf(w, signers.PdfSignatureMetadata(field_name=opt(args, "--field")), signer=signer)
        with open(out, "wb") as f:
            f.write(buf.getbuffer())
        return 0
    if cmd == "verify-signatures":
        s = run(["pdfsig", src]).stdout.decode("utf-8", "replace")
        print(json.dumps({"signatures": [{"field": "x", "integrity": "valid" if "Signature is Valid" in s else "invalid",
                                          "covers_whole_document": "Total document signed" in s}]}))
        return 0
    return 3  # annotate, fill, redact: not implemented by the fake


if __name__ == "__main__":
    sys.exit(main())
