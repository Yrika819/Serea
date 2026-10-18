package app.serea.android

import org.junit.Assert.assertEquals
import org.junit.Test
import kotlinx.coroutines.runBlocking

class SecurityPortsTest {
    @Test fun publicMayPassAndUnpreparedPersonalFailsClosed() {
        assertEquals(EgressDecision.ALLOW, ChatEgressGate.decide(DataClassification.PUBLIC))
        assertEquals(EgressDecision.DENY_UNPREPARED, ChatEgressGate.decide(DataClassification.PERSONAL))
    }

    @Test fun preparationMarkerCannotAuthorizePersonalData() {
        assertEquals(EgressDecision.DENY_UNPREPARED, ChatEgressGate.decide(DataClassification.PERSONAL, PreparedPersonalData("redacted")))
    }

    @Test fun privateSecretCredentialAndUnknownNeverPass() {
        listOf(DataClassification.PRIVATE, DataClassification.SECRET, DataClassification.CREDENTIAL).forEach {
            assertEquals(EgressDecision.DENY_SENSITIVE, ChatEgressGate.decide(it, PreparedPersonalData("cannot lower class")))
        }
        assertEquals(EgressDecision.DENY_UNCLASSIFIED, ChatEgressGate.decide(DataClassification.UNKNOWN))
    }

    @Test fun rejectedCloudRequestNeverReachesProvider() = runBlocking {
        var calls = 0
        val gated = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                return ModelChatResponse("provider response")
            }
        })
        listOf(DataClassification.UNKNOWN, DataClassification.PRIVATE, DataClassification.SECRET,
            DataClassification.CREDENTIAL, DataClassification.PERSONAL).forEach { classification ->
            try {
                gated.send(ModelChatRequest("conversation", "sensitive", classification = classification))
                throw AssertionError("request should have been rejected")
            } catch (_: ModelChatFailure) { }
        }
        assertEquals(0, calls)
    }

    @Test fun preparationMarkerNeverReachesProvider() = runBlocking {
        var calls = 0
        val gated = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                return ModelChatResponse("ok")
            }
        })
        try {
            gated.send(ModelChatRequest("conversation", "raw private text", classification = DataClassification.PERSONAL,
                preparedPersonal = PreparedPersonalData("minimized and redacted")))
            throw AssertionError("Personal data must be denied until a trusted preparation policy exists")
        } catch (_: ModelChatFailure) { }
        assertEquals(0, calls)
    }
}
