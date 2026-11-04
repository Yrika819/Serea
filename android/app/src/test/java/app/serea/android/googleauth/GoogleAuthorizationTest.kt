package app.serea.android.googleauth

import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.gmail.GmailPermissionPolicy
import app.serea.android.googleauth.GoogleScope.CALENDAR_EVENTS_READ_ONLY
import app.serea.android.googleauth.GoogleScope.CALENDAR_LIST_READ_ONLY
import app.serea.android.googleauth.GoogleScope.GMAIL_READ_ONLY
import app.serea.android.googleauth.GoogleFeature.CALENDAR
import app.serea.android.googleauth.GoogleFeature.GMAIL
import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
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

    @Test fun revokeStartInvalidatesLeaseAndPermissionsBeforeRemoteWorkAndPreservesAppDisable() {
        val knownPermissions = mutableMapOf(
            GMAIL_READ_ONLY to ScopePermission.GRANTED,
            CALENDAR_EVENTS_READ_ONLY to ScopePermission.GRANTED,
        )
        val grantState = GoogleGrantLeaseState()
        val gmailVersions = grantState.leaseVersions(setOf(GMAIL))
        val calendarVersions = grantState.leaseVersions(setOf(CALENDAR))
        val gmailLease = AccessTokenLease("fixture-gmail", setOf(GMAIL_READ_ONLY), null,
            validityCheck = { grantState.isLeaseCurrent(gmailVersions) })
        val calendarLease = AccessTokenLease("fixture-calendar", setOf(CALENDAR_EVENTS_READ_ONLY), null,
            validityCheck = { grantState.isLeaseCurrent(calendarVersions) })

        // The real gateway calls this same helper synchronously before awaiting Play Services.
        grantState.disableInApp(GMAIL)
        invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)

        assertTrue(knownPermissions.isEmpty())
        assertEquals(setOf(GMAIL), grantState.disabledFeaturesSnapshot())
        assertEquals(2L, grantState.version(GMAIL))
        assertEquals(1L, grantState.version(CALENDAR))
        assertFalse(runCatching { gmailLease.useToken { it } }.isSuccess)
        assertFalse(runCatching { calendarLease.useToken { it } }.isSuccess)

        // Only an explicit app-local reconnect clears the disable gate.
        grantState.enableInApp(GMAIL)
        assertTrue(grantState.disabledFeaturesSnapshot().isEmpty())
        assertEquals(3L, grantState.version(GMAIL))
        assertFalse(runCatching { gmailLease.useToken { it } }.isSuccess)
    }

    @Test fun cancelledRevokeCallerKeepsAuthorizationBlockedUntilRemoteTaskIsTerminal() = runBlocking {
        val lock = Any()
        val grantState = GoogleGrantLeaseState()
        val knownPermissions = mutableMapOf(GMAIL_READ_ONLY to ScopePermission.GRANTED)
        val oldLeaseVersions = grantState.leaseVersions(setOf(GMAIL))
        val oldLease = AccessTokenLease(
            "fixture-gmail-before-revoke",
            setOf(GMAIL_READ_ONLY),
            null,
            validityCheck = { grantState.isLeaseCurrent(oldLeaseVersions) },
        )
        synchronized(lock) {
            assertTrue(grantState.tryBeginAuthorizationOperation())
            invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)
        }

        val remoteTaskStarted = CompletableDeferred<Unit>()
        val remoteTask = CompletableDeferred<Unit>()
        val revokeCaller = launch {
            try {
                awaitGoogleIdentityTaskToTerminal {
                    remoteTaskStarted.complete(Unit)
                    remoteTask.await()
                }
            } finally {
                synchronized(lock) { grantState.finishAuthorizationOperation() }
            }
        }
        remoteTaskStarted.await()

        revokeCaller.cancel()
        yield()

        assertFalse("caller cancellation must wait for the remote revoke task's terminal state", revokeCaller.isCompleted)
        assertTrue(grantState.isAuthorizationOperationInFlight())
        assertFalse("a new authorization reservation is blocked while revoke is unresolved",
            grantState.tryBeginAuthorizationOperation())
        assertTrue("shared permissions were cleared at revoke initiation", knownPermissions.isEmpty())
        assertFalse("the pre-revoke lease stays invalid during the pending task",
            runCatching { oldLease.useToken { it } }.isSuccess)

        remoteTask.complete(Unit)
        revokeCaller.join()

        assertTrue(revokeCaller.isCancelled)
        assertFalse("the gate opens only after remote task completion", grantState.isAuthorizationOperationInFlight())
        assertTrue("a fresh authorization can reserve the gate after revoke is terminal",
            grantState.tryBeginAuthorizationOperation())
        grantState.finishAuthorizationOperation()
        assertTrue("remote completion must not restore the old grant", knownPermissions.isEmpty())
        assertFalse("remote completion must not revive the old lease", runCatching { oldLease.useToken { it } }.isSuccess)
    }

    @Test fun cancelledRevokeCallerPropagatesCancellationAfterRemoteFailureBecomesTerminal() = runBlocking {
        val grantState = GoogleGrantLeaseState()
        val knownPermissions = mutableMapOf(GMAIL_READ_ONLY to ScopePermission.GRANTED)
        val oldLeaseVersions = grantState.leaseVersions(setOf(GMAIL))
        val oldLease = AccessTokenLease(
            "fixture-gmail-before-failed-revoke",
            setOf(GMAIL_READ_ONLY),
            null,
            validityCheck = { grantState.isLeaseCurrent(oldLeaseVersions) },
        )
        assertTrue(grantState.tryBeginAuthorizationOperation())
        invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)

        val remoteTaskStarted = CompletableDeferred<Unit>()
        val remoteTask = CompletableDeferred<Unit>()
        var classifiedFailure: AuthorizationFailure? = null
        val revokeCaller = launch {
            try {
                try {
                    awaitGoogleIdentityTaskToTerminal {
                        remoteTaskStarted.complete(Unit)
                        remoteTask.await()
                    }
                    currentCoroutineContext().ensureActive()
                } catch (cancelled: kotlinx.coroutines.CancellationException) {
                    throw cancelled
                } catch (exception: Exception) {
                    currentCoroutineContext().ensureActive()
                    classifiedFailure = if (exception is IOException) {
                        AuthorizationFailure.NETWORK
                    } else {
                        AuthorizationFailure.UNAVAILABLE
                    }
                }
            } finally {
                grantState.finishAuthorizationOperation()
            }
        }
        remoteTaskStarted.await()

        revokeCaller.cancel()
        yield()

        assertFalse("caller cancellation waits for the failed remote revoke task to become terminal",
            revokeCaller.isCompleted)
        assertTrue(grantState.isAuthorizationOperationInFlight())
        assertFalse("a new operation remains blocked while remote revoke is unresolved",
            grantState.tryBeginAuthorizationOperation())
        assertTrue("permissions remain cleared while revoke is pending", knownPermissions.isEmpty())
        assertFalse("the old lease remains invalid while revoke is pending",
            runCatching { oldLease.useToken { it } }.isSuccess)

        remoteTask.completeExceptionally(IOException("synthetic remote revoke failure"))
        revokeCaller.join()

        assertTrue("cancellation wins over classifying the late task failure", revokeCaller.isCancelled)
        assertNull("a canceled revoke caller does not receive NETWORK/UNAVAILABLE", classifiedFailure)
        assertFalse("the gate releases after the remote task is terminal", grantState.isAuthorizationOperationInFlight())
        assertTrue("a fresh operation may begin after terminal completion",
            grantState.tryBeginAuthorizationOperation())
        grantState.finishAuthorizationOperation()
        assertTrue("remote failure never restores revoked permissions", knownPermissions.isEmpty())
        assertFalse("remote failure never revives the invalidated lease",
            runCatching { oldLease.useToken { it } }.isSuccess)
    }

    @Test fun cancelledAuthorizationWaitsForIdentityTaskBeforeReleasingSharedGate() = runBlocking {
        val grantState = GoogleGrantLeaseState()
        assertTrue(grantState.tryBeginAuthorizationOperation())
        val remoteTaskStarted = CompletableDeferred<Unit>()
        val remoteAuthorizationTask = CompletableDeferred<String>()
        var authorizationResultApplied = false

        val authorizeCaller = launch {
            try {
                val result = awaitGoogleIdentityTaskToTerminal {
                    remoteTaskStarted.complete(Unit)
                    remoteAuthorizationTask.await()
                }
                currentCoroutineContext().ensureActive()
                // This stands for mapping/caching the returned Google token into a lease.
                authorizationResultApplied = result.isNotEmpty()
            } finally {
                grantState.finishAuthorizationOperation()
            }
        }
        remoteTaskStarted.await()

        authorizeCaller.cancel()
        yield()

        assertFalse("cancelled authorization must remain pending while its Play Services task is pending",
            authorizeCaller.isCompleted)
        assertTrue(grantState.isAuthorizationOperationInFlight())
        assertFalse("a revoke or second authorization cannot claim the shared gate",
            grantState.tryBeginAuthorizationOperation())
        assertFalse("a late task result is not mapped while the caller is cancelled", authorizationResultApplied)

        remoteAuthorizationTask.complete("synthetic-result")
        authorizeCaller.join()

        assertTrue(authorizeCaller.isCancelled)
        assertFalse(grantState.isAuthorizationOperationInFlight())
        assertFalse("cancellation prevents storing or delivering the task result", authorizationResultApplied)
        assertTrue("a new operation may begin only after the task is terminal",
            grantState.tryBeginAuthorizationOperation())
        grantState.finishAuthorizationOperation()
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

    @Test fun concurrentAuthorizationOperationsShareASingleFlightReservation() {
        val grantState = GoogleGrantLeaseState()
        val consentLock = Any()
        val ready = CountDownLatch(2)
        val startTogether = CountDownLatch(1)
        val executor = Executors.newFixedThreadPool(2)

        try {
            val attempts = (1..2).map {
                executor.submit<Boolean> {
                    ready.countDown()
                    check(startTogether.await(5, TimeUnit.SECONDS))
                    synchronized(consentLock) { grantState.tryBeginAuthorizationOperation() }
                }
            }
            assertTrue("both contenders must be ready before the gate opens", ready.await(5, TimeUnit.SECONDS))
            startTogether.countDown()

            val reservations = attempts.map { it.get(5, TimeUnit.SECONDS) }
            assertEquals("only one concurrent authorization/revoke may reserve the shared operation gate",
                1, reservations.count { it })
            synchronized(consentLock) {
                assertTrue(grantState.isAuthorizationOperationInFlight())
                grantState.finishAuthorizationOperation()
                assertFalse(grantState.isAuthorizationOperationInFlight())
            }
        } finally {
            startTogether.countDown()
            synchronized(consentLock) {
                if (grantState.isAuthorizationOperationInFlight()) grantState.finishAuthorizationOperation()
            }
            executor.shutdownNow()
        }
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

    @Test fun disableThenReenableRejectsLateAuthorizationTaskResult() = runBlocking {
        val grantState = GoogleGrantLeaseState()
        val scopes = setOf(GMAIL_READ_ONLY)
        val featureVersions = grantState.leaseVersions(googleFeaturesForScopes(scopes))
        val remoteResult = CompletableDeferred<AccessTokenLease>()
        var resultApplied = false
        val authorization = async {
            val lease = remoteResult.await()
            val stale = googleAuthorizationOperationFailure(grantState, scopes, featureVersions)
            if (stale != null) {
                lease.close()
                AuthorizationOutcome.Rejected(stale)
            } else {
                resultApplied = true
                AuthorizationOutcome.Authorized(lease)
            }
        }

        grantState.disableInApp(GMAIL)
        grantState.enableInApp(GMAIL)
        val lateLease = AccessTokenLease("fixture-late-authorization", scopes, null)
        remoteResult.complete(lateLease)

        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT), authorization.await())
        assertFalse("the old generation cannot publish its authorization result", resultApplied)
        assertFalse("the stale result lease is closed", runCatching { lateLease.useToken { it } }.isSuccess)
    }

    @Test fun disablingFeatureCancelsItsPendingConsentAcrossReconnect() {
        val registry = ConsentRequestRegistry()
        val grantState = GoogleGrantLeaseState()
        val scopes = setOf(GMAIL_READ_ONLY)
        val featureVersions = grantState.leaseVersions(googleFeaturesForScopes(scopes))
        val request = ConsentRequestRegistry.Request(
            id = "gmail-consent-generation-1",
            scopes = scopes,
            grantedScopes = emptySet(),
            pendingIntent = null,
            launched = true,
            featureVersions = featureVersions,
        )
        assertTrue(registry.register(request))

        grantState.disableInApp(GMAIL)
        assertTrue(registry.cancelFeature(GMAIL))
        grantState.enableInApp(GMAIL)

        assertNull("a late result from the cancelled consent handoff is stale", registry.complete(request.id))
    }

    @Test fun realOAuthWithoutOwnerConfigurationRemainsDisabled() = runBlocking {
        val fake = FakeGoogleAuthorizationGateway(both)
        fake.setEnabled(false)
        assertEquals(AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED), fake.authorize(both))
        assertEquals(AuthorizationFailure.NOT_CONFIGURED, fake.status(both).failure)
    }
}
