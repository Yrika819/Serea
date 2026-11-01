package app.serea.android

import java.time.Instant

/** Builds the ordinary Chat path as a local-only, deterministic demo. */
internal object DemoChatComposition {
    const val DEMO_PROVIDER_ID = "serea-demo"
    const val DEMO_MODEL_ID = ChatEgressGate.LOCAL_DEMO_MODEL_ID

    fun create(
        usage: UsageRepository,
        demoPort: ModelChatPort = MockModelChatPort(),
        clock: () -> Instant = Instant::now,
    ): ModelChatPort {
        val catalog = FakeModelCatalog(
            entries = listOf(
                ModelDescriptor(
                    id = DEMO_MODEL_ID,
                    providerId = DEMO_PROVIDER_ID,
                    displayName = "Serea Local Demo (fictional)",
                    contextWindow = Knowledge.Known(65_536),
                    maxOutput = Knowledge.Known(512),
                    capabilities = ModelCapabilities(
                        input = setOf("text/plain"),
                        reasoning = Knowledge.Known(true),
                    ),
                    pricing = ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN),
                    availability = ModelAvailability.AVAILABLE,
                    authorized = true,
                    lastVerifiedAt = clock().toString(),
                    source = "bundled deterministic local demo fixture; no external service",
                    verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
                ),
            ),
            clock = clock,
        )
        val localAdapter = LocalDemoModelChatPort(demoPort)
        val usageControlled = UsageControlledModelChatPort(
            localAdapter,
            usage,
            attributionResolver = CatalogUsageAttributionResolver(catalog, clock = clock),
        )
        val egressChecked = EgressCheckedModelChatPort(
            usageControlled,
            expectedTarget = EgressTarget.LOCAL_DEMO,
        )
        return RoutedModelChatPort(
            delegate = egressChecked,
            router = AdaptiveModelRouter(catalog),
            policy = { ModelRoutingPolicy(allowedProviderIds = setOf(DEMO_PROVIDER_ID)) },
            contractFactory = { request, selected ->
                EgressContract(
                    conversationId = request.conversationId,
                    currentMessageId = request.sendId,
                    origin = DataOrigin.USER_AUTHORED,
                    classification = request.classification,
                    destination = ChatEgressGate.LOCAL_DEMO_DESTINATION,
                    plannedBody = request.message,
                    context = request.context,
                    preparation = PayloadPreparation.RAW,
                    explicitSendIntentAtMillis = null,
                    expiresAtMillis = null,
                    selectedModelId = selected.id,
                    target = EgressTarget.LOCAL_DEMO,
                    plannedBodySourceLineage = request.bodySourceLineage.toSet(),
                    localDemoGoogleHandoffAuthorization = request.localDemoGoogleHandoffAuthorization,
                )
            },
        )
    }
}

/** A typed defense at the only local demo provider boundary; it has no HTTP or credential path. */
private class LocalDemoModelChatPort(private val delegate: ModelChatPort) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        val contract = request.egressContract
        if (contract == null || contract.target != EgressTarget.LOCAL_DEMO ||
            contract.destination != ChatEgressGate.LOCAL_DEMO_DESTINATION ||
            contract.selectedModelId != DemoChatComposition.DEMO_MODEL_ID ||
            request.selectedModelId != DemoChatComposition.DEMO_MODEL_ID ||
            contract.origin != DataOrigin.USER_AUTHORED || contract.classification != request.classification ||
            contract.explicitSendIntentAtMillis != null || contract.expiresAtMillis != null ||
            contract.conversationId != request.conversationId || contract.currentMessageId != request.sendId ||
            contract.plannedBody != request.message || contract.context != request.context) {
            throw PreDispatchModelChatFailure("The local demo route requires its exact local-only contract.")
        }
        return delegate.send(request)
    }
}
