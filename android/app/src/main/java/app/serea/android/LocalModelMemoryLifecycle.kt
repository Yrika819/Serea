package app.serea.android

import android.app.Application
import android.content.ComponentCallbacks2
import android.content.res.Configuration
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicLong

/** Coalesces callback storms without allowing stale completions to clear a newer request. */
internal class LocalModelEvictionCoalescer {
    private val nextTicket = AtomicLong(0)
    private val activeTicket = AtomicLong(0)

    fun begin(): Long? {
        if (activeTicket.get() != 0L) return null
        val ticket = nextTicket.incrementAndGet()
        return if (activeTicket.compareAndSet(0, ticket)) ticket else null
    }

    fun complete(ticket: Long) { activeTicket.compareAndSet(ticket, 0) }
}

/**
 * Optional app-scoped memory bridge. UI-hidden is recorded but never interrupts inference by
 * itself. Background pressure asks the adapter to release only after any active operation exits.
 * Registration belongs to the owner; close() unregisters the actual Android callback.
 */
internal class LocalModelMemoryLifecycle(
    private val callbacks: ComponentCallbacks2Owner,
    private val adapter: LocalModelChatAdapter,
    private val authorizations: LocalModelAuthorizationRegistry,
    private val scope: CoroutineScope,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val eventObserver: (Event) -> Unit = {},
) : ComponentCallbacks2, AutoCloseable {
    enum class Source { ANDROID, SYNTHETIC_TEST }
    data class Event(val level: Int, val source: Source)

    private val coalescer = LocalModelEvictionCoalescer()
    @Volatile private var foreground = true
    @Volatile private var background = false
    @Volatile private var allowBackgroundWork = false
    @Volatile private var registered = false

    fun register() {
        if (!registered) {
            callbacks.registerComponentCallbacks(this)
            registered = true
        }
    }

    fun setUserAllowedBackgroundWork(allowed: Boolean) { allowBackgroundWork = allowed }

    override fun onTrimMemory(level: Int) = handleTrim(level, Source.ANDROID)

    /** Explicit hook for tests; it is labeled synthetic and never impersonates an OS callback. */
    internal fun dispatchSyntheticTrimForTest(level: Int) = handleTrim(level, Source.SYNTHETIC_TEST)

    private fun handleTrim(level: Int, source: Source) {
        eventObserver(Event(level, source))
        when (level) {
            ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN -> {
                foreground = false
                // UI visibility alone does not cancel a running generation or force eviction.
            }
            ComponentCallbacks2.TRIM_MEMORY_BACKGROUND -> {
                foreground = false
                background = true
                if (!allowBackgroundWork) enqueueEviction()
            }
            else -> Unit // RUNNING_LOW and other legacy pressure levels are not relied on.
        }
    }

    /** Called by ProcessLifecycleOwner/activity composition; visible foreground resumes work. */
    internal fun setForeground(isForeground: Boolean) {
        foreground = isForeground
        if (isForeground) background = false
        else if (!allowBackgroundWork) enqueueEviction()
    }

    /** Explicit owner request, independent of Android trim callbacks. */
    fun requestEviction() = enqueueEviction(force = true)

    private fun enqueueEviction(force: Boolean = false) {
        val ticket = coalescer.begin() ?: return
        try {
            val job = scope.launch(dispatcher) {
                try {
                    if (force || (background && !foreground && !allowBackgroundWork)) {
                        adapter.evictModel { authorizations.invalidateForEviction() }
                    }
                } finally {
                    coalescer.complete(ticket)
                }
            }
            job.invokeOnCompletion { coalescer.complete(ticket) }
        } catch (_: Throwable) {
            coalescer.complete(ticket)
        }
    }

    override fun onLowMemory() = Unit
    override fun onConfigurationChanged(newConfig: Configuration) = Unit

    override fun close() {
        if (registered) {
            callbacks.unregisterComponentCallbacks(this)
            registered = false
        }
    }
}

/** Keeps callback registration testable without tying the lifecycle bridge to an Activity. */
internal interface ComponentCallbacks2Owner {
    fun registerComponentCallbacks(callback: ComponentCallbacks2)
    fun unregisterComponentCallbacks(callback: ComponentCallbacks2)
}

internal class ApplicationComponentCallbacks2Owner(
    private val application: Application,
) : ComponentCallbacks2Owner {
    override fun registerComponentCallbacks(callback: ComponentCallbacks2) =
        application.registerComponentCallbacks(callback)

    override fun unregisterComponentCallbacks(callback: ComponentCallbacks2) =
        application.unregisterComponentCallbacks(callback)
}
