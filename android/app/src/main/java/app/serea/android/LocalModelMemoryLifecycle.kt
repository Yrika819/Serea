package app.serea.android

import android.content.ComponentCallbacks2
import android.content.res.Configuration
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Optional memory-pressure bridge for an owner that explicitly registers it with Android.
 * This candidate is not installed by product composition. The supplied scope owns the eviction
 * work; callers must unregister this callback before cancelling that scope.
 */
internal class LocalModelMemoryLifecycle(
    private val adapter: LocalModelChatAdapter,
    private val scope: CoroutineScope,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : ComponentCallbacks2 {
    private val evictionPending = AtomicBoolean(false)

    override fun onTrimMemory(level: Int) {
        if (level < ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW ||
            !evictionPending.compareAndSet(false, true)) return

        val eviction = scope.launch(dispatcher) {
            try {
                // Adapter's lifecycle mutex lets an active inference finish before releasing JNI.
                adapter.evictModel()
            } finally {
                evictionPending.set(false)
            }
        }
        // Also clear the coalescing guard if its parent scope cancels before the coroutine starts.
        eviction.invokeOnCompletion { evictionPending.set(false) }
    }

    override fun onLowMemory() = onTrimMemory(ComponentCallbacks2.TRIM_MEMORY_COMPLETE)

    override fun onConfigurationChanged(newConfig: Configuration) = Unit
}
