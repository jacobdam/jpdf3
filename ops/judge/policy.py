#!/usr/bin/env python3
"""Static policy checks on a source tree. PROTECTED.

usage: policy.py --root TREE [--deps] [--summary FILE] [--signed-range BASE HEAD]
Exit 0 = pass, 1 = violations (listed on stdout and in the summary file).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))

SKIP_DIRS = {"target", ".git", "node_modules", "dist", "build", "DerivedData", ".build", "worktrees"}
REQUIRED_CLIPPY = ["unwrap_used", "expect_used", "panic", "indexing_slicing", "unimplemented", "todo",
                   "unreachable", "dbg_macro", "exit", "mem_forget", "cast_possible_truncation",
                   "cast_sign_loss", "cast_possible_wrap"]
DENIED = ["unsafe_code", *REQUIRED_CLIPPY]
SPEC_SECTIONS = ["Persistence", "Undo", "Errors", "Limits", "Security", "Accessibility", "Platforms", "Performance"]
REPO = "jacobdam/jpdf3"

_COMMENT = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)
_STRING = re.compile(r'r(#*)".*?"\1|b?"(?:\\.|[^"\\])*"', re.S)
_CHAR = re.compile(r"'(?:\\.|[^'\\])'")
_UNSAFE = re.compile(r"\bunsafe\b")
_ALLOW = re.compile(r"\b(allow|expect|warn)\s*\(([^)]*)\)")
_IGNORE = re.compile(r"#\[ignore\b(?:\s*=\s*\"([^\"]*)\")?")
_SWIFT_BAD = re.compile(r"\b(Unsafe[A-Z]\w*|withUnsafe\w*|unsafeBitCast|unsafeDowncast|Unmanaged|assumingMemoryBound)\b")
_TS_BAD = re.compile(r"\beval\s*\(|\bnew\s+Function\s*\(|dangerouslySetInnerHTML|\.innerHTML\s*=|\.outerHTML\s*=|document\.write\s*\(")


def strip_rust(src: str) -> str:
    return _CHAR.sub("' '", _STRING.sub('""', _COMMENT.sub(" ", src)))


def walk(root: Path, suffixes: tuple[str, ...]):
    for dp, dns, fns in os.walk(root):
        dns[:] = [d for d in dns if d not in SKIP_DIRS and not (Path(dp) / d).match("fuzz/corpus")
                  and not (Path(dp) / d).match("fuzz/artifacts")]
        for f in fns:
            if f.endswith(suffixes):
                yield Path(dp) / f


class Report:
    def __init__(self):
        self.v: list[str] = []

    def add(self, msg: str):
        self.v.append(msg)


def check_toolchain(root: Path, r: Report):
    p = root / "rust-toolchain.toml"
    if not p.exists():
        return r.add("rust-toolchain.toml missing")
    ch = tomllib.loads(p.read_text()).get("toolchain", {}).get("channel", "")
    if not re.fullmatch(r"\d+\.\d+\.\d+", ch):
        r.add(f"rust-toolchain.toml channel must be an exact stable version, got {ch!r}")


def check_workspace(root: Path, r: Report):
    t = tomllib.loads((root / "Cargo.toml").read_text())
    lints = t.get("workspace", {}).get("lints", {})
    if lints.get("rust", {}).get("unsafe_code") != "forbid":
        r.add("workspace.lints.rust.unsafe_code must be \"forbid\"")
    for l in REQUIRED_CLIPPY:
        v = lints.get("clippy", {}).get(l)
        lvl = v.get("level") if isinstance(v, dict) else v
        if lvl not in ("deny", "forbid"):
            r.add(f"workspace.lints.clippy.{l} must be deny/forbid, got {lvl!r}")
    for prof in ("release", "dev"):
        if t.get("profile", {}).get(prof, {}).get("overflow-checks") is not True:
            r.add(f"profile.{prof}.overflow-checks must be true")
    return lints


def check_packages(root: Path, wlints: dict, r: Report):
    for man in walk(root, ("Cargo.toml",)):
        rel = man.relative_to(root).as_posix()
        t = tomllib.loads(man.read_text())
        if "package" not in t:
            continue
        pdir = man.parent
        in_fuzz = rel.startswith("fuzz/")
        in_bindings = rel.startswith("bindings/")
        lints = t.get("lints")
        if in_fuzz:
            pass  # cargo-fuzz crate: not a workspace member; source scan still applies
        elif in_bindings and lints and lints != {"workspace": True}:
            # Binding generators emit unsafe glue; only unsafe_code may be lowered to deny/allow.
            want = json.loads(json.dumps(wlints))
            got = json.loads(json.dumps(lints))
            uc = got.get("rust", {}).pop("unsafe_code", None)
            want.get("rust", {}).pop("unsafe_code", None)
            if uc not in ("deny", "allow") or got != want:
                r.add(f"{rel}: bindings lints must equal the workspace lints except rust.unsafe_code")
        elif lints != {"workspace": True}:
            r.add(f"{rel}: must contain exactly `[lints] workspace = true`")
        if in_fuzz or in_bindings:
            continue
        roots = [pdir / "src/lib.rs", pdir / "src/main.rs", *sorted((pdir / "src/bin").glob("*.rs"))]
        for cr in roots:
            if cr.exists() and "#![forbid(unsafe_code)]" not in cr.read_text():
                r.add(f"{cr.relative_to(root)}: missing #![forbid(unsafe_code)]")


def issue_open(n: int) -> bool | None:
    tok = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not tok:
        return None
    req = urllib.request.Request(f"https://api.github.com/repos/{REPO}/issues/{n}",
                                 headers={"Authorization": f"Bearer {tok}", "User-Agent": "jpdf-judge"})
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            return json.load(resp).get("state") == "open"
    except Exception:
        return None


def check_sources(root: Path, r: Report):
    for f in walk(root, (".rs",)):
        rel = f.relative_to(root).as_posix()
        src = f.read_text("utf-8", "replace")
        code = strip_rust(src)
        for i, line in enumerate(code.splitlines(), 1):
            if _UNSAFE.search(line):
                r.add(f"{rel}:{i}: `unsafe` is forbidden in first-party code")
            for m in _ALLOW.finditer(line):
                bad = [n for n in re.findall(r"[a-z_]+", m.group(2)) if n in DENIED]
                if bad:
                    r.add(f"{rel}:{i}: relaxing denied lint(s) {bad}")
        for m in _IGNORE.finditer(src):
            link = m.group(1) or ""
            mm = re.fullmatch(rf"https://github\.com/{REPO}/issues/(\d+)", link)
            line = src[: m.start()].count("\n") + 1
            if not mm:
                r.add(f"{rel}:{line}: #[ignore] needs = \"https://github.com/{REPO}/issues/<n>\"")
            elif issue_open(int(mm.group(1))) is False:
                r.add(f"{rel}:{line}: #[ignore] links to closed issue #{mm.group(1)}")
    for f in walk(root, (".swift",)):
        for i, line in enumerate(f.read_text("utf-8", "replace").splitlines(), 1):
            if _SWIFT_BAD.search(line.split("//")[0]):
                r.add(f"{f.relative_to(root)}:{i}: unsafe Swift memory API (native layer must stay safe and thin)")
    for f in walk(root, (".ts", ".tsx", ".js", ".jsx")):
        if "/node_modules/" in f.as_posix():
            continue
        for i, line in enumerate(f.read_text("utf-8", "replace").splitlines(), 1):
            if _TS_BAD.search(line):
                r.add(f"{f.relative_to(root)}:{i}: forbidden web API (eval/innerHTML/dangerouslySetInnerHTML)")


def check_specs(root: Path, r: Report):
    sd = root / "specs"
    if not sd.exists():
        return
    for f in sd.rglob("*.md"):
        rel = f.relative_to(sd).as_posix()
        if "/" not in rel or rel.split("/")[0] in ("journeys", "ui") or f.name.startswith(("README", "_")):
            continue
        heads = set(re.findall(r"^##\s+(\w+)", f.read_text("utf-8", "replace"), re.M))
        missing = [s for s in SPEC_SECTIONS if s not in heads]
        if missing:
            r.add(f"specs/{rel}: missing checklist section(s) {missing} (see specs/_TEMPLATE.md)")


def check_supply_chain(root: Path, r: Report):
    cfg = root / "supply-chain/config.toml"
    aud = root / "supply-chain/audits.toml"
    if not cfg.exists():
        return r.add("supply-chain/config.toml missing (cargo vet)")
    if "[[exemptions." in cfg.read_text():
        r.add("supply-chain/config.toml: exemptions are not allowed (imported audits only)")
    if aud.exists() and re.search(r"^\[\[(audits|trusted)\.", aud.read_text(), re.M):
        r.add("supply-chain/audits.toml: own audits/trust entries are not allowed")


def check_deps(root: Path, r: Report):
    allow: dict[str, str] = {}
    al = Path(__file__).resolve().parent / "unsafe-allowlist.txt"
    for line in al.read_text().splitlines():
        parts = line.split("#")[0].split()
        if parts:
            allow[parts[0]] = (parts[1] if len(parts) > 1 else "github.com/rustcrypto/").lower()
    p = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"], cwd=root,
                       capture_output=True, text=True, check=False)
    if p.returncode != 0:
        return r.add(f"cargo metadata failed: {p.stderr[-500:]}")
    meta = json.loads(p.stdout)
    for pkg in meta["packages"]:
        if pkg.get("source") is None:
            continue  # first-party, scanned above
        pdir = Path(pkg["manifest_path"]).parent
        hits = 0
        for f in walk(pdir, (".rs",)):
            if _UNSAFE.search(strip_rust(f.read_text("utf-8", "replace"))):
                hits += 1
        if hits:
            repo = (pkg.get("repository") or "").lower()
            if pkg["name"] in allow and allow[pkg["name"]] in repo:
                continue
            r.add(f"dependency {pkg['name']} {pkg['version']} contains `unsafe` ({hits} files) and is not allowlisted")


JUDGE_DIR = Path(__file__).resolve().parent


def protected_list() -> list[str]:
    return [l.strip() for l in (JUDGE_DIR / "protected.txt").read_text().splitlines()
            if l.strip() and not l.startswith("#")]


def is_protected(rel: str, plist: list[str]) -> bool:
    return any(rel == p.rstrip("/") or (p.endswith("/") and rel.startswith(p)) for p in plist)


def check_protected_signatures(root: Path, base: str, head: str, r: Report,
                               signers: Path | None = None) -> None:
    """Protected paths change only through commits signed by the owner (CONSTITUTION 12).

    The agent shares the owner's GitHub account, so authorship proves nothing; a signature by a
    key the agent cannot reach does. Every commit in the PR must verify, so nobody can stack an
    unsigned commit on top of a signed one.
    """
    signers = signers or JUDGE_DIR / "allowed_signers"
    git = ["git", "-C", str(root)]
    d = subprocess.run([*git, "diff", "--name-only", f"{base}...{head}"], capture_output=True, text=True, check=False)
    if d.returncode != 0:
        return r.add(f"cannot diff {base[:12]}...{head[:12]}: {d.stderr.strip()[-300:]}")
    plist = protected_list()
    hits = [f for f in d.stdout.splitlines() if is_protected(f, plist)]
    if not hits:
        return
    revs = subprocess.run([*git, "rev-list", f"{base}..{head}"], capture_output=True, text=True, check=False)
    commits = revs.stdout.split()
    if revs.returncode != 0 or not commits:
        return r.add(f"cannot list commits {base[:12]}..{head[:12]}")
    bad = []
    for c in commits:
        v = subprocess.run([*git, "-c", "gpg.format=ssh", "-c", f"gpg.ssh.allowedSignersFile={signers}",
                            "verify-commit", c], capture_output=True, text=True, check=False)
        if v.returncode != 0:
            bad.append(c[:12])
    if bad:
        r.add(f"protected path(s) changed ({', '.join(hits[:5])}) but commit(s) {', '.join(bad[:10])} "
              f"are not signed by the owner's key (ops/judge/allowed_signers). Agents: write "
              f"ops/proposals/<topic>.md instead.")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", type=Path, required=True)
    ap.add_argument("--deps", action="store_true")
    ap.add_argument("--summary", type=Path)
    ap.add_argument("--signed-range", nargs=2, metavar=("BASE", "HEAD"))
    a = ap.parse_args()
    r = Report()
    check_toolchain(a.root, r)
    wl = check_workspace(a.root, r)
    check_packages(a.root, wl, r)
    check_sources(a.root, r)
    check_specs(a.root, r)
    check_supply_chain(a.root, r)
    if a.deps:
        check_deps(a.root, r)
    if a.signed_range:
        check_protected_signatures(a.root, a.signed_range[0], a.signed_range[1], r)
    text = "## Policy: " + ("PASS" if not r.v else f"FAIL ({len(r.v)})") + "\n\n" + "".join(f"- {v}\n" for v in r.v[:200])
    print(text)
    if a.summary:
        a.summary.write_text(text)
    sys.exit(1 if r.v else 0)


if __name__ == "__main__":
    main()
