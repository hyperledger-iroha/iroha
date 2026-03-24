package org.hyperledger.iroha.sdk.address

class MultisigMemberPayload(
    @JvmField val curveId: Int,
    @JvmField val weight: Int,
    publicKey: ByteArray,
) {
    private val _publicKey: ByteArray = publicKey.copyOf()

    val publicKey: ByteArray get() = _publicKey.copyOf()
}
