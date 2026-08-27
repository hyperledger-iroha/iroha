package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for `SetKaigiRelayManifest` instructions. */
class SetKaigiRelayManifestInstruction internal constructor(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val relayManifest: KaigiInstructionUtils.RelayManifest?,
    arguments: Map<String, String>,
) : InstructionTemplate {

    override val arguments: Map<String, String> =
        KaigiInstructionUtils.immutableArguments(arguments)

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SetKaigiRelayManifestInstruction) return false
        return callId.domainId == other.callId.domainId
            && callId.callName == other.callId.callName
            && CreateKaigiInstruction.relayManifestEquals(relayManifest, other.relayManifest)
    }

    override fun hashCode(): Int {
        var result = callId.domainId.hashCode()
        result = 31 * result + callId.callName.hashCode()
        result = 31 * result + CreateKaigiInstruction.relayManifestHash(relayManifest)
        return result
    }

    class Builder internal constructor() {
        private var callId: KaigiInstructionUtils.CallId? = null
        private var relayManifestExpiry: Long? = null
        private val relayManifestHops: MutableList<KaigiInstructionUtils.RelayManifestHop> = mutableListOf()

        fun setCallId(domainId: String, callName: String) = apply {
            this.callId = KaigiInstructionUtils.CallId(domainId, callName)
        }

        fun setCallId(callId: KaigiInstructionUtils.CallId) = apply {
            this.callId = requireNotNull(callId) { "callId" }
        }

        fun clearRelayManifest() = apply {
            relayManifestExpiry = null
            relayManifestHops.clear()
        }

        fun setRelayManifestExpiryMs(expiryMs: Long?) = apply {
            this.relayManifestExpiry = expiryMs
        }

        fun addRelayManifestHop(relayId: String, hpkePublicKeyBase64: String, weight: Int) = apply {
            require(relayId.isNotBlank()) { "relayId must not be blank" }
            require(relayManifestHops.size < KaigiInstructionUtils.KAIGI_RELAY_MANIFEST_MAX_HOPS_V1) {
                "relay manifest must not contain more than " +
                    "${KaigiInstructionUtils.KAIGI_RELAY_MANIFEST_MAX_HOPS_V1} hops"
            }
            val normalizedKey = KaigiInstructionUtils.requireHpkePublicKeyBase64(
                hpkePublicKeyBase64,
                "hpkePublicKey",
            )
            require(weight in 1..0xFF) { "relay hop weight must be between 1 and 255" }
            relayManifestHops.add(
                KaigiInstructionUtils.RelayManifestHop(relayId, normalizedKey, weight),
            )
        }

        fun addRelayManifestHop(relayId: String, hpkePublicKey: ByteArray, weight: Int) = apply {
            addRelayManifestHop(
                relayId,
                KaigiInstructionUtils.toHpkePublicKeyBase64(hpkePublicKey, "hpkePublicKey"),
                weight,
            )
        }

        fun setRelayManifest(manifest: KaigiInstructionUtils.RelayManifest?) = apply {
            relayManifestExpiry = null
            relayManifestHops.clear()
            if (manifest != null) {
                relayManifestExpiry = manifest.expiryMs
                for (hop in manifest.hops) {
                    relayManifestHops.add(
                        KaigiInstructionUtils.RelayManifestHop(hop.relayId, hop.hpkePublicKey, hop.weight),
                    )
                }
            }
        }

        fun build(): SetKaigiRelayManifestInstruction {
            val id = checkNotNull(callId) { "callId must be provided" }
            val manifest = buildRelayManifest()
            return SetKaigiRelayManifestInstruction(id, manifest, canonicalArguments(id, manifest))
        }

        private fun buildRelayManifest(): KaigiInstructionUtils.RelayManifest? {
            if (relayManifestHops.isEmpty() && relayManifestExpiry == null) return null
            val copy = relayManifestHops.map {
                KaigiInstructionUtils.RelayManifestHop(it.relayId, it.hpkePublicKey, it.weight)
            }
            return KaigiInstructionUtils.validateRelayManifest(
                KaigiInstructionUtils.RelayManifest(relayManifestExpiry, copy.toList()),
            )
        }

        private fun canonicalArguments(
            id: KaigiInstructionUtils.CallId,
            manifest: KaigiInstructionUtils.RelayManifest?,
        ): Map<String, String> {
            val args = LinkedHashMap<String, String>()
            args["action"] = ACTION
            KaigiInstructionUtils.appendCallId(id, args, "call")
            KaigiInstructionUtils.appendRelayManifest(manifest, args, "relay_manifest")
            return args
        }
    }

    companion object {
        private const val ACTION = "SetKaigiRelayManifest"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): SetKaigiRelayManifestInstruction {
            KaigiInstructionUtils.requireKnownArguments(
                arguments,
                SET_KAIGI_RELAY_MANIFEST_ARGUMENTS,
                "relay_manifest.hop.",
            )
            KaigiInstructionUtils.requireAction(arguments, ACTION)
            val callId = KaigiInstructionUtils.parseCallId(arguments, "call")
            val relayManifest = KaigiInstructionUtils.parseRelayManifest(arguments, "relay_manifest")
            return builder()
                .setCallId(callId)
                .setRelayManifest(relayManifest)
                .build()
        }

        private val SET_KAIGI_RELAY_MANIFEST_ARGUMENTS = setOf(
            "action",
            "call.domain_id",
            "call.call_name",
            "relay_manifest.expiry_ms",
        )
    }
}
