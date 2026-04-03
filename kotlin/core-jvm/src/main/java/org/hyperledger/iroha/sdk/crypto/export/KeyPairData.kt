package org.hyperledger.iroha.sdk.crypto.export

import java.security.PrivateKey
import java.security.PublicKey
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm

/** Holds derived key pair material. */
class KeyPairData internal constructor(
    val privateKey: PrivateKey,
    val publicKey: PublicKey,
    val signingAlgorithm: SigningAlgorithm,
)
