#!/usr/bin/env python3
"""Static guard that every product JNI export catches C++ exceptions."""

from pathlib import Path
import re


source_path = Path(__file__).parents[2] / "android/app/src/main/cpp/llama_jni.cpp"
source = source_path.read_text(encoding="utf-8")
export_pattern = re.compile(
    r'extern\s+"C"\s+JNIEXPORT\s+[^;]*?\b(Java_app_serea_android_[A-Za-z0-9_]+)\s*\([^;]*?\)\s*\{',
    re.DOTALL,
)
exports = []
for match in export_pattern.finditer(source):
    opening = source.find("{", match.start())
    depth = 0
    closing = None
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                closing = index
                break
    if closing is None:
        raise SystemExit(f"unclosed JNI export: {match.group(1)}")
    body = source[opening + 1 : closing]
    exports.append((match.group(1), body))

if len(exports) < 7:
    raise SystemExit(f"expected at least seven product JNI exports, found {len(exports)}")

uncontained = [
    name
    for name, body in exports
    if "jni_boundary<" not in body and "jni_boundary_void(" not in body
]
if uncontained:
    raise SystemExit(f"JNI exports without exception containment: {', '.join(uncontained)}")

load = next(body for name, body in exports if name.endswith("_nativeLoad"))
if "java_string_to_utf8" not in load or "load_rollback rollback" not in load:
    raise SystemExit("nativeLoad must use scoped Java-string conversion and rollback ownership")
if "GetStringUTFChars" in source or "ReleaseStringUTFChars" in source:
    raise SystemExit("raw JNI UTF char acquisition is forbidden; use scoped conversion")

if not any(name.endswith("NativeJniFailureSeam_failNextLoadAfterModel") for name, _ in exports):
    raise SystemExit("diagnostic JNI failure seam is missing")

print(f"jni_exception_contract=PASS exports={len(exports)} test_failure_seam=present")
