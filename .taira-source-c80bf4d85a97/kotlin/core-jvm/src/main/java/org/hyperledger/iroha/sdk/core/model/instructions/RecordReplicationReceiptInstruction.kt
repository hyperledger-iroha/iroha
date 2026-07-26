package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RecordReplicationReceipt"

/** Typed representation of the `RecordReplicationReceipt` instruction. */
class RecordReplicationReceiptInstruction(
    @JvmField val orderIdHex: String,
    @JvmField val providerIdHex: String,
    @JvmField val status: Status,
    @JvmField val timestamp: Long,
    @JvmField val porSampleDigestHex: String? = null,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(orderIdHex.isNotBlank()) { "orderIdHex must not be blank" }
        require(providerIdHex.isNotBlank()) { "providerIdHex must not be blank" }
        require(timestamp >= 0) { "timestamp must be non-negative" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RecordReplicationReceiptInstruction) return false
        return timestamp == other.timestamp
            && orderIdHex == other.orderIdHex
            && providerIdHex == other.providerIdHex
            && status == other.status
            && porSampleDigestHex == other.porSampleDigestHex
    }

    override fun hashCode(): Int =
        listOf(orderIdHex, providerIdHex, status, timestamp, porSampleDigestHex).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("order_id_hex", orderIdHex)
        put("provider_id_hex", providerIdHex)
        put("status", status.label)
        put("timestamp", timestamp.toString())
        if (!porSampleDigestHex.isNullOrBlank()) {
            put("por_sample_digest_hex", porSampleDigestHex)
        }
    }

    enum class Status(@JvmField val label: String) {
        ACCEPTED("Accepted"),
        COMPLETED("Completed"),
        REJECTED("Rejected");

        companion object {
            @JvmStatic
            fun fromLabel(value: String): Status {
                val normalized = value.trim()
                return entries.firstOrNull { it.label.equals(normalized, ignoreCase = true) }
                    ?: throw IllegalArgumentException("Unknown replication receipt status: $value")
            }
        }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RecordReplicationReceiptInstruction {
            return RecordReplicationReceiptInstruction(
                orderIdHex = requireArgument(arguments, "order_id_hex"),
                providerIdHex = requireArgument(arguments, "provider_id_hex"),
                status = Status.fromLabel(requireArgument(arguments, "status")),
                timestamp = requireLong(arguments, "timestamp"),
                porSampleDigestHex = arguments["por_sample_digest_hex"],
                arguments = arguments,
            )
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
    }
}
