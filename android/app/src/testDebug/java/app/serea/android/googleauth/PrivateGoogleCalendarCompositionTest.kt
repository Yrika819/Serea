package app.serea.android.googleauth

import app.serea.android.calendar.CalendarDateRange
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarReadFailure
import kotlinx.coroutines.test.runTest
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

class PrivateGoogleCalendarCompositionTest {
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

    @Test fun productDefaultsOpenOnlyPrimaryFakeForBothGatewayModes() {
        GoogleAuthorizationMode.entries.forEach { mode ->
            PrivateGoogleCalendarComposition().use { composition ->
                val gateway = Gateway(mode)
                assertFalse(composition.opensGoogleProvider)
                val session = composition.openAuthorizedPrimary(gateway)
                assertEquals(CalendarProviderKind.FAKE, session.kind)
                assertTrue(session.primaryCalendarOnly)
                assertEquals(listOf(PrivatePrimaryCalendarScopeContract.scopes), gateway.statusScopes)
                assertTrue(gateway.tokenScopes.isEmpty())
            }
        }
    }

    @Test fun privateGateAndHttpGateAreIndependent() {
        MockWebServer().use { server ->
            server.start()
            listOf(false to true, true to false).forEach { (privateGate, httpGate) ->
                PrivateGoogleCalendarComposition(privateGate, httpGate, OkHttpClient(), server.url("/calendar/v3/")).use {
                    assertFalse(it.opensGoogleProvider)
                    assertEquals(CalendarProviderKind.FAKE, it.openAuthorizedPrimary(Gateway()).kind)
                }
            }
            assertEquals(0, server.requestCount)
        }
    }

    @Test fun explicitTestGatesSelectGoogleAndUseExactPrimaryScope() = runTest {
        MockWebServer().use { server ->
            server.start()
            server.enqueue(MockResponse().setBody("{\"items\":[]}"))
            PrivateGoogleCalendarComposition(true, true, OkHttpClient(), server.url("/calendar/v3/")).use { composition ->
                val gateway = Gateway()
                assertTrue(composition.opensGoogleProvider)
                val session = composition.openAuthorizedPrimary(gateway)
                assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, session.kind)
                assertTrue(session.primaryCalendarOnly)
                assertTrue(session.listEventsPage(CalendarId("primary"), range(), null).events.isEmpty())
                assertTrue(gateway.statusScopes.all { it == PrivatePrimaryCalendarScopeContract.scopes })
                assertEquals(listOf(PrivatePrimaryCalendarScopeContract.scopes), gateway.tokenScopes)
                assertTrue(server.takeRequest().path!!.startsWith("/calendar/v3/calendars/primary/events"))
            }
        }
    }

    @Test fun liveRejectsMockMissingBlankAndDisabledSessionsBeforeStatusTokenOrHttp() {
        MockWebServer().use { server ->
            server.start()
            listOf(Gateway(GoogleAuthorizationMode.MOCK_ONLY), Gateway(sessionId = null),
                Gateway(sessionId = " "), Gateway(enabled = false)).forEach { gateway ->
                PrivateGoogleCalendarComposition(true, true, OkHttpClient(), server.url("/calendar/v3/")).use {
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
            PrivateGoogleCalendarComposition(true, true, OkHttpClient(), server.url("/calendar/v3/")).use {
                val failure = runCatching { it.openAuthorizedPrimary(gateway) }.exceptionOrNull()
                assertTrue(failure is IllegalStateException)
                assertTrue(failure!!.message!!.contains("read-only authorization"))
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
            val composition = PrivateGoogleCalendarComposition(true, true, OkHttpClient(), server.url("/calendar/v3/"))
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
            PrivateGoogleCalendarComposition(true, true, OkHttpClient(), server.url("/calendar/v3/")).use {
                val failure = runCatching { it.openAuthorizedPrimary(gateway) }.exceptionOrNull()
                assertTrue(failure is IllegalStateException)
                assertTrue(failure!!.message!!.contains("read-only authorization"))
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
            PrivateGoogleCalendarComposition(true, true, OkHttpClient(), server.url("/calendar/v3/")).use { composition ->
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
            PrivateGoogleCalendarComposition(true, true, OkHttpClient(), server.url("/calendar/v3/")).use {
                assertTrue(runCatching { it.openAuthorizedPrimary(changing) }.isFailure)
                assertTrue(gateway.tokenScopes.isEmpty())
                assertEquals(0, server.requestCount)
            }
        }
    }

    private fun range() = CalendarDateRange(Instant.parse("2026-11-18T00:00:00Z"), Instant.parse("2026-11-19T00:00:00Z"))
}
