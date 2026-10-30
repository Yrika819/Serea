package app.serea.android

import android.content.ComponentCallbacks2
import android.content.res.Configuration
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicLong

/** Ticketed guard: a delayed completion can only release the trim event that acquired it. */
internal class LocalModelEvictionCoalescer {
    private val nextTicket = AtomicLong(0)
    private val activeTicket = AtomicLong(0)

    fun begin(): Long? {
        if (activeTicket.get() != 0L) return null
        val ticket = nextTicket.incrementAndGet()
        return if (activeTicket.compareAndSet(0, ticket)) ticket else null
    }

    fun complete(ticket: Long) {
        activeTicket.compareAndSet(ticket, 0)
    }
}

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
    private val evictions = LocalModelEvictionCoalescer()

    override fun onTrimMemory(level: Int) {
        if (level < ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW) return
        val ticket = evictions.begin() ?: return

        // Adapter's lifecycle mutex lets an active inference finish before releasing JNI.
        // Completion owns the guard release; an older completion cannot clear a newer ticket.
        scope.launch(dispatcher) { adapter.evictModel() }
            .invokeOnCompletion { evictions.complete(ticket) }
    }

    override fun onLowMemory() = onTrimMemory(ComponentCallbacks2.TRIM_MEMORY_COMPLETE)

    override fun onConfigurationChanged(newConfig: Configuration) = Unit
}
