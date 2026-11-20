"""Shared helpers for the jpdf judge. PROTECTED: see CONSTITUTION.md.

Stdlib only, except image helpers, which import numpy/Pillow lazily (judge job only).
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
import time
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

# ---------------------------------------------------------------- constants
RENDER_DPI = 72
PAGES_PER_FILE = 2           # first N pages of every corpus file are graded
TIMEOUT_INFO_S = 20
TIMEOUT_RENDER_S = 30
TIMEOUT_PROBE_S = 60
MEM_LIMIT_BYTES = 2 * 1024**3  # 2 GiB address space per jpdf-cli invocation (Linux)

# Image comparison (calibrated against MuPDF vs Poppler, see README.md).
PIXEL_DELTA = 64             # per-channel difference that counts as "different"
MATCH_MAX_FRACTION = 0.02    # page matches if <= 2% of pixels differ (after 2x box downscale)
DIM_TOLERANCE_PX = 2

TEXT_DROP_TOLERANCE = 0.02   # per-page text F1 may drop at most this much vs baseline

INFRA_MARKER = "JUDGE-INFRA-FAILURE"


# ---------------------------------------------------------------- manifests
@dataclass(frozen=True)
class Entry:
    blob: str
    size: int
    url: str


def load_manifest(path: Path) -> list[Entry]:
    out: list[Entry] = []
    for line in path.read_text().splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        blob, size, url = line.split("\t")
        out.append(Entry(blob, int(size), url))
    out.sort(key=lambda e: e.blob)
    return out


def git_blob_sha1(data: bytes) -> str:
    h = hashlib.sha1()
    h.update(b"blob %d\0" % len(data))
    h.update(data)
    return h.hexdigest()


def infra_fail(msg: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"{INFRA_MARKER}: {msg}", file=sys.stderr)
    sys.exit(75)


# ---------------------------------------------------------------- running untrusted code
@dataclass
class RunResult:
    status: str        # ok | error | unsupported | crash | timeout | missing
    code: int | None
    ms: int
    stdout: bytes = b""
    stderr: bytes = b""


def _limit_resources() -> None:  # runs in the child before exec
    try:
        import resource
        resource.setrlimit(resource.RLIMIT_AS, (MEM_LIMIT_BYTES, MEM_LIMIT_BYTES))
    except (ValueError, OSError, ImportError):
        pass  # macOS may refuse RLIMIT_AS; CI (Linux) enforces it
    os.setsid()


def classify(code: int | None, timed_out: bool) -> str:
    if timed_out:
        return "timeout"
    if code == 0:
        return "ok"
    if code == 2:
        return "error"         # clean, typed error: allowed
    if code == 3:
        return "unsupported"   # feature not implemented yet: allowed
    return "crash"             # panic (101), signal (<0), anything else


def run_limited(cmd: list[str], timeout_s: float, cwd: Path | None = None) -> RunResult:
    if not Path(cmd[0]).exists():
        return RunResult("missing", None, 0)
    t0 = time.monotonic()
    try:
        p = subprocess.run(
            cmd, cwd=cwd, stdin=subprocess.DEVNULL, capture_output=True,
            timeout=timeout_s, preexec_fn=_limit_resources, check=False,
        )
    except subprocess.TimeoutExpired as e:
        ms = int((time.monotonic() - t0) * 1000)
        return RunResult("timeout", None, ms, e.stdout or b"", e.stderr or b"")
    ms = int((time.monotonic() - t0) * 1000)
    return RunResult(classify(p.returncode, False), p.returncode, ms, p.stdout[-65536:], p.stderr[-8192:])


REF_MEM_LIMIT_BYTES = 3 * 1024**3  # reference tools: one hostile file must not OOM the runner


def _limit_reference() -> None:  # runs in the child before exec
    try:
        import resource
        resource.setrlimit(resource.RLIMIT_AS, (REF_MEM_LIMIT_BYTES, REF_MEM_LIMIT_BYTES))
    except (ValueError, OSError, ImportError):
        pass
    os.setsid()


def run_tool(cmd: list[str], timeout_s: float = 120) -> subprocess.CompletedProcess:
    """Run a trusted reference tool (mutool, pdftoppm, qpdf, ...).

    A hung or crashing reference tool means "no reference" for that item (returncode 124/125),
    never a judge crash.
    """
    try:
        return subprocess.run(cmd, capture_output=True, timeout=timeout_s, check=False, stdin=subprocess.DEVNULL,
                              preexec_fn=_limit_reference)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, 124, b"", b"reference tool timed out")
    except OSError as e:
        return subprocess.CompletedProcess(cmd, 125, b"", str(e).encode())


# ---------------------------------------------------------------- text
_WORD = re.compile(r"\w+", re.UNICODE)


def words(text: str) -> Counter:
    return Counter(w.lower() for w in _WORD.findall(text))


def text_f1(ours: str, ref: str) -> float:
    a, b = words(ours), words(ref)
    total = sum(a.values()) + sum(b.values())
    if total == 0:
        return 1.0
    inter = sum((a & b).values())
    return 2.0 * inter / total


# ---------------------------------------------------------------- images
def load_rgb(path: Path):
    import numpy as np
    from PIL import Image
    with Image.open(path) as im:
        im.load()
        if im.mode in ("RGBA", "LA") or (im.mode == "P" and "transparency" in im.info):
            im = im.convert("RGBA")
            bg = Image.new("RGBA", im.size, (255, 255, 255, 255))
            im = Image.alpha_composite(bg, im)
        im = im.convert("RGB")
        return np.asarray(im, dtype=np.int16)


def _down2(a):
    h, w = (a.shape[0] // 2) * 2, (a.shape[1] // 2) * 2
    a = a[:h, :w]
    return (a[0::2, 0::2] + a[1::2, 0::2] + a[0::2, 1::2] + a[1::2, 1::2]) // 4


def diff_fraction(pa: Path, pb: Path) -> float:
    """Fraction of differing pixels (0..1). 1.0 if unreadable or sizes differ."""
    import numpy as np
    try:
        a, b = load_rgb(pa), load_rgb(pb)
    except Exception:
        return 1.0
    if abs(a.shape[0] - b.shape[0]) > DIM_TOLERANCE_PX or abs(a.shape[1] - b.shape[1]) > DIM_TOLERANCE_PX:
        return 1.0
    h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
    if h < 2 or w < 2:
        return 1.0
    a, b = _down2(a[:h, :w]), _down2(b[:h, :w])
    d = np.abs(a - b).max(axis=2)
    return float((d > PIXEL_DELTA).mean())


def region_change(pa: Path, pb: Path, rect: tuple[float, float, float, float]) -> tuple[float, float]:
    """(changed fraction inside rect, changed fraction outside rect). rect in view px, top-left origin."""
    import numpy as np
    a, b = load_rgb(pa), load_rgb(pb)
    h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
    a, b = a[:h, :w], b[:h, :w]
    d = np.abs(a - b).max(axis=2) > 32
    x0, y0, x1, y1 = (int(round(v)) for v in rect)
    x0, x1 = max(0, min(x0, w)), max(0, min(x1, w))
    y0, y1 = max(0, min(y0, h)), max(0, min(y1, h))
    mask = np.zeros_like(d)
    mask[y0:y1, x0:x1] = True
    inside = float(d[mask].mean()) if mask.any() else 0.0
    outside = float(d[~mask].mean()) if (~mask).any() else 0.0
    return inside, outside


# ---------------------------------------------------------------- pdf raw scanning (redaction leak check)
_STREAM = re.compile(rb"stream\r?\n(.*?)\r?\nendstream", re.S)


def raw_and_inflated(data: bytes) -> bytes:
    """All bytes of the file plus every stream we can inflate (covers old incremental revisions)."""
    import zlib
    parts = [data]
    for m in _STREAM.finditer(data):
        body = m.group(1)
        for wbits in (15, -15):
            try:
                parts.append(zlib.decompressobj(wbits).decompress(body, 50_000_000))
                break
            except zlib.error:
                continue
    return b"\n".join(parts)


# ---------------------------------------------------------------- json io
def dump(path: Path, obj) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(obj, indent=1, sort_keys=True))
    tmp.replace(path)


def load(path: Path, default=None):
    if not path.exists():
        return default
    return json.loads(path.read_text())
