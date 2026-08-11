package org.hyperledger.iroha.sdk.connect

import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Decoded payload for an OPEN control frame. */
class OpenControl internal constructor(
    appPublicKey: ByteArray,
    @JvmField val networkId: NetworkId,
) {
    private val _appPublicKey: ByteArray = appPublicKey.copyOf()

    fun appPublicKey(): ByteArray = _appPublicKey.clone()
}
