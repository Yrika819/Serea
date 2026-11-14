package app.serea.android.googleauth

import android.accounts.Account
import android.content.ContextWrapper
import android.content.Intent
import app.serea.android.BuildConfig
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Gateway contract tests inject synthetic Play Services operations; no Google client is constructed. */
class PrivateGoogleGatewayOperationsTest {
    private val accountA = Account("account-a@example.invalid", "com.google")
    private val primary = setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)

    private class FakeOperations : GoogleAuthorizationOperations {
        var authorizeCalls = 0
        var revokeCalls = 0
        var account: Account? = null
        var requestedScopes: Set<GoogleScope>? = null
        var optOutIncludingGrantedScopes: Boolean? = null
        var result = GoogleAuthorizationResultSnapshot(
            hasResolution = false,
            grantedScopes = setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY.value),
            accessToken = "synthetic-token",
            pendingIntent = null,
        )
        var revokeStarted: CompletableDeferred<Unit>? = null
        var finishRevoke: CompletableDeferred<Unit>? = null
        var revokeFailure: Exception? = null

        override suspend fun authorize(
            account: Account,
            scopes: Set<GoogleScope>,
            optOutIncludingGrantedScopes: Boolean,
        ): GoogleAuthorizationResultSnapshot {
            authorizeCalls++
            this.account = account
            requestedScopes = scopes
            this.optOutIncludingGrantedScopes = optOutIncludingGrantedScopes
            return result
        }

        override fun resultFromIntent(data: Intent?) = result

        override suspend fun revoke(account: Account, scopes: Set<GoogleScope>) {
            revokeCalls++
            this.account = account
            requestedScopes = scopes
            revokeStarted?.complete(Unit)
            finishRevoke?.await()
            revokeFailure?.let { throw it }
        }
    }

    private fun gateway(operations: FakeOperations) = PlayServicesGoogleAuthorizationGateway(
        context = ContextWrapper(null),
        realOAuthEnabled = true,
        selectedAccount = accountA,
        authorizationOperations = operations,
        serviceStateOverride = { GoogleServiceState.AVAILABLE },
        elapsedRealtimeMillis = { 1_000L },
    )

    @Test fun invalidFeatureOrScopeIsRejectedBeforeAnyGoogleOperation() = runBlocking {
        val operations = FakeOperations()
        val gateway = gateway(operations)
        assertEquals(
            TokenSupplyResult.Unavailable(AuthorizationFailure.SCOPE_MISMATCH),
            gateway.acquire(GoogleFeature.GMAIL, primary),
        )
        assertEquals(
            TokenSupplyResult.Unavailable(AuthorizationFailure.SCOPE_MISMATCH),
            gateway.acquire(GoogleFeature.CALENDAR, emptySet()),
        )
        assertEquals(
            AuthorizationOutcome.Rejected(AuthorizationFailure.SCOPE_MISMATCH),
            gateway.authorize(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY, GoogleScope.GMAIL_READ_ONLY)),
        )
        assertEquals(0, operations.authorizeCalls)
        assertEquals(0, operations.revokeCalls)
    }

    @Test fun validAuthorizationUsesOnlySelectedAccountPrimaryScopeAndOptsOutOfExistingGrants() = runBlocking {
        val grantState = GoogleGrantLeaseState()
        val initialVersions = grantState.leaseVersions(setOf(GoogleFeature.CALENDAR))
        assertEquals(null, googleAuthorizationOperationFailure(grantState, primary, initialVersions))
        val operations = FakeOperations()
        val gateway = gateway(operations)
        if (!BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED) {
            assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED), gateway.authorize(primary))
            assertEquals(0, operations.authorizeCalls)
            return@runBlocking
        }

        val outcome = gateway.authorize(primary)
        assertTrue("outcome=$outcome operations=${operations.authorizeCalls} scope=${operations.requestedScopes}", outcome is AuthorizationOutcome.Authorized)
        assertEquals(1, operations.authorizeCalls)
        assertEquals(accountA.name, operations.account?.name)
        assertEquals(accountA.type, operations.account?.type)
        assertEquals(primary, operations.requestedScopes)
        assertEquals(true, operations.optOutIncludingGrantedScopes)
        (outcome as AuthorizationOutcome.Authorized).lease.close()
    }

    @Test fun overgrantResolutionAndIncompleteTokenNeverIssueConsentOrLease() = runBlocking {
        val operations = FakeOperations()
        val gateway = gateway(operations)
        if (!BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED) {
            assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED), gateway.authorize(primary))
            return@runBlocking
        }

        operations.result = operations.result.copy(
            hasResolution = true,
            grantedScopes = setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY.value, GoogleScope.GMAIL_READ_ONLY.value),
            pendingIntent = null,
        )
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.SCOPE_MISMATCH), gateway.authorize(primary))

        operations.result = operations.result.copy(
            hasResolution = false,
            grantedScopes = emptySet(),
            accessToken = "synthetic-token-with-incomplete-grant",
        )
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.DENIED), gateway.authorize(primary))
        assertEquals(2, operations.authorizeCalls)
    }

    @Test fun revokeInvalidatesLeaseBeforeCallingAccountBoundScopeSpecificOperation() = runBlocking {
        val operations = FakeOperations()
        val gateway = gateway(operations)
        if (!BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED) {
            assertEquals(AuthorizationFailure.NOT_CONFIGURED, gateway.revokeGoogleGrant())
            assertEquals(0, operations.revokeCalls)
            return@runBlocking
        }

        val lease = (gateway.authorize(primary) as AuthorizationOutcome.Authorized).lease
        val started = CompletableDeferred<Unit>()
        val finish = CompletableDeferred<Unit>()
        operations.revokeStarted = started
        operations.finishRevoke = finish
        val revoke = async { gateway.revokeGoogleGrant() }
        withTimeout(2_000L) { started.await() }

        assertTrue("the local token lease is dead before remote revocation completes", runCatching {
            lease.useToken { it }
        }.isFailure)
        assertEquals(1, operations.revokeCalls)
        assertEquals(accountA.name, operations.account?.name)
        assertEquals(accountA.type, operations.account?.type)
        assertEquals(primary, operations.requestedScopes)

        finish.complete(Unit)
        assertEquals(null, revoke.await())
        assertFalse(runCatching { lease.useToken { it } }.isSuccess)
    }

    @Test fun unavailablePlayServicesKeepsLocalConnectionStoppedWithoutRemoteCall() = runBlocking {
        val operations = FakeOperations()
        var service = GoogleServiceState.AVAILABLE
        val gateway = PlayServicesGoogleAuthorizationGateway(
            context = ContextWrapper(null),
            realOAuthEnabled = true,
            selectedAccount = accountA,
            authorizationOperations = operations,
            serviceStateOverride = { service },
            elapsedRealtimeMillis = { 1_000L },
        )
        if (!BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED) {
            assertEquals(AuthorizationFailure.NOT_CONFIGURED, gateway.revokeGoogleGrant())
            assertEquals(0, operations.revokeCalls)
            return@runBlocking
        }

        val lease = (gateway.authorize(primary) as AuthorizationOutcome.Authorized).lease
        service = GoogleServiceState.MISSING
        assertEquals(AuthorizationFailure.SERVICE_MISSING, gateway.revokeGoogleGrant())
        assertEquals(0, operations.revokeCalls)
        assertFalse(runCatching { lease.useToken { it } }.isSuccess)
    }

    @Test fun remoteRevokeExceptionReportsFailureAfterLocalLeaseInvalidation() = runBlocking {
        val operations = FakeOperations().apply { revokeFailure = java.io.IOException("synthetic only") }
        val gateway = gateway(operations)
        if (!BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED) {
            assertEquals(AuthorizationFailure.NOT_CONFIGURED, gateway.revokeGoogleGrant())
            assertEquals(0, operations.revokeCalls)
            return@runBlocking
        }

        val lease = (gateway.authorize(primary) as AuthorizationOutcome.Authorized).lease
        assertEquals(AuthorizationFailure.NETWORK, gateway.revokeGoogleGrant())
        assertEquals(1, operations.revokeCalls)
        assertFalse(runCatching { lease.useToken { it } }.isSuccess)
    }
}
