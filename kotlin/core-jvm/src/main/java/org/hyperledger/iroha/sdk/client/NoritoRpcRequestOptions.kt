package org.hyperledger.iroha.sdk.client

import java.time.Duration

/**
 * Optional request overrides for `NoritoRpcClient.call(String, ByteArray, NoritoRpcRequestOptions)`.
 */
class NoritoRpcRequestOptions(
    @JvmField val timeout: Duration? = null,
    headers: Map<String, String> = emptyMap(),
    queryParameters: Map<String, String> = emptyMap(),
    @JvmField val method: String = "POST",
    @JvmField val accept: String? = NoritoRpcClient.DEFAULT_ACCEPT,
    @JvmField internal val acceptConfigured: Boolean = false,
) {
    @JvmField val headers: Map<String, String> = headers.toMap()
    @JvmField val queryParameters: Map<String, String> = queryParameters.toMap()

    companion object {
        @JvmStatic
        internal fun defaultOptions(): NoritoRpcRequestOptions = NoritoRpcRequestOptions()
    }
}
