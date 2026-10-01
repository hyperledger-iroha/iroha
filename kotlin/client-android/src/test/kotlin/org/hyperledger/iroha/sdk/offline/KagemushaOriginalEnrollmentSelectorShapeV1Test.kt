// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertFailsWith

/** The read-only original ID remains exact even after explicit reservation was added. */
class KagemushaOriginalEnrollmentSelectorShapeV1Test {
    @Test fun originalIdResponseRequiresExactlyOneNonzero32ByteDigest() {
        val method = KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(11)))
        val original = ByteArray(32) { 1 }
        val response = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request,
            listOf(original))
        assertContentEquals(original,
            KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, response).single())
        // Preserve all four original response refusal cases through the current codec.
        for (fields in listOf(emptyList(), listOf(ByteArray(32)),
            listOf(original.copyOf(31)), listOf(original, ByteArray(32) { 2 }))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, fields)
            }
        }
    }
}
