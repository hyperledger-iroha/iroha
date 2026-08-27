package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterKaigiRelay"
private val REGISTER_KAIGI_RELAY_ARGUMENTS = setOf(
    "action",
    "relay.relay_id",
    "relay.hpke_public_key",
    "relay.bandwidth_class",
)

/** Typed representation of `RegisterKaigiRelay` instructions. */
class RegisterKaigiRelayInstruction(
    @JvmField val relayId: String,
    @JvmField val hpkePublicKeyBase64: String,
    @JvmField val bandwidthClass: Int,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> =
        KaigiInstructionUtils.immutableArguments(canonicalArguments())

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(relayId.isNotBlank()) { "relayId must not be blank" }
        KaigiInstructionUtils.requireHpkePublicKeyBase64(
            hpkePublicKeyBase64,
            "hpkePublicKeyBase64",
        )
        require(bandwidthClass in 1..0xFF) { "bandwidthClass must be between 1 and 255" }
        require(arguments == null || arguments == _arguments) {
            "arguments must match the canonical RegisterKaigiRelay representation"
        }
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
            KaigiInstructionUtils.requireKnownArguments(arguments, REGISTER_KAIGI_RELAY_ARGUMENTS)
            KaigiInstructionUtils.requireAction(arguments, ACTION)
            return RegisterKaigiRelayInstruction(
                relayId = KaigiInstructionUtils.require(arguments, "relay.relay_id"),
                hpkePublicKeyBase64 = KaigiInstructionUtils.requireHpkePublicKeyBase64(
                    KaigiInstructionUtils.require(arguments, "relay.hpke_public_key"),
                    "relay.hpke_public_key",
                ),
                bandwidthClass = KaigiInstructionUtils.parseNonNegativeInt(
                    KaigiInstructionUtils.require(arguments, "relay.bandwidth_class"),
                    "relay.bandwidth_class",
                ),
            )
        }
    }
}
