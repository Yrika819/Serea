#include <jni.h>
#include <llama.h>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdlib>
#include <limits>
#include <mutex>
#include <new>
#include <optional>
#include <string>
#include <unordered_set>
#include <vector>

namespace {
std::mutex engine_mutex;
std::mutex cancellation_mutex;
std::mutex abort_test_gate_mutex;
std::condition_variable abort_test_gate_changed;
bool abort_test_gate_armed = false;
bool abort_test_gate_entered = false;
bool abort_test_gate_released = false;
jlong abort_test_gate_engine_id = 0;
std::atomic<int> abort_callback_entries{0};
std::atomic<int> abort_callback_cancellations{0};
std::atomic<jlong> active_engine_id{0};
struct RequestIdentity {
    jlong engine_id;
    jlong generation_id;
    bool operator==(const RequestIdentity & other) const {
        return engine_id == other.engine_id && generation_id == other.generation_id;
    }
};
struct RequestIdentityHash {
    size_t operator()(const RequestIdentity & request) const {
        const auto owner_hash = std::hash<jlong>{}(request.engine_id);
        const auto request_hash = std::hash<jlong>{}(request.generation_id);
        return owner_hash ^ (request_hash + 0x9e3779b9 + (owner_hash << 6) + (owner_hash >> 2));
    }
};
std::unordered_set<RequestIdentity, RequestIdentityHash> cancelled_requests;
std::once_flag backend_once;
llama_model * model = nullptr;
llama_context * context = nullptr;
llama_sampler * sampler = nullptr;
const llama_vocab * vocab = nullptr;

void throw_java(JNIEnv * env, const char * class_name, const char * message);

bool generation_cancelled(jlong engine_id, jlong generation_id) {
    std::lock_guard<std::mutex> lock(cancellation_mutex);
    return cancelled_requests.find({engine_id, generation_id}) != cancelled_requests.end();
}

bool abort_requested(void * user_data) {
    const auto request = *static_cast<const RequestIdentity *>(user_data);
    abort_callback_entries.fetch_add(1, std::memory_order_relaxed);
    {
        std::unique_lock<std::mutex> lock(abort_test_gate_mutex);
        if (abort_test_gate_armed && request.engine_id == abort_test_gate_engine_id) {
            abort_test_gate_entered = true;
            abort_test_gate_changed.notify_all();
            abort_test_gate_changed.wait(lock, [] { return abort_test_gate_released; });
            abort_test_gate_armed = false;
        }
    }
    const bool cancelled = generation_cancelled(request.engine_id, request.generation_id);
    if (cancelled) abort_callback_cancellations.fetch_add(1, std::memory_order_relaxed);
    return cancelled;
}

void throw_cancelled(JNIEnv * env) {
    throw_java(env, "java/util/concurrent/CancellationException", "Inference cancelled");
}

class abort_callback_reset {
public:
    explicit abort_callback_reset(llama_context * context) : context_(context) {}
    ~abort_callback_reset() {
        if (context_ != nullptr) llama_set_abort_callback(context_, nullptr, nullptr);
    }
private:
    llama_context * context_;
};

void forget_cancellation(jlong engine_id, jlong generation_id) {
    std::lock_guard<std::mutex> lock(cancellation_mutex);
    cancelled_requests.erase({engine_id, generation_id});
}

void forget_owner_cancellations(jlong engine_id) {
    std::lock_guard<std::mutex> lock(cancellation_mutex);
    for (auto request = cancelled_requests.begin(); request != cancelled_requests.end();) {
        if (request->engine_id == engine_id) {
            request = cancelled_requests.erase(request);
        } else {
            ++request;
        }
    }
}

void clear_engine_owner(jlong engine_id) {
    std::lock_guard<std::mutex> lock(cancellation_mutex);
    active_engine_id.store(0, std::memory_order_release);
    for (auto request = cancelled_requests.begin(); request != cancelled_requests.end();) {
        if (request->engine_id == engine_id) {
            request = cancelled_requests.erase(request);
        } else {
            ++request;
        }
    }
}

void throw_java(JNIEnv * env, const char * class_name, const char * message) {
    jclass klass = env->FindClass(class_name);
    if (klass != nullptr) env->ThrowNew(klass, message);
}

void append_utf8(std::string & output, uint32_t code_point) {
    if (code_point <= 0x7f) {
        output.push_back(static_cast<char>(code_point));
    } else if (code_point <= 0x7ff) {
        output.push_back(static_cast<char>(0xc0 | (code_point >> 6)));
        output.push_back(static_cast<char>(0x80 | (code_point & 0x3f)));
    } else if (code_point <= 0xffff) {
        output.push_back(static_cast<char>(0xe0 | (code_point >> 12)));
        output.push_back(static_cast<char>(0x80 | ((code_point >> 6) & 0x3f)));
        output.push_back(static_cast<char>(0x80 | (code_point & 0x3f)));
    } else {
        output.push_back(static_cast<char>(0xf0 | (code_point >> 18)));
        output.push_back(static_cast<char>(0x80 | ((code_point >> 12) & 0x3f)));
        output.push_back(static_cast<char>(0x80 | ((code_point >> 6) & 0x3f)));
        output.push_back(static_cast<char>(0x80 | (code_point & 0x3f)));
    }
}

class scoped_java_string_chars {
public:
    scoped_java_string_chars(JNIEnv * env, jstring value)
            : env_(env), value_(value), chars_(env->GetStringChars(value, nullptr)) {}
    ~scoped_java_string_chars() {
        if (chars_ != nullptr) env_->ReleaseStringChars(value_, chars_);
    }
    scoped_java_string_chars(const scoped_java_string_chars &) = delete;
    scoped_java_string_chars & operator=(const scoped_java_string_chars &) = delete;
    const jchar * get() const { return chars_; }

private:
    JNIEnv * env_;
    jstring value_;
    const jchar * chars_;
};

std::optional<std::string> java_string_to_utf8(JNIEnv * env, jstring value) {
    if (value == nullptr) return std::nullopt;
    const jsize length = env->GetStringLength(value);
    const scoped_java_string_chars acquired_chars(env, value);
    const jchar * chars = acquired_chars.get();
    if (chars == nullptr) return std::nullopt;
    std::string output;
    output.reserve(static_cast<size_t>(length));
    for (jsize i = 0; i < length; ++i) {
        uint32_t code_point = chars[i];
        if (code_point >= 0xd800 && code_point <= 0xdbff) {
            if (i + 1 >= length || chars[i + 1] < 0xdc00 || chars[i + 1] > 0xdfff) return std::nullopt;
            const uint32_t low = chars[++i];
            code_point = 0x10000 + ((code_point - 0xd800) << 10) + (low - 0xdc00);
        } else if (code_point >= 0xdc00 && code_point <= 0xdfff) {
            return std::nullopt;
        }
        append_utf8(output, code_point);
    }
    return output;
}

jstring utf8_to_java_string(JNIEnv * env, const std::string & value) {
    std::vector<jchar> utf16;
    utf16.reserve(value.size());
    for (size_t i = 0; i < value.size();) {
        const auto first = static_cast<uint8_t>(value[i++]);
        uint32_t code_point = 0;
        size_t continuation_count = 0;
        if (first <= 0x7f) {
            code_point = first;
        } else if ((first & 0xe0) == 0xc0) {
            code_point = first & 0x1f;
            continuation_count = 1;
        } else if ((first & 0xf0) == 0xe0) {
            code_point = first & 0x0f;
            continuation_count = 2;
        } else if ((first & 0xf8) == 0xf0) {
            code_point = first & 0x07;
            continuation_count = 3;
        } else {
            throw_java(env, "java/lang/IllegalStateException", "Native output is not valid UTF-8");
            return nullptr;
        }
        if (i + continuation_count > value.size()) {
            throw_java(env, "java/lang/IllegalStateException", "Native output contains truncated UTF-8");
            return nullptr;
        }
        for (size_t j = 0; j < continuation_count; ++j) {
            const auto next = static_cast<uint8_t>(value[i++]);
            if ((next & 0xc0) != 0x80) {
                throw_java(env, "java/lang/IllegalStateException", "Native output contains invalid UTF-8");
                return nullptr;
            }
            code_point = (code_point << 6) | (next & 0x3f);
        }
        const bool overlong = (continuation_count == 1 && code_point < 0x80) ||
                (continuation_count == 2 && code_point < 0x800) ||
                (continuation_count == 3 && code_point < 0x10000);
        if (overlong || code_point > 0x10ffff ||
                (code_point >= 0xd800 && code_point <= 0xdfff)) {
            throw_java(env, "java/lang/IllegalStateException", "Native output contains invalid UTF-8 code point");
            return nullptr;
        }
        if (code_point <= 0xffff) {
            utf16.push_back(static_cast<jchar>(code_point));
        } else {
            code_point -= 0x10000;
            utf16.push_back(static_cast<jchar>(0xd800 + (code_point >> 10)));
            utf16.push_back(static_cast<jchar>(0xdc00 + (code_point & 0x3ff)));
        }
    }
    if (utf16.size() > static_cast<size_t>(std::numeric_limits<jsize>::max())) {
        throw_java(env, "java/lang/IllegalStateException", "Native output exceeds the Java string bound");
        return nullptr;
    }
    static constexpr jchar empty = 0;
    return env->NewString(utf16.empty() ? &empty : utf16.data(), static_cast<jsize>(utf16.size()));
}

void free_locked() {
    if (sampler != nullptr) {
        llama_sampler_free(sampler);
        sampler = nullptr;
    }
    if (context != nullptr) {
        llama_free(context);
        context = nullptr;
    }
    if (model != nullptr) {
        llama_model_free(model);
        model = nullptr;
    }
    vocab = nullptr;
}

void free_batch_storage(llama_batch & batch, size_t initialized_seq_ids) {
    if (batch.seq_id != nullptr) {
        for (size_t i = 0; i < initialized_seq_ids; ++i) {
            std::free(batch.seq_id[i]);
        }
    }
    std::free(batch.seq_id);
    std::free(batch.logits);
    std::free(batch.n_seq_id);
    std::free(batch.pos);
    std::free(batch.token);
    batch = {};
}

bool make_batch(const std::vector<llama_token> & tokens, llama_pos start_position, llama_batch & batch) {
    batch = {};
    if (tokens.empty() || tokens.size() > static_cast<size_t>(std::numeric_limits<int32_t>::max())) {
        return false;
    }
    const auto token_count = static_cast<int32_t>(tokens.size());
    batch.token = static_cast<llama_token *>(std::malloc(sizeof(llama_token) * tokens.size()));
    batch.pos = static_cast<llama_pos *>(std::malloc(sizeof(llama_pos) * tokens.size()));
    batch.n_seq_id = static_cast<int32_t *>(std::malloc(sizeof(int32_t) * tokens.size()));
    batch.seq_id = static_cast<llama_seq_id **>(
            std::calloc(tokens.size() + 1, sizeof(llama_seq_id *)));
    batch.logits = static_cast<int8_t *>(std::malloc(sizeof(int8_t) * tokens.size()));
    if (batch.token == nullptr || batch.pos == nullptr || batch.n_seq_id == nullptr ||
            batch.seq_id == nullptr || batch.logits == nullptr) {
        free_batch_storage(batch, 0);
        return false;
    }
    size_t initialized_seq_ids = 0;
    for (int32_t i = 0; i < token_count; ++i) {
        batch.seq_id[i] = static_cast<llama_seq_id *>(std::malloc(sizeof(llama_seq_id)));
        if (batch.seq_id[i] == nullptr) {
            free_batch_storage(batch, initialized_seq_ids);
            return false;
        }
        ++initialized_seq_ids;
    }
    batch.n_tokens = token_count;
    for (int32_t i = 0; i < batch.n_tokens; ++i) {
        batch.token[i] = tokens[static_cast<size_t>(i)];
        batch.pos[i] = start_position + i;
        batch.n_seq_id[i] = 1;
        batch.seq_id[i][0] = 0;
        batch.logits[i] = (i == batch.n_tokens - 1) ? 1 : 0;
    }
    return true;
}
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeLoad(JNIEnv * env, jobject, jlong engine_id, jstring path) {
    const char * model_path = env->GetStringUTFChars(path, nullptr);
    if (model_path == nullptr) return JNI_FALSE;
    std::lock_guard<std::mutex> lock(engine_mutex);
    const jlong owner = active_engine_id.load(std::memory_order_acquire);
    if (owner != 0 && owner != engine_id) {
        env->ReleaseStringUTFChars(path, model_path);
        throw_java(env, "java/lang/IllegalStateException", "Another NativeLlamaEngine owns the process-wide model lease");
        return JNI_FALSE;
    }
    active_engine_id.store(engine_id, std::memory_order_release);
    forget_owner_cancellations(engine_id);
    free_locked();
    std::call_once(backend_once, [] { llama_backend_init(); });
    llama_model_params model_params = llama_model_default_params();
    model = llama_model_load_from_file(model_path, model_params);
    env->ReleaseStringUTFChars(path, model_path);
    if (model == nullptr) {
        clear_engine_owner(engine_id);
        return JNI_FALSE;
    }

    vocab = llama_model_get_vocab(model);
    llama_context_params context_params = llama_context_default_params();
    context_params.n_ctx = 512;
    context_params.n_batch = 128;
    context_params.n_ubatch = 128;
    context_params.n_threads = 2;
    context_params.n_threads_batch = 2;
    context_params.no_perf = true;
    context = llama_init_from_model(model, context_params);
    if (context == nullptr) {
        free_locked();
        clear_engine_owner(engine_id);
        return JNI_FALSE;
    }
    auto params = llama_sampler_chain_default_params();
    params.no_perf = true;
    sampler = llama_sampler_chain_init(params);
    if (sampler == nullptr) {
        free_locked();
        clear_engine_owner(engine_id);
        return JNI_FALSE;
    }
    llama_sampler_chain_add(sampler, llama_sampler_init_greedy());
    return JNI_TRUE;
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeGenerate(
        JNIEnv * env, jobject engine_instance, jlong engine_id, jstring prompt_value, jint max_tokens, jlong generation_id) {
    try {
        const auto prompt_value_utf8 = java_string_to_utf8(env, prompt_value);
        if (!prompt_value_utf8.has_value()) {
            if (!env->ExceptionCheck()) {
                throw_java(env, "java/lang/IllegalArgumentException", "Prompt contains invalid UTF-16");
            }
            return nullptr;
        }
        const std::string & prompt = *prompt_value_utf8;
        std::lock_guard<std::mutex> lock(engine_mutex);
        if (active_engine_id.load(std::memory_order_acquire) != engine_id || model == nullptr || context == nullptr || sampler == nullptr) {
            throw_java(env, "java/lang/IllegalStateException", "This engine does not own the loaded model");
            return nullptr;
        }

        RequestIdentity request{engine_id, generation_id};
        if (generation_cancelled(engine_id, generation_id)) {
            throw_cancelled(env);
            return nullptr;
        }
        llama_set_abort_callback(context, abort_requested, &request);
        abort_callback_reset reset_abort_callback(context);
        llama_memory_clear(llama_get_memory(context), true);
        llama_sampler_reset(sampler);
        if (prompt.size() > static_cast<size_t>(std::numeric_limits<int32_t>::max())) {
            throw_java(env, "java/lang/IllegalArgumentException", "Prompt exceeds the native byte bound");
            return nullptr;
        }
        const int32_t prompt_length = static_cast<int32_t>(prompt.size());
        const int32_t required = -llama_tokenize(vocab, prompt.data(), prompt_length, nullptr, 0, true, true);
        if (required <= 0 || required + max_tokens > 512) {
            throw_java(env, "java/lang/IllegalArgumentException", "Prompt exceeds the PoC context bound");
            return nullptr;
        }
        std::vector<llama_token> tokens(static_cast<size_t>(required));
        const int32_t token_count = llama_tokenize(
                vocab, prompt.data(), prompt_length, tokens.data(), required, true, true);
        if (generation_cancelled(engine_id, generation_id)) {
            throw_cancelled(env);
            return nullptr;
        }
        if (token_count <= 0) {
            throw_java(env, "java/lang/IllegalArgumentException", "Prompt tokenization failed");
            return nullptr;
        }

        jclass engine_class = env->GetObjectClass(engine_instance);
        jmethodID started_callback = env->GetMethodID(engine_class, "onNativeGenerationStart", "()V");
        if (started_callback == nullptr) return nullptr;
        env->CallVoidMethod(engine_instance, started_callback);
        if (env->ExceptionCheck()) return nullptr;
        if (generation_cancelled(engine_id, generation_id)) {
            throw_cancelled(env);
            return nullptr;
        }

        llama_batch prompt_batch{};
        if (!make_batch(tokens, 0, prompt_batch)) {
            throw_java(env, "java/lang/OutOfMemoryError", "Unable to allocate prompt batch buffers");
            return nullptr;
        }
        const int32_t prompt_result = llama_decode(context, prompt_batch);
        free_batch_storage(prompt_batch, tokens.size());
        if (generation_cancelled(engine_id, generation_id) || prompt_result == 2) {
            throw_cancelled(env);
            return nullptr;
        }
        if (prompt_result != 0) {
            throw_java(env, "java/lang/IllegalStateException", "Prompt evaluation failed");
            return nullptr;
        }

        std::string result;
        llama_pos position = token_count;
        for (jint generated = 0; generated < max_tokens; ++generated) {
            if (generation_cancelled(engine_id, generation_id)) {
                throw_cancelled(env);
                return nullptr;
            }
            const llama_token token = llama_sampler_sample(sampler, context, -1);
            if (llama_vocab_is_eog(vocab, token)) break;
            llama_sampler_accept(sampler, token);

            char piece[256];
            const int32_t piece_length = llama_token_to_piece(vocab, token, piece, sizeof(piece), 0, true);
            if (piece_length < 0) {
                throw_java(env, "java/lang/IllegalStateException", "Token-to-text conversion failed");
                return nullptr;
            }
            result.append(piece, static_cast<size_t>(piece_length));
            std::vector<llama_token> next_token{token};
            llama_batch batch{};
            if (!make_batch(next_token, position++, batch)) {
                throw_java(env, "java/lang/OutOfMemoryError", "Unable to allocate generation batch buffers");
                return nullptr;
            }
            const int32_t decode_result = llama_decode(context, batch);
            free_batch_storage(batch, next_token.size());
            if (generation_cancelled(engine_id, generation_id) || decode_result == 2) {
                throw_cancelled(env);
                return nullptr;
            }
            if (decode_result != 0) {
                throw_java(env, "java/lang/IllegalStateException", "Token evaluation failed");
                return nullptr;
            }
        }
        return utf8_to_java_string(env, result);
    } catch (const std::bad_alloc &) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/OutOfMemoryError", "Native inference allocation failed");
        }
        return nullptr;
    }
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeUnicodeRoundTrip(JNIEnv * env, jobject, jstring value) {
    try {
        const auto converted = java_string_to_utf8(env, value);
        if (!converted.has_value()) {
            if (!env->ExceptionCheck()) {
                throw_java(env, "java/lang/IllegalArgumentException", "Input contains invalid UTF-16");
            }
            return nullptr;
        }
        return utf8_to_java_string(env, *converted);
    } catch (const std::bad_alloc &) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/OutOfMemoryError", "Native Unicode conversion allocation failed");
        }
        return nullptr;
    }
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeCancel(JNIEnv *, jobject, jlong engine_id, jlong generation_id) {
    std::lock_guard<std::mutex> lock(cancellation_mutex);
    if (active_engine_id.load(std::memory_order_acquire) != engine_id) return JNI_FALSE;
    cancelled_requests.insert({engine_id, generation_id});
    return JNI_TRUE;
}

extern "C" JNIEXPORT void JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeForgetCancellation(JNIEnv *, jobject, jlong engine_id, jlong generation_id) {
    forget_cancellation(engine_id, generation_id);
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeFree(JNIEnv *, jobject, jlong engine_id) {
    std::lock_guard<std::mutex> lock(engine_mutex);
    if (active_engine_id.load(std::memory_order_acquire) != engine_id) return JNI_FALSE;
    free_locked();
    clear_engine_owner(engine_id);
    return JNI_TRUE;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeHasModel(JNIEnv *, jobject, jlong engine_id) {
    std::lock_guard<std::mutex> lock(engine_mutex);
    return active_engine_id.load(std::memory_order_acquire) == engine_id &&
        model != nullptr && context != nullptr && sampler != nullptr ? JNI_TRUE : JNI_FALSE;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeArmAbortCallbackGateForTest(
        JNIEnv *, jobject, jlong engine_id) {
    if (active_engine_id.load(std::memory_order_acquire) != engine_id) return JNI_FALSE;
    std::lock_guard<std::mutex> lock(abort_test_gate_mutex);
    abort_test_gate_engine_id = engine_id;
    abort_test_gate_entered = false;
    abort_test_gate_released = false;
    abort_test_gate_armed = true;
    abort_callback_entries.store(0, std::memory_order_relaxed);
    abort_callback_cancellations.store(0, std::memory_order_relaxed);
    return JNI_TRUE;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeAwaitAbortCallbackGateForTest(
        JNIEnv *, jobject, jlong timeout_ms) {
    std::unique_lock<std::mutex> lock(abort_test_gate_mutex);
    const bool entered = abort_test_gate_changed.wait_for(
            lock, std::chrono::milliseconds(timeout_ms), [] { return abort_test_gate_entered; });
    return entered ? JNI_TRUE : JNI_FALSE;
}

extern "C" JNIEXPORT void JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeReleaseAbortCallbackGateForTest(JNIEnv *, jobject) {
    {
        std::lock_guard<std::mutex> lock(abort_test_gate_mutex);
        abort_test_gate_released = true;
        abort_test_gate_armed = false;
    }
    abort_test_gate_changed.notify_all();
}

extern "C" JNIEXPORT jint JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeAbortCallbackEntriesForTest(JNIEnv *, jobject) {
    return abort_callback_entries.load(std::memory_order_relaxed);
}

extern "C" JNIEXPORT jint JNICALL
Java_app_serea_g3poc_NativeLlamaEngine_nativeAbortCallbackCancellationsForTest(JNIEnv *, jobject) {
    return abort_callback_cancellations.load(std::memory_order_relaxed);
}
