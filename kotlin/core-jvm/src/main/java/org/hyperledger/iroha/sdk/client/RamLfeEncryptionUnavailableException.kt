package org.hyperledger.iroha.sdk.client

/** Encryption cannot proceed because no secure RAM-LFE encryption profile is available. */
class RamLfeEncryptionUnavailableException : UnsupportedOperationException(
    "RAM-LFE encryption is unavailable: the insecure exact-lift BFV profile must be replaced",
) {
    /** Stable code shared with Torii's encrypted RAM-LFE refusal. */
    @JvmField
    val code: String = "ram_lfe_encryption_unavailable"
}
