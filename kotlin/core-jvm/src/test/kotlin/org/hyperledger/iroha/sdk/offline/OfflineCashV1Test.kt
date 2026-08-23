package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class OfflineCashV1Test {
    @Test
    fun `release probe is dual-identity and fail closed without a published release`() {
        val status = OfflineCashReleaseStatusV1.installed()
        assertFalse(status.available)
        assertNull(status.installedReleaseId)
        assertNull(status.installedArtifactManifestSHA256)
        assertTrue(status.blocker?.startsWith("offline-cash-v1-") == true)
    }

    @Test
    fun `public constants freeze the exact first release transport caps`() {
        assertEquals(22, OfflineCashReleaseStatusV1.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(768, OfflineCashPaymentRequestV1.MAX_CANONICAL_BYTES)
        assertEquals(7_936, OfflineCashPaymentV1.MAX_CANONICAL_BYTES)
        assertEquals(256, OfflineCashAcknowledgementV1.MAX_CANONICAL_BYTES)
        assertEquals("kgm2:", OfflineCashPeerAdapterV1.TEXT_PREFIX)
        assertEquals(12_288, OfflineCashPeerAdapterV1.MAX_TEXT_SESSION_BYTES)
    }
}
