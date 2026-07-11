@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val SORAFS_REGISTER_PIN_ACTION = "RegisterPinManifest"
private const val MAX_MANIFEST_PAYLOAD_BYTES = 512 * 1024
private const val MAX_ALIAS_PROOF_BYTES = 1024 * 1024
private val canonicalSorafsHex = Regex("^[0-9a-f]+$")

private fun requireCanonicalHex(
    value: String?,
    fieldName: String,
    expectedBytes: Int? = null,
    maximumBytes: Int? = null,
): String {
    require(!value.isNullOrEmpty()) { "$fieldName must not be empty" }
    require(value.length % 2 == 0 && canonicalSorafsHex.matches(value)) {
        "$fieldName must be canonical lowercase even-length hex without a prefix"
    }
    expectedBytes?.let {
        require(value.length == it * 2) { "$fieldName must encode exactly $it bytes" }
    }
    maximumBytes?.let {
        require(value.length <= it * 2) { "$fieldName must encode at most $it bytes" }
    }
    return value
}

private fun requireNonzeroDigest(value: String?, fieldName: String): String {
    val digest = requireCanonicalHex(value, fieldName, expectedBytes = 32)
    require(digest.any { it != '0' }) { "$fieldName must not be the all-zero digest" }
    return digest
}

private fun requireCanonicalManifestPayload(value: String?): String {
    require(!value.isNullOrEmpty()) { "manifestPayloadBase64 must not be empty" }
    val maximumEncodedLength = ((MAX_MANIFEST_PAYLOAD_BYTES + 2) / 3) * 4
    require(value.length <= maximumEncodedLength) {
        "manifestPayloadBase64 exceeds the $MAX_MANIFEST_PAYLOAD_BYTES-byte limit"
    }
    val decoded = try {
        Base64.decode(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("manifestPayloadBase64 must be valid base64", ex)
    }
    require(decoded.isNotEmpty() && decoded.size <= MAX_MANIFEST_PAYLOAD_BYTES) {
        "manifestPayloadBase64 must encode 1..$MAX_MANIFEST_PAYLOAD_BYTES bytes"
    }
    require(Base64.encode(decoded) == value) {
        "manifestPayloadBase64 must use canonical padded base64"
    }
    return value
}

private fun requireAliasText(value: String?, fieldName: String): String {
    require(!value.isNullOrEmpty()) { "$fieldName must not be empty" }
    require(value == value.trim() && value.toByteArray(Charsets.UTF_8).size <= 128) {
        "$fieldName must be unpadded UTF-8 of at most 128 bytes"
    }
    require(value.none(Char::isISOControl)) { "$fieldName must not contain control characters" }
    return value
}

/** Typed first-release builder for the consensus `RegisterPinManifest` instruction. */
class RegisterPinManifestInstruction private constructor(
    @JvmField val manifestPayloadBase64: String,
    @JvmField val chunkDigestSha3Hex: String,
    @JvmField val submittedEpoch: Long,
    @JvmField val successorOfHex: String?,
    @JvmField val aliasBinding: AliasBinding?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REGISTER

    /** Return a new array containing the canonical Norito manifest payload. */
    fun manifestPayloadBytes(): ByteArray = Base64.decode(manifestPayloadBase64)

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterPinManifestInstruction) return false
        return manifestPayloadBase64 == other.manifestPayloadBase64 &&
            chunkDigestSha3Hex == other.chunkDigestSha3Hex &&
            submittedEpoch == other.submittedEpoch &&
            successorOfHex == other.successorOfHex &&
            aliasBinding == other.aliasBinding
    }

    override fun hashCode(): Int {
        var result = manifestPayloadBase64.hashCode()
        result = 31 * result + chunkDigestSha3Hex.hashCode()
        result = 31 * result + submittedEpoch.hashCode()
        result = 31 * result + (successorOfHex?.hashCode() ?: 0)
        result = 31 * result + (aliasBinding?.hashCode() ?: 0)
        return result
    }

    /** Builder for a canonical register-pin instruction. */
    class Builder internal constructor() {
        private var manifestPayloadBase64: String? = null
        private var chunkDigestSha3Hex: String? = null
        private var submittedEpoch: Long? = null
        private var successorOfHex: String? = null
        private var aliasBinding: AliasBinding? = null

        fun setManifestPayloadBase64(manifestPayloadBase64: String) = apply {
            this.manifestPayloadBase64 = requireCanonicalManifestPayload(manifestPayloadBase64)
        }

        fun setManifestPayload(manifestPayload: ByteArray) = apply {
            require(manifestPayload.isNotEmpty() && manifestPayload.size <= MAX_MANIFEST_PAYLOAD_BYTES) {
                "manifestPayload must contain 1..$MAX_MANIFEST_PAYLOAD_BYTES bytes"
            }
            this.manifestPayloadBase64 = Base64.encode(manifestPayload.copyOf())
        }

        fun setChunkDigestSha3Hex(chunkDigestSha3Hex: String) = apply {
            this.chunkDigestSha3Hex = requireNonzeroDigest(chunkDigestSha3Hex, "chunkDigestSha3Hex")
        }

        fun setSubmittedEpoch(submittedEpoch: Long) = apply {
            require(submittedEpoch >= 0) { "submittedEpoch must be non-negative" }
            this.submittedEpoch = submittedEpoch
        }

        fun setSuccessorOfHex(successorOfHex: String?) = apply {
            this.successorOfHex = successorOfHex?.let {
                requireNonzeroDigest(it, "successorOfHex")
            }
        }

        fun setAliasBinding(aliasBinding: AliasBinding?) = apply {
            this.aliasBinding = aliasBinding
        }

        fun build(): RegisterPinManifestInstruction {
            val payload = checkNotNull(manifestPayloadBase64) { "manifestPayload must be set" }
            val chunkDigest = checkNotNull(chunkDigestSha3Hex) { "chunkDigestSha3Hex must be set" }
            val epoch = checkNotNull(submittedEpoch) { "submittedEpoch must be set" }
            val args = canonicalArguments(
                payload,
                chunkDigest,
                epoch,
                successorOfHex,
                aliasBinding,
            )
            return RegisterPinManifestInstruction(
                payload,
                chunkDigest,
                epoch,
                successorOfHex,
                aliasBinding,
                args,
            )
        }
    }

    /** Optional manifest alias binding. */
    class AliasBinding private constructor(
        @JvmField val name: String,
        @JvmField val namespace: String,
        @JvmField val proofHex: String,
    ) {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is AliasBinding) return false
            return name == other.name && namespace == other.namespace && proofHex == other.proofHex
        }

        override fun hashCode(): Int {
            var result = name.hashCode()
            result = 31 * result + namespace.hashCode()
            result = 31 * result + proofHex.hashCode()
            return result
        }

        internal fun appendArguments(target: MutableMap<String, String>) {
            target["alias.name"] = name
            target["alias.namespace"] = namespace
            target["alias.proof_hex"] = proofHex
        }

        class Builder internal constructor() {
            private var name: String? = null
            private var namespace: String? = null
            private var proofHex: String? = null

            fun setName(name: String) = apply {
                this.name = requireAliasText(name, "alias.name")
            }

            fun setNamespace(namespace: String) = apply {
                this.namespace = requireAliasText(namespace, "alias.namespace")
            }

            fun setProofHex(proofHex: String) = apply {
                this.proofHex = requireCanonicalHex(
                    proofHex,
                    "alias.proofHex",
                    maximumBytes = MAX_ALIAS_PROOF_BYTES,
                )
            }

            fun build(): AliasBinding = AliasBinding(
                checkNotNull(name) { "alias.name must be set" },
                checkNotNull(namespace) { "alias.namespace must be set" },
                checkNotNull(proofHex) { "alias.proofHex must be set" },
            )
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()

            internal fun fromArguments(arguments: Map<String, String>): AliasBinding? {
                val keys = setOf("alias.name", "alias.namespace", "alias.proof_hex")
                val present = keys.count(arguments::containsKey)
                require(present == 0 || present == keys.size) {
                    "alias binding requires alias.name, alias.namespace, and alias.proof_hex together"
                }
                if (present == 0) return null
                return builder()
                    .setName(requireArgument(arguments, "alias.name"))
                    .setNamespace(requireArgument(arguments, "alias.namespace"))
                    .setProofHex(requireArgument(arguments, "alias.proof_hex"))
                    .build()
            }
        }
    }

    companion object {
        const val ACTION: String = SORAFS_REGISTER_PIN_ACTION

        private val mandatoryArgumentKeys = setOf(
            "action",
            "manifest_payload_base64",
            "chunk_digest_sha3_256_hex",
            "submitted_epoch",
        )
        private val optionalArgumentKeys = setOf(
            "successor_of_hex",
            "alias.name",
            "alias.namespace",
            "alias.proof_hex",
        )

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterPinManifestInstruction {
            require(arguments["action"] == ACTION) { "Instruction argument 'action' must be $ACTION" }
            require(arguments.keys.all { it in mandatoryArgumentKeys || it in optionalArgumentKeys }) {
                "RegisterPinManifest arguments contain unsupported fields"
            }
            require(mandatoryArgumentKeys.all(arguments::containsKey)) {
                "RegisterPinManifest arguments are missing required fields"
            }
            return builder()
                .setManifestPayloadBase64(requireArgument(arguments, "manifest_payload_base64"))
                .setChunkDigestSha3Hex(requireArgument(arguments, "chunk_digest_sha3_256_hex"))
                .setSubmittedEpoch(requireLong(arguments, "submitted_epoch"))
                .setSuccessorOfHex(arguments["successor_of_hex"])
                .setAliasBinding(AliasBinding.fromArguments(arguments))
                .build()
        }

        private fun canonicalArguments(
            manifestPayloadBase64: String,
            chunkDigestSha3Hex: String,
            submittedEpoch: Long,
            successorOfHex: String?,
            aliasBinding: AliasBinding?,
        ): Map<String, String> = buildMap {
            put("action", ACTION)
            put("manifest_payload_base64", manifestPayloadBase64)
            put("chunk_digest_sha3_256_hex", chunkDigestSha3Hex)
            put("submitted_epoch", submittedEpoch.toString())
            successorOfHex?.let { put("successor_of_hex", it) }
            aliasBinding?.appendArguments(this)
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrEmpty()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = requireArgument(arguments, key)
            return try {
                value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("Instruction argument '$key' must be a number: $value", ex)
            }
        }
    }
}
