package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Signs one SDK-built exact-network operator request message. */
fun interface OperatorRequestSignatureProvider {
    /** Return the detached signature bytes for [message]. */
    fun sign(message: ByteArray): ByteArray
}

/** Immutable exact-network signing context for operator-only Torii APIs. */
class OperatorSigningContext(
    private val networkId: NetworkId,
    private val publicKey: String,
    private val signatureProvider: OperatorRequestSignatureProvider,
) {
    init {
        require(publicKey.isNotEmpty() && publicKey.length <= 512 && publicKey == publicKey.trim()) {
            "operator publicKey must be exact non-empty printable ASCII"
        }
        require(publicKey.all { it.code in 0x21..0x7e }) {
            "operator publicKey must be exact non-empty printable ASCII"
        }
    }

    /** Exact genesis-derived NetworkId included in every operator signature. */
    fun networkId(): NetworkId = networkId

    /** Canonical public-key multihash sent with the detached signature. */
    fun publicKey(): String = publicKey

    internal fun sign(message: ByteArray): ByteArray {
        val signature = signatureProvider.sign(message.copyOf())
        require(signature.isNotEmpty()) { "operator signer returned an empty signature" }
        return signature.copyOf()
    }
}
