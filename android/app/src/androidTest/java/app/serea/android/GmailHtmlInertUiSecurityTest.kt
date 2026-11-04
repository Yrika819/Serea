package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.gmail.FakeGmailProvider
import app.serea.android.gmail.GmailMessageSource
import app.serea.android.gmail.GmailMimeParser
import app.serea.android.gmail.GmailProviderSessions
import app.serea.android.gmail.ui.GmailScreen
import app.serea.android.gmail.ui.GmailViewModel
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.nio.charset.StandardCharsets
import java.util.Base64
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
class GmailHtmlInertUiSecurityTest {
    @get:Rule val compose = createComposeRule()

    @Test fun fakeGmailRendersAdversarialHtmlAsInertTextWithoutRemoteRequests() {
        val server = MockWebServer().apply { start() }
        val session = try {
            val imageUrl = server.url("/tracking-pixel.gif")
            val scriptUrl = server.url("/remote-script.js")
            val html = """
                <html><body>
                  <p>SAFE_VISIBLE: this synthetic message is inert.</p>
                  <img src="$imageUrl" alt="tracking pixel" onerror="fetch('$scriptUrl?event=error')">
                  <script src="$scriptUrl"></script>
                  <script>fetch('$imageUrl?event=inline'); document.cookie='injected=yes';</script>
                  <p>SAFE_TAIL: message text only.</p>
                </body></html>
            """.trimIndent()
            val htmlBytes = html.toByteArray(StandardCharsets.UTF_8)
            val encodedBody = Base64.getUrlEncoder().withoutPadding().encodeToString(htmlBytes)
            val rawMessage = """
                {"id":"html-security-001","threadId":"html-security-thread-001","snippet":"Synthetic HTML fixture",
                 "payload":{"mimeType":"text/html","headers":[
                   {"name":"From","value":"fixture@example.invalid"},
                   {"name":"Subject","value":"Adversarial HTML fixture"}],
                   "body":{"size":${htmlBytes.size},"data":"$encodedBody"}}}
            """.trimIndent()

            val parsed = GmailMimeParser.parseMessage(rawMessage, GmailMessageSource.MOCK)
            val safeBody = requireNotNull(parsed.htmlTextBody)
            assertTrue(safeBody.contains("SAFE_VISIBLE: this synthetic message is inert."))
            assertTrue(safeBody.contains("SAFE_TAIL: message text only."))
            assertFalse(safeBody.contains(imageUrl.toString()))
            assertFalse(safeBody.contains(scriptUrl.toString()))
            assertFalse(safeBody.contains("fetch("))
            assertFalse(safeBody.contains("document.cookie"))
            assertFalse(safeBody.contains("<"))

            GmailProviderSessions(
                fakeProvider = {
                    FakeGmailProvider(messages = listOf(parsed), inboxIds = setOf(parsed.id))
                },
            ).openFake()
        } catch (failure: Throwable) {
            server.shutdown()
            throw failure
        }

        try {
            val viewModel = GmailViewModel(session, session)
            compose.setContent {
                SereaTheme {
                    GmailScreen(viewModel = viewModel, onBack = {})
                }
            }

            compose.waitUntil(5_000) {
                compose.onAllNodesWithText("Adversarial HTML fixture").fetchSemanticsNodes().isNotEmpty()
            }
            compose.onNodeWithText("Adversarial HTML fixture").performClick()
            compose.onNodeWithText("SAFE_VISIBLE: this synthetic message is inert.", substring = true)
                .assertIsDisplayed()
            compose.onNodeWithText("SAFE_TAIL: message text only.", substring = true)
                .assertIsDisplayed()
            assertTrue(compose.onAllNodesWithText("tracking pixel").fetchSemanticsNodes().isEmpty())
            assertTrue(compose.onAllNodesWithText("fetch(").fetchSemanticsNodes().isEmpty())
            assertTrue(compose.onAllNodesWithText("document.cookie").fetchSemanticsNodes().isEmpty())
            assertTrue(compose.onAllNodesWithText("<script").fetchSemanticsNodes().isEmpty())

            // Give any accidental WebView/resource loader a bounded window to reach the
            // loopback server. Compose Text should leave it untouched.
            assertNull("Gmail rendered a remote resource from message content", server.takeRequest(2, TimeUnit.SECONDS))
            assertEquals(0, server.requestCount)
        } finally {
            session.close()
            server.shutdown()
        }
    }
}
