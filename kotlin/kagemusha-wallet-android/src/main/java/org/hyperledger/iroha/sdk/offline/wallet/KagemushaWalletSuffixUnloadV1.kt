// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Bounds of the native suffix carriage; ordinary wallet and peer-message bounds are unchanged. */
object KagemushaWalletSuffixUnloadV1 {
    /** Exact bounded stage carriage; ordinary actions cannot select the suffix size allowance. */
    fun instructionOriginal(nativeFrame: ByteArray): ByteArray = suffixInstructionOriginal(nativeFrame)
    /** Suffix stages append variants to the already registered ledger instruction. */
    const val INSTRUCTION_WIRE_ID = "iroha.kagemusha.wallet.ledger.v1"
    /** Eight complete semantic steps, their length prefixes and bounded chunk framing. */
    const val CHUNK_MAX_BYTES = 528_416
    /** Canonical native stage frame, before its signed outer envelope. */
    const val INSTRUCTION_MAX_BYTES = CHUNK_MAX_BYTES + 16_384
    /** Canonical signed envelope bound shared with the suffix-only Core relay. */
    const val SIGNED_WIRE_MAX_BYTES = CHUNK_MAX_BYTES + 65_536
    internal const val QUOTE_MAX_BYTES = 1_024
    internal const val CERTIFICATES_MAX_BYTES = 10_034
    internal const val ACCOUNT_MAX_BYTES = 4_096
}

/** Immutable request DATA. Native alone selects the account, verifies the suffix and issues a review. */
class KagemushaWalletSuffixUnloadRequestV1 @JvmOverloads constructor(
    requestId: ByteArray,
    val amount: KagemushaWalletUInt128V1,
    quote: ByteArray? = null,
    certificates: ByteArray? = null,
    beneficiary: ByteArray? = null,
) {
    private val identity: ByteArray
    private val quoteOriginal: ByteArray?
    private val certificateOriginal: ByteArray?
    private val beneficiaryOriginal: ByteArray?

    init {
        require(requestId.size == 32 && requestId.any { it != 0.toByte() }) { "nonzero request identity required" }
        require(amount.low != 0L || amount.high != 0L) { "positive amount required" }
        require((quote == null) == (certificates == null) && (quote == null) == (beneficiary == null)) {
            "all charge originals must be present together"
        }
        if (quote != null) {
            require(quote.size in 1..KagemushaWalletSuffixUnloadV1.QUOTE_MAX_BYTES) { "charge quote bound" }
            require(certificates!!.size in 1..KagemushaWalletSuffixUnloadV1.CERTIFICATES_MAX_BYTES) { "charge certificates bound" }
            require(beneficiary!!.size in 1..KagemushaWalletSuffixUnloadV1.ACCOUNT_MAX_BYTES) { "charge beneficiary bound" }
        }
        identity = requestId.copyOf()
        quoteOriginal = quote?.copyOf()
        certificateOriginal = certificates?.copyOf()
        beneficiaryOriginal = beneficiary?.copyOf()
    }

    fun requestId(): ByteArray = identity.copyOf()
    fun quote(): ByteArray? = quoteOriginal?.copyOf()
    fun certificates(): ByteArray? = certificateOriginal?.copyOf()
    fun beneficiary(): ByteArray? = beneficiaryOriginal?.copyOf()
    override fun toString(): String = "KagemushaWalletSuffixUnloadRequestV1(source=[REDACTED])"
}
