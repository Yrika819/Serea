#!/usr/bin/env python3
"""Fetch one pinned, public GGUF test fixture after validating repository metadata."""

import hashlib
import json
import os
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

REPOSITORY = "ggml-org/SmolLM2-135M-GGUF"
REVISION = "1076b85d135017971845b19b8d07df1479155988"
FILENAME = "SmolLM2-135M-Q4_K_M.gguf"
EXPECTED_SIZE = 101_016_128
EXPECTED_SHA256 = "e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5"


def open_with_bounded_retry(request: urllib.request.Request, timeout: int):
    for attempt in range(3):
        try:
            return urllib.request.urlopen(request, timeout=timeout)
        except urllib.error.HTTPError as error:
            if error.code not in (429, 500, 502, 503, 504) or attempt == 2:
                raise
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            if attempt == 2:
                raise
        time.sleep(2**attempt)
    raise RuntimeError("bounded download retry exhausted")


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: fetch_verified_smollm2_test_model.py DESTINATION")
    destination = Path(sys.argv[1]).resolve()
    destination.parent.mkdir(parents=True, exist_ok=True)

    metadata_url = (
        f"https://huggingface.co/api/models/{REPOSITORY}/revision/{REVISION}?blobs=true"
    )
    request = urllib.request.Request(metadata_url, headers={"User-Agent": "Serea-Native-Diagnostic/1"})
    with open_with_bounded_retry(request, timeout=30) as response:
        metadata = json.load(response)
    if metadata.get("sha") != REVISION:
        raise SystemExit("Pinned test model repository revision did not match.")
    if metadata.get("cardData", {}).get("license", "").lower() != "apache-2.0":
        raise SystemExit("Pinned test model license metadata was not Apache-2.0.")
    file_metadata = next(
        (item for item in metadata.get("siblings", []) if item.get("rfilename") == FILENAME),
        None,
    )
    lfs = (file_metadata or {}).get("lfs", {})
    if (
        (file_metadata or {}).get("size") != EXPECTED_SIZE
        or lfs.get("size") != EXPECTED_SIZE
        or lfs.get("sha256") != EXPECTED_SHA256
    ):
        raise SystemExit("Pinned GGUF filename, size, or repository SHA-256 did not match.")

    download_url = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/{FILENAME}?download=true"
    request = urllib.request.Request(download_url, headers={"User-Agent": "Serea-Native-Diagnostic/1"})
    digest = hashlib.sha256()
    size = 0
    temporary_name = ""
    for attempt in range(3):
        fd, temporary_name = tempfile.mkstemp(prefix=".smollm2-", suffix=".gguf", dir=destination.parent)
        digest = hashlib.sha256()
        size = 0
        try:
            with os.fdopen(fd, "wb") as output, open_with_bounded_retry(request, timeout=60) as response:
                while block := response.read(1024 * 1024):
                    output.write(block)
                    digest.update(block)
                    size += len(block)
            break
        except (urllib.error.URLError, TimeoutError, ConnectionError, urllib.error.HTTPError):
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass
            if attempt == 2:
                raise
            time.sleep(2**attempt)
    try:
        if size != EXPECTED_SIZE or digest.hexdigest() != EXPECTED_SHA256:
            raise SystemExit("Downloaded GGUF did not match the pinned size and SHA-256; it was discarded.")
        os.replace(temporary_name, destination)
    finally:
        try:
            os.unlink(temporary_name)
        except FileNotFoundError:
            pass
    print(f"verified model={REPOSITORY}/{FILENAME}@{REVISION} size={size} sha256={digest.hexdigest()} license=Apache-2.0")


if __name__ == "__main__":
    main()
