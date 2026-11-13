package app.serea.android

import android.app.Application
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.ProcessLifecycleOwner
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import java.io.File

/** Application-owned, process-lifetime entry point for lifecycle registration. */
class SereaApplication : Application() {
    private val lifecycleScope by lazy { CoroutineScope(SupervisorJob() + Dispatchers.IO) }
    private var localModelComposition: LocalModelRuntimeApplicationComposition? = null

    override fun onCreate() {
        super.onCreate()
        check(localModelComposition == null) { "Local-model lifecycle composition was already created." }
        localModelComposition = LocalModelRuntimeApplicationComposition.create(
            modelDirectory = noBackupFilesDir,
            callbacks = ApplicationComponentCallbacks2Owner(this),
            processVisibility = AndroidProcessVisibilityOwner(ProcessLifecycleOwner.get().lifecycle),
            scope = lifecycleScope,
        ).also { it.register() }
    }

    override fun onTerminate() {
        localModelComposition?.close()
        localModelComposition = null
        lifecycleScope.coroutineContext[kotlinx.coroutines.Job]?.cancel()
        super.onTerminate()
    }
}

/**
 * Holds one adapter and injects that exact object into its lifecycle bridge. Product Chat does not
 * receive this adapter. Its backend factory is deliberately inert in ordinary application builds.
 */
internal class LocalModelRuntimeApplicationComposition private constructor(
    val adapter: LocalModelChatAdapter,
    val lifecycle: LocalModelMemoryLifecycle,
    internal val authorizations: LocalModelAuthorizationRegistry,
    private val processVisibility: ProcessVisibilityOwner,
    private val visibilityObserver: ProcessVisibilityObserver,
) : AutoCloseable {
    private var registered = false

    @Synchronized fun register() {
        if (registered) return
        try {
            lifecycle.register()
            processVisibility.register(visibilityObserver)
            registered = true
        } catch (failure: Throwable) {
            cleanupSuppressed(
                failure,
                { processVisibility.unregister(visibilityObserver) },
                { lifecycle.close() },
                { adapter.close() },
            )
            throw failure
        }
    }

    @Synchronized override fun close() {
        registered = false
        var failure: Throwable? = null
        listOf<() -> Unit>(
            { processVisibility.unregister(visibilityObserver) },
            { lifecycle.close() },
            { adapter.close() },
        ).forEach { cleanup ->
            try {
                cleanup()
            } catch (cleanupFailure: Throwable) {
                val original = failure
                if (original == null) failure = cleanupFailure
                else if (cleanupFailure !== original) original.addSuppressed(cleanupFailure)
            }
        }
        failure?.let { throw it }
    }

    companion object {
        fun create(
            modelDirectory: File,
            callbacks: ComponentCallbacks2Owner,
            processVisibility: ProcessVisibilityOwner,
            scope: CoroutineScope,
            dispatcher: CoroutineDispatcher = Dispatchers.IO,
            artifact: LocalModelArtifact = inactiveArtifact,
            modelFile: File = File(modelDirectory, "local-model-runtime-disabled.gguf"),
            backendFactory: LocalModelInferenceBackendFactory = LocalModelInferenceBackendFactory {
                throw PreDispatchModelChatFailure("Native inference is disabled in the application composition.")
            },
            deviceAbi: String = artifact.supportedAbis.single(),
            runtimeLimits: LocalModelRuntimeLimits = LocalModelRuntimeLimits(),
            memoryFitCheck: LocalModelMemoryFitCheck = ProcMemAvailableFitCheck,
            authorizations: LocalModelAuthorizationRegistry = LocalModelAuthorizationRegistry(),
            eventObserver: (LocalModelMemoryLifecycle.Event) -> Unit = {},
        ): LocalModelRuntimeApplicationComposition {
            val adapter = LocalModelChatAdapter(
                artifact = artifact,
                modelFile = modelFile,
                backendFactory = backendFactory,
                deviceAbi = deviceAbi,
                runtimeLimits = runtimeLimits,
                memoryFitCheck = memoryFitCheck,
            )
            val lifecycle = LocalModelMemoryLifecycle(
                callbacks = callbacks,
                adapter = adapter,
                authorizations = authorizations,
                scope = scope,
                dispatcher = dispatcher,
                eventObserver = eventObserver,
            )
            val visibilityObserver = ProcessVisibilityObserver { isForeground ->
                lifecycle.setForeground(isForeground)
            }
            return LocalModelRuntimeApplicationComposition(
                adapter, lifecycle, authorizations, processVisibility, visibilityObserver,
            )
        }

        private val inactiveArtifact = LocalModelArtifact(
            modelId = "serea-local/inactive-lifecycle",
            displayName = "Inactive lifecycle adapter",
            modelRepository = "https://example.invalid/serea/inactive-lifecycle",
            modelRevision = "0".repeat(40),
            licenseId = "MIT",
            expectedSha256 = "0".repeat(64),
            expectedSizeBytes = 1,
            runtimeName = "inactive",
            runtimeRevision = "0000000",
            supportedAbis = setOf("x86_64"),
        )
    }
}

private fun cleanupSuppressed(original: Throwable, vararg cleanups: () -> Unit) {
    cleanups.forEach { cleanup ->
        try {
            cleanup()
        } catch (cleanupFailure: Throwable) {
            if (cleanupFailure !== original) original.addSuppressed(cleanupFailure)
        }
    }
}

/** Process-scoped visibility events avoid coupling Native lifecycle state to Activity recreation. */
internal fun interface ProcessVisibilityObserver {
    fun onProcessVisibilityChanged(isForeground: Boolean)
}

internal interface ProcessVisibilityOwner {
    fun register(observer: ProcessVisibilityObserver)
    fun unregister(observer: ProcessVisibilityObserver)
}

internal class AndroidProcessVisibilityOwner(
    private val lifecycle: Lifecycle,
) : ProcessVisibilityOwner {
    private val observers = java.util.IdentityHashMap<ProcessVisibilityObserver, LifecycleEventObserver>()

    @Synchronized override fun register(observer: ProcessVisibilityObserver) {
        if (observers.containsKey(observer)) return
        val lifecycleObserver = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> observer.onProcessVisibilityChanged(true)
                Lifecycle.Event.ON_STOP -> observer.onProcessVisibilityChanged(false)
                else -> Unit
            }
        }
        observers[observer] = lifecycleObserver
        lifecycle.addObserver(lifecycleObserver)
    }

    @Synchronized override fun unregister(observer: ProcessVisibilityObserver) {
        observers.remove(observer)?.let(lifecycle::removeObserver)
    }
}
