// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

class KagemushaWalletInvalidReasonV1Test {
    @Test fun supportNamesMatchTheNativeClosedReasonContract() {
        // Published C ABI reason table; changing these names/codes changes support references.
        val contract = mapOf(
            200 to "ACTIVATION_UNCONFIRMED", 201 to "INVALID_LOAD_REQUEST",
            202 to "LOAD_AMOUNT_CHANGED", 203 to "LOAD_LIFECYCLE_INACTIVE",
            204 to "G1_VALIDATION", 205 to "ARCHIVE_ENCODING", 206 to "INDEX_SIZE",
            207 to "PROVIDER_POISONED", 208 to "NO_HEAD", 209 to "COLLECTED",
            220 to "ARCHIVE_MANIFEST_SIZE", 221 to "ARCHIVE_HEAD_NOT_RELEASED",
            222 to "ARCHIVE_STALE_CHECKPOINT", 223 to "ARCHIVE_IMMUTABLE_RECORD",
            224 to "ARCHIVE_CALL_FAILED", 225 to "MARKER_ARCHIVE_CHECKPOINT",
            226 to "MARKER_GENERATION", 227 to "MARKER_STATE", 228 to "MARKER_PHASE",
            229 to "MARKER_FRAME", 230 to "MARKER_SELECTED_GENERATION",
            231 to "MARKER_COMPLETION_DIGEST", 232 to "MARKER_SLOT",
            233 to "MARKER_ANCHOR_KIND", 234 to "MARKER_SUPERSEDED",
            235 to "MARKER_RETIRED_GENERATION", 236 to "ATOMIC_PUBLICATION_UNSUPPORTED",
            237 to "ARCHIVE_SLOT", 238 to "ARCHIVE_CALL", 239 to "ARCHIVE_PENDING_RECORDS",
            240 to "ARCHIVE_RECORD_SIZE", 241 to "CURRENT_SOURCE", 242 to "INCOMING_LINEAGE_VERDICT",
        )
        assertEquals(contract, KagemushaWalletInvalidReasonV1.values().associate { it.code to it.name })
        for ((code, name) in contract) {
            assertEquals(name, KagemushaWalletInvalidReasonV1.fromNative(-1, code)?.name)
        }
    }

    @Test fun exceptionAndOutputKeepRawTripletsAndExposeOnlyStatusScopedCauses() {
        for (reason in listOf(-1, 0, 6, 99, 200, 209, 210, 219, 220, 236, 237, 241, 242, 243, Int.MAX_VALUE)) {
            for (status in listOf(-1, -2, -5, -6, -7, -100)) {
                val error = KagemushaWalletExceptionV1(status, reason, 31)
                val output = KagemushaWalletCallV1(status, reason, 31, 0, 0, 0, byteArrayOf())
                val expected = if (status == -1) when (reason) {
                    200 -> KagemushaWalletInvalidReasonV1.ACTIVATION_UNCONFIRMED
                    209 -> KagemushaWalletInvalidReasonV1.COLLECTED
                    220 -> KagemushaWalletInvalidReasonV1.ARCHIVE_MANIFEST_SIZE
                    236 -> KagemushaWalletInvalidReasonV1.ATOMIC_PUBLICATION_UNSUPPORTED
                    237 -> KagemushaWalletInvalidReasonV1.ARCHIVE_SLOT
                    241 -> KagemushaWalletInvalidReasonV1.CURRENT_SOURCE
                    242 -> KagemushaWalletInvalidReasonV1.INCOMING_LINEAGE_VERDICT
                    else -> null
                } else null
                assertEquals(expected, error.invalidReason)
                assertEquals(expected, output.invalidReason)
                assertEquals(listOf(status, reason, 31), listOf(error.status, error.reason, error.platformCode))
                assertEquals(listOf(status, reason, 31), listOf(output.status, output.reason, output.platformCode))
            }
        }
        assertNull(KagemushaWalletInvalidReasonV1.fromNative(12, 200))
        assertNull(KagemushaWalletExceptionV1(-1).invalidReason)
    }
}
