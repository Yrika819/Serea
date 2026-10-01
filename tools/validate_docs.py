#!/usr/bin/env python3
"""Validate Serea docs: identifier grammar, JSON validity, and cross-reference anchors.

Usage: python3 tools/validate_docs.py [docs_root]

Exits non-zero on any error. This is the mechanical verification gate for P0
and later documentation changes; it does not replace human review.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

DOCS = Path(sys.argv[1] if len(sys.argv) > 1 else "docs")

ULID_RE = re.compile(r"\b(req|tsk|stp|apr|grt|evt|dev|sch|prop|rcp|ses)_([A-Za-z0-9_-]+)(?![A-Za-z0-9_-])")
CROCKFORD_OK = set("0123456789ABCDEFGHJKMNPQRSTVWXYZ")
LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")

errors: list[str] = []


def slug(heading: str) -> str:
    s = heading.strip().lower().replace("`", "")
    s = re.sub(r"[^\w\s-]", "", s)
    s = re.sub(r"\s+", "-", s)
    return s.strip("-")


files = sorted(DOCS.rglob("*.md"))
anchors: dict[Path, set[str]] = {}

for f in files:
    text = f.read_text(encoding="utf-8")
    slugs: set[str] = set()
    in_fence = False
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        m = re.match(r"^#{1,6}\s+(.*)$", line)
        if m:
            slugs.add(slug(m.group(1)))
    anchors[f.resolve()] = slugs

for f in files:
    text = f.read_text(encoding="utf-8")
    rel = f.relative_to(DOCS).as_posix()

    for lineno, line in enumerate(text.splitlines(), 1):
        for m in ULID_RE.finditer(line):
            body = m.group(2)
            bad = set(body.upper()) - CROCKFORD_OK
            if len(body) != 26:
                errors.append(f"{rel}:{lineno} ULID body len {len(body)} != 26: {m.group(0)}")
            if bad or body != body.upper():
                errors.append(f"{rel}:{lineno} invalid Crockford ULID body in {m.group(0)}")
            elif body and body[0] > "7":
                errors.append(f"{rel}:{lineno} ULID body exceeds 128-bit range: {m.group(0)}")
        for m in re.finditer(r"\bidk_([A-Za-z0-9_-]+…?)", line):
            body = m.group(1)
            abbreviated = body.endswith("…") and re.fullmatch(r"[0-9a-f]{1,63}…", body)
            if not abbreviated and (len(body) != 64 or not re.fullmatch(r"[0-9a-f]{64}", body)):
                errors.append(f"{rel}:{lineno} idk_ body must be exactly 64 lowercase hex characters (or an explicitly abbreviated example): {m.group(0)}")

    id_fields = {
        "task_id": "tsk", "task_binding": "tsk", "step_id": "stp",
        "approval_id": "apr", "grant_id": "grt", "request_id": "req",
        "message_id": "evt", "event_id": "evt", "device_id": "dev",
        "schedule_id": "sch", "proposal_id": "prop", "receipt_id": "rcp",
        "evidence_id": "evt", "session_id": "ses",
    }

    def validate_json_values(value: object, lineno: int) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                nullable_id = key in {"task_id", "step_id"} and child is None
                if key in id_fields and not nullable_id:
                    if not isinstance(child, str):
                        errors.append(f"{rel}:{lineno} {key} must be a string identifier or null when optional")
                    else:
                        prefix, body = id_fields[key], child.removeprefix(f"{id_fields[key]}_")
                        abbreviated = child == "…" or body == "…" or (body.endswith("…") and re.fullmatch(r"[0-9A-HJKMNP-TV-Z]{1,25}…", body))
                        valid_body = (len(body) == 26 and not (set(body) - CROCKFORD_OK) and body == body.upper() and body[0] <= "7")
                        if child != "…" and (not child.startswith(f"{prefix}_") or (not abbreviated and not valid_body)):
                            errors.append(f"{rel}:{lineno} {key} must be {prefix}_ + a 26-character Crockford ULID: {child}")
                if key == "idempotency_key":
                    if not isinstance(child, str):
                        errors.append(f"{rel}:{lineno} idempotency_key must be a string")
                    else:
                        body = child.removeprefix("idk_")
                        abbreviated = body.endswith("…") and re.fullmatch(r"[0-9a-f]{1,63}…", body)
                        if not child.startswith("idk_") or (not abbreviated and not re.fullmatch(r"[0-9a-f]{64}", body)):
                            errors.append(f"{rel}:{lineno} idempotency_key must be idk_ + exactly 64 lowercase hex characters (or an explicitly abbreviated example): {child}")
                validate_json_values(child, lineno)
        elif isinstance(value, list):
            for child in value:
                validate_json_values(child, lineno)

    for block in re.findall(r"```json\n(.*?)```", text, re.DOTALL):
        try:
            parsed = json.loads(block)
            block_start = text.find(block)
            lineno = text[:block_start].count("\n") + 1
            validate_json_values(parsed, lineno)
        except json.JSONDecodeError as e:
            errors.append(f"{rel} invalid JSON block: {str(e)[:100]}")

    for kw in ("TBD", "TODO", "FIXME", "XXX"):
        for m in re.finditer(rf"\b{kw}\b", text):
            line_no = text[: m.start()].count("\n") + 1
            errors.append(f"{rel}:{line_no} placeholder token {kw}")

    for m in LINK_RE.finditer(text):
        target = m.group(2)
        if target.startswith(("http://", "https://", "mailto:")):
            continue
        path, _, frag = target.partition("#")
        if not path:
            continue
        dest = (f.parent / path).resolve()
        if not dest.exists():
            errors.append(f"{rel} broken link target: {target}")
            continue
        if frag and dest.suffix == ".md":
            known = anchors.get(dest)
            if known is not None and frag not in known:
                line_no = text[: m.start()].count("\n") + 1
                errors.append(f"{rel}:{line_no} broken anchor: {target}")

print(f"scanned {len(files)} markdown files under {DOCS}")
if errors:
    for e in errors:
        print(f"ERROR: {e}")
    print(f"\n{len(errors)} error(s)")
    sys.exit(1)
print("OK: identifiers, JSON, placeholders, and cross-references all valid")