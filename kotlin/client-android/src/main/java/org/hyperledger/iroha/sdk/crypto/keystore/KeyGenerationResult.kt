package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.KeyPair

/** Result of a keystore-backed key generation attempt. */
class KeyGenerationResult(
    @JvmField val keyPair: KeyPair,
    /** `true` when the backend produced a StrongBox-backed key. */
    @JvmField val strongBoxBacked: Boolean,
)
