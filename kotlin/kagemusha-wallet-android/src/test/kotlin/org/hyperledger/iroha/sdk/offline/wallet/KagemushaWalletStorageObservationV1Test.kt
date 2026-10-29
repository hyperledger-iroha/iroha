// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.*
import org.junit.jupiter.api.Test
import java.nio.ByteBuffer

class KagemushaWalletStorageObservationV1Test {
    @Test fun `storage observations omit the actual custody path and preserve the native answer`() {
        val path = "/private/application/custody-secret".toByteArray()
        val reply = KagemushaWalletNativeReplyV1(0, bytes = path)
        val messages = mutableListOf<String>()
        assertSame(reply, WalletStorageObservationV1(messages::add).reply(9, reply))
        assertEquals(listOf("operation=custody_root status=0 reason=4 platform_code=0"), messages)
        assertFalse(messages.single().contains("private"))
        assertContentEquals(path, reply.bytes())
    }

    @Test fun `key and attestation operations publish no observation`() {
        val messages = mutableListOf<String>()
        val reply = KagemushaWalletNativeReplyV1(2, reason = 3, code = 13,
            bytes = "sensitive payload".toByteArray())
        val observer = WalletStorageObservationV1(messages::add)
        for (operation in listOf(0, 1, 2, 3, 10, 11, 12, 13, -1, Int.MAX_VALUE)) {
            assertSame(reply, observer.reply(operation, reply))
        }
        assertTrue(messages.isEmpty())
    }

    @Test fun `unavailable storage retains its reason and platform code even if diagnostics fail`() {
        val reply = KagemushaWalletNativeReplyV1.unavailable(KagemushaWalletAndroidUnavailableV1.io(13))
        val messages = mutableListOf<String>()
        assertSame(reply, WalletStorageObservationV1(messages::add).reply(7, reply))
        assertEquals(listOf("operation=storage_state status=2 reason=3 platform_code=13"), messages)
        assertSame(reply, WalletStorageObservationV1 { error("log sink unavailable") }.reply(7, reply))
        assertEquals(2, reply.tag)
        assertEquals(3, reply.reason)
        assertEquals(13, reply.code)
    }

    @Test fun `each platform records each success once and every unavailable answer`() {
        val messages = mutableListOf<String>()
        val observer = WalletStorageObservationV1(messages::add)
        val present = KagemushaWalletNativeReplyV1(0)
        val unavailable = KagemushaWalletNativeReplyV1.unavailable(KagemushaWalletAndroidUnavailableV1.BUSY)
        repeat(100) {
            assertSame(present, observer.reply(7, present))
            assertSame(present, observer.reply(9, present))
        }
        repeat(3) { assertSame(unavailable, observer.reply(7, unavailable)) }
        assertEquals(2, messages.count { "status=0" in it })
        assertEquals(3, messages.count { "status=2" in it })
        WalletStorageObservationV1(messages::add).reply(7, present)
        assertEquals(3, messages.count { "status=0" in it })
    }

    @Test fun `preparation failure publishes only its closed category and does not propagate sink failure`() {
        val messages = mutableListOf<String>()
        val reason = KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE)
        observeWalletStoragePreparationFailureV1(WalletStoragePreparationFailureV1.PLATFORM, reason, publish = messages::add)
        assertEquals(listOf("operation=custody_root_prepare status=2 reason=4 platform_code=8 failure=PLATFORM"), messages)
        for (failure in WalletStoragePreparationFailureV1.values()) {
            observeWalletStoragePreparationFailureV1(failure, reason) { error("log sink unavailable") }
        }
    }

    @Test fun `witness comparisons expose only field bits and malformed shape remains a refusal`() {
        fun binding(device: Long = 7, inode: Long = 20, uid: Int = 10384, gid: Int = 10384) =
            ByteBuffer.allocate(32).put("KGMROOT1".toByteArray(Charsets.US_ASCII))
                .putLong(device).putLong(inode).putInt(uid).putInt(gid).array()
        val original = binding()
        assertEquals(0, walletStorageWitnessMismatchV1(original, original.copyOf()))
        assertEquals(1, walletStorageWitnessMismatchV1(original, binding(device = 8)))
        assertEquals(2, walletStorageWitnessMismatchV1(original, binding(inode = 21)))
        assertEquals(4, walletStorageWitnessMismatchV1(original, binding(uid = 10385)))
        assertEquals(8, walletStorageWitnessMismatchV1(original, binding(gid = 10385)))
        assertEquals(15, walletStorageWitnessMismatchV1(original, binding(8, 21, 10385, 10385)))
        assertEquals(16, walletStorageWitnessMismatchV1(original.copyOf(31), original))
        assertEquals(16, walletStorageWitnessMismatchV1(original.copyOf().also { it[0] = 0 }, original))
        val messages = mutableListOf<String>()
        val reason = KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE)
        observeWalletStoragePreparationFailureV1(WalletStoragePreparationFailureV1.WITNESS_BINDING,
            reason, 1, messages::add)
        assertEquals(listOf("operation=custody_root_prepare status=2 reason=4 platform_code=8 failure=WITNESS_BINDING witness_mismatch=1"),
            messages)
        assertFalse(messages.single().contains("KGMROOT1"))
        assertFalse(messages.single().contains("10384"))
    }
}
