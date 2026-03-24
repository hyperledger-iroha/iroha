package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash

class MultisigSignature private constructor(
    @JvmField val curveId: Int,
    @JvmField val algorithmTag: Int,
    publicKey: ByteArray,
    signature: ByteArray,
) {
    private val _publicKey: ByteArray
    private val _signature: ByteArray

    init {
        require(curveId in 0..0xFF) { "curveId must fit in u8" }
        require(algorithmTag in 0..0xFF) { "algorithm tag must fit in u8" }
        _publicKey = publicKey.copyOf()
        _signature = signature.copyOf()
        require(_publicKey.isNotEmpty()) { "publicKey must not be empty" }
        require(_signature.isNotEmpty()) { "signature must not be empty" }
    }

    fun publicKey(): ByteArray = _publicKey.copyOf()

    fun signature(): ByteArray = _signature.copyOf()

    fun publicKeyMultihash(): String = encodePublicKeyMultihash(curveId, _publicKey)

    fun publicKeyNoritoPayload(): ByteArray {
        val payload = ByteArray(_publicKey.size + 1)
        payload[0] = algorithmTag.toByte()
        System.arraycopy(_publicKey, 0, payload, 1, _publicKey.size)
        return payload
    }

    companion object {
        @JvmStatic
        fun fromCurveId(curveId: Int, publicKey: ByteArray, signature: ByteArray): MultisigSignature {
            val algorithmTag = algorithmTagForCurveId(curveId)
            require(algorithmTag >= 0) { "Unsupported curve id: $curveId" }
            return MultisigSignature(curveId, algorithmTag, publicKey, signature)
        }

        @JvmStatic
        fun fromPublicKeyLiteral(publicKeyLiteral: String, signature: ByteArray): MultisigSignature {
            val payload = decodePublicKeyLiteral(publicKeyLiteral)
                ?: throw IllegalArgumentException("Invalid public key literal")
            return fromCurveId(payload.curveId, payload.keyBytes, signature)
        }

        private fun algorithmTagForCurveId(curveId: Int): Int = when (curveId) {
            0x01 -> 0
            0x02 -> 4
            0x0A -> 5
            0x0B -> 6
            0x0C -> 7
            0x0D -> 8
            0x0E -> 9
            0x0F -> 10
            else -> -1
        }
    }
}
