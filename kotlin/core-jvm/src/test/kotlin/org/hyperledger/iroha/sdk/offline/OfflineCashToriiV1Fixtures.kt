package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.core.model.instructions.FixtureGeneratorRunner

/** Authoritative Rust-generated, semantically valid Offline Cash Torii fixtures. */
internal object OfflineCashToriiV1Fixtures {
    private val expectedNames = linkedSetOf(
        "network_id",
        "top_up_operation_id",
        "top_up_submitted_at_ms",
        "top_up_request",
        "top_up_reference",
        "top_up_pending_status",
        "top_up_finalized_block_height",
        "top_up_server_time_ms",
        "top_up_applied_status",
        "invalid_top_up_anchor_status",
        "invalid_top_up_proof_status",
        "wrong_top_up_operation_status",
        "wrong_top_up_transaction_status",
        "wrong_top_up_height_status",
        "wrong_top_up_proof_network_status",
        "foreign_network_top_up_status",
        "wrong_top_up_proof_anchor_status",
        "wrong_top_up_proof_height_status",
        "redeem_operation_id",
        "redeem_submitted_at_ms",
        "redeem_request",
        "redeem_reference",
        "redeem_pending_status",
        "redeem_applied_status",
        "rejected_status",
        "invalid_binding_top_up_request",
        "wrong_id_reference",
        "wrong_kind_reference",
        "wrong_time_reference",
        "zero_time_reference",
        "wrong_uri_reference",
        "invalid_transaction_hash_reference",
        "wrong_id_status",
        "zero_submitted_pending_status",
        "zero_height_status",
        "zero_time_status",
        "invalid_transaction_hash_status",
        "wrong_rejection_code_status",
        "rejection_details_status",
        "oversized_rejection_message_status",
    )
    private val digestNames = setOf(
        "network_id",
        "top_up_operation_id",
        "redeem_operation_id",
    )
    private val positiveDecimalNames = setOf(
        "top_up_submitted_at_ms",
        "top_up_finalized_block_height",
        "top_up_server_time_ms",
        "redeem_submitted_at_ms",
    )

    private val values: Map<String, String> by lazy {
        parseRows(
            FixtureGeneratorRunner.run(
                "offline-cash-v1",
                FixtureGeneratorRunner.OFFLINE_CASH_BINARY_ENVIRONMENT_VARIABLE,
            ),
        )
    }

    internal fun parseRows(rows: List<String>): Map<String, String> {
        val parsed = LinkedHashMap<String, String>()
        for (row in rows) {
            val separator = row.indexOf('=')
            check(separator > 0 && separator < row.lastIndex) {
                "invalid offline-cash-v1 fixture row"
            }
            val name = row.substring(0, separator)
            check(name in expectedNames) {
                "unexpected offline-cash-v1 fixture row $name"
            }
            check(!parsed.containsKey(name)) {
                "duplicate offline-cash-v1 fixture row $name"
            }
            val value = row.substring(separator + 1)
            validateValue(name, value)
            parsed[name] = value
        }
        check(parsed.keys == expectedNames) {
            "offline-cash-v1 fixture rows do not match the exact ${expectedNames.size}-row contract"
        }
        return java.util.Collections.unmodifiableMap(parsed)
    }

    internal fun canonicalRowsForTest(): List<String> =
        values.entries.map { (name, value) -> "$name=$value" }

    val networkId: String get() = text("network_id")
    val topUpOperationId: String get() = text("top_up_operation_id")
    val topUpSubmittedAtMilliseconds: Long get() = text("top_up_submitted_at_ms").toLong()
    val topUpRequest: ByteArray get() = bytes("top_up_request")
    val topUpReference: ByteArray get() = bytes("top_up_reference")
    val topUpPendingStatus: ByteArray get() = bytes("top_up_pending_status")
    val topUpFinalizedBlockHeight: Long get() = text("top_up_finalized_block_height").toLong()
    val topUpServerTimeMilliseconds: Long get() = text("top_up_server_time_ms").toLong()
    val topUpAppliedStatus: ByteArray get() = bytes("top_up_applied_status")
    val invalidTopUpAnchorStatus: ByteArray get() = bytes("invalid_top_up_anchor_status")
    val invalidTopUpProofStatus: ByteArray get() = bytes("invalid_top_up_proof_status")
    val wrongTopUpOperationStatus: ByteArray get() = bytes("wrong_top_up_operation_status")
    val wrongTopUpTransactionStatus: ByteArray get() = bytes("wrong_top_up_transaction_status")
    val wrongTopUpHeightStatus: ByteArray get() = bytes("wrong_top_up_height_status")
    val wrongTopUpProofNetworkStatus: ByteArray get() = bytes("wrong_top_up_proof_network_status")
    val foreignNetworkTopUpStatus: ByteArray get() = bytes("foreign_network_top_up_status")
    val wrongTopUpProofAnchorStatus: ByteArray get() = bytes("wrong_top_up_proof_anchor_status")
    val wrongTopUpProofHeightStatus: ByteArray get() = bytes("wrong_top_up_proof_height_status")
    val redeemOperationId: String get() = text("redeem_operation_id")
    val redeemSubmittedAtMilliseconds: Long get() = text("redeem_submitted_at_ms").toLong()
    val redeemRequest: ByteArray get() = bytes("redeem_request")
    val redeemReference: ByteArray get() = bytes("redeem_reference")
    val redeemPendingStatus: ByteArray get() = bytes("redeem_pending_status")
    val redeemAppliedStatus: ByteArray get() = bytes("redeem_applied_status")
    val rejectedStatus: ByteArray get() = bytes("rejected_status")
    val invalidBindingTopUpRequest: ByteArray get() = bytes("invalid_binding_top_up_request")
    val wrongIdReference: ByteArray get() = bytes("wrong_id_reference")
    val wrongKindReference: ByteArray get() = bytes("wrong_kind_reference")
    val wrongTimeReference: ByteArray get() = bytes("wrong_time_reference")
    val zeroTimeReference: ByteArray get() = bytes("zero_time_reference")
    val wrongUriReference: ByteArray get() = bytes("wrong_uri_reference")
    val invalidTransactionHashReference: ByteArray
        get() = bytes("invalid_transaction_hash_reference")
    val wrongIdStatus: ByteArray get() = bytes("wrong_id_status")
    val zeroSubmittedPendingStatus: ByteArray get() = bytes("zero_submitted_pending_status")
    val zeroHeightStatus: ByteArray get() = bytes("zero_height_status")
    val zeroTimeStatus: ByteArray get() = bytes("zero_time_status")
    val invalidTransactionHashStatus: ByteArray get() = bytes("invalid_transaction_hash_status")
    val wrongRejectionCodeStatus: ByteArray get() = bytes("wrong_rejection_code_status")
    val rejectionDetailsStatus: ByteArray get() = bytes("rejection_details_status")
    val oversizedRejectionMessageStatus: ByteArray
        get() = bytes("oversized_rejection_message_status")

    private fun validateValue(name: String, value: String) {
        when (name) {
            in digestNames -> {
                check(
                    value.length == 64 &&
                        value.any { it != '0' } &&
                        value.all { it in '0'..'9' || it in 'a'..'f' },
                ) {
                    "$name must be exactly 32 non-zero lowercase hexadecimal bytes"
                }
                check(name != "network_id" || value.takeLast(2).toInt(16) and 1 == 1) {
                    "network_id must contain a canonical marked Iroha hash"
                }
            }
            in positiveDecimalNames -> check(
                value.firstOrNull() in '1'..'9' &&
                    value.drop(1).all { it in '0'..'9' } &&
                    value.toLongOrNull()?.let { it > 0 } == true,
            ) {
                "$name must be a canonical positive signed 64-bit decimal"
            }
            else -> check(
                value.length % 2 == 0 &&
                    value.all { it in '0'..'9' || it in 'a'..'f' },
            ) {
                "$name must be non-empty even-length lowercase hexadecimal"
            }
        }
    }

    private fun text(name: String): String = checkNotNull(values[name]) {
        "missing offline-cash-v1 fixture $name"
    }

    private fun bytes(name: String): ByteArray = FixtureGeneratorRunner.hexToBytes(text(name))
}
