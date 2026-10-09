// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.nio.CharBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import org.hyperledger.iroha.sdk.client.ToriiKagemushaWalletLoadIssuanceOriginalV1
import org.hyperledger.iroha.sdk.client.ToriiKagemushaWalletLoadSelectionV1
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge

/**
 * Exact canonical receipt and compact finality originals bound to the signed read's selectors
 * and payer. Decoding grants no verified finality, wallet admission, balance or Load permission.
 * The existing native wallet [KagemushaWalletV1.load] independently verifies its actual proof,
 * installed history/source, enrolled account, current ordinal, policy and durable operation.
 */
class KagemushaWalletLoadOriginalV1 private constructor(
    private val input: KagemushaWalletLoadOriginalInputV1,
    /** Network retained from the original authenticated transport, not inferred from receipt data. */
    @JvmField val networkId: NetworkId,
    /** Exact canonical payer literal retained from the authenticated read. */
    @JvmField val payerAccountId: String,
) {
    /** Exact retained request selector, already bound to the canonical receipt data. */
    fun requestId(): ByteArray = input.requestId()
    /** Whole original receipt, never reconstructed from a managed projection. */
    fun receiptOriginal(): ByteArray = input.receipt()
    /** Whole original compact finality DATA; no proof verdict has been conferred. */
    fun finalityOriginal(): ByteArray = input.finality()
    override fun toString(): String = "KagemushaWalletLoadOriginalV1(originals=[REDACTED])"

    companion object {
        /** Decode with the maintained Native canonical types, preserving exact originals. */
        @JvmStatic
        fun decode(issuance: ToriiKagemushaWalletLoadIssuanceOriginalV1,
            finalityOriginal: ByteArray): KagemushaWalletLoadOriginalV1 {
            val input = KagemushaWalletLoadOriginalInputV1(issuance.selection, issuance.payerAccountId,
                issuance.unverifiedResponseOriginal, finalityOriginal)
            if (!PrivacyNativeBridge.isNativeAvailable()) {
                throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
            }
            val status = try {
                if (KagemushaWalletNativeV1.revision() != 1) {
                    throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
                }
                KagemushaWalletLoadOriginalNativeV1.validate(input.schemeId(), input.walletId(),
                    input.requestId(), input.payer(), input.receipt(), input.finality())
            } catch (_: LinkageError) {
                throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
            }
            if (status < 0) throw KagemushaWalletExceptionV1(status)
            if (status != 0) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
            return KagemushaWalletLoadOriginalV1(input, issuance.networkId, issuance.payerAccountId)
        }
    }
}

/** Finite owned inputs only; this constructor does not decode, authenticate or authorize value. */
internal class KagemushaWalletLoadOriginalInputV1(
    selection: ToriiKagemushaWalletLoadSelectionV1, payerAccountId: String,
    receipt: ByteArray, finality: ByteArray,
) {
    private val scheme = selection.schemeId
    private val wallet = selection.walletId
    private val request = selection.requestId
    private val account: ByteArray
    private val retainedReceipt: ByteArray
    private val retainedFinality: ByteArray
    init {
        // Retain exact DATA bytes, including I105 kana. Native owns canonical account parsing.
        require(payerAccountId.isNotEmpty() && payerAccountId.length <= 1024) {
            "payer exceeds its UTF-8 input bound"
        }
        val encodedPayer = try {
            Charsets.UTF_8.newEncoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .encode(CharBuffer.wrap(payerAccountId))
        } catch (error: CharacterCodingException) {
            throw IllegalArgumentException("payer must encode as exact UTF-8 without replacement", error)
        }
        require(encodedPayer.remaining() <= 1024) { "payer exceeds its UTF-8 byte bound" }
        account = ByteArray(encodedPayer.remaining()).also { encodedPayer.get(it) }
        require(receipt.isNotEmpty() && receipt.size <= 512) { "canonical receipt bound exceeded" }
        require(finality.isNotEmpty() && finality.size <= 16384) { "canonical finality bound exceeded" }
        retainedReceipt = receipt.copyOf()
        retainedFinality = finality.copyOf()
    }
    fun schemeId() = scheme.copyOf()
    fun walletId() = wallet.copyOf()
    fun requestId() = request.copyOf()
    fun payer() = account.copyOf()
    fun receipt() = retainedReceipt.copyOf()
    fun finality() = retainedFinality.copyOf()
    override fun toString(): String = "KagemushaWalletLoadOriginalInputV1(originals=[REDACTED])"
}

/** Calls the sole Native DATA decoder, without raw wallet handles or proof providers. */
internal object KagemushaWalletLoadOriginalNativeV1 {
    @JvmStatic external fun validate(scheme: ByteArray, wallet: ByteArray, request: ByteArray,
        payer: ByteArray, receipt: ByteArray, finality: ByteArray): Int
}
