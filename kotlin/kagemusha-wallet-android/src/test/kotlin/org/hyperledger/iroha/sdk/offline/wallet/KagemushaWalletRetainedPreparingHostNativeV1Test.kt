// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Tag
import org.junit.jupiter.api.Test

/** Actual JNI request framing; an absent owner grants no review, custody or monetary authority. */
@Tag("host-native")
class KagemushaWalletRetainedPreparingHostNativeV1Test {
    private val identity = ByteArray(32) { 1 }

    @BeforeEach fun requireCurrentBridge() {
        assertTrue(NativeSignerBridge.isNativeAvailable(), "The freshly rebuilt Native bridge must load")
        assertEquals(1, KagemushaWalletNativeV1.revision())
    }

    private fun review(selector: Int, expectedProjection: ByteArray = byteArrayOf()): KagemushaWalletReviewReplyV1 {
        val expectation = KagemushaWalletReviewExpectationV1.RetainedPreparing
        return assertNotNull(KagemushaWalletNativeV1.review(0, 1, identity, selector,
            if (selector == 8) 7 else 0, 0, expectation.tag,
            if (expectedProjection.isEmpty()) expectation.bytes() else expectedProjection,
            if (selector == 1) byteArrayOf(2) else byteArrayOf(),
            if (selector == 1) byteArrayOf(3) else byteArrayOf()))
    }

    @Test fun retainedPreparingSendAndUnloadPassJniFramingBeforeRefusingAnAbsentOwner() {
        for (selector in listOf(1, 8)) {
            val reply = review(selector)
            assertEquals(-2, reply.status)
            assertEquals(-1, reply.reason); assertEquals(0, reply.platformCode); assertNull(reply.cleanupToken())
            val refusal = assertFailsWith<KagemushaWalletExceptionV1> {
                reply.decision(Any(), identity, if (selector == 1) KagemushaWalletReviewProjectionV1.Kind.SEND
                    else KagemushaWalletReviewProjectionV1.Kind.UNLOAD)
            }
            assertEquals(-2, refusal.status)
        }
    }

    @Test fun retainedPreparingRejectsProjectedApprovalDataBeforeOwnerLookup() {
        for (selector in listOf(1, 8)) {
            val reply = review(selector, byteArrayOf(1))
            assertEquals(-1, reply.status)
            assertEquals(-1, reply.reason); assertEquals(0, reply.platformCode); assertNull(reply.cleanupToken())
        }
    }
}
