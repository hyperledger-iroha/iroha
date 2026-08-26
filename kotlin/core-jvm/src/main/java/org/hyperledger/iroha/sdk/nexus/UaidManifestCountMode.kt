package org.hyperledger.iroha.sdk.nexus

/** Counting mode accepted and emitted by the UAID manifest inventory endpoint. */
enum class UaidManifestCountMode(
    /** Exact lowercase query/response spelling used by Torii. */
    val parameterValue: String,
) {
    /** Return a bounded observed count. */
    BOUNDED("bounded"),

    /** Return the exact filtered count. */
    EXACT("exact"),
}
