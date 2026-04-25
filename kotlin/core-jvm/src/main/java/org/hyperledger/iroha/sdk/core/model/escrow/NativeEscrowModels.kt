package org.hyperledger.iroha.sdk.core.model.escrow

/** Stable identifier for a native asset escrow. */
data class EscrowId(@JvmField val value: String) {
    init {
        require(value.isNotBlank()) { "EscrowId value must not be blank" }
    }

    override fun toString(): String = value
}

/** Lifecycle states returned by native asset escrow queries and events. */
enum class AssetEscrowStatus(@JvmField val wireName: String) {
    OPEN("Open"),
    ACCEPTED("Accepted"),
    PAYMENT_SENT("PaymentSent"),
    DISPUTED("Disputed"),
    RELEASED("Released"),
    CANCELLED("Cancelled"),
    RESOLVED("Resolved");

    companion object {
        @JvmStatic
        fun fromWireName(value: String): AssetEscrowStatus {
            val normalized = value.trim()
            return entries.firstOrNull { it.wireName == normalized }
                ?: throw IllegalArgumentException("Unknown native escrow status: $value")
        }
    }
}

/** Court resolution details for a disputed native asset escrow. */
data class AssetEscrowResolution(
    @JvmField val resolver: String,
    @JvmField val buyerAmount: String,
    @JvmField val sellerAmount: String,
    @JvmField val evidenceHashes: List<String> = emptyList(),
    @JvmField val resolvedAtMs: Long,
)

/** Ledger-managed numeric asset escrow record as exposed by native escrow queries. */
data class AssetEscrowRecord(
    @JvmField val id: EscrowId,
    @JvmField val seller: String,
    @JvmField val buyer: String?,
    @JvmField val assetDefinition: String,
    @JvmField val amount: String,
    @JvmField val custody: String,
    @JvmField val status: AssetEscrowStatus,
    @JvmField val evidenceHashes: List<String> = emptyList(),
    @JvmField val createdAtMs: Long,
    @JvmField val acceptedAtMs: Long? = null,
    @JvmField val paymentSentAtMs: Long? = null,
    @JvmField val disputedAtMs: Long? = null,
    @JvmField val closedAtMs: Long? = null,
    @JvmField val resolution: AssetEscrowResolution? = null,
)

/** Native escrow permission token names. */
object NativeEscrowPermissions {
    /** Permission allowing a court account or role to resolve disputed native escrows. */
    const val CAN_RESOLVE_ESCROW_DISPUTE: String = "CanResolveEscrowDispute"
}
