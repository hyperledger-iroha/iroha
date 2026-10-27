// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/**
 * Closed diagnostic causes of Native's [KagemushaWalletExceptionV1.INVALID] refusal.
 *
 * Names and codes are stable support identifiers. They contain no wallet, account, amount or
 * callback text and confer no retry, recovery or completion authority. Other failure statuses
 * have separate reason namespaces; unknown reasons remain unclassified.
 */
enum class KagemushaWalletInvalidReasonV1(@JvmField val code: Int) {
    ACTIVATION_UNCONFIRMED(200),
    INVALID_LOAD_REQUEST(201),
    LOAD_AMOUNT_CHANGED(202),
    LOAD_LIFECYCLE_INACTIVE(203),
    G1_VALIDATION(204),
    ARCHIVE_ENCODING(205),
    INDEX_SIZE(206),
    PROVIDER_POISONED(207),
    NO_HEAD(208),
    COLLECTED(209),
    ARCHIVE_MANIFEST_SIZE(220),
    ARCHIVE_HEAD_NOT_RELEASED(221),
    ARCHIVE_STALE_CHECKPOINT(222),
    ARCHIVE_IMMUTABLE_RECORD(223),
    ARCHIVE_CALL_FAILED(224),
    MARKER_ARCHIVE_CHECKPOINT(225),
    MARKER_GENERATION(226),
    MARKER_STATE(227),
    MARKER_PHASE(228),
    MARKER_FRAME(229),
    MARKER_SELECTED_GENERATION(230),
    MARKER_COMPLETION_DIGEST(231),
    MARKER_SLOT(232),
    MARKER_ANCHOR_KIND(233),
    MARKER_SUPERSEDED(234),
    MARKER_RETIRED_GENERATION(235),
    ATOMIC_PUBLICATION_UNSUPPORTED(236),
    ARCHIVE_SLOT(237),
    ARCHIVE_CALL(238),
    ARCHIVE_PENDING_RECORDS(239),
    ARCHIVE_RECORD_SIZE(240),
    CURRENT_SOURCE(241),
    INCOMING_LINEAGE_VERDICT(242);

    companion object {
        /** Resolve only the invalid-refusal namespace; preserve unknown reasons as `null`. */
        @JvmStatic
        fun fromNative(status: Int, reason: Int): KagemushaWalletInvalidReasonV1? =
            if (status == KagemushaWalletExceptionV1.INVALID) values().firstOrNull { it.code == reason }
            else null
    }
}
