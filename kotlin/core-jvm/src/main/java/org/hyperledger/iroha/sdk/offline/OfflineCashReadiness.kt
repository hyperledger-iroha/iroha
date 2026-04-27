package org.hyperledger.iroha.sdk.offline

/** Response body of `GET /v1/offline/cash/readiness`. */
class OfflineCashReadiness(
    val offlineRecursiveStark: Boolean,
)
