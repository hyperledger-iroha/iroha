package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterKaigiRelay"

/** Typed representation of `RegisterKaigiRelay` instructions. */
class RegisterKaigiRelayInstruction(
    @JvmField val relayId: String,
    @JvmField val hpkePublicKeyBase64: String,
    @JvmField val bandwidthClass: Int,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(relayId.isNotBlank()) { "relayId must not be blank" }
        require(hpkePublicKeyBase64.isNotBlank()) { "hpkePublicKeyBase64 must not be blank" }
        require(bandwidthClass in 0..0xFF) { "bandwidthClass must be between 0 and 255" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterKaigiRelayInstruction) return false
        return relayId == other.relayId
            && hpkePublicKeyBase64 == other.hpkePublicKeyBase64
            && bandwidthClass == other.bandwidthClass
    }

    override fun hashCode(): Int = listOf(relayId, hpkePublicKeyBase64, bandwidthClass).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("relay.relay_id", relayId)
        put("relay.hpke_public_key", hpkePublicKeyBase64)
        put("relay.bandwidth_class", bandwidthClass.toUInt().toString())
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterKaigiRelayInstruction {
            return RegisterKaigiRelayInstruction(
                relayId = KaigiInstructionUtils.require(arguments, "relay.relay_id"),
                hpkePublicKeyBase64 = KaigiInstructionUtils.requireBase64(
                    KaigiInstructionUtils.require(arguments, "relay.hpke_public_key"),
                    "relay.hpke_public_key",
                ),
                bandwidthClass = KaigiInstructionUtils.parseNonNegativeInt(
                    KaigiInstructionUtils.require(arguments, "relay.bandwidth_class"),
                    "relay.bandwidth_class",
                ),
                arguments = arguments,
            )
        }
    }
}
