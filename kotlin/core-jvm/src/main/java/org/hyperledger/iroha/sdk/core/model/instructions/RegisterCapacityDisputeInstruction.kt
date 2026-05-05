package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val ACTION = "RegisterCapacityDispute"

/** Typed representation of the `RegisterCapacityDispute` instruction (SoraFS dispute registry). */
class RegisterCapacityDisputeInstruction(
    @JvmField val disputeIdHex: String,
    @JvmField val disputePayloadBase64: String,
    @JvmField val providerIdHex: String,
    @JvmField val complainantIdHex: String,
    @JvmField val disputeKind: DisputeKind,
    @JvmField val submittedEpoch: Long,
    @JvmField val description: String,
    @JvmField val evidence: DisputeEvidence,
    @JvmField val replicationOrderIdHex: String? = null,
    @JvmField val requestedRemedy: String? = null,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(disputeIdHex.isNotBlank()) { "disputeIdHex must not be blank" }
        require(disputePayloadBase64.isNotBlank()) { "disputePayloadBase64 must not be blank" }
        require(providerIdHex.isNotBlank()) { "providerIdHex must not be blank" }
        require(complainantIdHex.isNotBlank()) { "complainantIdHex must not be blank" }
        require(submittedEpoch >= 0) { "submittedEpoch must be non-negative" }
        require(description.isNotBlank()) { "description must not be blank" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterCapacityDisputeInstruction) return false
        return submittedEpoch == other.submittedEpoch
            && disputeIdHex == other.disputeIdHex
            && disputePayloadBase64 == other.disputePayloadBase64
            && providerIdHex == other.providerIdHex
            && complainantIdHex == other.complainantIdHex
            && replicationOrderIdHex == other.replicationOrderIdHex
            && disputeKind == other.disputeKind
            && description == other.description
            && requestedRemedy == other.requestedRemedy
            && evidence == other.evidence
    }

    override fun hashCode(): Int = listOf(
        disputeIdHex, disputePayloadBase64, providerIdHex, complainantIdHex,
        replicationOrderIdHex, disputeKind, submittedEpoch, description,
        requestedRemedy, evidence,
    ).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("dispute_id_hex", disputeIdHex)
        put("dispute_b64", disputePayloadBase64)
        put("provider_id_hex", providerIdHex)
        put("complainant_id_hex", complainantIdHex)
        if (!replicationOrderIdHex.isNullOrBlank()) {
            put("replication_order_id_hex", replicationOrderIdHex)
        }
        put("kind", disputeKind.label)
        put("submitted_epoch", submittedEpoch.toString())
        put("description", description)
        if (!requestedRemedy.isNullOrBlank()) {
            put("requested_remedy", requestedRemedy)
        }
        evidence.appendArguments(this)
    }

    enum class DisputeKind(@JvmField val label: String) {
        REPLICATION_SHORTFALL("replication_shortfall"),
        UPTIME_BREACH("uptime_breach"),
        PROOF_FAILURE("proof_failure"),
        FEE_DISPUTE("fee_dispute"),
        OTHER("other");

        companion object {
            @JvmStatic
            fun fromLabel(value: String): DisputeKind {
                val normalized = value.trim().lowercase()
                return entries.firstOrNull { it.label == normalized }
                    ?: throw IllegalArgumentException("Unknown capacity dispute kind: $value")
            }
        }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterCapacityDisputeInstruction {
            val evidence = DisputeEvidence(
                digestHex = requireArgument(arguments, "evidence.digest_hex"),
                mediaType = arguments["evidence.media_type"],
                uri = arguments["evidence.uri"],
                sizeBytes = parseOptionalLong(arguments["evidence.size_bytes"], "evidence.size"),
            )
            return RegisterCapacityDisputeInstruction(
                disputeIdHex = requireArgument(arguments, "dispute_id_hex"),
                disputePayloadBase64 = requireArgument(arguments, "dispute_b64"),
                providerIdHex = requireArgument(arguments, "provider_id_hex"),
                complainantIdHex = requireArgument(arguments, "complainant_id_hex"),
                disputeKind = DisputeKind.fromLabel(requireArgument(arguments, "kind")),
                submittedEpoch = requireLong(arguments, "submitted_epoch"),
                description = requireArgument(arguments, "description"),
                evidence = evidence,
                replicationOrderIdHex = arguments["replication_order_id_hex"]?.takeIf { it.isNotBlank() },
                requestedRemedy = arguments["requested_remedy"]?.takeIf { it.isNotBlank() },
                arguments = arguments,
            )
        }

        @OptIn(ExperimentalEncodingApi::class)
        @JvmStatic
        fun requireBase64(value: String, fieldName: String): String {
            val trimmed = value.trim()
            require(trimmed.isNotEmpty()) { "$fieldName must not be blank" }
            val decoded = try {
                Base64.decode(trimmed)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("$fieldName must be base64", ex)
            }
            require(decoded.isNotEmpty()) { "$fieldName must decode to non-empty bytes" }
            return trimmed
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val raw = requireArgument(arguments, key)
            return raw.toLongOrNull()
                ?: throw IllegalArgumentException("Instruction argument '$key' must be a number: $raw")
        }

        private fun parseOptionalLong(value: String?, label: String): Long? {
            if (value.isNullOrBlank()) return null
            return value.toLongOrNull()
                ?: throw IllegalArgumentException("$label must be a number: $value")
        }
    }
}
