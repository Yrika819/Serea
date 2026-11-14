package app.serea.android.googleauth

import android.accounts.Account
import android.app.PendingIntent
import android.content.ContextWrapper
import android.content.Intent
import app.serea.android.BuildConfig
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Exercises the private OAuth gateway with synthetic Play Services snapshots only. */
class PrivateGoogleConsentResultTest {
    private val primary = setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)
    private val account = Account("synthetic-consent@example.invalid", "com.google")

    private class FakeOperations : GoogleAuthorizationOperations {
        var authorizeCalls = 0
        var resultFromIntentCalls = 0
        var lastIntent: Intent? = null
        var initialResult = resolutionResult()
        var consentResult = GoogleAuthorizationResultSnapshot(
            hasResolution = false,
            grantedScopes = PrivatePrimaryCalendarScopeContract.rawScopes,
            accessToken = "synthetic-result-token",
            pendingIntent = null,
        )

        override suspend fun authorize(
            account: Account,
            scopes: Set<GoogleScope>,
            optOutIncludingGrantedScopes: Boolean,
        ): GoogleAuthorizationResultSnapshot {
            authorizeCalls++
            assertEquals(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY), scopes)
            assertTrue(optOutIncludingGrantedScopes)
            return initialResult
        }

        override fun resultFromIntent(data: Intent?): GoogleAuthorizationResultSnapshot {
            resultFromIntentCalls++
            lastIntent = data
            return consentResult
        }

        override suspend fun revoke(account: Account, scopes: Set<GoogleScope>) {
            error("Consent regression tests must never invoke Google revoke.")
        }
    }

    private fun gateway(operations: FakeOperations) = PlayServicesGoogleAuthorizationGateway(
        context = ContextWrapper(null),
        realOAuthEnabled = true,
        selectedAccount = account,
        authorizationOperations = operations,
        serviceStateOverride = { GoogleServiceState.AVAILABLE },
        elapsedRealtimeMillis = { 1_000L },
    )

    @Test fun privateOAuthBuildFlagIsEnabledForTheseFakeGatewayRegressions() {
        assertTrue("run only with SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED=true", BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
        assertEquals("app.serea.android.privatebeta", BuildConfig.APPLICATION_ID)
    }

    @Test fun resultFromIntentAcceptsOnlyExactScopeAndConsumesConsentOnce() = runBlocking {
        assertTrue("private OAuth BuildConfig flag must be enabled", BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
        val cases = listOf(
            "exact primary scope" to (setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY.value) to null),
            "overgrant" to (setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY.value, GoogleScope.GMAIL_READ_ONLY.value) to AuthorizationFailure.SCOPE_MISMATCH),
            "unknown scope" to (setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY.value, "https://example.invalid/unknown") to AuthorizationFailure.SCOPE_MISMATCH),
            "missing scope" to (emptySet<String>() to AuthorizationFailure.DENIED),
        )

        cases.forEach { (label, response) ->
            val operations = FakeOperations()
            val gateway = gateway(operations)
            operations.consentResult = operations.consentResult.copy(grantedScopes = response.first)

            val authorization = gateway.authorize(primary) as AuthorizationOutcome.ConsentRequired
            val requestId = assertNotNullAndGet(authorization.requestId)
            assertNotNull("$label should claim the synthetic resolution", gateway.takePendingConsentIntent(requestId))
            val result = gateway.completeConsentResult(requestId, null)

            assertEquals("$label", 1, operations.authorizeCalls)
            assertEquals("$label must use resultFromIntent", 1, operations.resultFromIntentCalls)
            assertNull("null synthetic Intent is forwarded without Android/Google parsing", operations.lastIntent)
            if (response.second == null) {
                assertTrue("$label should issue a lease", result is AuthorizationOutcome.Authorized)
                (result as AuthorizationOutcome.Authorized).lease.close()
            } else {
                assertEquals("$label", AuthorizationOutcome.Rejected(requireNotNull(response.second)), result)
                assertFalse("$label must not issue an access token lease", result is AuthorizationOutcome.Authorized)
            }
            assertEquals(
                "replayed $label result must be rejected before resultFromIntent",
                AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT),
                gateway.completeConsentResult(requestId, null),
            )
            assertEquals("replayed $label result must not be parsed again", 1, operations.resultFromIntentCalls)
        }
    }

    @Test fun unsafeScopeResponseInvalidatesOldLeasePermissionsAndProviderGenerationWithoutRetry() = runBlocking {
        assertTrue("private OAuth BuildConfig flag must be enabled", BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
        val operations = FakeOperations()
        operations.initialResult = GoogleAuthorizationResultSnapshot(
            hasResolution = false,
            grantedScopes = PrivatePrimaryCalendarScopeContract.rawScopes,
            accessToken = "synthetic-old-lease-token",
            pendingIntent = null,
        )
        val gateway = gateway(operations)
        val lease = (gateway.authorize(primary) as AuthorizationOutcome.Authorized).lease
        var providerInvalidated = false
        val observation = gateway.observeGoogleGrantInvalidation { providerInvalidated = true }
        val generationBeforeBadResult = gateway.googleGrantInvalidationGeneration
        operations.initialResult = resolutionResult().copy(
            grantedScopes = setOf(
                GoogleScope.CALENDAR_EVENTS_READ_ONLY.value,
                "https://example.invalid/unrecognized-grant",
            ),
        )

        assertEquals(
            AuthorizationOutcome.Rejected(AuthorizationFailure.SCOPE_MISMATCH),
            gateway.authorize(primary),
        )
        assertFalse("old token lease must be invalid", runCatching { lease.useToken { it } }.isSuccess)
        assertEquals(generationBeforeBadResult + 1, gateway.googleGrantInvalidationGeneration)
        assertTrue("provider session observer must be notified", providerInvalidated)
        assertEquals(ScopePermission.NEEDS_CONSENT, gateway.status(primary).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY])

        val callsBeforeAcquire = operations.authorizeCalls
        assertEquals(TokenSupplyResult.UserActionRequired(primary), gateway.acquire(GoogleFeature.CALENDAR, primary))
        assertEquals("acquire must not silently call Play Services again", callsBeforeAcquire, operations.authorizeCalls)
        assertEquals("no Consent Result was parsed in the malformed authorize response", 0, operations.resultFromIntentCalls)
        observation.close()
    }

    @Test fun obsoleteConsentResultIsRejectedBeforeResultFromIntentAndCannotBeReplayed() = runBlocking {
        assertTrue("private OAuth BuildConfig flag must be enabled", BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
        val operations = FakeOperations()
        val gateway = gateway(operations)
        val authorization = gateway.authorize(primary) as AuthorizationOutcome.ConsentRequired
        val requestId = assertNotNullAndGet(authorization.requestId)
        assertNotNull(gateway.takePendingConsentIntent(requestId))

        gateway.disableInApp(GoogleFeature.CALENDAR)
        assertEquals(
            AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT),
            gateway.completeConsentResult(requestId, null),
        )
        assertEquals("stale generation is rejected before result parsing", 0, operations.resultFromIntentCalls)
        assertEquals(
            AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT),
            gateway.completeConsentResult(requestId, null),
        )
        assertEquals("stale result replay never reaches resultFromIntent", 0, operations.resultFromIntentCalls)
        assertEquals("no lease can be acquired after the feature was disabled", TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED),
            gateway.acquire(GoogleFeature.CALENDAR, primary))
        assertEquals(1, operations.authorizeCalls)
    }

    private fun assertNotNullAndGet(value: String?): String {
        assertNotNull(value)
        return requireNotNull(value)
    }

    companion object {
        private fun resolutionResult() = GoogleAuthorizationResultSnapshot(
            hasResolution = true,
            grantedScopes = emptySet(),
            accessToken = null,
            pendingIntent = syntheticPendingIntent(),
        )

        /** The fake stores, returns, and identity-compares this sentinel; no PendingIntent API is called. */
        private fun syntheticPendingIntent(): PendingIntent {
            val unsafeClass = Class.forName("sun.misc.Unsafe")
            val unsafeField = unsafeClass.getDeclaredField("theUnsafe")
            unsafeField.isAccessible = true
            val unsafe = unsafeField.get(null)
            return unsafeClass.getMethod("allocateInstance", Class::class.java)
                .invoke(unsafe, PendingIntent::class.java) as PendingIntent
        }
    }
}
