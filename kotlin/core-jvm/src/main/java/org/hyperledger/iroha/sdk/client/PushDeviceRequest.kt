package org.hyperledger.iroha.sdk.client

/** Request body for Torii push device registration and unregistration. */
class PushDeviceRequest(
    @JvmField val accountId: String,
    @JvmField val platform: String,
    @JvmField val token: String,
    @JvmField val topics: List<String>? = null,
)
