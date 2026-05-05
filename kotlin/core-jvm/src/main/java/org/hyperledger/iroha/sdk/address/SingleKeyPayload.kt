package org.hyperledger.iroha.sdk.address

class SingleKeyPayload(
    @JvmField val curveId: Int,
    publicKey: ByteArray,
) {
    private val _publicKey: ByteArray = publicKey.copyOf()

    val publicKey: ByteArray get() = _publicKey.copyOf()
}
