package org.hyperledger.iroha.sdk.client

/** Proof verifier metadata returned by identifier and RAM-LFE policy summaries. */
class RamLfeProofVerifierMetadata(
    @JvmField val proofBackend: String,
    @JvmField val circuitId: String,
    @JvmField val publicInputsSchemaHash: String,
    @JvmField val verifyingKeyBytesB64: String,
)
