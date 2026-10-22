package app.serea.android.calendar

import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationFailure
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.FakeGoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarProviderSessionsTest {
    private val server = MockWebServer().apply { start() }

    @After fun close() = server.shutdown()

    private class PlayServicesFixture(
        private val granted: Set<GoogleScope> = setOf(
            GoogleScope.CALENDAR_EVENTS_READ_ONLY,
            GoogleScope.CALENDAR_LIST_READ_ONLY,
        ),
        private val permission: ScopePermission = ScopePermission.GRANTED,
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            if (feature != GoogleFeature.CALENDAR || !granted.containsAll(scopes)) {
                return TokenSupplyResult.UserActionRequired(scopes - granted)
            }
            return TokenSupplyResult.Available(AccessTokenLease("fixture-never-valid", granted, null))
        }
        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { scope -> if (scope in granted) permission else ScopePermission.NEEDS_CONSENT },
            if (permission == ScopePermission.DENIED) AuthorizationFailure.DENIED else null,
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
}
