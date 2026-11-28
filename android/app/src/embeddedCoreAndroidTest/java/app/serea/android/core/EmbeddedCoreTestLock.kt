package app.serea.android.core

/** Serializes instrumentation that touches the process-wide native Core owner. */
internal object EmbeddedCoreTestLock {
    val monitor = Any()
}
