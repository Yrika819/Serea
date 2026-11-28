package app.serea.android

import android.content.Context
import app.serea.android.core.EmbeddedCoreAdapter
import app.serea.android.core.SyntheticCoreLease
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.ExecutionException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext

/**
 * Isolated Android Chat composition: Embedded Core readiness gates the existing local Mock AI port.
 * Chat/ViewModel owns presentation and turn status; Core owns its native process/store lifecycle.
 * This adapter adds no provider, network, credential, device/2 protocol, or durable Chat-history path.
 */
internal object EmbeddedCoreChatComposition {
    fun create(context: Context, providerPort: ModelChatPort = MockModelChatPort()): EmbeddedCoreModelChatPort =
        EmbeddedCoreModelChatPort(EmbeddedCoreAdapter(context.applicationContext), providerPort)
}

/** Cloud integration is explicit and accepts only the fully gated direct-provider composition. */
internal object EmbeddedCoreCloudChatComposition {
    fun create(core: EmbeddedCoreAdapter, cloudPort: GatedCloudModelChatPort): EmbeddedCoreModelChatPort =
        EmbeddedCoreModelChatPort(core, cloudPort)
}

internal class EmbeddedCoreModelChatPort(
    private val core: EmbeddedCoreAdapter,
    private val providerPort: ModelChatPort = MockModelChatPort(),
) : ModelChatPort {
    private val lifecycle = Mutex()
    private var lease: SyntheticCoreLease? = null

    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        ensureCoreReady()
        return providerPort.send(request)
    }

    /** Explicit owner cleanup; failure retains the lease so a caller may retry. */
    suspend fun close() = lifecycle.withLock {
        val current = lease ?: return@withLock
        try {
            core.closeIfOwned(current).awaitUncancelled()
            lease = null
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            throw coreUnavailable()
        }
    }

    private suspend fun ensureCoreReady() = lifecycle.withLock {
        var current = lease
        if (current == null) {
            current = try {
                initializeWithOwnedCancellation()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                throw coreUnavailable()
            }
            lease = current
        }
        try {
            check(core.getStatus().awaitUncancelled())
            check(core.schemaVersion().awaitUncancelled() > 0u)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            // Do not continue into Mock AI after a Core state/health failure.
            try {
                withContext(NonCancellable) { core.closeIfOwned(current).awaitUncancelled() }
                lease = null
            } catch (_: Exception) {
                // Keep ownership after an ambiguous close; explicit close() may retry it.
            }
            throw coreUnavailable()
        }
    }

    private suspend fun initializeWithOwnedCancellation(): SyntheticCoreLease =
        suspendCancellableCoroutine { continuation ->
            val pending = core.initializeForCleanup()
            continuation.invokeOnCancellation { pending.cancel(false) }
            pending.whenComplete { value, failure ->
                if (!continuation.isActive) return@whenComplete
                val cause = failure?.unwrapFutureFailure()
                continuation.resumeWith(
                    if (cause == null) Result.success(value) else Result.failure(cause),
                )
            }
        }

    private suspend fun <T> CompletableFuture<T>.awaitUncancelled(): T =
        suspendCancellableCoroutine { continuation ->
            whenComplete { value, failure ->
                if (!continuation.isActive) return@whenComplete
                val cause = failure?.unwrapFutureFailure()
                continuation.resumeWith(
                    if (cause == null) Result.success(value) else Result.failure(cause),
                )
            }
        }

    private fun Throwable.unwrapFutureFailure(): Throwable =
        (this as? CompletionException)?.cause ?: (this as? ExecutionException)?.cause ?: this

    private fun coreUnavailable() = ModelChatFailure("Embedded Core is unavailable; the local Chat request was not sent.")
}
