package app.serea.android.googleauth

import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.gmail.GmailPermissionPolicy
import app.serea.android.googleauth.GoogleScope.CALENDAR_EVENTS_READ_ONLY
import app.serea.android.googleauth.GoogleScope.CALENDAR_LIST_READ_ONLY
import app.serea.android.googleauth.GoogleScope.GMAIL_READ_ONLY
import app.serea.android.googleauth.GoogleFeature.CALENDAR
import app.serea.android.googleauth.GoogleFeature.GMAIL
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class GoogleAuthorizationTest {
    private val both = setOf(GMAIL_READ_ONLY, CALENDAR_EVENTS_READ_ONLY)

    @Test fun googleScopesMatchTheGmailAndCalendarDomainPolicies() {
        assertEquals(setOf(GMAIL_READ_ONLY.value), GmailPermissionPolicy.requiredScopes())
        assertEquals(
            setOf(CALENDAR_EVENTS_READ_ONLY.value),
            CalendarPermissionPolicy.requiredScopes(listCalendars = false),
        )
        assertEquals(
            setOf(CALENDAR_EVENTS_READ_ONLY.value, CALENDAR_LIST_READ_ONLY.value),
            CalendarPermissionPolicy.requiredScopes(listCalendars = true),
        )
    }

    @Test fun calendarBridgeKeepsListScopeOptionalAndNeverTreatsMockAsGoogleAuthorization() = runBlocking {
        val mock = FakeGoogleAuthorizationGateway(setOf(CALENDAR_EVENTS_READ_ONLY, CALENDAR_LIST_READ_ONLY))
        val mockAdapter = GoogleCalendarAuthorizationAdapter(mock)
        assertEquals(CalendarAuthorizationState.MOCK_ONLY, mockAdapter.status().state)
        assertNull(mockAdapter.accessToken(CalendarPermissionPolicy.EVENTS_READ_ONLY))
        assertNull(mockAdapter.accessToken(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY))

        val requestedScopes = mutableListOf<Set<GoogleScope>>()
        val leases = mutableListOf<AccessTokenLease>()
        val gateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId = "calendar-fixture-session"
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                assertEquals(GoogleFeature.CALENDAR, feature)
                requestedScopes += scopes
                val lease = AccessTokenLease("fixture-calendar-token", scopes, null,
                    authorizationSessionId = authorizationSessionId)
                leases += lease
                return TokenSupplyResult.Available(lease)
            }
        }
        val adapter = GoogleCalendarAuthorizationAdapter(gateway)
        assertEquals(CalendarAuthorizationState.AUTHORIZED, adapter.status().state)
        val eventsToken = checkNotNull(adapter.accessToken(CalendarPermissionPolicy.EVENTS_READ_ONLY))
        val calendarListToken = checkNotNull(adapter.accessToken(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY))
        eventsToken.close()
        calendarListToken.close()
        assertEquals(
            listOf(setOf(CALENDAR_EVENTS_READ_ONLY), setOf(CALENDAR_LIST_READ_ONLY)),
            requestedScopes,
        )
        assertTrue(leases.all { lease -> runCatching { lease.useToken { it } }.isFailure })
    }

    @Test fun featureDisableIsVisibleAndGmailDisableLeavesCalendarGrantUsable() = runBlocking {
        val disabled = mutableSetOf<GoogleFeature>()
        var tokenRequests = 0
        val gateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId = "calendar-disable-fixture-session"
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
            override fun isEnabledInApp(feature: GoogleFeature) = feature !in disabled
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                tokenRequests++
                if (!isEnabledInApp(feature)) return TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED)
                return TokenSupplyResult.Available(AccessTokenLease("fixture-calendar-token", scopes, null,
                    authorizationSessionId = authorizationSessionId))
            }
        }
        val adapter = GoogleCalendarAuthorizationAdapter(gateway)

        disabled += GMAIL
        assertEquals(CalendarAuthorizationState.AUTHORIZED, adapter.status().state)
        val calendarToken = checkNotNull(adapter.accessToken(CalendarPermissionPolicy.EVENTS_READ_ONLY))
        calendarToken.useToken { assertEquals("fixture-calendar-token", it) }
        calendarToken.close()
        assertEquals(1, tokenRequests)

        disabled += CALENDAR
        val disabledStatus = adapter.status()
        assertEquals(CalendarAuthorizationState.FEATURE_DISABLED, disabledStatus.state)
        assertEquals(ScopePermission.GRANTED,
            gateway.status(setOf(CALENDAR_EVENTS_READ_ONLY)).permissions[CALENDAR_EVENTS_READ_ONLY])
        assertNull(adapter.accessToken(CalendarPermissionPolicy.EVENTS_READ_ONLY))
        assertEquals(1, tokenRequests)
    }

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
        val authorized = fake.authorize(both) as AuthorizationOutcome.Authorized
        val firstSession = fake.authorizationSessionId
        fake.switchAccount()
        assertTrue(firstSession != fake.authorizationSessionId)
        assertTrue(runCatching { authorized.lease.useToken { it } }.isFailure)
        assertEquals(AuthorizationFailure.ACCOUNT_CHANGED, fake.status(both).failure)
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
        val calendarLease = (fake.acquire(CALENDAR, setOf(CALENDAR_EVENTS_READ_ONLY)) as TokenSupplyResult.Available).lease
        fake.disableInApp(GMAIL)
        assertFalse(fake.isEnabledInApp(GMAIL))
        assertTrue(fake.isEnabledInApp(CALENDAR))
        assertEquals(ScopePermission.GRANTED, fake.status(both).permissions[GMAIL_READ_ONLY])
        assertEquals(AuthorizationFailure.FEATURE_DISABLED,
            (fake.acquire(GMAIL, setOf(GMAIL_READ_ONLY)) as TokenSupplyResult.Unavailable).failure)
        assertFalse(runCatching { oldGmailLease.useToken { it } }.isSuccess)
        assertTrue("disabling Gmail must not invalidate Calendar's lease", runCatching { calendarLease.useToken { it } }.isSuccess)
        val calendar = fake.acquire(CALENDAR, setOf(CALENDAR_EVENTS_READ_ONLY))
        assertTrue(calendar is TokenSupplyResult.Available)
        (calendar as TokenSupplyResult.Available).lease.close()
        assertEquals(0, fake.revokeCount)
        fake.enableInApp(GMAIL)
        assertTrue(fake.isEnabledInApp(GMAIL))
        assertFalse(runCatching { oldGmailLease.useToken { it } }.isSuccess)

        val freshGmailLease = (fake.acquire(GMAIL, setOf(GMAIL_READ_ONLY)) as TokenSupplyResult.Available).lease
        fake.disableInApp(CALENDAR)
        assertTrue("disabling Calendar must not invalidate Gmail's lease", runCatching { freshGmailLease.useToken { it } }.isSuccess)
        assertFalse(runCatching { calendarLease.useToken { it } }.isSuccess)
        freshGmailLease.close()
        calendarLease.close()
    }

    @Test fun googleGrantRevocationRemovesAllServiceScopes() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both + CALENDAR_LIST_READ_ONLY)
        fake.revokeGoogleGrant()
        assertEquals(1, fake.revokeCount)
        assertEquals(AuthorizationOutcome.ConsentRequired(both), fake.authorize(both))
    }

    @Test fun appDisableDuringSuccessfulSharedRevokePersistsUntilExplicitReconnect() {
        val knownPermissions = mutableMapOf(
            GMAIL_READ_ONLY to ScopePermission.GRANTED,
            CALENDAR_EVENTS_READ_ONLY to ScopePermission.GRANTED,
        )
        val disabledFeatures = mutableSetOf<GoogleFeature>()
        val versions = GoogleFeature.entries.associateWith { 0L }.toMutableMap()

        // Model a revoke request awaiting Play Services while the user disables Gmail.
        disabledFeatures += GMAIL
        versions[GMAIL] = versions.getValue(GMAIL) + 1
        applySuccessfulGoogleGrantRevocation(knownPermissions, versions)

        assertTrue(knownPermissions.isEmpty())
        assertEquals(setOf(GMAIL), disabledFeatures)
        assertEquals(2L, versions.getValue(GMAIL))
        assertEquals(1L, versions.getValue(CALENDAR))

        // Only an explicit app-local reconnect clears the disable gate.
        disabledFeatures -= GMAIL
        versions[GMAIL] = versions.getValue(GMAIL) + 1
        assertTrue(disabledFeatures.isEmpty())
        assertEquals(3L, versions.getValue(GMAIL))
    }

    @Test fun fakeRevokeClearsScopesAndInvalidatesLeasesButPreservesDisabledFeature() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        val gmailLease = (fake.authorize(setOf(GMAIL_READ_ONLY)) as AuthorizationOutcome.Authorized).lease
        val calendarLease = (fake.authorize(setOf(CALENDAR_EVENTS_READ_ONLY)) as AuthorizationOutcome.Authorized).lease

        fake.disableInApp(GMAIL)
        assertFalse(runCatching { gmailLease.useToken { it } }.isSuccess)
        fake.revokeGoogleGrant()

        assertFalse(fake.isEnabledInApp(GMAIL))
        assertFalse(runCatching { calendarLease.useToken { it } }.isSuccess)
        assertEquals(
            AuthorizationOutcome.ConsentRequired(setOf(CALENDAR_EVENTS_READ_ONLY)),
            fake.authorize(setOf(CALENDAR_EVENTS_READ_ONLY)),
        )
        fake.enableInApp(GMAIL)
        assertTrue(fake.isEnabledInApp(GMAIL))
        assertEquals(
            AuthorizationOutcome.ConsentRequired(setOf(GMAIL_READ_ONLY)),
            fake.authorize(setOf(GMAIL_READ_ONLY)),
        )
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
        assertFalse(runCatching { lease.ensureValid() }.isSuccess)
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

    @Test fun googleReconsentResolutionReplacesStaleGrantedPermission() {
        val known = mutableMapOf(
            GMAIL_READ_ONLY to ScopePermission.GRANTED,
            CALENDAR_EVENTS_READ_ONLY to ScopePermission.GRANTED,
        )
        recordConsentResolution(known, setOf(GMAIL_READ_ONLY), emptySet())
        assertEquals(ScopePermission.NEEDS_CONSENT, known[GMAIL_READ_ONLY])
        assertEquals(ScopePermission.GRANTED, known[CALENDAR_EVENTS_READ_ONLY])

        recordConsentResolution(known, setOf(GMAIL_READ_ONLY, CALENDAR_EVENTS_READ_ONLY), setOf(GMAIL_READ_ONLY))
        assertEquals(ScopePermission.GRANTED, known[GMAIL_READ_ONLY])
        assertEquals(ScopePermission.NEEDS_CONSENT, known[CALENDAR_EVENTS_READ_ONLY])
    }

    @Test fun explicitConsentCannotRequestScopesForAnInAppDisabledFeature() {
        assertTrue(requestedScopesTouchDisabledFeature(setOf(GMAIL_READ_ONLY), setOf(GMAIL)))
        assertFalse(requestedScopesTouchDisabledFeature(setOf(CALENDAR_EVENTS_READ_ONLY), setOf(GMAIL)))
        assertTrue(requestedScopesTouchDisabledFeature(both, setOf(GMAIL)))
    }

    @Test fun realOAuthWithoutOwnerConfigurationRemainsDisabled() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        fake.setEnabled(false)
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED), fake.authorize(both))
        assertEquals(AuthorizationFailure.NOT_CONFIGURED, fake.status(both).failure)
    }
}
