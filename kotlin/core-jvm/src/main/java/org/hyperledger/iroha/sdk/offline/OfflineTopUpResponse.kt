package org.hyperledger.iroha.sdk.offline

/** Aggregated result for offline top-up (issue + register). */
class OfflineTopUpResponse(
    val certificate: OfflineCertificateIssueResponse,
    val registration: OfflineAllowanceRegisterResponse,
)
