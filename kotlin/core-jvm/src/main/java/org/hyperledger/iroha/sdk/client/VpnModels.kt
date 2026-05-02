package org.hyperledger.iroha.sdk.client

/** Canonical native instruction descriptor returned by Sora VPN Torii endpoints. */
class VpnTxInstruction(
    @JvmField val wireId: String,
    @JvmField val payloadHex: String,
)

/** Response emitted by `GET /v1/vpn/profile`. */
class VpnProfile(
    @JvmField val available: Boolean,
    @JvmField val relayEndpoint: String,
    @JvmField val supportedExitClasses: List<String>,
    @JvmField val defaultExitClass: String,
    @JvmField val leaseSecs: Long,
    @JvmField val dnsPushIntervalSecs: Long,
    @JvmField val meterFamily: String,
    @JvmField val routePushes: List<String>,
    @JvmField val excludedRoutes: List<String>,
    @JvmField val dnsServers: List<String>,
    @JvmField val tunnelAddresses: List<String>,
    @JvmField val mtuBytes: Long,
    @JvmField val displayBillingLabel: String,
    @JvmField val feeAssetId: String,
    @JvmField val escrowAccountId: String,
    @JvmField val operatorAccountId: String,
    @JvmField val leaseFeeNanos: Long,
    @JvmField val settlementGraceSecs: Long,
    @JvmField val flowLabelBits: Int,
    @JvmField val paddingBudgetMs: Int,
    @JvmField val relayTlsSpkiSha256Hex: String?,
)

/** Request body for `POST /v1/vpn/quotes`. */
class VpnQuoteCreateRequest(
    @JvmField val exitClass: String?,
    @JvmField val meteringPublicKeyHex: String,
) {
    constructor(meteringPublicKeyHex: String) : this(null, meteringPublicKeyHex)
}

/** Quote response binding XOR lease escrow terms before a VPN session is opened. */
class VpnQuote(
    @JvmField val quoteId: String,
    @JvmField val leaseIdHex: String,
    @JvmField val sessionIdHex: String,
    @JvmField val paymentReference: String,
    @JvmField val accountId: String,
    @JvmField val exitClass: String,
    @JvmField val relayEndpoint: String,
    @JvmField val leaseSecs: Long,
    @JvmField val quoteExpiresAtMs: Long,
    @JvmField val feeAssetId: String,
    @JvmField val escrowAccountId: String,
    @JvmField val operatorAccountId: String,
    @JvmField val leaseFeeNanos: Long,
    @JvmField val routePushes: List<String>,
    @JvmField val excludedRoutes: List<String>,
    @JvmField val dnsServers: List<String>,
    @JvmField val tunnelAddresses: List<String>,
    @JvmField val mtuBytes: Long,
    @JvmField val meterFamily: String,
    @JvmField val flowLabelBits: Int,
    @JvmField val paddingBudgetMs: Int,
    @JvmField val relayTlsSpkiSha256Hex: String?,
    @JvmField val meteringPublicKeyHex: String,
    @JvmField val openLeaseInstruction: VpnTxInstruction?,
    @JvmField val txInstructions: List<VpnTxInstruction>,
)

/** Request body for `POST /v1/vpn/sessions`. */
class VpnSessionCreateRequest(
    @JvmField val exitClass: String?,
    @JvmField val quoteId: String,
    @JvmField val paymentTxHash: String,
    @JvmField val meteringPublicKeyHex: String,
) {
    constructor(quoteId: String, paymentTxHash: String, meteringPublicKeyHex: String) :
        this(null, quoteId, paymentTxHash, meteringPublicKeyHex)
}

/** Active VPN session response. */
class VpnSession(
    @JvmField val sessionId: String,
    @JvmField val accountId: String,
    @JvmField val exitClass: String,
    @JvmField val relayEndpoint: String,
    @JvmField val leaseSecs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val connectedAtMs: Long,
    @JvmField val meterFamily: String,
    @JvmField val quoteId: String,
    @JvmField val paymentReference: String,
    @JvmField val paymentTxHash: String,
    @JvmField val feeAssetId: String,
    @JvmField val escrowAccountId: String,
    @JvmField val operatorAccountId: String,
    @JvmField val leaseFeeNanos: Long,
    @JvmField val flowLabelBits: Int,
    @JvmField val paddingBudgetMs: Int,
    @JvmField val relayTlsSpkiSha256Hex: String?,
    @JvmField val routePushes: List<String>,
    @JvmField val excludedRoutes: List<String>,
    @JvmField val dnsServers: List<String>,
    @JvmField val tunnelAddresses: List<String>,
    @JvmField val mtuBytes: Long,
    @JvmField val helperTicketHex: String,
    @JvmField val bytesIn: Long,
    @JvmField val bytesOut: Long,
    @JvmField val status: String,
)

/** Request body for `POST /v1/vpn/receipts`. */
class VpnReceiptSubmitRequest(
    @JvmField val relayReceiptHex: String,
    @JvmField val clientVoucherHex: String,
    @JvmField val leaseIdHex: String?,
) {
    constructor(relayReceiptHex: String, clientVoucherHex: String) :
        this(relayReceiptHex, clientVoucherHex, null)
}

/** VPN receipt response including earned/refunded XOR and native settlement instructions. */
class VpnReceipt(
    @JvmField val sessionId: String,
    @JvmField val accountId: String,
    @JvmField val exitClass: String,
    @JvmField val relayEndpoint: String,
    @JvmField val meterFamily: String,
    @JvmField val connectedAtMs: Long,
    @JvmField val disconnectedAtMs: Long,
    @JvmField val durationMs: Long,
    @JvmField val bytesIn: Long,
    @JvmField val bytesOut: Long,
    @JvmField val status: String,
    @JvmField val receiptSource: String,
    @JvmField val quoteId: String,
    @JvmField val paymentTxHash: String,
    @JvmField val feeAssetId: String,
    @JvmField val escrowAccountId: String,
    @JvmField val operatorAccountId: String,
    @JvmField val leaseFeeNanos: Long,
    @JvmField val earnedFeeNanos: Long,
    @JvmField val refundedFeeNanos: Long,
    @JvmField val leaseIdHex: String,
    @JvmField val settleLeaseInstruction: VpnTxInstruction?,
    @JvmField val txInstructions: List<VpnTxInstruction>,
)

/** Response emitted by `GET /v1/vpn/receipts`. */
class VpnReceiptListResponse(
    @JvmField val items: List<VpnReceipt>,
    @JvmField val total: Long,
)
