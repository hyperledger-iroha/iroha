package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.util.ArrayList
import java.util.Collections
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceEligibilityOutcomeV1
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceEligibilityReasonV1

/** Terminal state of exactly one native-authenticated offline-device registration. */
enum class OfflineDeviceRegistrationTerminalStateV1(val canonicalLabel: String) {
    APPLIED("applied"),
    ELIGIBILITY_REJECTED("eligibility_rejected"),
    OTHER_REJECTED("other_rejected"),
}

/**
 * Native-verified committed result for exactly one `RegisterOfflineDeviceAttestation` instruction.
 *
 * The native projection authenticates the transaction-details response and requires the exact
 * registration instruction shape. This model then enforces the closed ABI-22 JSON field set and
 * the mutually exclusive applied, typed-eligibility-rejected, and other-rejected shapes.
 */
class AuthenticatedOfflineDeviceRegistrationResultV1 internal constructor(
    val transactionHashHex: String,
    val transactionAuthorityAccountId: String,
    val blockHashHex: String,
    val resultHashHex: String,
    val committedBlockHeight: BigInteger,
    val terminalState: OfflineDeviceRegistrationTerminalStateV1,
    val eligibilityOutcome: OfflineDeviceEligibilityOutcomeV1?,
    val eligibilityReason: OfflineDeviceEligibilityReasonV1?,
    matchedRuleIds: List<String>,
    val rejectionCode: String?,
    val rejectionMessage: String?,
) {
    val matchedRuleIds: List<String> = Collections.unmodifiableList(ArrayList(matchedRuleIds))

    companion object {
        private const val JSON_MAX_BYTES = 128 * 1024
        private val EXACT_KEYS = setOf(
            "version",
            "transaction_hash_hex",
            "transaction_authority",
            "block_hash_hex",
            "result_hash_hex",
            "committed_block_height",
            "terminal_state",
            "eligibility_outcome",
            "eligibility_reason",
            "matched_rule_ids",
            "rejection_code",
            "rejection_message",
        )
        private val OTHER_REJECTION_CODES = setOf(
            "account_does_not_exist",
            "limit_check",
            "validation",
            "instruction_execution",
            "ivm_execution",
            "trigger_execution",
        )

        internal fun parseNativeJson(payload: ByteArray): AuthenticatedOfflineDeviceRegistrationResultV1 {
            check(payload.isNotEmpty() && payload.size <= JSON_MAX_BYTES) {
                "native registration-result JSON violates its byte bound"
            }
            val json = payload.toString(Charsets.UTF_8)
            check(json.toByteArray(Charsets.UTF_8).contentEquals(payload)) {
                "native registration-result JSON is not exact UTF-8"
            }
            val fields = runCatching { Json.parseToJsonElement(json) }
                .getOrElse { throw IllegalStateException("native registration-result JSON is invalid", it) }
            check(fields is JsonObject && fields.keys == EXACT_KEYS) {
                "native registration-result JSON has an invalid field set"
            }
            val version = fields.getValue("version")
            check(version is JsonPrimitive && !version.isString && version.content == "1") {
                "native registration-result version must be 1"
            }

            val terminalState = when (requiredString(fields, "terminal_state", 64)) {
                "applied" -> OfflineDeviceRegistrationTerminalStateV1.APPLIED
                "eligibility_rejected" ->
                    OfflineDeviceRegistrationTerminalStateV1.ELIGIBILITY_REJECTED
                "other_rejected" -> OfflineDeviceRegistrationTerminalStateV1.OTHER_REJECTED
                else -> error("unknown registration terminal state")
            }
            val eligibilityOutcome = when (
                val value = optionalString(fields, "eligibility_outcome", 64)
            ) {
                null -> null
                "drain_only" -> OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY
                "cryptographically_rejected" ->
                    OfflineDeviceEligibilityOutcomeV1.CRYPTOGRAPHICALLY_REJECTED
                else -> error("unknown registration eligibility outcome: $value")
            }
            val eligibilityReason = when (
                val value = optionalString(fields, "eligibility_reason", 128)
            ) {
                null -> null
                "cryptographic_attestation_rejected" ->
                    OfflineDeviceEligibilityReasonV1.CRYPTOGRAPHIC_ATTESTATION_REJECTED
                "policy_not_fresh" -> OfflineDeviceEligibilityReasonV1.POLICY_NOT_FRESH
                "incomplete_attested_properties" ->
                    OfflineDeviceEligibilityReasonV1.INCOMPLETE_ATTESTED_PROPERTIES
                "unsupported_pre_android_12_tee" ->
                    OfflineDeviceEligibilityReasonV1.UNSUPPORTED_PRE_ANDROID_12_TEE
                "vulnerable_firmware" ->
                    OfflineDeviceEligibilityReasonV1.VULNERABLE_FIRMWARE
                "permanently_blocked_device" ->
                    OfflineDeviceEligibilityReasonV1.PERMANENTLY_BLOCKED_DEVICE
                else -> error("unknown registration eligibility reason: $value")
            }
            val matchedRuleIds = matchedRuleIds(fields.getValue("matched_rule_ids"))
            val rejectionCode = optionalString(fields, "rejection_code", 128)
            val rejectionMessage = optionalString(fields, "rejection_message", 1_024)
            validateTerminalShape(
                terminalState,
                eligibilityOutcome,
                eligibilityReason,
                matchedRuleIds,
                rejectionCode,
                rejectionMessage,
            )

            val heightText = requiredString(fields, "committed_block_height", 20)
            check(
                heightText.first() in '1'..'9' &&
                    heightText.all { it in '0'..'9' },
            ) { "committed block height is not a positive canonical decimal" }
            val committedBlockHeight = heightText.toBigIntegerOrNull()
                ?: error("committed block height is invalid")
            check(committedBlockHeight.signum() > 0 && committedBlockHeight.bitLength() <= 64) {
                "committed block height must be a positive u64"
            }

            return AuthenticatedOfflineDeviceRegistrationResultV1(
                transactionHashHex = requiredHash(fields, "transaction_hash_hex"),
                transactionAuthorityAccountId = requiredString(
                    fields,
                    "transaction_authority",
                    16 * 1024,
                ),
                blockHashHex = requiredHash(fields, "block_hash_hex"),
                resultHashHex = requiredHash(fields, "result_hash_hex"),
                committedBlockHeight = committedBlockHeight,
                terminalState = terminalState,
                eligibilityOutcome = eligibilityOutcome,
                eligibilityReason = eligibilityReason,
                matchedRuleIds = matchedRuleIds,
                rejectionCode = rejectionCode,
                rejectionMessage = rejectionMessage,
            )
        }

        private fun requiredHash(fields: JsonObject, key: String): String {
            val value = requiredString(fields, key, 64)
            check(value.matches(Regex("[0-9a-f]{64}"))) {
                "$key must be an exact lowercase 32-byte hash"
            }
            return value
        }

        private fun requiredString(fields: JsonObject, key: String, maximumBytes: Int): String {
            val value = fields.getValue(key)
            check(value is JsonPrimitive && value.isString) { "$key must be a string" }
            return canonicalText(value.content, key, maximumBytes)
        }

        private fun optionalString(fields: JsonObject, key: String, maximumBytes: Int): String? {
            val value = fields.getValue(key)
            if (value is JsonNull) return null
            check(value is JsonPrimitive && value.isString) { "$key must be null or a string" }
            return canonicalText(value.content, key, maximumBytes)
        }

        private fun canonicalText(value: String, field: String, maximumBytes: Int): String {
            check(
                value.isNotEmpty() &&
                    value.toByteArray(Charsets.UTF_8).size <= maximumBytes &&
                    value == value.trim() &&
                    value.none(Char::isISOControl),
            ) { "$field violates its closed text bound" }
            return value
        }

        private fun matchedRuleIds(value: JsonElement): List<String> {
            check(value is JsonArray && value.size <= 256) {
                "matched_rule_ids must be a bounded array"
            }
            val rules = value.mapIndexed { index, element ->
                check(element is JsonPrimitive && element.isString) {
                    "matched_rule_ids[$index] must be a string"
                }
                val rule = canonicalText(element.content, "matched_rule_ids[$index]", 128)
                check(rule.all { it.code in 0x20..0x7e }) {
                    "matched_rule_ids must contain printable ASCII"
                }
                rule
            }
            check(rules.zipWithNext().all { (left, right) -> left < right }) {
                "matched_rule_ids must be sorted and unique"
            }
            return rules
        }

        private fun validateTerminalShape(
            terminalState: OfflineDeviceRegistrationTerminalStateV1,
            eligibilityOutcome: OfflineDeviceEligibilityOutcomeV1?,
            eligibilityReason: OfflineDeviceEligibilityReasonV1?,
            matchedRuleIds: List<String>,
            rejectionCode: String?,
            rejectionMessage: String?,
        ) {
            when (terminalState) {
                OfflineDeviceRegistrationTerminalStateV1.APPLIED -> check(
                    eligibilityOutcome == null &&
                        eligibilityReason == null &&
                        matchedRuleIds.isEmpty() &&
                        rejectionCode == null &&
                        rejectionMessage == null,
                ) { "applied registration result carries rejection fields" }

                OfflineDeviceRegistrationTerminalStateV1.OTHER_REJECTED -> check(
                    eligibilityOutcome == null &&
                        eligibilityReason == null &&
                        matchedRuleIds.isEmpty() &&
                        rejectionCode in OTHER_REJECTION_CODES &&
                        rejectionMessage != null,
                ) { "other registration rejection has an invalid authenticated reason" }

                OfflineDeviceRegistrationTerminalStateV1.ELIGIBILITY_REJECTED -> {
                    check(rejectionCode == "offline_device_eligibility" && rejectionMessage != null) {
                        "typed registration rejection has an invalid authenticated reason"
                    }
                    val decisionIsValid =
                        eligibilityOutcome == OfflineDeviceEligibilityOutcomeV1.CRYPTOGRAPHICALLY_REJECTED &&
                            eligibilityReason ==
                            OfflineDeviceEligibilityReasonV1.CRYPTOGRAPHIC_ATTESTATION_REJECTED &&
                            matchedRuleIds.isEmpty() ||
                            eligibilityOutcome == OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY &&
                            when (eligibilityReason) {
                                OfflineDeviceEligibilityReasonV1.POLICY_NOT_FRESH,
                                OfflineDeviceEligibilityReasonV1.INCOMPLETE_ATTESTED_PROPERTIES,
                                OfflineDeviceEligibilityReasonV1.UNSUPPORTED_PRE_ANDROID_12_TEE,
                                -> matchedRuleIds.isEmpty()
                                OfflineDeviceEligibilityReasonV1.VULNERABLE_FIRMWARE,
                                OfflineDeviceEligibilityReasonV1.PERMANENTLY_BLOCKED_DEVICE,
                                -> matchedRuleIds.isNotEmpty()
                                else -> false
                            }
                    check(decisionIsValid) {
                        "typed registration rejection has an invalid eligibility decision"
                    }
                }
            }
        }
    }
}
