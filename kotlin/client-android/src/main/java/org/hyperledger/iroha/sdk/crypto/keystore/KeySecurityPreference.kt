package org.hyperledger.iroha.sdk.crypto.keystore

/** Hardware security preference for key generation via Android Keystore. */
enum class KeySecurityPreference {
    STRONGBOX_REQUIRED,
    STRONGBOX_PREFERRED,
    HARDWARE_REQUIRED,
    HARDWARE_PREFERRED,
    SOFTWARE_ONLY,
}
