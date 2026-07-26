package org.hyperledger.iroha.sdk.connect.error

/** Canonical Connect error categories shared across SDKs. */
enum class ConnectErrorCategory {
    TRANSPORT,
    CODEC,
    AUTHORIZATION,
    TIMEOUT,
    QUEUE_OVERFLOW,
    INTERNAL,
}
