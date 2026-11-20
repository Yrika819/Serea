package app.serea.android.googleauth

import app.serea.android.calendar.CalendarDateRange
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarReadFailure
import app.serea.android.BuildConfig
import kotlinx.coroutines.test.runTest
import okhttp3.OkHttpClient
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant
import java.util.concurrent.TimeUnit

class PrivateGoogleCalendarCompositionTest {
    @Test
    fun isolatedBuildKeepsProductHttpGateOff() {
        assertEquals("isolated", BuildConfig.BUILD_TYPE)
        assertFalse(BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED)
    }

    private class Gateway(
        override val mode: GoogleAuthorizationMode = GoogleAuthorizationMode.PLAY_SERVICES,
        var sessionId: String? = "synthetic-session",
        var granted: Boolean = true,
        var enabled: Boolean = true,
        var missingScope: Boolean = false,
    ) : GoogleAuthorizationGateway {
        override val authorizationSessionId get() = sessionId
        val statusScopes = mutableListOf<Set<GoogleScope>>()
        val tokenScopes = mutableListOf<Set<GoogleScope>>()
        override fun isEnabledInApp(feature: GoogleFeature) = enabled
        override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
            statusScopes += scopes
            return AuthorizationStatus(GoogleServiceState.AVAILABLE,
                if (missingScope) emptyMap() else scopes.associateWith { if (granted) ScopePermission.GRANTED else ScopePermission.EXPIRED })
        }
        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            assertEquals(GoogleFeature.CALENDAR, feature)
            tokenScopes += scopes
            return TokenSupplyResult.Available(AccessTokenLease("synthetic-token", scopes, null, sessionId))
        }
    }

    @Test fun liveCompositionRejectsMissingOrProductionEndpointBeforeAuthorization() {
        listOf<okhttp3.HttpUrl?>(
            null,
            "https://www.googleapis.com/calendar/v3/".toHttpUrl(),
        ).forEach { endpoint ->
            val gateway = Gateway()
            val failure = runCatching {
                IsolatedGoogleCalendarTestComposition(
                    client = OkHttpClient(),
                    baseUrl = endpoint,
                ).use { it.openAuthorizedPrimary(gateway) }
            }.exceptionOrNull()

            assertTrue("Live composition must require an explicit loopback endpoint", failure is IllegalStateException)
            assertTrue(gateway.statusScopes.isEmpty())
            assertTrue(gateway.tokenScopes.isEmpty())
        }
    }

    @Test fun explicitTestGatesSelectGoogleAndUseExactPrimaryScope() = runTest {
        MockWebServer().use { server ->
            server.start()
            server.enqueue(MockResponse().setBody("{\"items\":[]}"))
            IsolatedGoogleCalendarTestComposition(OkHttpClient(), server.url("/calendar/v3/")).use { composition ->
                val gateway = Gateway()
                assertFalse(BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED)
                val session = composition.openAuthorizedPrimary(gateway)
                assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, session.kind)
                assertTrue(session.primaryCalendarOnly)
                assertTrue(session.listEventsPage(CalendarId("primary"), range(), null).events.isEmpty())
                assertFalse(gateway.statusScopes.isEmpty())
                assertTrue(gateway.statusScopes.all { it == PrivatePrimaryCalendarScopeContract.scopes })
                assertEquals(listOf(PrivatePrimaryCalendarScopeContract.scopes), gateway.tokenScopes)
                val request = checkNotNull(server.takeRequest(5, TimeUnit.SECONDS))
                assertTrue(request.path!!.startsWith("/calendar/v3/calendars/primary/events"))
            }
        }
    }

    @Test fun liveRejectsMockMissingBlankAndDisabledSessionsBeforeStatusTokenOrHttp() {
        MockWebServer().use { server ->
            server.start()
            listOf(Gateway(GoogleAuthorizationMode.MOCK_ONLY), Gateway(sessionId = null),
                Gateway(sessionId = " "), Gateway(enabled = false)).forEach { gateway ->
                IsolatedGoogleCalendarTestComposition(OkHttpClient(), server.url("/calendar/v3/")).use {
                    assertTrue(runCatching { it.openAuthorizedPrimary(gateway) }.isFailure)
                    assertTrue(gateway.statusScopes.isEmpty())
                    assertTrue(gateway.tokenScopes.isEmpty())
                }
            }
            assertEquals(0, server.requestCount)
        }
    }

    @Test fun expiredScopeFailsReadablyWithoutTokenOrHttp() {
        MockWebServer().use { server ->
            server.start()
            val gateway = Gateway(granted = false)
            IsolatedGoogleCalendarTestComposition(OkHttpClient(), server.url("/calendar/v3/")).use {
                val failure = runCatching { it.openAuthorizedPrimary(gateway) }.exceptionOrNull()
                assertTrue(failure is IllegalStateException)
                assertTrue(failure!!.message!!.contains("read-only grant"))
                assertEquals(listOf(PrivatePrimaryCalendarScopeContract.scopes), gateway.statusScopes)
                assertTrue(gateway.tokenScopes.isEmpty())
                assertEquals(0, server.requestCount)
            }
        }
    }

    @Test fun googleTransportFailureIsNotReplacedWithFakeAndCloseInvalidatesSession() = runTest {
        MockWebServer().use { server ->
            server.start()
            server.enqueue(MockResponse().setResponseCode(503))
            val composition = IsolatedGoogleCalendarTestComposition(OkHttpClient(), server.url("/calendar/v3/"))
            val session = composition.openAuthorizedPrimary(Gateway())
            assertTrue(runCatching { session.listEventsPage(CalendarId("primary"), range(), null) }
                .exceptionOrNull() is CalendarReadFailure)
            assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, session.kind)
            composition.close()
            assertTrue(runCatching { session.status() }.isFailure)
            assertEquals(1, server.requestCount)
        }
    }

    @Test fun missingRequiredScopeFailsReadablyBeforeTokenOrHttp() {
        MockWebServer().use { server ->
            server.start()
            val gateway = Gateway(missingScope = true)
            IsolatedGoogleCalendarTestComposition(OkHttpClient(), server.url("/calendar/v3/")).use {
                val failure = runCatching { it.openAuthorizedPrimary(gateway) }.exceptionOrNull()
                assertTrue(failure is IllegalStateException)
                assertTrue(failure!!.message!!.contains("read-only grant"))
                assertEquals(listOf(PrivatePrimaryCalendarScopeContract.scopes), gateway.statusScopes)
                assertTrue(gateway.tokenScopes.isEmpty())
                assertEquals(0, server.requestCount)
            }
        }
    }

    @Test fun staleSelectedSessionCannotAcquireTokenOrSendHttp() = runTest {
        MockWebServer().use { server ->
            server.start()
            val gateway = Gateway()
            IsolatedGoogleCalendarTestComposition(OkHttpClient(), server.url("/calendar/v3/")).use { composition ->
                val session = composition.openAuthorizedPrimary(gateway)
                gateway.sessionId = "replacement-synthetic-session"
                assertTrue(runCatching { session.listEventsPage(CalendarId("primary"), range(), null) }.isFailure)
                assertTrue(gateway.tokenScopes.isEmpty())
                assertEquals(0, server.requestCount)
            }
        }
    }

    @Test fun sessionChangingDuringStatusIsRejectedBeforeConstruction() {
        MockWebServer().use { server ->
            server.start()
            val gateway = Gateway()
            val changing = object : GoogleAuthorizationGateway by gateway {
                override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
                    val result = gateway.status(scopes)
                    gateway.sessionId = "replacement-synthetic-session"
                    return result
                }
            }
            IsolatedGoogleCalendarTestComposition(OkHttpClient(), server.url("/calendar/v3/")).use {
                assertTrue(runCatching { it.openAuthorizedPrimary(changing) }.isFailure)
                assertTrue(gateway.tokenScopes.isEmpty())
                assertEquals(0, server.requestCount)
            }
        }
    }

    @Test fun providerCreatedAcrossSessionChangeIsClosedBeforeReturning() = runTest {
        val gateway = Gateway()
        var sessionReads = 0
        val switchingGateway = object : GoogleAuthorizationGateway by gateway {
            override val authorizationSessionId: String?
                get() = if (++sessionReads >= 2) "replacement-synthetic-session" else "synthetic-session"
        }
        val sessions = app.serea.android.calendar.CalendarProviderSessions(googleHttpEnabledByOwner = true)
        var created: app.serea.android.calendar.CalendarProviderSession? = null

        val failure = runCatching {
            openGoogleSessionBoundToAuthorization(switchingGateway, "synthetic-session") {
                sessions.openPrimaryGoogleReadOnly(switchingGateway).also { created = it }
            }
        }.exceptionOrNull()

        assertTrue(failure is IllegalStateException)
        val createdSession = checkNotNull(created)
        assertTrue(runCatching { createdSession.status() }.isFailure)
        assertTrue(gateway.tokenScopes.isEmpty())
    }

    @Test fun sessionChangingDuringAdapterConstructionIsRejectedBeforeTokenOrHttp() {
        MockWebServer().use { server ->
            server.start()
            val gateway = Gateway()
            var sessionReads = 0
            val switchingGateway = object : GoogleAuthorizationGateway by gateway {
                override val authorizationSessionId: String?
                    get() = if (++sessionReads >= 4) "replacement-synthetic-session" else "synthetic-session"
            }

            IsolatedGoogleCalendarTestComposition(
                client = OkHttpClient(),
                baseUrl = server.url("/calendar/v3/"),
            ).use { composition ->
                val failure = runCatching { composition.openAuthorizedPrimary(switchingGateway) }.exceptionOrNull()

                assertTrue("A session switch during adapter construction must fail closed", failure is IllegalStateException)
                assertEquals(1, gateway.statusScopes.size)
                assertTrue(gateway.tokenScopes.isEmpty())
                assertEquals(0, server.requestCount)
            }
        }
    }

    @Test fun canonicalIpv6LoopbackEndpointIsAccepted() {
        val endpoint = "http://[::1]:8080/calendar/v3/".toHttpUrl()
        IsolatedGoogleCalendarTestComposition(
            client = OkHttpClient(),
            baseUrl = endpoint,
        ).use { composition ->
            assertFalse(BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED)
        }
    }

    private fun range() = CalendarDateRange(Instant.parse("2026-11-18T00:00:00Z"), Instant.parse("2026-11-19T00:00:00Z"))
}
