package app.serea.android.googleauth

import app.serea.android.googleauth.GoogleScope.CALENDAR_EVENTS_READ_ONLY
import app.serea.android.googleauth.GoogleScope.CALENDAR_LIST_READ_ONLY
import app.serea.android.googleauth.GoogleScope.GMAIL_READ_ONLY
import app.serea.android.googleauth.GoogleFeature.CALENDAR
import app.serea.android.googleauth.GoogleFeature.GMAIL
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GoogleAuthorizationTest {
    private val both = setOf(GMAIL_READ_ONLY, CALENDAR_EVENTS_READ_ONLY)

    @Test fun firstConsentRequestsOnlyRequestedScopesAndIsExplicit() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway()
        val result = fake.authorize(setOf(GMAIL_READ_ONLY))
        assertEquals(AuthorizationOutcome.ConsentRequired(setOf(GMAIL_READ_ONLY)), result)
        assertEquals(ScopePermission.NEEDS_CONSENT, fake.status(setOf(GMAIL_READ_ONLY)).permissions[GMAIL_READ_ONLY])
    }

    @Test fun addingCalendarScopeRequiresConsentWithoutLosingGmailGrant() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(setOf(GMAIL_READ_ONLY))
        assertEquals(AuthorizationOutcome.ConsentRequired(setOf(CALENDAR_EVENTS_READ_ONLY)), fake.authorize(both))
        assertEquals(ScopePermission.GRANTED, fake.status(both).permissions[GMAIL_READ_ONLY])
    }

    @Test fun consentApprovalAndReauthorizationReturnsOnlyRequestedGrant() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway()
        fake.grant(GMAIL_READ_ONLY, CALENDAR_EVENTS_READ_ONLY)
        val first = fake.authorize(both) as AuthorizationOutcome.Authorized
        assertEquals(both, first.lease.grantedScopes)
        first.lease.close()
        assertEquals(AuthorizationOutcome.Authorized::class, fake.authorize(both)::class)
    }

    @Test fun missingScopeIsNotTreatedAsAuthorized() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(setOf(GMAIL_READ_ONLY))
        assertEquals(AuthorizationOutcome.ConsentRequired(setOf(CALENDAR_EVENTS_READ_ONLY)), fake.authorize(both))
        assertFalse(fake.status(both).permits(both))
    }

    @Test fun denialIsTypedAndDoesNotExposeItsDetails() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway()
        fake.denyNext()
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.DENIED), fake.authorize(both))
    }

    @Test fun expirationRequiresReauthorization() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        fake.setExpired(true)
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.EXPIRED), fake.authorize(both))
        assertEquals(AuthorizationFailure.EXPIRED, fake.status(both).failure)
    }

    @Test fun accountSwitchClearsTheOldAccountGrant() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        fake.switchAccount()
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.ACCOUNT_CHANGED), fake.authorize(both))
        assertEquals(AuthorizationOutcome.ConsentRequired(both), fake.authorize(both))
    }

    @Test fun unavailableAndOutdatedGooglePlayServicesAreTyped() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway()
        fake.setGooglePlayServices(GoogleServiceState.MISSING)
        assertEquals(AuthorizationFailure.SERVICE_MISSING, (fake.authorize(both) as AuthorizationOutcome.Rejected).failure)
        fake.setGooglePlayServices(GoogleServiceState.OUTDATED)
        assertEquals(AuthorizationFailure.SERVICE_OUTDATED, (fake.authorize(both) as AuthorizationOutcome.Rejected).failure)
    }

    @Test fun cancellationAndNetworkFailuresAreTyped() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        fake.denyNext(AuthorizationFailure.CANCELLED)
        assertEquals(AuthorizationFailure.CANCELLED, (fake.authorize(both) as AuthorizationOutcome.Rejected).failure)
        fake.denyNext(AuthorizationFailure.NETWORK)
        assertEquals(AuthorizationFailure.NETWORK, (fake.authorize(both) as AuthorizationOutcome.Rejected).failure)
    }

    @Test fun appFeatureDisableDoesNotRevokeSharedGoogleGrant() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        val oldGmailLease = (fake.acquire(GMAIL, setOf(GMAIL_READ_ONLY)) as TokenSupplyResult.Available).lease
        fake.disableInApp(GMAIL)
        assertFalse(fake.isEnabledInApp(GMAIL))
        assertTrue(fake.isEnabledInApp(CALENDAR))
        assertEquals(ScopePermission.GRANTED, fake.status(both).permissions[GMAIL_READ_ONLY])
        assertEquals(AuthorizationFailure.FEATURE_DISABLED,
            (fake.acquire(GMAIL, setOf(GMAIL_READ_ONLY)) as TokenSupplyResult.Unavailable).failure)
        assertFalse(runCatching { oldGmailLease.useToken { it } }.isSuccess)
        val calendar = fake.acquire(CALENDAR, setOf(CALENDAR_EVENTS_READ_ONLY))
        assertTrue(calendar is TokenSupplyResult.Available)
        (calendar as TokenSupplyResult.Available).lease.close()
        assertEquals(0, fake.revokeCount)
        fake.enableInApp(GMAIL)
        assertTrue(fake.isEnabledInApp(GMAIL))
        assertFalse(runCatching { oldGmailLease.useToken { it } }.isSuccess)
    }

    @Test fun googleGrantRevocationRemovesAllServiceScopes() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both + CALENDAR_LIST_READ_ONLY)
        fake.revokeGoogleGrant()
        assertEquals(1, fake.revokeCount)
        assertEquals(AuthorizationOutcome.ConsentRequired(both), fake.authorize(both))
    }

    @Test fun tokenIsTransientRedactedAndInvalidatedWhenLeaseCloses() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        val lease = (fake.authorize(both) as AuthorizationOutcome.Authorized).lease
        assertFalse(lease.toString().contains("fixture-access-token-never-real"))
        var used = false
        lease.useToken { used = it == "fixture-access-token-never-real" }
        assertTrue(used)
        lease.close()
        assertFalse(runCatching { lease.useToken { it } }.isSuccess)
    }

    @Test fun expiredLeaseCannotYieldToken() {
        val lease = AccessTokenLease("fixture-expired-token", both, 0)
        assertFalse(runCatching { lease.useToken { it } }.isSuccess)
    }

    @Test fun oldClosedLeaseCannotBeReusedAfterFreshLeaseIsIssued() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        val old = (fake.acquire(GMAIL, setOf(GMAIL_READ_ONLY)) as TokenSupplyResult.Available).lease
        old.close()
        val fresh = (fake.acquire(GMAIL, setOf(GMAIL_READ_ONLY)) as TokenSupplyResult.Available).lease
        assertFalse(runCatching { old.useToken { it } }.isSuccess)
        assertTrue(runCatching { fresh.useToken { it } }.isSuccess)
        fresh.close()
    }

    @Test fun googleDenialBlocksTokenSupply() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        fake.denyNext(AuthorizationFailure.DENIED)
        assertEquals(AuthorizationFailure.DENIED,
            (fake.acquire(CALENDAR, setOf(CALENDAR_EVENTS_READ_ONLY)) as TokenSupplyResult.Unavailable).failure)
    }

    @Test fun consentRegistryRejectsOverlapAndStaleResultsAndAllowsCancellation() {
        val registry = ConsentRequestRegistry()
        val gmail = ConsentRequestRegistry.Request("gmail-1", setOf(GMAIL_READ_ONLY), emptySet(), null)
        assertTrue(registry.register(gmail))
        assertFalse(registry.register(ConsentRequestRegistry.Request("calendar-1", setOf(CALENDAR_EVENTS_READ_ONLY), emptySet(), null)))
        assertEquals(null, registry.complete("calendar-1"))
        assertEquals(null, registry.takeIntent("calendar-1"))
        assertEquals(null, registry.takeIntent("gmail-1"))
        assertEquals(null, registry.takeIntent("gmail-1"))
        assertEquals(null, registry.complete("gmail-1"))
        assertTrue(registry.cancel("gmail-1"))
        assertEquals(null, registry.complete("gmail-1"))

        val calendar = ConsentRequestRegistry.Request("calendar-2", setOf(CALENDAR_EVENTS_READ_ONLY), emptySet(), null)
        assertTrue(registry.register(calendar))
        assertTrue(registry.cancel("calendar-2"))
        assertFalse(registry.cancel("calendar-2"))
        assertEquals(null, registry.complete("calendar-2"))
    }

    @Test fun realOAuthWithoutOwnerConfigurationRemainsDisabled() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        fake.setEnabled(false)
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED), fake.authorize(both))
        assertEquals(AuthorizationFailure.NOT_CONFIGURED, fake.status(both).failure)
    }
}
