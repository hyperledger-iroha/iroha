package org.hyperledger.iroha.sdk.core.model.instructions

/**
 * Typed builder for the `RegisterPinManifest` instruction.
 *
 * Surfaces the SoraFS pin-manifest metadata (chunker handle, policy, alias bindings, and
 * epoch hints) so Android tests can build deterministic fixtures aligned with the Rust
 * implementation.
 */
class RegisterPinManifestInstruction private constructor(
    @JvmField val digestHex: String,
    @JvmField val chunkerProfile: ChunkerProfile,
    @JvmField val chunkDigestSha3Hex: String,
    @JvmField val pinPolicy: PinPolicy,
    @JvmField val submittedEpoch: Long,
    @JvmField val successorOfHex: String?,
    @JvmField val aliasBinding: AliasBinding?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REGISTER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterPinManifestInstruction) return false
        return digestHex == other.digestHex
            && chunkerProfile == other.chunkerProfile
            && chunkDigestSha3Hex == other.chunkDigestSha3Hex
            && pinPolicy == other.pinPolicy
            && submittedEpoch == other.submittedEpoch
            && successorOfHex == other.successorOfHex
            && aliasBinding == other.aliasBinding
    }

    override fun hashCode(): Int {
        var result = digestHex.hashCode()
        result = 31 * result + chunkerProfile.hashCode()
        result = 31 * result + chunkDigestSha3Hex.hashCode()
        result = 31 * result + pinPolicy.hashCode()
        result = 31 * result + submittedEpoch.hashCode()
        result = 31 * result + (successorOfHex?.hashCode() ?: 0)
        result = 31 * result + (aliasBinding?.hashCode() ?: 0)
        return result
    }

    class Builder internal constructor() {
        private var digestHex: String? = null
        private var chunkerProfile: ChunkerProfile? = null
        private var chunkDigestSha3Hex: String? = null
        private var pinPolicy: PinPolicy? = null
        private var submittedEpoch: Long? = null
        private var successorOfHex: String? = null
        private var aliasBinding: AliasBinding? = null

        fun setDigestHex(digestHex: String) = apply {
            this.digestHex = requireNotNull(digestHex) { "digestHex" }
        }

        fun setChunkerProfile(chunkerProfile: ChunkerProfile) = apply {
            this.chunkerProfile = requireNotNull(chunkerProfile) { "chunkerProfile" }
        }

        fun setChunkDigestSha3Hex(chunkDigestSha3Hex: String) = apply {
            this.chunkDigestSha3Hex = requireNotNull(chunkDigestSha3Hex) { "chunkDigestSha3Hex" }
        }

        fun setPinPolicy(pinPolicy: PinPolicy) = apply {
            this.pinPolicy = requireNotNull(pinPolicy) { "pinPolicy" }
        }

        fun setSubmittedEpoch(submittedEpoch: Long) = apply {
            require(submittedEpoch >= 0) { "submittedEpoch must be non-negative" }
            this.submittedEpoch = submittedEpoch
        }

        fun setSuccessorOfHex(successorOfHex: String?) = apply {
            this.successorOfHex = successorOfHex
        }

        fun setAliasBinding(aliasBinding: AliasBinding?) = apply {
            this.aliasBinding = aliasBinding
        }

        fun build(): RegisterPinManifestInstruction {
            val dh = digestHex
            check(!dh.isNullOrBlank()) { "digestHex must be set" }
            val cp = checkNotNull(chunkerProfile) { "chunkerProfile must be set" }
            val cd = chunkDigestSha3Hex
            check(!cd.isNullOrBlank()) { "chunkDigestSha3Hex must be set" }
            val pp = checkNotNull(pinPolicy) { "pinPolicy must be set" }
            val se = checkNotNull(submittedEpoch) { "submittedEpoch must be set" }

            val args = buildCanonicalArguments(dh, cp, cd, pp, se, successorOfHex, aliasBinding)
            return RegisterPinManifestInstruction(dh, cp, cd, pp, se, successorOfHex, aliasBinding, args)
        }
    }

    /** Chunker profile metadata recorded alongside the manifest. */
    data class ChunkerProfile(
        @JvmField val profileId: Int,
        @JvmField val namespace: String,
        @JvmField val name: String,
        @JvmField val semver: String,
        @JvmField val handle: String?,
        @JvmField val multihashCode: Long,
    ) {
        internal fun appendArguments(target: MutableMap<String, String>) {
            target["chunker.profile_id"] = profileId.toString()
            target["chunker.namespace"] = namespace
            target["chunker.name"] = name
            target["chunker.semver"] = semver
            if (!handle.isNullOrBlank()) {
                target["chunker.handle"] = handle
            }
            target["chunker.multihash_code"] = multihashCode.toString()
        }

        class Builder internal constructor() {
            private var profileId: Int? = null
            private var namespace: String? = null
            private var name: String? = null
            private var semver: String? = null
            private var handle: String? = null
            private var multihashCode: Long? = null

            fun setProfileId(profileId: Int) = apply { this.profileId = profileId }
            fun setNamespace(namespace: String) = apply { this.namespace = requireNotNull(namespace) { "namespace" } }
            fun setName(name: String) = apply { this.name = requireNotNull(name) { "name" } }
            fun setSemver(semver: String) = apply { this.semver = requireNotNull(semver) { "semver" } }
            fun setHandle(handle: String?) = apply { this.handle = handle }
            fun setMultihashCode(multihashCode: Long) = apply { this.multihashCode = multihashCode }

            fun build(): ChunkerProfile {
                val pid = checkNotNull(profileId) { "profileId must be set" }
                val ns = checkNotNull(namespace) { "namespace must be set" }
                val n = checkNotNull(name) { "name must be set" }
                val sv = checkNotNull(semver) { "semver must be set" }
                val mhc = checkNotNull(multihashCode) { "multihashCode must be set" }
                return ChunkerProfile(pid, ns, n, sv, handle, mhc)
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()

            @JvmStatic
            fun fromArguments(arguments: Map<String, String>): ChunkerProfile =
                builder()
                    .setProfileId(requireArg(arguments, "chunker.profile_id").toInt())
                    .setNamespace(requireArg(arguments, "chunker.namespace"))
                    .setName(requireArg(arguments, "chunker.name"))
                    .setSemver(requireArg(arguments, "chunker.semver"))
                    .setHandle(arguments["chunker.handle"])
                    .setMultihashCode(requireArg(arguments, "chunker.multihash_code").toLong())
                    .build()

            private fun requireArg(arguments: Map<String, String>, key: String): String {
                val value = arguments[key]
                require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
                return value
            }
        }
    }

    /** Pin policy metadata encoded alongside the manifest. */
    data class PinPolicy(
        @JvmField val minReplicas: Int,
        @JvmField val storageClass: String,
        @JvmField val retentionEpoch: Long,
    ) {
        internal fun appendArguments(target: MutableMap<String, String>) {
            target["policy.min_replicas"] = minReplicas.toString()
            target["policy.storage_class"] = storageClass
            target["policy.retention_epoch"] = retentionEpoch.toString()
        }

        class Builder internal constructor() {
            private var minReplicas: Int? = null
            private var storageClass: String? = null
            private var retentionEpoch: Long? = null

            fun setMinReplicas(minReplicas: Int) = apply { this.minReplicas = minReplicas }
            fun setStorageClass(storageClass: String) = apply {
                this.storageClass = requireNotNull(storageClass) { "storageClass" }
            }

            fun setRetentionEpoch(retentionEpoch: Long) = apply {
                require(retentionEpoch >= 0) { "retentionEpoch must be non-negative" }
                this.retentionEpoch = retentionEpoch
            }

            fun build(): PinPolicy {
                val mr = checkNotNull(minReplicas) { "minReplicas must be set" }
                val sc = checkNotNull(storageClass) { "storageClass must be set" }
                val re = checkNotNull(retentionEpoch) { "retentionEpoch must be set" }
                return PinPolicy(mr, sc, re)
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()

            @JvmStatic
            fun fromArguments(arguments: Map<String, String>): PinPolicy =
                builder()
                    .setMinReplicas(requireArg(arguments, "policy.min_replicas").toInt())
                    .setStorageClass(requireArg(arguments, "policy.storage_class"))
                    .setRetentionEpoch(requireArg(arguments, "policy.retention_epoch").toLong())
                    .build()

            private fun requireArg(arguments: Map<String, String>, key: String): String {
                val value = arguments[key]
                require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
                return value
            }
        }
    }

    /** Optional alias binding recorded with the manifest. */
    data class AliasBinding(
        @JvmField val name: String,
        @JvmField val namespace: String,
        @JvmField val proofHex: String,
    ) {
        internal fun appendArguments(target: MutableMap<String, String>) {
            target["alias.name"] = name
            target["alias.namespace"] = namespace
            target["alias.proof_hex"] = proofHex
        }

        class Builder internal constructor() {
            private var name: String? = null
            private var namespace: String? = null
            private var proofHex: String? = null

            fun setName(name: String) = apply { this.name = requireNotNull(name) { "name" } }
            fun setNamespace(namespace: String) = apply { this.namespace = requireNotNull(namespace) { "namespace" } }
            fun setProofHex(proofHex: String) = apply { this.proofHex = requireNotNull(proofHex) { "proofHex" } }

            fun build(): AliasBinding {
                val n = checkNotNull(name) { "name must be set" }
                val ns = checkNotNull(namespace) { "namespace must be set" }
                val ph = checkNotNull(proofHex) { "proofHex must be set" }
                return AliasBinding(n, ns, ph)
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()

            @JvmStatic
            fun fromArguments(arguments: Map<String, String>, required: Boolean): AliasBinding? {
                val hasAliasFields = arguments.containsKey("alias.name")
                    || arguments.containsKey("alias.namespace")
                    || arguments.containsKey("alias.proof_hex")
                if (!hasAliasFields) {
                    if (required) {
                        throw IllegalArgumentException("alias binding arguments missing")
                    }
                    return null
                }
                return builder()
                    .setName(requireArg(arguments, "alias.name"))
                    .setNamespace(requireArg(arguments, "alias.namespace"))
                    .setProofHex(requireArg(arguments, "alias.proof_hex"))
                    .build()
            }

            private fun requireArg(arguments: Map<String, String>, key: String): String {
                val value = arguments[key]
                require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
                return value
            }
        }
    }

    companion object {
        const val ACTION: String = "RegisterPinManifest"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterPinManifestInstruction {
            val digestHex = requireArg(arguments, "digest_hex")
            val chunkDigestSha3Hex = requireArg(arguments, "chunk_digest_sha3_256_hex")
            val submittedEpoch = requireLong(arguments, "submitted_epoch")
            val successorOfHex = arguments["successor_of_hex"]
            val chunkerProfile = ChunkerProfile.fromArguments(arguments)
            val pinPolicy = PinPolicy.fromArguments(arguments)
            val aliasBinding = AliasBinding.fromArguments(arguments, required = false)

            return RegisterPinManifestInstruction(
                digestHex = digestHex,
                chunkerProfile = chunkerProfile,
                chunkDigestSha3Hex = chunkDigestSha3Hex,
                pinPolicy = pinPolicy,
                submittedEpoch = submittedEpoch,
                successorOfHex = successorOfHex,
                aliasBinding = aliasBinding,
                arguments = LinkedHashMap(arguments),
            )
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = requireArg(arguments, key)
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException(
                    "Instruction argument '$key' must be a number: $value", ex,
                )
            }
        }

        private fun buildCanonicalArguments(
            digestHex: String,
            chunkerProfile: ChunkerProfile,
            chunkDigestSha3Hex: String,
            pinPolicy: PinPolicy,
            submittedEpoch: Long,
            successorOfHex: String?,
            aliasBinding: AliasBinding?,
        ): Map<String, String> = buildMap {
            put("action", ACTION)
            put("digest_hex", digestHex)
            put("chunk_digest_sha3_256_hex", chunkDigestSha3Hex)
            put("submitted_epoch", submittedEpoch.toString())
            if (!successorOfHex.isNullOrBlank()) {
                put("successor_of_hex", successorOfHex)
            }
            chunkerProfile.appendArguments(this)
            pinPolicy.appendArguments(this)
            aliasBinding?.appendArguments(this)
        }
    }
}
