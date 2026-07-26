package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.client.transport.UrlConnectionTransportExecutor

/** Factory for constructing the canonical JVM transport executor. */
class PlatformHttpTransportExecutor private constructor() {

    companion object {
        /** Returns the canonical executor used by default clients. */
        @JvmStatic
        fun createDefault(): HttpTransportExecutor =
            UrlConnectionTransportExecutor()
    }
}
