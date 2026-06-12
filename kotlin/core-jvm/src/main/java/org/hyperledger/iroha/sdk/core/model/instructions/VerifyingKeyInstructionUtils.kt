@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTag
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyRecordDescription
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyStatus
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

internal object VerifyingKeyInstructionUtils {

    internal fun Map<String, String>.nonEmptyString(key: String): String {
        return requireNotNull(getValue(key).takeIf { it.isNotBlank() }) {
            "Instruction argument '$key' is required"
        }
    }

    internal fun Map<String, String>.exactNonEmptyString(key: String): String {
        val value = nonEmptyString(key)
        require(value.trim() == value) {
            "Instruction argument '$key' must not contain surrounding whitespace"
        }
        return value
    }

    internal fun exactNonBlank(value: String, name: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$name must not be blank" }
        require(trimmed == value) { "$name must not contain surrounding whitespace" }
        return value
    }

    internal fun Map<String, String>.productionBackend(key: String): String =
        VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(nonEmptyString(key), key)

    internal fun productionBackend(value: String): String =
        VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(value)

    internal fun Map<String, String>.nonEmptyOrNull(key: String): String? {
        val value = this[key] ?: return null
        val trimmed = value.trim()
        if (trimmed.isEmpty()) return null
        require(trimmed == value) {
            "Instruction argument '$key' must not contain surrounding whitespace"
        }
        return value
    }
    internal fun Map<String, String>.intOrNull(key: String): Int? = nonEmptyOrNull(key)?.toInt()
    internal fun Map<String, String>.longOrNull(key: String): Long? = nonEmptyOrNull(key)?.toLong()

    internal fun Map<String, String>.parseRecord(backend: String): VerifyingKeyRecordDescription {
        val vkBytesB64 = nonEmptyOrNull("record.vk_bytes_b64")
        val inlineKeyBytes = vkBytesB64?.let { Base64.decode(it) }
        val withdrawHeight = longOrNull("record.withdraw_height")
        val deprecationHeight = longOrNull("record.deprecation_height")
        if (withdrawHeight != null && deprecationHeight != null && withdrawHeight != deprecationHeight) {
            throw IllegalArgumentException(
                "record.deprecation_height must match record.withdraw_height when both are set"
            )
        }

        val status: VerifyingKeyStatus = nonEmptyOrNull("record.status")
            ?.let { VerifyingKeyStatus.parse(it) }
            ?: VerifyingKeyStatus.ACTIVE

        return VerifyingKeyRecordDescription.create(
            backend = backend,
            version = Integer.parseUnsignedInt(nonEmptyString("record.version")),
            circuitId = exactNonEmptyString("record.circuit_id"),
            schemaHashHex = nonEmptyString("record.public_inputs_schema_hash_hex"),
            gasScheduleId = exactNonEmptyString("record.gas_schedule_id"),
            backendTag = VerifyingKeyBackendTag.parse(nonEmptyString("record.backend_tag")),
            curve = nonEmptyOrNull("record.curve") ?: "unknown",
            commitmentHex = nonEmptyOrNull("record.commitment_hex"),
            inlineKeyBytes = inlineKeyBytes,
            vkLength = intOrNull("record.vk_len"),
            maxProofBytes = intOrNull("record.max_proof_bytes"),
            metadataUriCid = nonEmptyOrNull("record.metadata_uri_cid"),
            vkBytesCid = nonEmptyOrNull("record.vk_bytes_cid"),
            activationHeight = longOrNull("record.activation_height"),
            withdrawHeight = withdrawHeight ?: deprecationHeight,
            status = status,
        )
    }
}
