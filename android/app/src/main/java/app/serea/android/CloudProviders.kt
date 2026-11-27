package app.serea.android

import okhttp3.HttpUrl.Companion.toHttpUrl

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

/** Provider-specific wire protocol behind the existing approved Chat payload contract. */
internal interface CloudChatProvider {
    val providerId: String
    /** Canonical Egress destination, independent of a loopback test endpoint. */
    val destination: String
    val capabilities: CloudProviderCapabilities
    /** Test fixture catalogs are permitted only by explicitly synthetic/loopback adapters. */
    val allowsTestFixtureModels: Boolean get() = false

    suspend fun send(
        payload: EgressApprovedPayload,
        onDelta: (String) -> Unit = {},
        selectedModelId: String?,
    ): CloudReply
}

/** Resolves an approved model to one exact provider transport; it never falls back or retries. */
internal class CloudProviderRegistry(
    providers: Collection<CloudChatProvider>,
    private val allowedProviderIds: Set<String>,
) {
    private val providersById: Map<String, CloudChatProvider>

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
    }

    internal fun resolve(
        payload: EgressApprovedPayload,
        selectedModelId: String?,
        catalog: ModelCatalog,
    ): CloudChatProvider {
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
        if (knownInt(model.maxOutput) == null) throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        if (payload.origin != DataOrigin.USER_AUTHORED || payload.classification != DataClassification.PUBLIC ||
            payload.conversationId.isNullOrBlank() || payload.sendId.isNullOrBlank()) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.FORBIDDEN)
        }
        val inputBytes = payload.messages.sumOf { (role, content) -> role.length.toLong() + content.toByteArray(Charsets.UTF_8).size + 64L }
        if (inputBytes > provider.capabilities.maxInputBytes || payload.messages.any { it.first !in provider.capabilities.supportedRoles }) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.LIMIT_EXCEEDED)
        }
        return provider
    }

    internal suspend fun sendOnce(
        payload: EgressApprovedPayload,
        selectedModelId: String?,
        catalog: ModelCatalog,
        onDelta: (String) -> Unit = {},
    ): CloudReply {
        val provider = resolve(payload, selectedModelId, catalog)
        // Exactly one selected transport invocation; retry policy belongs to that transport.
        return provider.send(payload, onDelta, selectedModelId)
    }

    private fun knownInt(value: Knowledge<Int>): Int? = (value as? Knowledge.Known<*>)?.value as? Int
}
