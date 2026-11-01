package app.serea.android

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class GoogleOriginEgressAdversarialTest {
    @Test fun gmailAndCalendarOriginCannotReachTheUsageOrProviderBoundaryEvenWhenMarkedPublic() = runBlocking {
        val now = 50_000L
        var reservationOrProviderCalls = 0
        val egress = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                // This delegate stands for the inner Usage reservation + provider dispatch layer.
                reservationOrProviderCalls++
                return ModelChatResponse("must not be produced")
            }
        }, clock = { now })

        for (origin in listOf(DataOrigin.GMAIL, DataOrigin.CALENDAR)) {
            val sendId = "fixture-${origin.name.lowercase()}-egress"
            val message = "synthetic ${origin.name.lowercase()} content"
            val contract = EgressContract(
                conversationId = "fixture-conversation",
                currentMessageId = sendId,
                origin = origin,
                classification = DataClassification.PUBLIC,
                destination = "https://ollama.com/api/chat",
                plannedBody = message,
                context = emptyList(),
                preparation = PayloadPreparation.RAW,
                explicitSendIntentAtMillis = now,
                expiresAtMillis = now + 30_000,
            )

            assertEquals(EgressContractDecision.ORIGIN_DENIED, ChatEgressGate.evaluate(contract, now).decision)
            assertThrows(ModelChatFailure::class.java) {
                runBlocking {
                    egress.send(
                        ModelChatRequest(
                            conversationId = "fixture-conversation",
                            message = message,
                            sendId = sendId,
                            classification = DataClassification.PUBLIC,
                            egressContract = contract,
                        ),
                    )
                }
            }
        }

        assertEquals(0, reservationOrProviderCalls)
    }
}
