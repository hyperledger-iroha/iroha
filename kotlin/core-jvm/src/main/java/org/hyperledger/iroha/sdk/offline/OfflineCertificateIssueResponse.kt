package org.hyperledger.iroha.sdk.offline

/** Response payload for issuing or renewing an offline certificate. */
class OfflineCertificateIssueResponse(
    val certificateIdHex: String,
    val certificate: OfflineWalletCertificate,
)
