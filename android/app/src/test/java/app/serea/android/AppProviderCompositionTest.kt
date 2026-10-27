package app.serea.android

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailReadFailure
import app.serea.android.gmail.GmailReadFailureCode
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

class AppProviderCompositionTest {
    @Test fun providerSessionsFollowTheConfigurationRetainedViewModelStoreLifetime() = runTest {
        val store = ViewModelStore()
        val factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T =
                AppProviderCompositionViewModel() as T
        }

        val firstActivityViewModel = ViewModelProvider(store, factory)[AppProviderCompositionViewModel::class.java]
        val firstProviders = firstActivityViewModel.providers
        assertEquals(GmailAuthorizationState.MOCK_ONLY, firstProviders.gmail.status().state)

        // A configuration replacement resolves the same retained ViewModel and keeps its ports live.
        val recreatedActivityViewModel = ViewModelProvider(store, factory)[AppProviderCompositionViewModel::class.java]
        assertSame(firstActivityViewModel, recreatedActivityViewModel)
        assertEquals(GmailAuthorizationState.MOCK_ONLY, recreatedActivityViewModel.providers.gmail.status().state)

        store.clear()
        val closedSessionFailure = try {
            firstProviders.gmail.status()
            null
        } catch (failure: GmailReadFailure) {
            failure
        }
        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, closedSessionFailure?.code)
    }
}
