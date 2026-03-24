package org.hyperledger.iroha.sdk.crypto.export

import java.security.PrivateKey
import java.security.PublicKey

/** Holds derived key pair material. */
class KeyPairData internal constructor(
    val privateKey: PrivateKey,
    val publicKey: PublicKey,
)
