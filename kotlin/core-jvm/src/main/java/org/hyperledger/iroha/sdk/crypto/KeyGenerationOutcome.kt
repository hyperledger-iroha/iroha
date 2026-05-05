package org.hyperledger.iroha.sdk.crypto

import java.security.KeyPair

/** Describes the outcome of a key generation request, including the hardware path used. */
class KeyGenerationOutcome(
    @JvmField val keyPair: KeyPair,
    @JvmField val route: Route,
) {
    /** Route used by the provider when generating the key. */
    enum class Route {
        STRONGBOX,
        HARDWARE,
        SOFTWARE,
    }
}
