#include <jni.h>
#include <llama.h>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <mutex>
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

void throw_java(JNIEnv * env, const char * class_name, const std::string & message);

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

void throw_java(JNIEnv * env, const char * class_name, const std::string & message) {
    jclass klass = env->FindClass(class_name);
    if (klass != nullptr) env->ThrowNew(klass, message.c_str());
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

llama_batch make_batch(const std::vector<llama_token> & tokens, llama_pos start_position) {
    llama_batch batch = llama_batch_init(static_cast<int32_t>(tokens.size()), 0, 1);
    batch.n_tokens = static_cast<int32_t>(tokens.size());
    for (int32_t i = 0; i < batch.n_tokens; ++i) {
        batch.token[i] = tokens[static_cast<size_t>(i)];
        batch.pos[i] = start_position + i;
        batch.n_seq_id[i] = 1;
        batch.seq_id[i][0] = 0;
        batch.logits[i] = (i == batch.n_tokens - 1) ? 1 : 0;
    }
    return batch;
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
    const char * prompt = env->GetStringUTFChars(prompt_value, nullptr);
    if (prompt == nullptr) return nullptr;
    std::lock_guard<std::mutex> lock(engine_mutex);
    if (active_engine_id.load(std::memory_order_acquire) != engine_id || model == nullptr || context == nullptr || sampler == nullptr) {
        env->ReleaseStringUTFChars(prompt_value, prompt);
        throw_java(env, "java/lang/IllegalStateException", "This engine does not own the loaded model");
        return nullptr;
    }

    RequestIdentity request{engine_id, generation_id};
    if (generation_cancelled(engine_id, generation_id)) {
        env->ReleaseStringUTFChars(prompt_value, prompt);
        throw_cancelled(env);
        return nullptr;
    }
    llama_set_abort_callback(context, abort_requested, &request);
    abort_callback_reset reset_abort_callback(context);
    llama_memory_clear(llama_get_memory(context), true);
    llama_sampler_reset(sampler);
    const int32_t prompt_length = static_cast<int32_t>(std::char_traits<char>::length(prompt));
    const int32_t required = -llama_tokenize(vocab, prompt, prompt_length, nullptr, 0, true, true);
    if (required <= 0 || required + max_tokens > 512) {
        env->ReleaseStringUTFChars(prompt_value, prompt);
        throw_java(env, "java/lang/IllegalArgumentException", "Prompt exceeds the PoC context bound");
        return nullptr;
    }
    std::vector<llama_token> tokens(static_cast<size_t>(required));
    const int32_t token_count = llama_tokenize(
            vocab, prompt, prompt_length, tokens.data(), required, true, true);
    env->ReleaseStringUTFChars(prompt_value, prompt);
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

    llama_batch prompt_batch = make_batch(tokens, 0);
    const int32_t prompt_result = llama_decode(context, prompt_batch);
    llama_batch_free(prompt_batch);
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
        llama_batch batch = make_batch(next_token, position++);
        const int32_t decode_result = llama_decode(context, batch);
        llama_batch_free(batch);
        if (generation_cancelled(engine_id, generation_id) || decode_result == 2) {
            throw_cancelled(env);
            return nullptr;
        }
        if (decode_result != 0) {
            throw_java(env, "java/lang/IllegalStateException", "Token evaluation failed");
            return nullptr;
        }
    }
    return env->NewStringUTF(result.c_str());
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
