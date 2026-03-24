package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/**
 * Typed builder for `CastZkBallot` instructions.
 *
 * Public inputs must be provided as a JSON object. The builder rejects deprecated alias keys
 * and emits canonical JSON ordering so ballot fingerprints remain stable.
 * When any lock hint is supplied, all of `owner`, `amount`, and `duration_blocks` are required.
 */
@OptIn(ExperimentalEncodingApi::class)
class CastZkBallotInstruction private constructor(
    private val electionId: String,
    private val proofBase64: String,
    private val publicInputsJson: String,
    override val arguments: Map<String, String>
) : InstructionTemplate {

    override val kind: InstructionKind = InstructionKind.CUSTOM

    fun electionId(): String = electionId
    fun proofBase64(): String = proofBase64
    fun publicInputsJson(): String = publicInputsJson

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CastZkBallotInstruction) return false
        return electionId == other.electionId && proofBase64 == other.proofBase64 && publicInputsJson == other.publicInputsJson
    }

    override fun hashCode(): Int {
        var result = electionId.hashCode()
        result = 31 * result + proofBase64.hashCode()
        result = 31 * result + publicInputsJson.hashCode()
        return result
    }

    class Builder internal constructor() {
        internal var electionId: String? = null
        internal var proofBase64: String? = null
        internal var publicInputsJson: String? = null

        fun setElectionId(electionId: String): Builder { require(electionId.isNotBlank()) { "electionId must not be blank" }; this.electionId = electionId; return this }
        fun setProofBase64(proofBase64: String): Builder {
            require(proofBase64.isNotBlank()) { "proofBase64 must not be blank" }
            try { Base64.decode(proofBase64) } catch (ex: IllegalArgumentException) { throw IllegalArgumentException("proofBase64 must be valid base64", ex) }
            this.proofBase64 = proofBase64; return this
        }
        fun setPublicInputsJson(publicInputsJson: String): Builder { require(publicInputsJson.isNotBlank()) { "publicInputsJson must not be blank" }; this.publicInputsJson = normalizePublicInputsJson(publicInputsJson); return this }

        fun build(): CastZkBallotInstruction {
            checkNotNull(electionId) { "electionId must be provided" }
            checkNotNull(proofBase64) { "proofBase64 must be provided" }
            checkNotNull(publicInputsJson) { "publicInputsJson must be provided" }
            return CastZkBallotInstruction(electionId!!, proofBase64!!, publicInputsJson!!, canonicalArguments())
        }

        internal fun canonicalArguments(): Map<String, String> = linkedMapOf(
            "action" to ACTION, "election_id" to electionId!!, "proof_b64" to proofBase64!!, "public_inputs_json" to publicInputsJson!!
        )
    }

    companion object {
        private const val ACTION = "CastZkBallot"

        @JvmStatic fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CastZkBallotInstruction {
            val b = builder()
                .setElectionId(require(arguments, "election_id"))
                .setProofBase64(require(arguments, "proof_b64"))
                .setPublicInputsJson(require(arguments, "public_inputs_json"))
            return CastZkBallotInstruction(b.electionId!!, b.proofBase64!!, b.publicInputsJson!!, b.canonicalArguments())
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        @Suppress("UNCHECKED_CAST")
        private fun normalizePublicInputsJson(publicInputsJson: String): String {
            val trimmed = publicInputsJson.trim()
            require(trimmed.isNotEmpty()) { "publicInputsJson must not be blank" }
            val parsed: Any?
            try { parsed = JsonParser.parse(trimmed) } catch (ex: IllegalStateException) { throw IllegalArgumentException("publicInputsJson must be valid JSON", ex) }
            require(parsed is Map<*, *>) { "publicInputsJson must be a JSON object" }
            val normalized = LinkedHashMap(parsed as Map<String, Any?>)
            rejectPublicInputsKey(normalized, "durationBlocks", "duration_blocks")
            rejectPublicInputsKey(normalized, "root_hint_hex", "root_hint")
            rejectPublicInputsKey(normalized, "rootHintHex", "root_hint")
            rejectPublicInputsKey(normalized, "rootHint", "root_hint")
            rejectPublicInputsKey(normalized, "nullifier_hex", "nullifier")
            rejectPublicInputsKey(normalized, "nullifierHex", "nullifier")
            normalizePublicInputsHex(normalized, "root_hint")
            normalizePublicInputsHex(normalized, "nullifier")
            ensureLockHintsComplete(normalized)
            return JsonEncoder.encode(normalized)
        }

        private fun rejectPublicInputsKey(target: Map<String, Any?>, key: String, canonicalKey: String) {
            if (!target.containsKey(key)) return
            throw IllegalArgumentException("publicInputsJson must use $canonicalKey (unsupported key $key)")
        }

        private fun normalizePublicInputsHex(target: MutableMap<String, Any?>, key: String) {
            if (!target.containsKey(key)) return
            val value = target[key] ?: return
            require(value is String) { "publicInputsJson $key must be 32-byte hex" }
            target[key] = canonicalizeHex32(value, key)
        }

        private fun canonicalizeHex32(raw: String, field: String): String {
            var trimmed = raw.trim()
            val colonIndex = trimmed.indexOf(':')
            if (colonIndex >= 0) {
                val scheme = trimmed.substring(0, colonIndex)
                val rest = trimmed.substring(colonIndex + 1)
                require(scheme.isEmpty() || "blake2b32".equals(scheme, ignoreCase = true)) { "publicInputsJson $field must be 32-byte hex" }
                trimmed = rest.trim()
            }
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) trimmed = trimmed.substring(2)
            require(trimmed.length == 64 && trimmed.matches(Regex("^[0-9a-fA-F]+$"))) { "publicInputsJson $field must be 32-byte hex" }
            return trimmed.lowercase()
        }

        private fun ensureLockHintsComplete(publicInputs: Map<String, Any?>) {
            val hasOwner = publicInputs["owner"] != null
            val hasAmount = publicInputs["amount"] != null
            val hasDuration = publicInputs["duration_blocks"] != null
            if ((hasOwner || hasAmount || hasDuration) && !(hasOwner && hasAmount && hasDuration)) {
                throw IllegalArgumentException("publicInputsJson must include owner, amount, and duration_blocks when providing lock hints")
            }
        }
    }
}
