package org.hyperledger.iroha.sdk.client.websocket

/** Factory that selects a WebSocket connector per runtime (OkHttp on Android, JDK connector on JVM). */
object PlatformWebSocketConnector {

    /** Returns a platform-appropriate connector (prefers OkHttp when available). */
    @JvmStatic
    fun createDefault(): ToriiWebSocketClient.WebSocketConnector =
        createDefault(PlatformWebSocketConnector::class.java.classLoader)

    @JvmStatic
    internal fun createDefault(loader: ClassLoader?): ToriiWebSocketClient.WebSocketConnector {
        tryCreateOkHttpConnector(loader)?.let { return it }
        tryCreateJdkConnector(loader)?.let { return it }
        throw IllegalStateException("No WebSocket connector is available on the classpath.")
    }

    private fun tryCreateOkHttpConnector(loader: ClassLoader?): ToriiWebSocketClient.WebSocketConnector? {
        val effectiveLoader = loader ?: PlatformWebSocketConnector::class.java.classLoader
        return try {
            val factoryClass = Class.forName(
                "org.hyperledger.iroha.sdk.client.okhttp.OkHttpWebSocketConnectorFactory",
                true,
                effectiveLoader,
            )
            val method = factoryClass.getMethod("createDefault")
            method.invoke(null) as? ToriiWebSocketClient.WebSocketConnector
        } catch (_: ClassNotFoundException) {
            null
        } catch (_: Exception) {
            null
        }
    }

    private fun tryCreateJdkConnector(loader: ClassLoader?): ToriiWebSocketClient.WebSocketConnector? {
        val effectiveLoader = loader ?: PlatformWebSocketConnector::class.java.classLoader
        return try {
            val factoryClass = Class.forName(
                "org.hyperledger.iroha.sdk.client.websocket.JdkWebSocketConnectorFactory",
                true,
                effectiveLoader,
            )
            val method = factoryClass.getMethod("createDefault")
            method.invoke(null) as? ToriiWebSocketClient.WebSocketConnector
        } catch (_: ClassNotFoundException) {
            null
        } catch (_: Exception) {
            null
        }
    }
}
