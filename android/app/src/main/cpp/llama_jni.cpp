// Derived from the reviewed G3 PoC JNI implementation at a2bafeea; product wrapper is opt-in.
#include <jni.h>
#include <llama.h>

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstdlib>
#include <exception>
#include <limits>
#include <mutex>
#include <new>
#include <optional>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

namespace {
std::mutex engine_mutex;
std::mutex cancellation_mutex;
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
std::unordered_map<RequestIdentity, std::pair<jint, jint>, RequestIdentityHash> request_usage_counts;
std::once_flag backend_once;
#if defined(SEREA_NATIVE_JNI_TEST_SEAMS)
std::atomic<bool> fail_next_load_after_model{false};
#endif
llama_model * model = nullptr;
llama_context * context = nullptr;
llama_sampler * sampler = nullptr;
const llama_vocab * vocab = nullptr;
uint32_t model_context_tokens = 0;

void throw_java(JNIEnv * env, const char * class_name, const char * message);

bool generation_cancelled(jlong engine_id, jlong generation_id) {
    std::lock_guard<std::mutex> lock(cancellation_mutex);
    return cancelled_requests.find({engine_id, generation_id}) != cancelled_requests.end();
}

bool abort_requested(void * user_data) {
    const auto request = *static_cast<const RequestIdentity *>(user_data);
    return generation_cancelled(request.engine_id, request.generation_id);
}

void throw_cancelled(JNIEnv * env) {
    throw_java(env, "java/util/concurrent/CancellationException", "Inference cancelled");
}

class abort_callback_reset {
public:
    explicit abort_callback_reset(llama_context * context) : context_(context) {}
    ~abort_callback_reset() noexcept {
        try {
            if (context_ != nullptr) llama_set_abort_callback(context_, nullptr, nullptr);
        } catch (...) {}
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
    auto expected = engine_id;
    active_engine_id.compare_exchange_strong(expected, 0, std::memory_order_acq_rel);
    std::lock_guard<std::mutex> lock(cancellation_mutex);
    for (auto request = cancelled_requests.begin(); request != cancelled_requests.end();) {
        if (request->engine_id == engine_id) {
            request = cancelled_requests.erase(request);
        } else {
            ++request;
        }
    }
}

void clear_engine_owner_noexcept(jlong engine_id) noexcept {
    auto expected = engine_id;
    active_engine_id.compare_exchange_strong(expected, 0, std::memory_order_acq_rel);
    try {
        std::lock_guard<std::mutex> lock(cancellation_mutex);
        for (auto request = cancelled_requests.begin(); request != cancelled_requests.end();) {
            if (request->engine_id == engine_id) {
                request = cancelled_requests.erase(request);
            } else {
                ++request;
            }
        }
    } catch (...) {
        // The atomic lease is already released; stale cancellation entries cannot own a model.
    }
}

void throw_java(JNIEnv * env, const char * class_name, const char * message) {
    jclass klass = env->FindClass(class_name);
    if (klass != nullptr && !env->ExceptionCheck()) env->ThrowNew(klass, message);
}

template <typename Result, typename Body>
Result jni_boundary(JNIEnv * env, Result fallback, Body && body) noexcept {
    try {
        return body();
    } catch (const std::bad_alloc &) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/OutOfMemoryError", "Native runtime allocation failed");
        }
    } catch (const std::exception &) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/IllegalStateException", "Native runtime operation failed");
        }
    } catch (...) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/IllegalStateException", "Native runtime operation failed");
        }
    }
    return fallback;
}

template <typename Body>
void jni_boundary_void(JNIEnv * env, Body && body) noexcept {
    try {
        body();
    } catch (const std::bad_alloc &) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/OutOfMemoryError", "Native runtime allocation failed");
        }
    } catch (const std::exception &) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/IllegalStateException", "Native runtime operation failed");
        }
    } catch (...) {
        if (!env->ExceptionCheck()) {
            throw_java(env, "java/lang/IllegalStateException", "Native runtime operation failed");
        }
    }
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

void free_locked() noexcept {
    llama_sampler * sampler_to_free = std::exchange(sampler, nullptr);
    llama_context * context_to_free = std::exchange(context, nullptr);
    llama_model * model_to_free = std::exchange(model, nullptr);
    vocab = nullptr;
    model_context_tokens = 0;
    request_usage_counts.clear();
    try {
        if (sampler_to_free != nullptr) llama_sampler_free(sampler_to_free);
    } catch (...) {}
    try {
        if (context_to_free != nullptr) llama_free(context_to_free);
    } catch (...) {}
    try {
        if (model_to_free != nullptr) llama_model_free(model_to_free);
    } catch (...) {}
}

class load_rollback {
public:
    explicit load_rollback(jlong engine_id) noexcept : engine_id_(engine_id) {}
    ~load_rollback() noexcept {
        if (!committed_) {
            free_locked();
            clear_engine_owner_noexcept(engine_id_);
        }
    }
    void commit() noexcept { committed_ = true; }

private:
    jlong engine_id_;
    bool committed_ = false;
};

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

class scoped_batch_storage {
public:
    scoped_batch_storage(llama_batch & batch, size_t token_count) noexcept
            : batch_(batch), token_count_(token_count) {}
    ~scoped_batch_storage() noexcept { free_batch_storage(batch_, token_count_); }
    scoped_batch_storage(const scoped_batch_storage &) = delete;
    scoped_batch_storage & operator=(const scoped_batch_storage &) = delete;

private:
    llama_batch & batch_;
    size_t token_count_;
};

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
Java_app_serea_android_LlamaCppLocalModelInferenceBackend_nativeLoad(JNIEnv * env, jobject, jlong engine_id, jstring path, jint context_tokens, jint worker_threads) {
    return jni_boundary<jboolean>(env, JNI_FALSE, [&]() -> jboolean {
        if (context_tokens < 128 || context_tokens > 512 || worker_threads < 1 || worker_threads > 2) {
            throw_java(env, "java/lang/IllegalArgumentException", "Unsupported local runtime limits");
            return JNI_FALSE;
        }
        const auto model_path = java_string_to_utf8(env, path);
        if (!model_path.has_value()) {
            if (!env->ExceptionCheck()) {
                throw_java(env, "java/lang/IllegalArgumentException", "Model path is missing or invalid UTF-16");
            }
            return JNI_FALSE;
        }

        std::lock_guard<std::mutex> lock(engine_mutex);
        const jlong owner = active_engine_id.load(std::memory_order_acquire);
        if (owner != 0 && owner != engine_id) {
            throw_java(env, "java/lang/IllegalStateException", "Another local llama.cpp backend owns the process-wide model lease");
            return JNI_FALSE;
        }
        active_engine_id.store(engine_id, std::memory_order_release);
        load_rollback rollback(engine_id);
        forget_owner_cancellations(engine_id);
        free_locked();
        std::call_once(backend_once, [] { llama_backend_init(); });
        llama_model_params model_params = llama_model_default_params();
        model = llama_model_load_from_file(model_path->c_str(), model_params);
        if (model == nullptr) return JNI_FALSE;
#if defined(SEREA_NATIVE_JNI_TEST_SEAMS)
        if (fail_next_load_after_model.exchange(false, std::memory_order_acq_rel)) {
            throw std::runtime_error("Injected JNI load failure after model allocation");
        }
#endif

        vocab = llama_model_get_vocab(model);
        llama_context_params context_params = llama_context_default_params();
        context_params.n_ctx = static_cast<uint32_t>(context_tokens);
        context_params.n_batch = std::min<uint32_t>(context_params.n_ctx, 128U);
        context_params.n_ubatch = context_params.n_batch;
        context_params.n_threads = worker_threads;
        context_params.n_threads_batch = worker_threads;
        context_params.no_perf = true;
        context = llama_init_from_model(model, context_params);
        if (context == nullptr) return JNI_FALSE;
        model_context_tokens = context_params.n_ctx;
        auto params = llama_sampler_chain_default_params();
        params.no_perf = true;
        sampler = llama_sampler_chain_init(params);
        if (sampler == nullptr) return JNI_FALSE;
        llama_sampler * greedy = llama_sampler_init_greedy();
        if (greedy == nullptr) return JNI_FALSE;
        llama_sampler_chain_add(sampler, greedy);
        rollback.commit();
        return JNI_TRUE;
    });
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_serea_android_LlamaCppLocalModelInferenceBackend_nativeGenerate(
        JNIEnv * env, jobject, jlong engine_id, jstring prompt_value, jint max_tokens, jlong generation_id) {
    return jni_boundary<jstring>(env, nullptr, [&]() -> jstring {
        if (max_tokens < 1 || max_tokens > 256) {
            throw_java(env, "java/lang/IllegalArgumentException", "Output token limit is outside the native bound");
            return nullptr;
        }
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
        if (required <= 0 || model_context_tokens == 0 ||
                static_cast<uint64_t>(required) + static_cast<uint64_t>(max_tokens) > model_context_tokens) {
            throw_java(env, "java/lang/IllegalArgumentException", "Prompt exceeds the native context bound");
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

        if (generation_cancelled(engine_id, generation_id)) {
            throw_cancelled(env);
            return nullptr;
        }

        llama_batch prompt_batch{};
        if (!make_batch(tokens, 0, prompt_batch)) {
            throw_java(env, "java/lang/OutOfMemoryError", "Unable to allocate prompt batch buffers");
            return nullptr;
        }
        scoped_batch_storage prompt_batch_cleanup(prompt_batch, tokens.size());
        const int32_t prompt_result = llama_decode(context, prompt_batch);
        if (generation_cancelled(engine_id, generation_id) || prompt_result == 2) {
            throw_cancelled(env);
            return nullptr;
        }
        if (prompt_result != 0) {
            throw_java(env, "java/lang/IllegalStateException", "Prompt evaluation failed");
            return nullptr;
        }

        std::string result;
        jint output_token_count = 0;
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
            ++output_token_count;
            std::vector<llama_token> next_token{token};
            llama_batch batch{};
            if (!make_batch(next_token, position++, batch)) {
                throw_java(env, "java/lang/OutOfMemoryError", "Unable to allocate generation batch buffers");
                return nullptr;
            }
            scoped_batch_storage batch_cleanup(batch, next_token.size());
            const int32_t decode_result = llama_decode(context, batch);
            if (generation_cancelled(engine_id, generation_id) || decode_result == 2) {
                throw_cancelled(env);
                return nullptr;
            }
            if (decode_result != 0) {
                throw_java(env, "java/lang/IllegalStateException", "Token evaluation failed");
                return nullptr;
            }
        }
        request_usage_counts.emplace(request, std::make_pair(static_cast<jint>(token_count), output_token_count));
        return utf8_to_java_string(env, result);
    });
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_android_LlamaCppLocalModelInferenceBackend_nativeCancel(JNIEnv * env, jobject, jlong engine_id, jlong generation_id) {
    return jni_boundary<jboolean>(env, JNI_FALSE, [&]() -> jboolean {
        std::lock_guard<std::mutex> lock(cancellation_mutex);
        if (active_engine_id.load(std::memory_order_acquire) != engine_id) return JNI_FALSE;
        cancelled_requests.insert({engine_id, generation_id});
        return JNI_TRUE;
    });
}

extern "C" JNIEXPORT void JNICALL
Java_app_serea_android_LlamaCppLocalModelInferenceBackend_nativeForgetCancellation(JNIEnv * env, jobject, jlong engine_id, jlong generation_id) {
    jni_boundary_void(env, [&] { forget_cancellation(engine_id, generation_id); });
}

extern "C" JNIEXPORT jintArray JNICALL
Java_app_serea_android_LlamaCppLocalModelInferenceBackend_nativeUsageCounts(
        JNIEnv * env, jobject, jlong engine_id, jlong generation_id) {
    return jni_boundary<jintArray>(env, nullptr, [&]() -> jintArray {
        std::lock_guard<std::mutex> lock(engine_mutex);
        if (active_engine_id.load(std::memory_order_acquire) != engine_id) return nullptr;
        const auto found = request_usage_counts.find({engine_id, generation_id});
        if (found == request_usage_counts.end()) return nullptr;
        jint values[2] = {found->second.first, found->second.second};
        jintArray result = env->NewIntArray(2);
        if (result == nullptr) return nullptr;
        env->SetIntArrayRegion(result, 0, 2, values);
        return env->ExceptionCheck() ? nullptr : result;
    });
}

extern "C" JNIEXPORT void JNICALL
Java_app_serea_android_LlamaCppLocalModelInferenceBackend_nativeForgetUsage(JNIEnv * env, jobject, jlong engine_id, jlong generation_id) {
    jni_boundary_void(env, [&] {
        std::lock_guard<std::mutex> lock(engine_mutex);
        if (active_engine_id.load(std::memory_order_acquire) == engine_id) {
            request_usage_counts.erase({engine_id, generation_id});
        }
    });
}

extern "C" JNIEXPORT jboolean JNICALL
Java_app_serea_android_LlamaCppLocalModelInferenceBackend_nativeFree(JNIEnv * env, jobject, jlong engine_id) {
    return jni_boundary<jboolean>(env, JNI_FALSE, [&]() -> jboolean {
        std::lock_guard<std::mutex> lock(engine_mutex);
        if (active_engine_id.load(std::memory_order_acquire) != engine_id) return JNI_FALSE;
        free_locked();
        clear_engine_owner(engine_id);
        return JNI_TRUE;
    });
}

#if defined(SEREA_NATIVE_JNI_TEST_SEAMS)
extern "C" JNIEXPORT void JNICALL
Java_app_serea_android_NativeJniFailureSeam_failNextLoadAfterModel(JNIEnv * env, jobject) {
    jni_boundary_void(env, [&] { fail_next_load_after_model.store(true, std::memory_order_release); });
}
#endif
