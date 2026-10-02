// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.junit.jupiter.api.Tag
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Real main-wallet JNI owner, with no root registration or scripted endpoint.
 * One ordered case preserves the actual process-local initial-attempt gate.
 * No signer, account, release, clock, key or monetary capability is constructed.
 */
@Tag("host-native")
class KagemushaOrdinaryRuntimeHostNativeV1Test {
    @Test
    fun `actual ABI and absent startup root refuse before later managed Core open`() {
        // Missing library, ABI drift or signer-contract drift must fail this test.
        assertTrue(NativeSignerBridge.isNativeAvailable(), "Actual ABI25 signer-contract7 bridge is required")
        assertEquals(25, NativeSignerBridge.REQUIRED_BRIDGE_ABI_VERSION)
        assertEquals(7, NativeSignerBridge.REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION)

        // The public initial facade probes the actual Core contract, then calls real
        // wallet nativeStartupV1(1,0,empty). No independently registered root exists.
        val initial = assertFailsWith<IllegalStateException> {
            KagemushaOrdinaryRuntimeStartupV1.selectInitialAccount()
        }
        assertEquals("Actual initial ordinary Native account read is unavailable", initial.message)
        assertNull(initial.cause, "Missing JNI symbol or other linkage failure is not an absent-root pass")

        // Actual phase5/id0/empty must likewise fail; this is never a no-op success.
        val revoke = assertFailsWith<IllegalStateException> {
            KagemushaOrdinaryRuntimeLifecycleV1.revokeSelection()
        }
        assertEquals("Actual Native ordinary selection revocation is unavailable", revoke.message)
        assertNull(revoke.cause, "Missing JNI symbol or other linkage failure is not an absent-root pass")

        // Initial failure consumes the private production-entry gate. This assertion
        // is managed denial before Core install/open, not actual Core JNI refusal.
        val core = assertFailsWith<IllegalStateException> {
            KagemushaCoreCoordinatorBridgeV1.open("/host-diagnostic/not-opened")
        }
        assertEquals("Initial ordinary Native selection is incomplete or unavailable", core.message)
        assertNull(core.cause)
    }
}
