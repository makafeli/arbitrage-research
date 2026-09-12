#!/usr/bin/env python3
"""Emit allowlisted generated review files for manual GitHub blob reconstruction.

No uploads, commits, credentials, arbitrary paths, or network requests. Base64
protects source newlines from log formatting; SHA-256 checks complete recovery.
"""
import base64
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
FIXED = [
    "Cargo.lock",
    "rust-format.patch",
    "crates/arb-evm/tests/reference/package-lock.json",
    "crates/arb-evm/tests/reference/golden.json",
]
MAX_FILE = 2 * 1024 * 1024
MAX_TOTAL = 16 * 1024 * 1024
CHUNK_BYTES = 2250


def main():
    changed = subprocess.run(
        ["git", "diff", "--name-only", "--", "*.rs"],
        cwd=ROOT, text=True, check=True, capture_output=True,
    ).stdout.splitlines()
    rust_sources = [
        name for name in changed
        if name.endswith(".rs") and name.startswith(("apps/", "crates/"))
        and ".." not in Path(name).parts
    ]
    total = 0
    for name in sorted(set(FIXED + rust_sources)):
        path = ROOT / name
        if not path.exists():
            print("ARB_REVIEW_MISSING " + json.dumps({"path": name}), flush=True)
            continue
        if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(ROOT):
            raise RuntimeError("review artifact must be a regular repository file")
        content = path.read_bytes()
        total += len(content)
        if len(content) > MAX_FILE or total > MAX_TOTAL:
            raise RuntimeError("review artifact exceeds explicit size bound")
        chunks = [
            base64.b64encode(content[start:start + CHUNK_BYTES]).decode("ascii")
            for start in range(0, len(content), CHUNK_BYTES)
        ]
        metadata = {
            "path": name, "bytes": len(content),
            "sha256": hashlib.sha256(content).hexdigest(),
            "encoding": "base64", "chunks": len(chunks),
        }
        print("ARB_REVIEW_BEGIN " + json.dumps(metadata, separators=(",", ":")), flush=True)
        for index, chunk in enumerate(chunks):
            print("ARB_REVIEW_CHUNK " + json.dumps(
                {"path": name, "index": index, "data": chunk}, separators=(",", ":")
            ), flush=True)
        print("ARB_REVIEW_END " + json.dumps(
            {"path": name, "chunks": len(chunks)}, separators=(",", ":")
        ), flush=True)


if __name__ == "__main__":
    main()
