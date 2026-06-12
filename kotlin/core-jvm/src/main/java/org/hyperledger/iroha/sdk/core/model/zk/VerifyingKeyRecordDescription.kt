@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.zk

import java.security.MessageDigest
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/**
 * Describes the fields embedded in `RegisterVerifyingKey` and
 * `UpdateVerifyingKey` instructions.
 *
 * The factory method enforces the same invariants as the server-side DTO handling:
 * either embedded verifying key bytes must be provided or the commitment + length
 * pair must be supplied. Gas schedule identifiers are required, and
 * optional fields default to sensible values when omitted.
 */
class VerifyingKeyRecordDescription private constructor(
    val version: Int,
    val circuitId: String,
    val backendTag: VerifyingKeyBackendTag,
    val curve: String,
    val schemaHashHex: String,
    val commitmentHex: String,
    inlineKeyBytes: ByteArray?,
    val vkLength: Int?,
    val maxProofBytes: Int?,
    val gasScheduleId: String,
    val metadataUriCid: String?,
    val vkBytesCid: String?,
    val activationHeight: Long?,
    val withdrawHeight: Long?,
    val status: VerifyingKeyStatus,
) {

    private val _inlineKeyBytes: ByteArray? = inlineKeyBytes?.copyOf()

    val inlineKeyBytes: ByteArray? get() = _inlineKeyBytes?.copyOf()

    /**
     * Serialises the record into Norito-style arguments using the provided backend identifier for
     * embedded verifying key bytes.
     */
    fun toArguments(backend: String): Map<String, String> {
        if (_inlineKeyBytes != null) {
            require(computeCommitmentHex(backend, _inlineKeyBytes) == commitmentHex) {
                "backend does not match inline key commitment"
            }
        }
        val args = linkedMapOf<String, String>()
        args["record.version"] = version.toUInt().toString()
        args["record.circuit_id"] = circuitId
        args["record.backend_tag"] = backendTag.noritoValue
        args["record.curve"] = curve
        args["record.public_inputs_schema_hash_hex"] = schemaHashHex
        args["record.commitment_hex"] = commitmentHex
        if (_inlineKeyBytes != null) {
            args["record.vk_bytes_b64"] = Base64.encode(_inlineKeyBytes)
            args["record.vk_len"] = vkLength!!.toUInt().toString()
        } else if (vkLength != null) {
            args["record.vk_len"] = vkLength.toUInt().toString()
        }
        if (maxProofBytes != null) {
            args["record.max_proof_bytes"] = maxProofBytes.toUInt().toString()
        }
        args["record.gas_schedule_id"] = gasScheduleId
        if (metadataUriCid != null) {
            args["record.metadata_uri_cid"] = metadataUriCid
        }
        if (vkBytesCid != null) {
            args["record.vk_bytes_cid"] = vkBytesCid
        }
        if (activationHeight != null) {
            args["record.activation_height"] = activationHeight.toULong().toString()
        }
        if (withdrawHeight != null) {
            args["record.withdraw_height"] = withdrawHeight.toULong().toString()
        }
        args["record.status"] = status.wireName
        return args
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is VerifyingKeyRecordDescription) return false
        return version == other.version
            && circuitId == other.circuitId
            && backendTag == other.backendTag
            && curve == other.curve
            && schemaHashHex == other.schemaHashHex
            && commitmentHex == other.commitmentHex
            && _inlineKeyBytes.contentEquals(other._inlineKeyBytes)
            && vkLength == other.vkLength
            && maxProofBytes == other.maxProofBytes
            && gasScheduleId == other.gasScheduleId
            && metadataUriCid == other.metadataUriCid
            && vkBytesCid == other.vkBytesCid
            && activationHeight == other.activationHeight
            && withdrawHeight == other.withdrawHeight
            && status == other.status
    }

    override fun hashCode(): Int {
        var result = version.hashCode()
        result = 31 * result + circuitId.hashCode()
        result = 31 * result + backendTag.hashCode()
        result = 31 * result + curve.hashCode()
        result = 31 * result + schemaHashHex.hashCode()
        result = 31 * result + commitmentHex.hashCode()
        result = 31 * result + _inlineKeyBytes.contentHashCode()
        result = 31 * result + (vkLength?.hashCode() ?: 0)
        result = 31 * result + (maxProofBytes?.hashCode() ?: 0)
        result = 31 * result + gasScheduleId.hashCode()
        result = 31 * result + (metadataUriCid?.hashCode() ?: 0)
        result = 31 * result + (vkBytesCid?.hashCode() ?: 0)
        result = 31 * result + (activationHeight?.hashCode() ?: 0)
        result = 31 * result + (withdrawHeight?.hashCode() ?: 0)
        result = 31 * result + status.hashCode()
        return result
    }

    companion object {

        /**
         * Creates a validated `VerifyingKeyRecordDescription`.
         *
         * Either `inlineKeyBytes` must be provided (and the commitment is computed from `backend`
         * + bytes), or both `commitmentHex` and `vkLength` must be supplied.
         *
         * @param backend backend identifier used to compute the commitment hash for inline keys
         */
        fun create(
            backend: String,
            version: Int,
            circuitId: String,
            schemaHashHex: String,
            gasScheduleId: String,
            backendTag: VerifyingKeyBackendTag = VerifyingKeyBackendTag.UNSUPPORTED,
            curve: String = "unknown",
            commitmentHex: String? = null,
            inlineKeyBytes: ByteArray? = null,
            vkLength: Int? = null,
            maxProofBytes: Int? = null,
            metadataUriCid: String? = null,
            vkBytesCid: String? = null,
            activationHeight: Long? = null,
            withdrawHeight: Long? = null,
            status: VerifyingKeyStatus = VerifyingKeyStatus.ACTIVE,
        ): VerifyingKeyRecordDescription {
            require(version >= 0) { "version must be non-negative" }

            val exactCircuitId = requireExactNonBlank(circuitId, "circuitId")

            val normalizedSchemaHash = validateHex(schemaHashHex, "public_inputs_schema_hash_hex")
            requireNotNull(normalizedSchemaHash) { "public_inputs_schema_hash_hex must be provided" }

            val exactGasScheduleId = requireExactNonBlank(gasScheduleId, "gasScheduleId")

            if (vkLength != null) require(vkLength > 0) { "vkLength must be greater than zero" }
            if (maxProofBytes != null) require(maxProofBytes >= 0) { "maxProofBytes must be non-negative" }
            if (activationHeight != null) require(activationHeight >= 0) { "activationHeight must be non-negative" }
            if (withdrawHeight != null) require(withdrawHeight >= 0) { "withdrawHeight must be non-negative" }

            val normalizedCommitmentHex = validateHex(commitmentHex, "commitment_hex")
            val normalizedCurve = optionalExactNonBlank(curve, "curve") ?: "unknown"
            val normalizedMetadataUriCid = optionalExactNonBlank(metadataUriCid, "metadataUriCid")
            val normalizedVkBytesCid = optionalExactNonBlank(vkBytesCid, "vkBytesCid")

            val inlineBytes = inlineKeyBytes?.copyOf()
            val resolvedVkLength: Int?
            if (inlineBytes != null) {
                resolvedVkLength = inlineBytes.size
                if (vkLength != null) {
                    check(vkLength == resolvedVkLength) {
                        "vkLength does not match length of inline key bytes"
                    }
                }
            } else {
                checkNotNull(normalizedCommitmentHex) {
                    "commitmentHex must be provided when vk bytes are absent"
                }
                checkNotNull(vkLength) {
                    "vkLength must be provided when vk bytes are absent"
                }
                resolvedVkLength = vkLength
            }

            if (withdrawHeight != null && activationHeight != null) {
                check(withdrawHeight >= activationHeight) {
                    "withdrawHeight must be >= activationHeight"
                }
            }

            val computedCommitment = if (inlineBytes != null) {
                computeCommitmentHex(backend, inlineBytes)
            } else {
                normalizedCommitmentHex!!.lowercase()
            }

            if (normalizedCommitmentHex != null && inlineBytes != null) {
                check(normalizedCommitmentHex.equals(computedCommitment, ignoreCase = true)) {
                    "commitmentHex does not match computed inline key commitment"
                }
            }

            return VerifyingKeyRecordDescription(
                version = version,
                circuitId = exactCircuitId,
                backendTag = backendTag,
                curve = normalizedCurve,
                schemaHashHex = normalizedSchemaHash,
                commitmentHex = computedCommitment,
                inlineKeyBytes = inlineBytes,
                vkLength = resolvedVkLength,
                maxProofBytes = maxProofBytes,
                gasScheduleId = exactGasScheduleId,
                metadataUriCid = normalizedMetadataUriCid,
                vkBytesCid = normalizedVkBytesCid,
                activationHeight = activationHeight,
                withdrawHeight = withdrawHeight,
                status = status,
            )
        }

        private fun validateHex(value: String?, field: String): String? {
            if (value == null) return null
            if (value.isEmpty()) return null
            require(value.trim() == value) {
                "$field must not contain surrounding whitespace"
            }
            require(value.length == 64) {
                "$field must contain exactly 64 hexadecimal characters"
            }
            for (c in value) {
                require(c in '0'..'9' || c in 'a'..'f' || c in 'A'..'F') {
                    "$field must contain only hexadecimal characters"
                }
            }
            return value.lowercase()
        }

        private fun optionalExactNonBlank(value: String?, field: String): String? {
            if (value == null) return null
            val trimmed = value.trim()
            if (trimmed.isEmpty()) return null
            require(trimmed == value) {
                "$field must not contain surrounding whitespace"
            }
            return value
        }

        private fun requireExactNonBlank(value: String, field: String): String {
            val trimmed = value.trim()
            require(trimmed.isNotEmpty()) { "$field must not be blank" }
            require(trimmed == value) { "$field must not contain surrounding whitespace" }
            return value
        }

        private fun computeCommitmentHex(backend: String, bytes: ByteArray): String {
            val digest = MessageDigest.getInstance("SHA-256")
            digest.update(backend.toByteArray(Charsets.UTF_8))
            digest.update(bytes)
            return digest.digest().joinToString("") { "%02x".format(it) }
        }
    }
}
