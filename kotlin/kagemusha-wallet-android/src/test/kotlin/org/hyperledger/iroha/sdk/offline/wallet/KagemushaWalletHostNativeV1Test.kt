// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Tag
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

/** Actual host JNI boundary checks; no installed monetary provider or phone qualification. */
@Tag("host-native")
class KagemushaWalletHostNativeV1Test {
    @TempDir lateinit var directory: File

    @BeforeEach
    fun requireCurrentNativeBridge() {
        assertTrue(NativeSignerBridge.isNativeAvailable(), "The supplied ABI-26 native bridge must load")
        assertEquals(1, KagemushaWalletNativeV1.revision())
    }

    @Test
    fun unavailableArtifactsNeverOpenAnOwnerOrMutateCustody() {
        val keys = TestKeyStoreV1()
        val platform = KagemushaWalletAndroidPlatformV1.create(TestEnvironmentV1(directory), keys)
        fun files() = directory.walkTopDown().filter { it.isFile }
            .associate { it.relativeTo(directory).path to it.readBytes().toList() }
        val before = files()
        val probes = keys.getKeyCalls
        val id = ByteArray(32) { 1 }
        repeat(2) {
            val error = assertFailsWith<KagemushaWalletExceptionV1> {
                KagemushaWalletRuntimeV1(Long.MAX_VALUE).begin(KagemushaWalletOpenOriginalsV1(id, id, id, id))
            }
            assertEquals(KagemushaWalletExceptionV1.ARTIFACTS_UNAVAILABLE, error.status)
        }
        assertEquals(before, files())
        assertEquals(probes, keys.getKeyCalls)
        assertTrue(keys.generated.isEmpty())
        assertEquals(0, keys.signCalls)
        assertEquals(0, keys.deleteCalls)
    }

    @Test
    fun unknownHandlesReturnExactFailureObjectsAndNeverCompletionBytes() {
        val id = ByteArray(32) { 1 }
        for (operation in 1..5) {
            val first = if (operation == 1 || operation == 4 || operation == 5) id else byteArrayOf()
            val second = if (operation == 4) id else byteArrayOf()
            val reply = assertNotNull(KagemushaWalletNativeV1.call(0, operation, first, second))
            assertEquals(-2, reply.status)
            assertEquals(-1, reply.reason)
            assertEquals(0, reply.platformCode)
            assertTrue(reply.bytes().isEmpty())
        }
        val execute = assertNotNull(KagemushaWalletNativeV1.execute(0, id, 9, 0, 0, byteArrayOf(), byteArrayOf(), byteArrayOf()))
        assertEquals(-2, execute.status)
        assertTrue(execute.bytes().isEmpty())
        val snapshot = assertNotNull(KagemushaWalletNativeV1.snapshot(0))
        assertEquals(-2, snapshot.status)
        assertEquals(-1, snapshot.reason)
        assertEquals(0, snapshot.platformCode)
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletSnapshotV1(snapshot) }
        assertEquals(-2, KagemushaWalletNativeV1.close(0))
        assertEquals(-2, KagemushaWalletNativeV1.activity(0, 1, 0))
        assertEquals(-1, KagemushaWalletNativeV1.activity(0, 2, 0))
    }

    @Test
    fun malformedNativeCallsRejectBeforeUnknownHandleLookup() {
        val id = ByteArray(32) { 1 }
        val platform = KagemushaWalletAndroidPlatformV1.create(TestEnvironmentV1(directory), TestKeyStoreV1())
        assertEquals(-1, assertNotNull(KagemushaWalletNativeV1.openBegin(0, byteArrayOf(), id, id, id)).status)
        assertEquals(-1, assertNotNull(KagemushaWalletNativeV1.openBegin(0, id, id, ByteArray(4097), id)).status)
        assertEquals(-4, assertNotNull(KagemushaWalletNativeV1.openFinish(0, ByteArray(64))).status)
        assertEquals(-4, KagemushaWalletNativeV1.openCancel(0))
        val cases = listOf(
            Triple(0, byteArrayOf(), byteArrayOf()),
            Triple(1, ByteArray(31), byteArrayOf()),
            Triple(2, byteArrayOf(1), byteArrayOf()),
            Triple(3, byteArrayOf(), byteArrayOf(1)),
            Triple(4, ByteArray(31), id),
            Triple(4, id, ByteArray(31)),
            Triple(99, byteArrayOf(), byteArrayOf()),
        )
        for (selector in 0..10) {
            val reply = assertNotNull(KagemushaWalletNativeV1.execute(0, id, selector, 0, 0,
                ByteArray(2_228_737), byteArrayOf(), byteArrayOf()))
            assertEquals(-1, reply.status)
            assertTrue(reply.bytes().isEmpty())
        }
        for ((operation, first, second) in cases) {
            val reply = assertNotNull(KagemushaWalletNativeV1.call(0, operation, first, second))
            assertEquals(-1, reply.status)
            assertEquals(-1, reply.reason)
            assertEquals(0, reply.platformCode)
            assertTrue(reply.bytes().isEmpty())
        }
    }
    @Test
    fun realReviewJniReturnsSeparateFailuresWithoutAdmittingUnknownOwners() {
        val id = ByteArray(32) { 1 }
        val reply = assertNotNull(KagemushaWalletNativeV1.review(0, 1, 0, 0, byteArrayOf(1), byteArrayOf()))
        assertEquals(-2, reply.status); assertEquals(-1, reply.reason); assertEquals(0, reply.platformCode)
        // Strict negative Review parsing checks every private zero token/high/detail/bytes
        // field. A malformed negative reply yields INVALID_NATIVE_OUTPUT rather than -2.
        assertTrue(reply.cleanupToken()==null)
        val refusal=assertFailsWith<KagemushaWalletExceptionV1> {
            reply.review(Any(),KagemushaWalletReviewProjectionV1.Kind.SEND)
        }
        assertEquals(-2,refusal.status)
        for (selector in listOf(0, 2, 9, 99)) {
            assertEquals(-1, assertNotNull(KagemushaWalletNativeV1.review(0, selector, 0, 0, byteArrayOf(1), byteArrayOf())).status)
        }
        assertEquals(-1, assertNotNull(KagemushaWalletNativeV1.review(0, 1, 1, 0, byteArrayOf(1), byteArrayOf())).status)
        assertEquals(-1, assertNotNull(KagemushaWalletNativeV1.review(0, 1, 0, 0, ByteArray(10_001), byteArrayOf())).status)
        assertEquals(-1, assertNotNull(KagemushaWalletNativeV1.review(0, 8, 1, 0, byteArrayOf(1), byteArrayOf())).status)
        val execution = assertNotNull(KagemushaWalletNativeV1.executeReviewed(0, 1, id))
        assertEquals(-2, execution.status); assertTrue(execution.bytes().isEmpty())
        assertEquals(-1, assertNotNull(KagemushaWalletNativeV1.executeReviewed(0, 1, ByteArray(31))).status)
        assertEquals(-2, KagemushaWalletNativeV1.discardReview(0, 1))
    }

}
