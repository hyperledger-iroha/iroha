package org.hyperledger.iroha.sdk.multisig

/** TTL preview for multisig proposals/relayers. */
class MultisigProposalTtlPreview(
    @JvmField val effectiveTtlMs: Long,
    @JvmField val policyCapMs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val wasCapped: Boolean,
) {

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is MultisigProposalTtlPreview) return false
        return effectiveTtlMs == other.effectiveTtlMs
            && policyCapMs == other.policyCapMs
            && expiresAtMs == other.expiresAtMs
            && wasCapped == other.wasCapped
    }

    override fun hashCode(): Int =
        listOf(effectiveTtlMs, policyCapMs, expiresAtMs, wasCapped).hashCode()

    override fun toString(): String =
        "MultisigProposalTtlPreview{" +
            "effectiveTtlMs=$effectiveTtlMs" +
            ", policyCapMs=$policyCapMs" +
            ", expiresAtMs=$expiresAtMs" +
            ", wasCapped=$wasCapped" +
            '}'
}
