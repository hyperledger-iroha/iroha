package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.client.transport.UrlConnectionTransportExecutor

/**
 * Factory for selecting a transport executor per runtime (OkHttp on Android, JDK client elsewhere).
 */
class PlatformHttpTransportExecutor private constructor() {

    companion object {
        /** Returns a platform-appropriate executor (prefers OkHttp when available). */
        @JvmStatic
        fun createDefault(): HttpTransportExecutor =
            createDefault(PlatformHttpTransportExecutor::class.java.classLoader)

        @JvmStatic
        internal fun createDefault(loader: ClassLoader?): HttpTransportExecutor {
            tryCreateOkHttpExecutor(loader)?.let { return it }
            tryCreateJavaHttpExecutor(loader)?.let { return it }
            return UrlConnectionTransportExecutor()
        }

        private fun tryCreateOkHttpExecutor(loader: ClassLoader?): HttpTransportExecutor? {
            val effectiveLoader = loader ?: PlatformHttpTransportExecutor::class.java.classLoader
            return try {
                val factoryClass = Class.forName(
                    "org.hyperledger.iroha.sdk.client.okhttp.OkHttpTransportExecutorFactory",
                    true,
                    effectiveLoader,
                )
                val method = factoryClass.getMethod("createDefault")
                val result = method.invoke(null)
                result as? HttpTransportExecutor
            } catch (_: ClassNotFoundException) {
                null
            } catch (_: Exception) {
                null
            }
        }

        private fun tryCreateJavaHttpExecutor(loader: ClassLoader?): HttpTransportExecutor? {
            val effectiveLoader = loader ?: PlatformHttpTransportExecutor::class.java.classLoader
            return try {
                val factoryClass = Class.forName(
                    "org.hyperledger.iroha.sdk.client.JavaHttpExecutorFactory",
                    true,
                    effectiveLoader,
                )
                val method = factoryClass.getMethod("createDefault")
                val result = method.invoke(null)
                result as? HttpTransportExecutor
            } catch (_: ClassNotFoundException) {
                null
            } catch (_: Exception) {
                null
            }
        }
    }
}
