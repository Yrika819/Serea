package app.serea.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SereaThemeTest {
    @get:Rule val compose = createComposeRule()

    @Test fun darkModeSelectsMaterialDarkColorScheme() {
        compose.setContent {
            SereaTheme(darkTheme = true) {
                Box(
                    Modifier.size(48.dp)
                        .background(MaterialTheme.colorScheme.background)
                        .testTag("theme_swatch"),
                )
            }
        }
        val center = compose.onNodeWithTag("theme_swatch").captureToImage().toPixelMap()[24, 24]
        assertEquals(darkColorScheme().background, center)
    }

    @Test fun lightModeSelectsMaterialLightColorScheme() {
        compose.setContent {
            SereaTheme(darkTheme = false) {
                Box(
                    Modifier.size(48.dp)
                        .background(MaterialTheme.colorScheme.background)
                        .testTag("theme_swatch"),
                )
            }
        }
        val center = compose.onNodeWithTag("theme_swatch").captureToImage().toPixelMap()[24, 24]
        assertEquals(lightColorScheme().background, center)
    }
}
