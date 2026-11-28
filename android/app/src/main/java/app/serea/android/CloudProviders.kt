package app.serea.android

import okhttp3.HttpUrl.Companion.toHttpUrl
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.AbstractCoroutineContextElement
import kotlin.coroutines.CoroutineContext

/** Owner-approved client-side ceilings. They do not prove the account will incur no charge. */
internal object OllamaCloudRequestLimits {
    const val DAILY_REQUESTS = 20L
    const val MONTHLY_REQUESTS = 200L
    const val INPUT_TOKENS = 4_096
    const val OUTPUT_TOKENS = 1_024
    const val CONCURRENT_REQUESTS = 1
    const val DAILY_TOKEN_BUDGET = DAILY_REQUESTS * (INPUT_TOKENS + OUTPUT_TOKENS)
    const val MONTHLY_TOKEN_BUDGET = MONTHLY_REQUESTS * (INPUT_TOKENS + OUTPUT_TOKENS)
}

/** Per-send routing data passed outside the frozen common ModelChatRequest contract. */
internal class CloudProviderRequestContext(val outputTokenLimit: Int) : AbstractCoroutineContextElement(Key) {
    init { require(outputTokenLimit > 0) }

    private val dispatchPossible = AtomicBoolean(false)
    private val undispatchedConfirmed = AtomicBoolean(false)
    internal fun markDispatchPossible() {
        undispatchedConfirmed.set(false)
        dispatchPossible.set(true)
    }
    /** A cancellation may release usage only after an explicit positive no-dispatch proof. */
    internal fun markDefinitelyUndispatched() {
        if (!dispatchPossible.get()) undispatchedConfirmed.set(true)
    }
    internal fun isDefinitelyUndispatched(): Boolean = undispatchedConfirmed.get() && !dispatchPossible.get()
    internal fun dispatchMayHaveOccurred(): Boolean = dispatchPossible.get()

    companion object Key : CoroutineContext.Key<CloudProviderRequestContext>
}

/** Minimal transport differences that cannot be inferred safely from a model descriptor. */
data class CloudProviderCapabilities(
    val supportsStreaming: Boolean,
    val supportedRoles: Set<String>,
    val maxInputBytes: Int,
    val maxOutputTokens: Int,
) {
    init {
        require(supportedRoles.isNotEmpty() && supportedRoles.all { it in setOf("system", "user", "assistant") })
        require(maxInputBytes > 0 && maxOutputTokens > 0)
    }
}

/** Constrains dispatch by transport environment; external provider calls require explicit Owner authorization. */
enum class CloudProviderNetworkScope { SYNTHETIC, LOOPBACK_TEST, EXTERNAL }

/** Live external dispatch remains fail-closed until separately authorized by the Owner. */
internal object CloudLiveDispatchPolicy {
    const val EXTERNAL_DISPATCH_ENABLED = false
    fun allows(scope: CloudProviderNetworkScope): Boolean =
        scope != CloudProviderNetworkScope.EXTERNAL || EXTERNAL_DISPATCH_ENABLED
}

/** Provider-specific wire protocol behind the existing approved Chat payload contract. */
internal interface CloudChatProvider {
    val providerId: String
    val networkScope: CloudProviderNetworkScope
    /** Canonical Egress destination, independent of a loopback test endpoint. */
    val destination: String
    val capabilities: CloudProviderCapabilities
    /** Test fixture catalogs are permitted only by explicitly synthetic/loopback adapters. */
    val allowsTestFixtureModels: Boolean get() = false

    suspend fun send(
        payload: EgressApprovedPayload,
        onDelta: (String) -> Unit = {},
        selectedModelId: String?,
        outputTokenLimit: Int,
    ): CloudReply
}

internal data class ResolvedCloudProvider(val provider: CloudChatProvider, val outputTokenLimit: Int)

/** Resolves an approved model to one exact provider transport; it never falls back or retries. */
internal class CloudProviderRegistry(
    providers: Collection<CloudChatProvider>,
    private val allowedProviderIds: Set<String>,
) {
    private val providersById: Map<String, CloudChatProvider>
    internal val requiresLiveAuthorization: Boolean

    internal fun outputLimitFor(providerId: String): Int =
        if (providerId !in allowedProviderIds) 0 else providersById[providerId]?.capabilities?.maxOutputTokens ?: 0

    init {
        require(allowedProviderIds.all { it.matches(Regex("[a-z0-9-]{1,64}")) })
        require(providers.map { it.providerId }.distinct().size == providers.size) { "Provider IDs must be unique." }
        providers.forEach { provider ->
            require(provider.providerId.matches(Regex("[a-z0-9-]{1,64}")))
            val url = provider.destination.toHttpUrl()
            require(url.isHttps && url.username.isEmpty() && url.password.isEmpty() &&
                url.encodedQuery == null && url.encodedFragment == null) { "Provider destination must be a fixed HTTPS URL." }
        }
        providersById = providers.associateBy { it.providerId }
        requiresLiveAuthorization = providers.any { it.networkScope == CloudProviderNetworkScope.EXTERNAL }
    }

    internal fun resolve(
        payload: EgressApprovedPayload,
        selectedModelId: String?,
        catalog: ModelCatalog,
        outputTokenLimit: Int? = null,
    ): ResolvedCloudProvider {
        if (payload.target != EgressTarget.CLOUD) throw ProviderPreDispatchFailure(ProviderFailureKind.FORBIDDEN)
        if (selectedModelId.isNullOrBlank() || payload.selectedModelId != selectedModelId) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        }
        val model = catalog.all().singleOrNull { it.id == selectedModelId }
            ?: throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        if (model.providerId !in allowedProviderIds) throw ProviderPreDispatchFailure(ProviderFailureKind.FORBIDDEN)
        val provider = providersById[model.providerId]
            ?: throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        if (payload.destination != provider.destination) throw ProviderPreDispatchFailure(ProviderFailureKind.FORBIDDEN)

        val fixtureAllowed = model.verificationStatus == ModelVerificationStatus.TEST_FIXTURE && provider.allowsTestFixtureModels
        val evidenceAllowed = model.verificationStatus == ModelVerificationStatus.VERIFIED || fixtureAllowed
        if (model.availability != ModelAvailability.AVAILABLE || !model.authorized || !evidenceAllowed ||
            model.source.isNullOrBlank() || model.lastVerifiedAt.isNullOrBlank() || !catalog.isFresh(model)) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        }
        val modelOutputLimit = knownInt(model.maxOutput)
            ?: throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        val requestedOutputLimit = outputTokenLimit ?: minOf(modelOutputLimit, provider.capabilities.maxOutputTokens)
        if (requestedOutputLimit <= 0 || requestedOutputLimit > modelOutputLimit ||
            requestedOutputLimit > provider.capabilities.maxOutputTokens) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.LIMIT_EXCEEDED)
        }
        if (payload.origin != DataOrigin.USER_AUTHORED || payload.classification != DataClassification.PUBLIC ||
            payload.conversationId.isNullOrBlank() || payload.sendId.isNullOrBlank()) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.FORBIDDEN)
        }
        val inputBytes = payload.messages.sumOf { (role, content) -> role.length.toLong() + content.toByteArray(Charsets.UTF_8).size + 64L }
        if (inputBytes > provider.capabilities.maxInputBytes || payload.messages.any { it.first !in provider.capabilities.supportedRoles }) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.LIMIT_EXCEEDED)
        }
        return ResolvedCloudProvider(provider, requestedOutputLimit)
    }

    internal suspend fun sendOnce(
        payload: EgressApprovedPayload,
        selectedModelId: String?,
        catalog: ModelCatalog,
        outputTokenLimit: Int? = null,
        onDelta: (String) -> Unit = {},
    ): CloudReply {
        val route = resolve(payload, selectedModelId, catalog, outputTokenLimit)
        if (!CloudLiveDispatchPolicy.allows(route.provider.networkScope)) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.DISABLED)
        }
        // Exactly one selected transport invocation; retry policy belongs to that transport.
        return route.provider.send(payload, onDelta, selectedModelId, route.outputTokenLimit)
    }

    private fun knownInt(value: Knowledge<Int>): Int? = (value as? Knowledge.Known<*>)?.value as? Int
}
