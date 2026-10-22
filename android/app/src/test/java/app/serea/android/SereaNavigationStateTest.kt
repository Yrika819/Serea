package app.serea.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SereaNavigationStateTest {
    @Test fun allSixDestinationsAreRegisteredExactlyOnce() {
        assertEquals(SereaDestination.entries.toSet(), DestinationContract.all.map { it.destination }.toSet())
        assertEquals(SereaDestination.entries.size, DestinationContract.all.size)
    }

    @Test fun navigationBuildsAndPopsBackStackInOrder() {
        val state = SereaNavigationState()
            .navigate(SereaDestination.GMAIL)
            .navigate(SereaDestination.CALENDAR)
            .navigate(SereaDestination.SETTINGS)

        assertEquals(SereaDestination.SETTINGS, state.current)
        assertEquals(listOf(SereaDestination.CHAT, SereaDestination.GMAIL, SereaDestination.CALENDAR), state.backStack)
        assertEquals(SereaDestination.CALENDAR, state.back().current)
        assertEquals(SereaDestination.GMAIL, state.back().back().current)
        assertEquals(SereaDestination.CHAT, state.back().back().back().current)
        assertFalse(state.back().back().back().canGoBack)
    }

    @Test fun selectingCurrentDestinationDoesNotCreateDuplicateRoute() {
        val state = SereaNavigationState().navigate(SereaDestination.TASKS)
        assertEquals(state, state.navigate(SereaDestination.TASKS))
    }

    @Test fun restorationDropsUnknownRoutesAndBoundsBackStack() {
        val restored = SereaNavigationState.restore(
            "CALENDAR",
            listOf("UNKNOWN") + List(40) { if (it % 2 == 0) "CHAT" else "GMAIL" },
        )

        assertEquals(SereaDestination.CALENDAR, restored.current)
        assertEquals(SereaNavigationState.MAX_BACK_STACK, restored.backStack.size)
        assertTrue(restored.backStack.all { it in setOf(SereaDestination.CHAT, SereaDestination.GMAIL) })
    }

    @Test fun invalidCurrentDestinationFallsBackToChat() {
        assertEquals(SereaDestination.CHAT, SereaNavigationState.restore("SECRET", listOf("TASK_PROPOSAL_DEMO")).current)
    }
}
