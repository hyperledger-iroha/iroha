package org.hyperledger.iroha.sdk.client

import java.util.Base64
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.sccp.SccpNetworkV1
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import org.hyperledger.iroha.sdk.sccp.SccpV1

/** Closed SCCP payload kinds admitted by the first-release bridge flow. */
internal enum class SccpPayloadKindV1(val wireKey: String) {
    TRANSFER("transfer");

    companion object {
        fun fromWireKey(value: String): SccpPayloadKindV1? = values().firstOrNull { it.wireKey == value }
    }
}

/** Unified strict detached-signing response returned by both SCCP submit endpoints. */
internal class SccpBridgeSubmitResponse(
    val submitted: Boolean,
    val payloadKind: SccpPayloadKindV1,
    val messageIdHex: String,
    val backend: String,
    val counterpartyDomain: Int,
    val counterpartyChain: String,
    val routeConfigurationHashHex: String,
    val rangeStartHeight: Long,
    val rangeEndHeight: Long,
    val creationTimeMs: Long,
    val txHashHex: String?,
    val transactionPayloadB64: String?,
    val signingMessageB64: String?,
)

/** Exact decoder for the unified two-phase SCCP signing response. */
internal object SccpBridgeSubmitResponseParser {
    @JvmStatic fun parse(bytes: ByteArray): SccpBridgeSubmitResponse {
        val value = root(bytes)
        val unknown = value.keys.firstOrNull { it !in FIELDS }
        require(unknown == null) { "bridge response contains unknown or retired field `$unknown`" }
        val missing = FIELDS.firstOrNull { it !in value }
        require(missing == null) { "bridge response is missing required field `$missing`" }
        val submitted = boolean(value, "submitted")
        val start = long(value, "range_start_height", 1)
        val end = long(value, "range_end_height", start)
        val creationTime = long(value, "creation_time_ms", 1)
        val txHash = optionalTransactionHash(value, "tx_hash_hex")
        val transactionPayload = optionalText(value, "transaction_payload_b64")
        val signingMessage = optionalText(value, "signing_message_b64")
        if (submitted) {
            require(txHash != null && transactionPayload == null && signingMessage == null) {
                "submitted SCCP response must contain tx_hash_hex and no signing scaffold"
            }
        } else {
            require(txHash == null && transactionPayload != null && signingMessage != null) {
                "unsigned SCCP response requires transaction_payload_b64 and signing_message_b64"
            }
            val transactionBytes = validateCanonicalTransactionPayload(transactionPayload, creationTime)
            val signingBytes = decodeCanonicalBase64(signingMessage, "signing_message_b64", 32)
            require(signingBytes.contentEquals(IrohaHash.prehash(transactionBytes))) {
                "signing_message_b64 must be the exact transaction-payload prehash"
            }
        }
        val kindText = text(value, "payload_kind")
        val kind = SccpPayloadKindV1.fromWireKey(kindText)
            ?: throw IllegalArgumentException("payload_kind is unknown or retired")
        val backend = text(value, "backend")
        require(backend in CLOSED_BACKENDS) {
            "backend must be one closed SCCP verifier label"
        }
        val counterpartyDomain = integer(value, "counterparty_domain", 1, 4)
        val counterpartyChain = text(value, "counterparty_chain")
        val counterparty = SccpNetworkV1.fromProfileKey(counterpartyChain)
        require(counterparty?.isExternal == true && counterparty.domainId == counterpartyDomain) {
            "counterparty_chain and counterparty_domain must identify one exact external network"
        }
        require(backend in backendsForDomain(counterpartyDomain)) {
            "backend does not match the exact counterparty family"
        }
        return SccpBridgeSubmitResponse(
            submitted, kind, hash(value, "message_id_hex"), backend,
            counterpartyDomain, counterpartyChain,
            hash(value, "route_configuration_hash_hex"), start, end, creationTime,
            txHash, transactionPayload, signingMessage,
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun root(bytes: ByteArray): Map<String, Any?> {
        val text = String(bytes, Charsets.UTF_8)
        require(text.toByteArray(Charsets.UTF_8).contentEquals(bytes)) { "bridge response must be UTF-8 JSON" }
        val value = JsonParser.parse(text)
        require(value is Map<*, *> && value.keys.all { it is String }) { "bridge response must be an object" }
        return value as Map<String, Any?>
    }
    private fun text(value: Map<String, Any?>, field: String): String {
        val result = value[field] as? String ?: throw IllegalArgumentException("$field must be a string")
        require(result.isNotBlank() && result == result.trim()) { "$field must be canonical text" }
        return result
    }
    private fun optionalText(value: Map<String, Any?>, field: String): String? = if (value[field] == null) null else text(value, field)
    private fun boolean(value: Map<String, Any?>, field: String): Boolean = value[field] as? Boolean ?: throw IllegalArgumentException("$field must be boolean")
    private fun long(value: Map<String, Any?>, field: String, minimum: Long): Long {
        val number = value[field] as? Number ?: throw IllegalArgumentException("$field must be integer")
        val result = number.toLong()
        require(number.toString() == result.toString() && result >= minimum) { "$field is out of range" }
        return result
    }
    private fun integer(value: Map<String, Any?>, field: String, minimum: Int, maximum: Int): Int {
        val result = long(value, field, minimum.toLong())
        require(result <= maximum) { "$field is out of range" }
        return result.toInt()
    }
    private fun hash(value: Map<String, Any?>, field: String): String = text(value, field).also {
        require(Regex("[0-9a-f]{64}").matches(it) && it.any { char -> char != '0' }) {
            "$field must be canonical lowercase nonzero 32-byte hex"
        }
    }
    private fun optionalHash(value: Map<String, Any?>, field: String): String? = if (value[field] == null) null else hash(value, field)
    private fun optionalTransactionHash(value: Map<String, Any?>, field: String): String? {
        if (value[field] == null) return null
        val literal = text(value, field)
        require(Regex("[0-9a-f]{63}[13579bdf]").matches(literal)) {
            "$field must match [0-9a-f]{63}[13579bdf] with the Iroha HashOf marker"
        }
        return literal
    }
    private fun decodeCanonicalBase64(value: String, field: String, exactBytes: Int? = null): ByteArray {
        val maximumBytes = exactBytes ?: SCCP_MAX_TRANSACTION_PAYLOAD_BYTES
        require(value.length <= 4 * ((maximumBytes + 2) / 3)) {
            "$field exceeds its size bound"
        }
        val decoded = try { Base64.getDecoder().decode(value) } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$field must be canonical base64", ex)
        }
        require(decoded.isNotEmpty() && Base64.getEncoder().encodeToString(decoded) == value) {
            "$field must be canonical nonempty padded base64"
        }
        if (exactBytes != null) require(decoded.size == exactBytes) { "$field must contain exactly $exactBytes bytes" }
        return decoded
    }

    private fun validateCanonicalTransactionPayload(value: String, creationTimeMs: Long): ByteArray {
        val bytes = decodeCanonicalBase64(value, "transaction_payload_b64")
        val payload = try {
            TRANSACTION_CODEC.decodeTransaction(bytes)
        } catch (ex: Exception) {
            throw IllegalArgumentException(
                "transaction_payload_b64 must contain one canonical transaction payload",
                ex,
            )
        }
        val canonical = try {
            TRANSACTION_CODEC.encodeTransaction(payload)
        } catch (ex: Exception) {
            throw IllegalArgumentException("transaction_payload_b64 could not be canonically re-encoded", ex)
        }
        require(canonical.contentEquals(bytes)) { "transaction_payload_b64 is not canonical" }
        require(payload.creationTimeMs == creationTimeMs) {
            "transaction payload creation time does not match creation_time_ms"
        }
        require(payload.admissionIntent == TransactionAdmissionIntent.QUEUE_PLAN_SYNCED) {
            "transaction payload admission intent must be QueuePlanSynced"
        }
        return bytes
    }

    private val FIELDS = setOf(
        "submitted", "payload_kind", "message_id_hex", "backend", "counterparty_domain",
        "counterparty_chain", "route_configuration_hash_hex", "range_start_height", "range_end_height",
        "creation_time_ms", "tx_hash_hex", "transaction_payload_b64", "signing_message_b64",
    )
    private val CLOSED_BACKENDS = setOf(
        "evm-groth16-bn254-v1",
        "tron-groth16-bn254-v1",
        "ton-groth16-bls12381-v1",
        "bridge/sccp/native/ethereum-beacon-v1",
        "bridge/sccp/native/bsc-parlia-v1",
        "bridge/sccp/native/tron-dpos-v1",
        "bridge/sccp/native/ton-masterchain-v1",
    )

    private fun backendsForDomain(domain: Int): Set<String> = when (domain) {
        1 -> setOf("evm-groth16-bn254-v1", "bridge/sccp/native/ethereum-beacon-v1")
        2 -> setOf("evm-groth16-bn254-v1", "bridge/sccp/native/bsc-parlia-v1")
        4 -> setOf("ton-groth16-bls12381-v1", "bridge/sccp/native/ton-masterchain-v1")
        3 -> setOf("tron-groth16-bn254-v1", "bridge/sccp/native/tron-dpos-v1")
        else -> emptySet()
    }
    private val TRANSACTION_CODEC =
        NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
}
