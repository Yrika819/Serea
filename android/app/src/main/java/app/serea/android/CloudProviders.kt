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
    const val GENERAL_EXTERNAL_DISPATCH_ENABLED = false
    const val FIXED_SYNTHETIC_PROBE_ENABLED = true
    const val FIXED_PROBE_PROMPT = "Reply with exactly: Serea cloud contract probe passed."
    private const val CONVERSATION_PREFIX = "ollama-contract-probe-"
    private const val SEND_PREFIX = "ollama-contract-probe-send-"

    fun isFixedSyntheticProbeContract(contract: EgressContract): Boolean {
        val suffix = contract.conversationId.removePrefix(CONVERSATION_PREFIX)
        return FIXED_SYNTHETIC_PROBE_ENABLED &&
            contract.target == EgressTarget.CLOUD &&
            contract.destination == OllamaGemma4CloudCatalog.DESTINATION &&
            suffix.matches(Regex("[0-9a-fA-F-]{36}")) &&
            contract.currentMessageId == "$SEND_PREFIX$suffix" &&
            contract.selectedModelId in setOf(null, OllamaGemma4CloudCatalog.MODEL_ID) &&
            contract.origin == DataOrigin.USER_AUTHORED &&
            contract.classification == DataClassification.PUBLIC &&
            contract.plannedBody == FIXED_PROBE_PROMPT &&
            contract.context.isEmpty() &&
            contract.preparation == PayloadPreparation.RAW &&
            contract.cloudDispatchPurpose == CloudDispatchPurpose.FIXED_SYNTHETIC_PROBE &&
            contract.plannedBodySourceLineage == setOf(DataOrigin.USER_AUTHORED) &&
            contract.localDemoGoogleHandoffAuthorization == null &&
            contract.localModelConsent == null
    }

    fun isFixedSyntheticProbeRequest(request: ModelChatRequest): Boolean {
        val contract = request.egressContract ?: return false
        return isFixedSyntheticProbeContract(contract) &&
            request.conversationId == contract.conversationId &&
            request.sendId == contract.currentMessageId &&
            request.selectedModelId in setOf(null, OllamaGemma4CloudCatalog.MODEL_ID) &&
            request.message == FIXED_PROBE_PROMPT &&
            request.context.isEmpty() &&
            request.classification == DataClassification.PUBLIC &&
            request.bodySourceLineage == setOf(DataOrigin.USER_AUTHORED) &&
            request.preparedPersonal == null &&
            request.localDemoGoogleHandoffAuthorization == null
    }

    fun isFixedSyntheticProbePayload(payload: EgressApprovedPayload, modelId: String?): Boolean {
        val conversationId = payload.conversationId ?: return false
        val sendId = payload.sendId ?: return false
        val suffix = conversationId.removePrefix(CONVERSATION_PREFIX)
        return FIXED_SYNTHETIC_PROBE_ENABLED &&
            suffix.matches(Regex("[0-9a-fA-F-]{36}")) &&
            sendId == "$SEND_PREFIX$suffix" &&
            payload.selectedModelId == OllamaGemma4CloudCatalog.MODEL_ID &&
            modelId == OllamaGemma4CloudCatalog.MODEL_ID &&
            payload.destination == OllamaGemma4CloudCatalog.DESTINATION &&
            payload.target == EgressTarget.CLOUD &&
            payload.origin == DataOrigin.USER_AUTHORED &&
            payload.classification == DataClassification.PUBLIC &&
            payload.dispatchPurpose == CloudDispatchPurpose.FIXED_SYNTHETIC_PROBE &&
            payload.bodySourceLineage == setOf(DataOrigin.USER_AUTHORED) &&
            payload.messages == listOf("user" to FIXED_PROBE_PROMPT)
    }

    fun allows(scope: CloudProviderNetworkScope, request: ModelChatRequest): Boolean =
        scope != CloudProviderNetworkScope.EXTERNAL || GENERAL_EXTERNAL_DISPATCH_ENABLED ||
            isFixedSyntheticProbeRequest(request)

    fun allows(scope: CloudProviderNetworkScope, payload: EgressApprovedPayload, modelId: String?): Boolean =
        scope != CloudProviderNetworkScope.EXTERNAL || GENERAL_EXTERNAL_DISPATCH_ENABLED ||
            isFixedSyntheticProbePayload(payload, modelId)
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
        if (!CloudLiveDispatchPolicy.allows(route.provider.networkScope, payload, selectedModelId)) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.DISABLED)
        }
        // Exactly one selected transport invocation; retry policy belongs to that transport.
        return route.provider.send(payload, onDelta, selectedModelId, route.outputTokenLimit)
    }

    private fun knownInt(value: Knowledge<Int>): Int? = (value as? Knowledge.Known<*>)?.value as? Int
}
