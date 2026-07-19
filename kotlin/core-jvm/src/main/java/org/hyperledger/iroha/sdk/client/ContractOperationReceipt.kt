package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent

/** Public normalized evidence returned for a contract operation. */
class ContractOperationReceipt(
    @JvmField val operationKind: String,
    @JvmField val status: String,
    @JvmField val transport: String,
    @JvmField val dataspace: String,
    @JvmField val contractAlias: String?,
    @JvmField val contractAddress: String?,
    @JvmField val codeHashHex: String?,
    @JvmField val abiHashHex: String?,
    @JvmField val txHashHex: String?,
    @JvmField val entrypoint: String?,
    @JvmField val entrypointHashHex: String?,
    @JvmField val gasLimit: Long?,
    @JvmField val gasUsed: Long?,
    @JvmField val feePayment: FeePaymentIntent?,
    @JvmField val payloadDigestHex: String,
)
