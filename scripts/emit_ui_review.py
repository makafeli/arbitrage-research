#!/usr/bin/env python3
"""Emit eighteen explicitly named synthetic Playwright PNGs for visual review."""
import base64
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parent.parent
WIDTHS = (320, 390, 1440)
VIEWS = ("research-dashboard", "frozen-export", "collection-health", "cost-assessment", "chain-freshness", "adapter-support")
MAX_FILE = 1024 * 1024
CHUNK_BYTES = 2250


def main():
    selected = []
    for view, width in ((view, width) for view in VIEWS for width in WIDTHS):
        candidates = sorted(ROOT.glob(
            f"apps/web/test-results/**/{view}-{width}.png"
        ))
        if len(candidates) != 1:
            raise RuntimeError(
                f"expected exactly one synthetic {view} review PNG at width {width}; "
                f"found {len(candidates)}"
            )
        path = candidates[0]
        if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(ROOT):
            raise RuntimeError("review screenshot must be a regular repository file")
        if not 33 <= path.stat().st_size <= MAX_FILE:
            raise RuntimeError("review PNG must be at most 1 MiB")
        content = path.read_bytes()
        if content[:8] != b"\x89PNG\r\n\x1a\n" or content[12:16] != b"IHDR":
            raise RuntimeError("review screenshot must be PNG")
        actual_width, height = struct.unpack(">II", content[16:24])
        if actual_width != width or not 1 <= height <= 16384:
            raise RuntimeError("PNG dimensions do not match the declared CSS viewport")
        selected.append((path.relative_to(ROOT).as_posix(), content, width, height))
    for name, content, width, height in selected:
        chunks = [
            base64.b64encode(content[start:start + CHUNK_BYTES]).decode("ascii")
            for start in range(0, len(content), CHUNK_BYTES)
        ]
        print("ARB_REVIEW_BEGIN " + json.dumps({
            "path": name, "bytes": len(content), "sha256": hashlib.sha256(content).hexdigest(),
            "encoding": "base64", "mime_type": "image/png",
            "width": width, "height": height, "fixture_origin": "synthetic",
            "chunks": len(chunks),
        }, separators=(",", ":")), flush=True)
        for index, chunk in enumerate(chunks):
            print("ARB_REVIEW_CHUNK " + json.dumps(
                {"path": name, "index": index, "data": chunk}, separators=(",", ":")
            ), flush=True)
        print("ARB_REVIEW_END " + json.dumps(
            {"path": name, "chunks": len(chunks)}, separators=(",", ":")
        ), flush=True)


if __name__ == "__main__":
    main()
