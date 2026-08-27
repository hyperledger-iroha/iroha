package org.hyperledger.iroha.sdk.core.model.instructions

private const val UNREGISTER_KAIGI_RELAY_ACTION = "UnregisterKaigiRelay"
private val UNREGISTER_KAIGI_RELAY_ARGUMENTS = setOf("action", "relay_id")

/** Typed representation of `UnregisterKaigiRelay` instructions. */
class UnregisterKaigiRelayInstruction(
    @JvmField val relayId: String,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> =
        KaigiInstructionUtils.immutableArguments(
            mapOf(
                "action" to UNREGISTER_KAIGI_RELAY_ACTION,
                "relay_id" to relayId,
            ),
        )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(relayId.isNotBlank()) { "relayId must not be blank" }
        require(arguments == null || arguments == _arguments) {
            "arguments must match the canonical UnregisterKaigiRelay representation"
        }
    }

    override fun equals(other: Any?): Boolean =
        this === other || (other is UnregisterKaigiRelayInstruction && relayId == other.relayId)

    override fun hashCode(): Int = relayId.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): UnregisterKaigiRelayInstruction {
            KaigiInstructionUtils.requireKnownArguments(
                arguments,
                UNREGISTER_KAIGI_RELAY_ARGUMENTS,
            )
            KaigiInstructionUtils.requireAction(arguments, UNREGISTER_KAIGI_RELAY_ACTION)
            return UnregisterKaigiRelayInstruction(
                relayId = KaigiInstructionUtils.require(arguments, "relay_id"),
            )
        }
    }
}
