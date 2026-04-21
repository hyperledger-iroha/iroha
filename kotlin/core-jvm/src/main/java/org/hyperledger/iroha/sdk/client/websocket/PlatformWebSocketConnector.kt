package org.hyperledger.iroha.sdk.client.websocket

/** Factory that constructs the canonical WebSocket connector for the current runtime. */
object PlatformWebSocketConnector {

    /** Returns the canonical connector for Android or JVM. */
    @JvmStatic
    fun createDefault(): ToriiWebSocketClient.WebSocketConnector =
        createDefault(PlatformWebSocketConnector::class.java.classLoader)

    @JvmStatic
    internal fun createDefault(loader: ClassLoader?): ToriiWebSocketClient.WebSocketConnector {
        val factory = if (isAndroidRuntime()) {
            "org.hyperledger.iroha.sdk.client.okhttp.OkHttpWebSocketConnectorFactory"
        } else {
            "org.hyperledger.iroha.sdk.client.websocket.JdkWebSocketConnectorFactory"
        }
        return createFromFactory(loader, factory)
    }

    private fun createFromFactory(loader: ClassLoader?, factoryName: String): ToriiWebSocketClient.WebSocketConnector {
        val effectiveLoader = loader ?: PlatformWebSocketConnector::class.java.classLoader
        try {
            val factoryClass = Class.forName(
                factoryName,
                true,
                effectiveLoader,
            )
            val method = factoryClass.getMethod("createDefault")
            val connector = method.invoke(null)
            if (connector is ToriiWebSocketClient.WebSocketConnector) return connector
            throw IllegalStateException("$factoryName did not return a WebSocket connector")
        } catch (ex: ReflectiveOperationException) {
            throw IllegalStateException("Required WebSocket connector factory is unavailable", ex)
        }
    }

    private fun isAndroidRuntime(): Boolean =
        try {
            Class.forName("android.os.Build")
            true
        } catch (_: ClassNotFoundException) {
            false
        }
}
