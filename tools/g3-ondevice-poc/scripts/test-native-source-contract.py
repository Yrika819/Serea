#!/usr/bin/env python3
"""Host source-contract checks for native batch allocation and JNI Unicode handling."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
native = (ROOT / "app/src/main/cpp/llama_jni.cpp").read_text(encoding="utf-8")
engine = (ROOT / "app/src/main/java/app/serea/g3poc/GgufModelChatPort.kt").read_text(encoding="utf-8")
instrumentation = (
    ROOT / "app/src/androidTest/java/app/serea/g3poc/OnDeviceInferenceTest.kt"
).read_text(encoding="utf-8")

batch_start = native.find("bool make_batch(")
batch_end = native.find("\n}\n", batch_start)
if batch_start < 0 or batch_end < 0:
    raise SystemExit("native batch constructor is missing")
batch = native[batch_start:batch_end]
for marker in (
    "std::malloc(sizeof(llama_token) * tokens.size())",
    "std::malloc(sizeof(llama_pos) * tokens.size())",
    "std::malloc(sizeof(int32_t) * tokens.size())",
    "std::calloc(tokens.size() + 1, sizeof(llama_seq_id *))",
    "std::malloc(sizeof(llama_seq_id))",
    "batch.token == nullptr",
    "batch.pos == nullptr",
    "batch.n_seq_id == nullptr",
    "batch.seq_id == nullptr",
    "batch.logits == nullptr",
    "size_t initialized_seq_ids = 0",
    "free_batch_storage(batch, initialized_seq_ids)",
    "++initialized_seq_ids",
):
    if marker not in batch:
        raise SystemExit(f"native batch allocation must validate and free all partial allocations: {marker}")
if "llama_batch_init(" in batch:
    raise SystemExit("native batch construction must not call the pinned upstream allocator that dereferences unchecked allocations")

batch_cleanup_start = native.find("void free_batch_storage(")
batch_cleanup_end = native.find("\n}\n", batch_cleanup_start)
if batch_cleanup_start < 0 or batch_cleanup_end < 0:
    raise SystemExit("native batch cleanup helper is missing")
batch_cleanup = native[batch_cleanup_start:batch_cleanup_end]
for marker in (
    "i < initialized_seq_ids",
    "std::free(batch.seq_id[i])",
    "std::free(batch.seq_id)",
    "std::free(batch.logits)",
    "std::free(batch.n_seq_id)",
    "std::free(batch.pos)",
    "std::free(batch.token)",
):
    if marker not in batch_cleanup:
        raise SystemExit(f"native partial batch cleanup must release every allocated buffer: {marker}")

chars_raii_start = native.find("class scoped_java_string_chars {")
chars_raii_end = native.find("\n};", chars_raii_start)
if chars_raii_start < 0 or chars_raii_end < 0:
    raise SystemExit("JNI UTF-16 input must use an RAII string-char owner")
chars_raii = native[chars_raii_start:chars_raii_end]
if "env_->ReleaseStringChars(value_, chars_)" not in chars_raii or \
        "const scoped_java_string_chars acquired_chars(env, value)" not in native:
    raise SystemExit("JNI UTF-16 input must release acquired Java chars with RAII")

for jni_name in (
    "Java_app_serea_g3poc_NativeLlamaEngine_nativeGenerate(",
    "Java_app_serea_g3poc_NativeLlamaEngine_nativeUnicodeRoundTrip(",
):
    function_start = native.find(jni_name)
    function_end = native.find("\nextern \"C\" JNIEXPORT", function_start + 1)
    if function_start < 0 or function_end < 0:
        raise SystemExit(f"JNI allocation boundary is missing: {jni_name}")
    function = native[function_start:function_end]
    if "catch (const std::bad_alloc &)" not in function or \
            '"java/lang/OutOfMemoryError"' not in function:
        raise SystemExit(f"JNI allocations must translate bad_alloc into Java OutOfMemoryError: {jni_name}")

for marker in (
    "java_string_to_utf8(JNIEnv * env, jstring value)",
    "code_point >= 0xd800 && code_point <= 0xdbff",
    "code_point >= 0xdc00 && code_point <= 0xdfff",
    "utf8_to_java_string(JNIEnv * env, const std::string & value)",
    "Native output contains truncated UTF-8",
    "Native output contains invalid UTF-8 code point",
    "env->NewString(",
    "Java_app_serea_g3poc_NativeLlamaEngine_nativeUnicodeRoundTrip",
):
    if marker not in native:
        raise SystemExit(f"JNI string boundary must validate Unicode and use UTF-16 NewString: {marker}")
if "GetStringUTFChars(prompt_value" in native or "NewStringUTF(result" in native:
    raise SystemExit("prompt/output JNI must not use modified UTF-8 APIs for llama.cpp UTF-8 text")
if "nativeUnicodeRoundTrip(value: String): String" not in engine:
    raise SystemExit("Kotlin engine must expose the isolated JNI Unicode contract probe")
if 'val unicodeFixture = "Serea 日本語 🌱 café"' not in instrumentation:
    raise SystemExit("instrumentation must include a BMP + supplementary Unicode JNI fixture")

print("native_source_contract=PASS batch_allocation_fail_closed=jni_utf8_strict=unicode_fixture_present")
