package org.hyperledger.iroha.sdk.address

class MultisigPolicyPayload private constructor(
    @JvmField val version: Int,
    @JvmField val threshold: Int,
    members: List<MultisigMemberPayload>,
) {
    private val _members: List<MultisigMemberPayload> = members.toList()

    val members: List<MultisigMemberPayload> get() = _members

    companion object {
        @JvmStatic
        fun of(
            version: Int,
            threshold: Int,
            members: List<MultisigMemberPayload>,
        ): MultisigPolicyPayload = MultisigPolicyPayload(version, threshold, members)
    }
}
