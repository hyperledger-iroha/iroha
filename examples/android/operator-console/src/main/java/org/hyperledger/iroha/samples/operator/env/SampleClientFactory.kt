package org.hyperledger.iroha.samples.operator.env

import android.content.Context
import java.io.File
import java.net.http.HttpClient
import java.time.Duration
import org.hyperledger.iroha.android.client.ClientConfig
import org.hyperledger.iroha.android.client.HttpClientTransport
import org.hyperledger.iroha.android.client.RetryPolicy
import org.hyperledger.iroha.android.client.queue.FilePendingTransactionQueue
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue
import org.hyperledger.iroha.android.telemetry.AndroidDeviceProfileProvider
import org.hyperledger.iroha.android.telemetry.AndroidNetworkContextProvider
import org.hyperledger.iroha.android.telemetry.TelemetryOptions
import org.hyperledger.iroha.samples.operator.BuildConfig

class SampleClientFactory(
    context: Context,
    private val environment: SampleEnvironment
) {
    private val appContext = context.applicationContext
    private val queueFile = File(appContext.filesDir, "${environment.profile}-pending.queue").toPath()

    fun queuePath() = queueFile

    fun createArtifacts(): ClientArtifacts {
        val queue = FilePendingTransactionQueue(queueFile)
        val telemetryConfig = TelemetryConfig.fromEnvironment(environment)
        val telemetryOptions = telemetryConfig.toTelemetryOptions()
        val config = buildClientConfig(queue, telemetryConfig, telemetryOptions)
        val httpClient =
            HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(12)).build()
        val transport = HttpClientTransport(httpClient, config)
        return ClientArtifacts(config, transport, queue, httpClient)
    }

    private fun buildClientConfig(
        queue: PendingTransactionQueue,
        telemetryConfig: TelemetryConfig,
        telemetryOptions: TelemetryOptions
    ): ClientConfig {
        val retryPolicy =
            RetryPolicy.builder()
                .setMaxAttempts(4)
                .setBaseDelay(Duration.ofMillis(400))
                .build()
        val builder =
            ClientConfig.builder()
                .setBaseUri(environment.resolvedToriiUri())
                .setRequestTimeout(Duration.ofSeconds(10))
                .setRetryPolicy(retryPolicy)
                .setPendingQueue(queue)
                .setTelemetryOptions(telemetryOptions)
                .setNetworkContextProvider(AndroidNetworkContextProvider.fromContext(appContext))
                .setDeviceProfileProvider(AndroidDeviceProfileProvider.create())
                .putDefaultHeader(
                    "User-Agent",
                    "IrohaOperatorSample/${BuildConfig.VERSION_NAME}"
                )
                .putDefaultHeader("X-Iroha-Sample-Profile", environment.profile)
        environment.handoffEndpoint?.let {
            builder.putDefaultHeader("X-Iroha-Sample-Handoff", it)
        }
        telemetryConfig.buildObserver()?.let { observer -> builder.addObserver(observer) }
        return builder.build()
    }

    data class ClientArtifacts(
        val config: ClientConfig,
        val transport: HttpClientTransport,
        val queue: PendingTransactionQueue,
        val httpClient: HttpClient
    )
}
