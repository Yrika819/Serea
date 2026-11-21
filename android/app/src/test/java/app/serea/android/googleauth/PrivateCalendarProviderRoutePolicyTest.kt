package app.serea.android.googleauth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class PrivateCalendarProviderRoutePolicyTest {
    private class Gateway(
        override val mode: GoogleAuthorizationMode = GoogleAuthorizationMode.PLAY_SERVICES,
        override val authorizationSessionId: String? = "private-session",
        private val permissions: Map<GoogleScope, ScopePermission> = mapOf(
            GoogleScope.CALENDAR_EVENTS_READ_ONLY to ScopePermission.GRANTED,
        ),
        private val failure: AuthorizationFailure? = null,
        private val enabled: Boolean = true,
    ) : GoogleAuthorizationGateway {
        var statusCalls = 0
        var tokenCalls = 0
        override fun isEnabledInApp(feature: GoogleFeature) = enabled
        override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
            statusCalls++
            assertEquals(PrivatePrimaryCalendarScopeContract.scopes, scopes)
            return AuthorizationStatus(GoogleServiceState.AVAILABLE, permissions, failure)
        }
        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            tokenCalls++
            return TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        }
    }

    @Test
    fun gateOffAlwaysSelectsFakeWithoutConsultingOAuthOrGateway() {
        val gateway = Gateway(mode = GoogleAuthorizationMode.MOCK_ONLY, permissions = emptyMap())

        val decision = selectPrivateCalendarProvider(
            applicationId = "app.serea.android.privatebeta",
            buildType = "privateBeta",
            privateUiEnabled = true,
            httpEnabled = false,
            oauthEnabled = true,
            gateway = gateway,
        )

        assertSame(PrivateCalendarProviderDecision.Fake, decision)
        assertEquals(0, gateway.statusCalls)
    }

    @Test
    fun gateOnOutsideExactPrivateProductIdentityFailsBeforeGatewayStatus() {
        val gateway = Gateway()

        assertTrue(runCatching {
            selectPrivateCalendarProvider(
                applicationId = "app.serea.android.beta",
                buildType = "beta",
                privateUiEnabled = false,
                httpEnabled = true,
                oauthEnabled = true,
                gateway = gateway,
            )
        }.isFailure)
        assertEquals(0, gateway.statusCalls)
    }

    @Test
    fun gateOnWithOAuthDisabledFailsBeforeGatewayStatus() {
        val gateway = Gateway()

        assertTrue(runCatching {
            selectPrivateCalendarProvider(
                applicationId = "app.serea.android.privatebeta",
                buildType = "privateBeta",
                privateUiEnabled = true,
                httpEnabled = true,
                oauthEnabled = false,
                gateway = gateway,
            )
        }.isFailure)
        assertEquals(0, gateway.statusCalls)
    }

    @Test
    fun gateOnWithMissingOrIneligibleGatewayFailsClosed() {
        val failure = runCatching {
            selectPrivateCalendarProvider(
                "app.serea.android.privatebeta", "privateBeta", true, true, true, null,
            )
        }.exceptionOrNull()
        assertTrue(failure is IllegalStateException)

        val mockGateway = Gateway(mode = GoogleAuthorizationMode.MOCK_ONLY)
        assertTrue(runCatching {
            selectPrivateCalendarProvider(
                "app.serea.android.privatebeta", "privateBeta", true, true, true, mockGateway,
            )
        }.isFailure)
        assertEquals(0, mockGateway.statusCalls)
    }

    @Test
    fun gateOnRejectsMissingSessionDisabledFeatureAndInsufficientScope() {
        val invalidGateways = listOf(
            Gateway(authorizationSessionId = null),
            Gateway(authorizationSessionId = " "),
            Gateway(enabled = false),
            Gateway(permissions = mapOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY to ScopePermission.DENIED)),
            Gateway(
                permissions = emptyMap(),
                failure = AuthorizationFailure.UNAVAILABLE,
            ),
        )

        invalidGateways.forEach { gateway ->
            assertTrue(runCatching {
                selectPrivateCalendarProvider(
                    "app.serea.android.privatebeta", "privateBeta", true, true, true, gateway,
                )
            }.isFailure)
        }
    }

    @Test
    fun personalCreatorAccountMismatchReportedByMockIsRejectedBeforeTokenOrHttp() {
        val gateway = Gateway(
            permissions = emptyMap(),
            failure = AuthorizationFailure.UNAVAILABLE,
        )

        assertTrue(runCatching {
            selectPrivateCalendarProvider(
                "app.serea.android.privatebeta", "privateBeta", true, true, true, gateway,
            )
        }.isFailure)
        assertEquals(1, gateway.statusCalls)
        assertEquals(0, gateway.tokenCalls)
    }

    @Test
    fun allValidPrivateProductConditionsSelectLiveWithoutPerformingHttp() {
        val gateway = Gateway()

        val decision = selectPrivateCalendarProvider(
            "app.serea.android.privatebeta", "privateBeta", true, true, true, gateway,
        )

        assertTrue(decision is PrivateCalendarProviderDecision.Live)
        assertEquals("private-session", (decision as PrivateCalendarProviderDecision.Live).sessionId)
        assertEquals(1, gateway.statusCalls)
    }
}
