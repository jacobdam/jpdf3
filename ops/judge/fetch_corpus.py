#!/usr/bin/env python3
"""Download a judge corpus manifest into DIR/<blob>.pdf, verifying git blob hashes. PROTECTED.

usage: fetch_corpus.py MANIFEST DIR [--limit N]
Exit 75 (+ JUDGE-INFRA-FAILURE marker) if more than 1% of files cannot be fetched.
"""
from __future__ import annotations

import argparse
import sys
import time
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from common import git_blob_sha1, infra_fail, load_manifest  # noqa: E402


def fetch(entry, out: Path) -> bool:
    dst = out / f"{entry.blob}.pdf"
    if dst.exists() and git_blob_sha1(dst.read_bytes()) == entry.blob:
        return True
    for attempt in range(4):
        try:
            req = urllib.request.Request(urllib.parse.quote(entry.url, safe=":/"), headers={"User-Agent": "jpdf-judge"})
            with urllib.request.urlopen(req, timeout=60) as r:
                data = r.read(entry.size + 1)
            if git_blob_sha1(data) != entry.blob:
                print(f"hash mismatch: {entry.url}", file=sys.stderr)
                return False
            tmp = dst.with_suffix(".part")
            tmp.write_bytes(data)
            tmp.replace(dst)
            return True
        except Exception as e:  # network: retry with backoff
            print(f"retry {attempt}: {entry.url}: {e}", file=sys.stderr)
            time.sleep(2 ** attempt)
    return False


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("manifest", type=Path)
    ap.add_argument("dir", type=Path)
    ap.add_argument("--limit", type=int, default=0)
    a = ap.parse_args()
    entries = load_manifest(a.manifest)
    if a.limit:
        entries = entries[: a.limit]
    a.dir.mkdir(parents=True, exist_ok=True)
    with ThreadPoolExecutor(16) as ex:
        ok = list(ex.map(lambda e: fetch(e, a.dir), entries))
    failed = ok.count(False)
    print(f"fetched {len(entries) - failed}/{len(entries)}")
    if failed > max(1, len(entries) // 100):
        infra_fail(f"{failed} corpus files could not be fetched")


if __name__ == "__main__":
    main()
