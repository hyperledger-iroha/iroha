package org.hyperledger.iroha.sdk.client

/** Structured BFV public parameters published by identifier policies. */
class IdentifierBfvPublicParameters(
    @JvmField val parameters: Parameters,
    @JvmField val publicKey: PublicKey,
    @JvmField val maxInputBytes: Int,
) {
    /** Scalar BFV parameter set. */
    class Parameters(
        @JvmField val polynomialDegree: Long,
        @JvmField val plaintextModulus: Long,
        @JvmField val ciphertextModulus: Long,
        @JvmField val decompositionBaseLog: Int,
    )

    /** BFV public-key polynomials. */
    class PublicKey(
        b: List<Long>,
        a: List<Long>,
    ) {
        @JvmField val b: List<Long> = b.toList()
        @JvmField val a: List<Long> = a.toList()
    }
}
