package app.serea.android.calendar

import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationFailure
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.FakeGoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleGrantInvalidationObservers
import app.serea.android.googleauth.GoogleGrantLeaseState
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
import app.serea.android.googleauth.awaitGoogleIdentityTaskToTerminal
import app.serea.android.googleauth.invalidateGoogleGrantAtRevokeStart
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.yield
import okhttp3.OkHttpClient
import okhttp3.Call
import okhttp3.EventListener
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.TimeUnit
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicInteger

class CalendarProviderSessionsTest {
    private val server = MockWebServer().apply { start() }

    private enum class RevokeOutcome { SUCCESS, FAILURE, CANCELLATION }

    private class GatedResponseDispatcher(private val body: String) : Dispatcher() {
        val requestArrived = CountDownLatch(1)
        val releaseResponse = CountDownLatch(1)
        @Volatile var recordedRequest: RecordedRequest? = null

        override fun dispatch(request: RecordedRequest): MockResponse {
            recordedRequest = request
            requestArrived.countDown()
            return if (releaseResponse.await(30, TimeUnit.SECONDS)) {
                MockResponse().setBody(body)
            } else {
                MockResponse().setResponseCode(500).setBody("{}")
            }
        }
    }

    private class CancellationProbe : EventListener() {
        val callCancelled = CountDownLatch(1)
        override fun canceled(call: Call) { callCancelled.countDown() }
    }

    @After fun close() = server.shutdown()

    private class PlayServicesFixture(
        granted: Set<GoogleScope> = setOf(
            GoogleScope.CALENDAR_EVENTS_READ_ONLY,
            GoogleScope.CALENDAR_LIST_READ_ONLY,
        ),
        permission: ScopePermission = ScopePermission.GRANTED,
        private val notifyInvalidationObservers: Boolean = true,
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override var authorizationSessionId: String? = "calendar-session-fixture"
        private val invalidationObservers = GoogleGrantInvalidationObservers()
        var tokenRequests = 0
        var rotateAfterNextToken = false
        private var accountChanged = false
        private val stateLock = Any()
        private val grantState = GoogleGrantLeaseState()
        private val knownPermissions = mutableMapOf<GoogleScope, ScopePermission>().apply {
            granted.forEach { put(it, permission) }
        }
        lateinit var lastLease: AccessTokenLease
        var lastLeaseIdentity: Int = -1
        var capturedCalendarVersion: Long = -1
        var calendarVersionAtRevoke: Long = -1
        val validityTrace = CopyOnWriteArrayList<Pair<Long, Boolean>>()

        private fun beginRevoke() {
            synchronized(stateLock) {
                check(grantState.tryBeginAuthorizationOperation())
                invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)
                calendarVersionAtRevoke = grantState.version(GoogleFeature.CALENDAR)
            }
            if (notifyInvalidationObservers) invalidationObservers.notifyInvalidationStarted()
        }
        override fun observeGoogleGrantInvalidation(listener: () -> Unit): AutoCloseable =
            invalidationObservers.observe(listener)
        fun disableCalendar() {
            synchronized(stateLock) { grantState.disableInApp(GoogleFeature.CALENDAR) }
        }
        suspend fun revokeWithRemote(remoteRevoke: suspend () -> Unit) {
            beginRevoke()
            try {
                awaitGoogleIdentityTaskToTerminal(remoteRevoke)
            } finally {
                synchronized(stateLock) { grantState.finishAuthorizationOperation() }
            }
        }
        fun authorizationOperationInFlight(): Boolean = synchronized(stateLock) {
            grantState.isAuthorizationOperationInFlight()
        }
        fun tryBeginAuthorizationOperation(): Boolean = synchronized(stateLock) {
            grantState.tryBeginAuthorizationOperation()
        }
        fun finishAuthorizationOperation() = synchronized(stateLock) { grantState.finishAuthorizationOperation() }

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            if (feature != GoogleFeature.CALENDAR) {
                return TokenSupplyResult.UserActionRequired(scopes - knownPermissions.keys)
            }
            if (!grantState.isEnabledInApp(GoogleFeature.CALENDAR)) return TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED)
            val hasGrant = synchronized(stateLock) {
                if (grantState.isAuthorizationOperationInFlight()) {
                    return TokenSupplyResult.Unavailable(AuthorizationFailure.REQUEST_IN_PROGRESS)
                }
                knownPermissions.keys.containsAll(scopes)
            }
            if (!hasGrant) {
                return TokenSupplyResult.UserActionRequired(scopes - knownPermissions.keys)
            }
            tokenRequests++
            val boundSession = authorizationSessionId
            val boundVersions = synchronized(stateLock) { grantState.leaseVersions(setOf(GoogleFeature.CALENDAR)) }
            capturedCalendarVersion = boundVersions.getValue(GoogleFeature.CALENDAR)
            val lease = AccessTokenLease(
                "fixture-never-valid",
                synchronized(stateLock) { knownPermissions.keys.toSet() },
                null,
                authorizationSessionId = boundSession,
                validityCheck = {
                    val current = synchronized(stateLock) { grantState.isLeaseCurrent(boundVersions) }
                    validityTrace += grantState.version(GoogleFeature.CALENDAR) to current
                    current
                },
            )
            lastLease = lease
            lastLeaseIdentity = System.identityHashCode(lease)
            if (rotateAfterNextToken) {
                rotateAfterNextToken = false
                authorizationSessionId = "calendar-session-second"
                accountChanged = true
            }
            return TokenSupplyResult.Available(lease)
        }
        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { scope -> synchronized(stateLock) { knownPermissions[scope] } ?: ScopePermission.NEEDS_CONSENT },
            when {
                accountChanged -> AuthorizationFailure.ACCOUNT_CHANGED
                scopes.any { synchronized(stateLock) { knownPermissions[it] == ScopePermission.DENIED } } -> AuthorizationFailure.DENIED
                else -> null
            },
        )
    }

    @Test fun defaultAndMockAuthorizationStayOnFakeProvider() = runBlocking {
        val disabled = CalendarProviderSessions(googleHttpEnabledByOwner = false)
        val default = disabled.openDefault()
        assertEquals(CalendarProviderKind.FAKE, default.kind)
        assertEquals(CalendarAuthorizationState.MOCK_ONLY, default.status().state)

        val mockGateway = FakeGoogleAuthorizationGateway(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY))
        val alsoFake = CalendarProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(mockGateway, baseUrl = server.url("/calendar/v3/"))
        assertEquals(CalendarProviderKind.FAKE, alsoFake.kind)
        assertEquals(0, server.requestCount)
        assertTrue(alsoFake.listCalendars().isNotEmpty())
    }

    @Test fun ownerEnabledPlayServicesSessionUsesOnlyMockServerAndRevokesOldSession() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"items":[{"id":"fixture@example.test","timeZone":"UTC","accessRole":"reader"}]}"""))
        val sessions = CalendarProviderSessions(googleHttpEnabledByOwner = true)
        val first = sessions.openGoogleReadOnly(
            PlayServicesFixture(),
            OkHttpClient(),
            server.url("/calendar/v3/"),
        )
        assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, first.kind)
        assertEquals("fixture@example.test", first.listCalendars().single().id.value)
        assertEquals("Bearer fixture-never-valid", server.takeRequest().getHeader("Authorization"))

        val replacement = sessions.openFake()
        assertEquals(CalendarProviderKind.FAKE, replacement.kind)
        val failure = runCatching { first.listCalendars() }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, (failure as CalendarReadFailure).code)
        assertEquals(1, server.requestCount)
        assertFalse(replacement.status().state == CalendarAuthorizationState.AUTHORIZED)
    }

    @Test fun deniedAndExpiredAuthorizationDoNotReachTheHttpServer() = runBlocking {
        val sessions = CalendarProviderSessions(googleHttpEnabledByOwner = true)
        for (gateway in listOf(
            PlayServicesFixture(permission = ScopePermission.DENIED),
            object : GoogleAuthorizationGateway {
                override val mode = GoogleAuthorizationMode.PLAY_SERVICES
                override val authorizationSessionId = "expired-calendar-session-fixture"
                override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>) =
                    TokenSupplyResult.Unavailable(AuthorizationFailure.EXPIRED)
                override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                    GoogleServiceState.AVAILABLE,
                    scopes.associateWith { ScopePermission.EXPIRED },
                    AuthorizationFailure.EXPIRED,
                )
            },
        )) {
            val session = sessions.openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))
            try {
                session.getEvent(CalendarEventKey(CalendarId("fixture@example.test"), EventId("fixture")))
                throw AssertionError("authorization must fail closed")
            } catch (failure: CalendarReadFailure) {
                assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, failure.code)
            }
            assertEquals(0, server.requestCount)
        }
    }

    @Test fun accountSwitchDuringTokenLeaseAcquisitionCannotDispatchACalendarRequest() = runBlocking {
        val gateway = PlayServicesFixture().apply { rotateAfterNextToken = true }
        val session = CalendarProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))

        val failure = runCatching { session.listCalendars() }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertEquals(CalendarAuthorizationState.REVOKED, session.status().state)
        assertEquals(1, gateway.tokenRequests)
        assertEquals(0, server.requestCount)
    }

    @Test fun delayedCalendarResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome() = runBlocking {
        suspend fun assertInvalidatedRead(outcome: RevokeOutcome) {
            val gateway = PlayServicesFixture(notifyInvalidationObservers = false)
            val previousRequestCount = server.requestCount
            val response = GatedResponseDispatcher("""{"id":"fixture@example.test","timeZone":"UTC","accessRole":"reader"}""")
            server.dispatcher = response
            val session = CalendarProviderSessions(googleHttpEnabledByOwner = true)
                .openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))
            val read = async { runCatching { session.getCalendar(CalendarId("fixture@example.test")) } }

            assertTrue(withContext(Dispatchers.IO) { response.requestArrived.await(5, TimeUnit.SECONDS) })
            val request = requireNotNull(response.recordedRequest)
            assertEquals("GET", request.method)
            assertEquals("Bearer fixture-never-valid", request.getHeader("Authorization"))
            assertFalse("the delayed response body is still pending at request receipt", read.isCompleted)
            assertEquals(1, gateway.tokenRequests)
            assertTrue(gateway.lastLeaseIdentity != -1)

            try {
                val remoteRevokeStarted = CompletableDeferred<Unit>()
                val remoteRevokeGate = CompletableDeferred<Unit>()
                val revoke = async {
                    try {
                        gateway.revokeWithRemote {
                            remoteRevokeStarted.complete(Unit)
                            remoteRevokeGate.await()
                        }
                        null
                    } catch (cancelled: kotlinx.coroutines.CancellationException) {
                        throw cancelled
                    } catch (exception: Exception) {
                        exception
                    }
                }
                remoteRevokeStarted.await()

                assertFalse("the lease is invalid before remote revoke completes", runCatching {
                    gateway.lastLease.useToken { it }
                }.isSuccess)
                assertEquals(ScopePermission.NEEDS_CONSENT,
                    gateway.status(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY])
                assertEquals(gateway.capturedCalendarVersion + 1, gateway.calendarVersionAtRevoke)
                assertFalse("the dispatcher has not released the HTTP response", read.isCompleted)
                val checksBeforeResponseBodyCompletion = gateway.validityTrace.size

                // Release the real local HTTP response while the synthetic remote revoke remains gated.
                response.releaseResponse.countDown()
                val readOutcome = read.await()
                assertTrue(
                    "response must be denied by the active lease callback; identity=${gateway.lastLeaseIdentity}, " +
                        "capturedVersion=${gateway.capturedCalendarVersion}, revokedVersion=${gateway.calendarVersionAtRevoke}, " +
                        "checks=${gateway.validityTrace.toList()}, tokenRequests=${gateway.tokenRequests}, outcome=${readOutcome.exceptionOrNull()}",
                    readOutcome.exceptionOrNull() is CalendarReadFailure,
                )
                assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, (readOutcome.exceptionOrNull() as CalendarReadFailure).code)
                assertTrue("response-body validation must observe the invalidated production lease state",
                    gateway.validityTrace.drop(checksBeforeResponseBodyCompletion)
                        .any { (version, valid) -> version == gateway.calendarVersionAtRevoke && !valid })
                assertFalse("remote revoke must still be gated while the read is rejected", revoke.isCompleted)

                when (outcome) {
                    RevokeOutcome.SUCCESS -> {
                        remoteRevokeGate.complete(Unit)
                        assertEquals(null, revoke.await())
                    }
                    RevokeOutcome.FAILURE -> {
                        remoteRevokeGate.completeExceptionally(IllegalStateException("synthetic revoke failure"))
                        assertTrue(revoke.await() is IllegalStateException)
                    }
                    RevokeOutcome.CANCELLATION -> {
                        revoke.cancel()
                        yield()
                        assertFalse("caller cancellation must not complete before the remote revoke task", revoke.isCompleted)
                        assertTrue(gateway.authorizationOperationInFlight())
                        assertFalse("new authorization is blocked until remote revoke is terminal",
                            gateway.tryBeginAuthorizationOperation())
                        assertEquals(
                            TokenSupplyResult.Unavailable(AuthorizationFailure.REQUEST_IN_PROGRESS),
                            gateway.acquire(GoogleFeature.CALENDAR, setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)),
                        )
                        remoteRevokeGate.complete(Unit)
                        revoke.join()
                        assertFalse(gateway.authorizationOperationInFlight())
                        assertTrue("new authorization may start after the remote task is terminal",
                            gateway.tryBeginAuthorizationOperation())
                        gateway.finishAuthorizationOperation()
                        assertTrue(gateway.acquire(GoogleFeature.CALENDAR, setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY))
                            is TokenSupplyResult.UserActionRequired)
                    }
                }

                assertFalse("no remote outcome restores the invalidated lease", runCatching {
                    gateway.lastLease.useToken { it }
                }.isSuccess)
                assertEquals(ScopePermission.NEEDS_CONSENT,
                    gateway.status(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY])
                assertEquals(1, gateway.tokenRequests)
                assertEquals(previousRequestCount + 1, server.requestCount)
            } finally {
                response.releaseResponse.countDown()
            }
        }

        RevokeOutcome.entries.forEach { assertInvalidatedRead(it) }
    }

    @Test fun grantRevocationCancelsPendingCalendarHttpCallBeforeResponseBodyDelivery() = runBlocking {
        val response = GatedResponseDispatcher(
            """{"id":"fixture@example.test","timeZone":"UTC","accessRole":"reader"}""",
        )
        server.dispatcher = response
        val cancellationProbe = CancellationProbe()
        val gateway = PlayServicesFixture()
        val session = CalendarProviderSessions(googleHttpEnabledByOwner = true).openGoogleReadOnly(
            gateway,
            OkHttpClient.Builder().eventListenerFactory { cancellationProbe }.build(),
            server.url("/calendar/v3/"),
        )
        val read = async { runCatching { session.getCalendar(CalendarId("fixture@example.test")) } }
        val remoteRevokeStarted = CompletableDeferred<Unit>()
        val remoteRevokeGate = CompletableDeferred<Unit>()
        var revoke: kotlinx.coroutines.Deferred<Unit>? = null

        try {
            assertTrue(withContext(Dispatchers.IO) { response.requestArrived.await(5, TimeUnit.SECONDS) })
            assertEquals("GET", requireNotNull(response.recordedRequest).method)
            assertFalse("the response body has not been released", read.isCompleted)

            revoke = async {
                gateway.revokeWithRemote {
                    remoteRevokeStarted.complete(Unit)
                    remoteRevokeGate.await()
                }
            }
            remoteRevokeStarted.await()
            assertTrue(withContext(Dispatchers.IO) { cancellationProbe.callCancelled.await(5, TimeUnit.SECONDS) })
            val outcome = read.await()
            assertFalse("revoked data must not escape", outcome.isSuccess)
            assertTrue("the provider read is cancelled", outcome.exceptionOrNull() is kotlinx.coroutines.CancellationException)
            assertEquals("the server is still holding the response before body delivery", 1L, response.releaseResponse.count)
            assertFalse("revocation remains pending while the request has already been cancelled", revoke.isCompleted)
        } finally {
            remoteRevokeGate.complete(Unit)
            response.releaseResponse.countDown()
            revoke?.join()
        }
        assertEquals(1, server.requestCount)
    }

    @Test fun delayedCalendarResponseIsDeniedAfterAppLocalDisable() = runBlocking {
        val gateway = PlayServicesFixture()
        val response = GatedResponseDispatcher("""{"id":"fixture@example.test","timeZone":"UTC","accessRole":"reader"}""")
        server.dispatcher = response
        val session = CalendarProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))
        val read = async { calendarFailure { session.getCalendar(CalendarId("fixture@example.test")) } }
        assertTrue(withContext(Dispatchers.IO) { response.requestArrived.await(5, TimeUnit.SECONDS) })
        assertFalse("the dispatcher is holding the response before app-local disable", read.isCompleted)

        try {
            gateway.disableCalendar()
            assertFalse("the existing lease is invalid before the response is released",
                runCatching { gateway.lastLease.useToken { it } }.isSuccess)
            response.releaseResponse.countDown()

            assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, read.await().code)
            assertTrue("post-body validity check observes the disabled feature",
                gateway.validityTrace.any { (_, current) -> !current })
            assertFalse(runCatching { gateway.lastLease.useToken { it } }.isSuccess)
        } finally {
            response.releaseResponse.countDown()
        }
    }

    @Test fun replacingSessionImmediatelyAfterLeaseAcquisitionPreventsCalendarHttpDispatch() = runBlocking {
        lateinit var sessions: CalendarProviderSessions
        var replacement: CalendarProviderSession? = null
        val gateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId = "calendar-session-bound-to-first-account"
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                val lease = AccessTokenLease(
                    "fixture-calendar-token-before-replacement",
                    scopes,
                    null,
                    authorizationSessionId = authorizationSessionId,
                    validityCheck = {
                        replacement = sessions.openFake()
                        true
                    },
                )
                return TokenSupplyResult.Available(lease)
            }
        }
        sessions = CalendarProviderSessions(googleHttpEnabledByOwner = true)
        val oldSession = sessions.openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))

        val failure = runCatching { oldSession.getCalendar(CalendarId("fixture@example.test")) }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertTrue(currentCoroutineContext().isActive)
        assertEquals(CalendarProviderKind.FAKE, replacement?.kind)
        assertEquals(0, server.requestCount)
    }

    @Test fun unknownAccountBindingIsNotAuthorizedAndCannotReachCalendarHttp() = runBlocking {
        var tokenRequests = 0
        val unboundGateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId: String? = null
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                tokenRequests++
                return TokenSupplyResult.Available(AccessTokenLease("must-not-be-used", scopes, null))
            }
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
        }
        val session = CalendarProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(unboundGateway, baseUrl = server.url("/calendar/v3/"))

        assertEquals(CalendarAuthorizationState.NOT_CONNECTED, session.status().state)
        val failure = runCatching {
            session.listEventsPage(CalendarId("fixture@example.test"), range())
        }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertEquals(0, tokenRequests)
        assertEquals(0, server.requestCount)
    }

    @Test fun replacingSessionCancelsItsReadWithoutCancellingParentOrSiblingCommunication() = runBlocking {
        val started = Channel<Unit>(Channel.UNLIMITED)
        val pendingResponse = CompletableDeferred<List<CalendarDescriptor>>()
        val delegate = object : CalendarReadPort {
            override suspend fun listCalendars(): List<CalendarDescriptor> {
                started.send(Unit)
                return pendingResponse.await()
            }
            override suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor = error("not used")
            override suspend fun getEvent(key: CalendarEventKey): CalendarEvent = error("not used")
            override suspend fun listEventsPage(
                calendarId: CalendarId,
                range: CalendarDateRange,
                pageToken: String?,
            ): CalendarEventPage = error("not used")
        }
        val session = CalendarProviderSession(
            id = 8,
            kind = CalendarProviderKind.FAKE,
            delegate = delegate,
            authorizationDelegate = FakeCalendarProvider(),
            isCurrent = { true },
            onClose = {},
        )
        val unrelatedResult = CompletableDeferred<String>()
        val caller = launch {
            coroutineScope {
                val unrelated = async { unrelatedResult.await() }
                val read = async { calendarFailure { session.listCalendars() }.code }

                started.receive()
                session.invalidateFromController()

                assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, read.await())
                assertTrue(currentCoroutineContext().isActive)
                assertTrue("unrelated sibling communication must remain active", unrelated.isActive)
                unrelatedResult.complete("unrelated communication survived")
                assertEquals("unrelated communication survived", unrelated.await())
            }
        }

        caller.join()
        assertTrue("provider replacement must not cancel the caller job", caller.isCompleted && !caller.isCancelled)
        assertEquals(0, server.requestCount)
    }

    @Test fun grantRevocationBetweenDelegateCompletionAndDeliveryRejectsCalendarResult() = runBlocking {
        val gateway = PlayServicesFixture()
        val delegate = FakeCalendarProvider()
        val isCurrentCalls = AtomicInteger()
        val postDelegateCheckEntered = CountDownLatch(1)
        val releasePostDelegateCheck = CountDownLatch(1)
        val session = CalendarProviderSession(
            id = 9,
            kind = CalendarProviderKind.GOOGLE_READ_ONLY,
            delegate = delegate,
            authorizationDelegate = FakeCalendarProvider(),
            isCurrent = {
                if (isCurrentCalls.incrementAndGet() == 2) {
                    postDelegateCheckEntered.countDown()
                    check(releasePostDelegateCheck.await(5, TimeUnit.SECONDS))
                }
                true
            },
            onClose = {},
        )
        session.attachGrantInvalidationObserver(gateway.observeGoogleGrantInvalidation {
            session.cancelInFlightReadsForGrantInvalidation()
        })
        val read = async(Dispatchers.Default) { runCatching { session.listCalendars() } }
        val remoteRevokeStarted = CompletableDeferred<Unit>()
        val remoteRevokeGate = CompletableDeferred<Unit>()
        var revoke: kotlinx.coroutines.Deferred<Unit>? = null

        try {
            assertTrue(withContext(Dispatchers.IO) { postDelegateCheckEntered.await(5, TimeUnit.SECONDS) })
            revoke = async {
                gateway.revokeWithRemote {
                    remoteRevokeStarted.complete(Unit)
                    remoteRevokeGate.await()
                }
            }
            remoteRevokeStarted.await()
            releasePostDelegateCheck.countDown()

            val outcome = read.await()
            assertFalse("a result buffered before revoke must not escape", outcome.isSuccess)
            assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, (outcome.exceptionOrNull() as CalendarReadFailure).code)
            assertFalse("remote revoke remains pending while the delivery gate rejects the result", revoke.isCompleted)
        } finally {
            releasePostDelegateCheck.countDown()
            remoteRevokeGate.complete(Unit)
            revoke?.join()
            session.close()
        }
    }

    private suspend fun calendarFailure(block: suspend () -> Any): CalendarReadFailure = try {
        block()
        throw AssertionError("Expected CalendarReadFailure")
    } catch (failure: CalendarReadFailure) {
        failure
    }

    private fun range() = CalendarDateRange(
        java.time.Instant.parse("2026-10-20T00:00:00Z"),
        java.time.Instant.parse("2026-10-21T00:00:00Z"),
    )
}
