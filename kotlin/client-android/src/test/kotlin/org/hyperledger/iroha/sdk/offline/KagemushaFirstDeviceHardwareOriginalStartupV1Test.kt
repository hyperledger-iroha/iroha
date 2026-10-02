// Synthetic pure scheduling tests; no runtime authority or Native/Android device is installed.
package org.hyperledger.iroha.sdk.offline
import org.junit.Assert.*
import org.junit.Test
import java.util.concurrent.Executor

class KagemushaFirstDeviceHardwareOriginalStartupV1Test {
    @Test fun queuedOriginalSurvivesDetachedUiCancellation() {
        val queue=ArrayList<Runnable>();var calls=0
        val owner=KagemushaFirstDeviceHardwareOriginalStartupV1<String>(Executor { queue.add(it) })
        val first=owner.openOriginal { calls++;"actual owned result" };assertTrue(first.cancel(true))
        val second=owner.openOriginal { error("replacement worker forbidden") }
        assertEquals(1,queue.size);assertEquals(0,calls);assertFalse(second.isDone)
        queue.single().run();assertEquals("actual owned result",second.get());assertEquals(1,calls)
    }
    @Test fun failedWorkerRemainsSameOwnedOutcome() {
        var calls=0;val owner=KagemushaFirstDeviceHardwareOriginalStartupV1<Unit>(Executor { it.run() })
        val first=owner.openOriginal { calls++;error("declined actual startup") }
        val second=owner.openOriginal { calls++ }
        assertTrue(first.isCompletedExceptionally);assertTrue(second.isCompletedExceptionally);assertEquals(1,calls)
    }
    @Test fun executorRefusalCannotResetOriginal() {
        var submissions=0;var calls=0
        val owner=KagemushaFirstDeviceHardwareOriginalStartupV1<Unit>(Executor { submissions++;error("worker unavailable") })
        assertTrue(owner.openOriginal { calls++ }.isCompletedExceptionally)
        assertTrue(owner.openOriginal { calls++ }.isCompletedExceptionally)
        assertEquals(1,submissions);assertEquals(0,calls)
    }
    @Test fun completedOriginalIsSharedAcrossFactoryViews() {
        val value=Any();var calls=0;val owner=KagemushaFirstDeviceHardwareOriginalStartupV1<Any>(Executor { it.run() })
        assertSame(value,owner.openOriginal { calls++;value }.get())
        assertSame(value,owner.openOriginal { error("new owner forbidden") }.get());assertEquals(1,calls)
    }
}
