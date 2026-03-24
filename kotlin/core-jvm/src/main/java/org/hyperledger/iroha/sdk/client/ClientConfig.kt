package org.hyperledger.iroha.sdk.client

import java.io.IOException
import java.net.URI
import java.nio.file.Path
import java.time.Duration
import java.util.Collections
import java.util.LinkedHashMap
import java.util.Optional
import org.hyperledger.iroha.sdk.crypto.KeyProvider
import org.hyperledger.iroha.sdk.client.queue.DirectoryPendingTransactionQueue
import org.hyperledger.iroha.sdk.client.queue.FilePendingTransactionQueue
import org.hyperledger.iroha.sdk.client.queue.OfflineJournalPendingTransactionQueue
import org.hyperledger.iroha.sdk.client.queue.PendingTransactionQueue
import org.hyperledger.iroha.sdk.offline.OfflineJournal
import org.hyperledger.iroha.sdk.offline.OfflineJournalException
import org.hyperledger.iroha.sdk.offline.OfflineJournalKey
import org.hyperledger.iroha.sdk.telemetry.*

/** Configuration options for [IrohaClient] implementations. */
class ClientConfig private constructor(builder: Builder) {
    private val baseUri: URI = builder.baseUri
    private val sorafsGatewayUri: URI = builder.sorafsGatewayUri ?: builder.baseUri
    private val requestTimeout: Duration = builder.requestTimeout
    private val defaultHeaders: Map<String, String> = Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))
    private val observers: List<ClientObserver>
    private val retryPolicy: RetryPolicy = builder.retryPolicy
    private val pendingQueue: PendingTransactionQueue? = builder.pendingQueue
    private val exportOptions: ExportOptions?
    private val noritoRpcFlowController: NoritoRpcFlowController = builder.noritoRpcFlowController ?: NoritoRpcFlowController.unlimited()
    private val telemetryOptions: TelemetryOptions = builder.telemetryOptions
    private val telemetrySink: TelemetrySink?
    private val telemetryExporterName: String
    private val networkContextProvider: NetworkContextProvider = builder.networkContextProvider
    private val deviceProfileProvider: DeviceProfileProvider = builder.deviceProfileProvider
    private val crashTelemetryEnabled: Boolean = builder.crashTelemetryEnabled
    private val crashMetadataProvider: MetadataProvider = builder.crashMetadataProvider
    private val crashTelemetryHandler: CrashTelemetryHandler?

    init {
        val resolvedExporterName = builder.resolveTelemetryExporterName()
        val instrumentedSink = if (builder.telemetrySink == null) null else TelemetryExportStatusSink.wrap(builder.telemetrySink, resolvedExporterName)
        val observerList = ArrayList(builder.observers)
        if (builder.telemetryOptions.enabled && instrumentedSink != null) observerList.add(TelemetryObserver(builder.telemetryOptions, instrumentedSink))
        observers = observerList.toList()
        val keystoreTelemetry = KeystoreTelemetryEmitter.from(builder.telemetryOptions, instrumentedSink, builder.deviceProfileProvider)
        exportOptions = builder.exportOptions?.withTelemetry(keystoreTelemetry)
        telemetrySink = instrumentedSink
        telemetryExporterName = resolvedExporterName
        crashTelemetryHandler = maybeInstallCrashTelemetryHandler(builder, instrumentedSink)
    }

    fun baseUri(): URI = baseUri
    /** Base URI used for SoraFS gateway requests. Defaults to [baseUri] when unset. */
    fun sorafsGatewayUri(): URI = sorafsGatewayUri
    fun requestTimeout(): Duration = requestTimeout
    /** Headers that will be applied to every Torii request. */
    fun defaultHeaders(): Map<String, String> = defaultHeaders
    /** Registered observers that receive request lifecycle callbacks. */
    fun observers(): List<ClientObserver> = observers
    fun retryPolicy(): RetryPolicy = retryPolicy
    fun pendingQueue(): PendingTransactionQueue? = pendingQueue
    fun exportOptions(): ExportOptions? = exportOptions
    fun noritoRpcFlowController(): NoritoRpcFlowController = noritoRpcFlowController
    fun telemetryOptions(): TelemetryOptions = telemetryOptions
    fun telemetrySink(): Optional<TelemetrySink> = Optional.ofNullable(telemetrySink)
    fun telemetryExporterName(): String = telemetryExporterName
    fun networkContextProvider(): NetworkContextProvider = networkContextProvider
    fun deviceProfileProvider(): DeviceProfileProvider = deviceProfileProvider
    fun crashTelemetryHandler(): Optional<CrashTelemetryHandler> = Optional.ofNullable(crashTelemetryHandler)

    fun crashTelemetryReporter(): Optional<CrashTelemetryReporter> {
        if (!telemetryOptions.enabled || telemetrySink == null) return Optional.empty()
        return Optional.of(CrashTelemetryReporter(telemetryOptions, telemetrySink))
    }

    fun toBuilder(): Builder {
        val nonTelemetryObservers = observers.filter { it !is TelemetryObserver }
        return Builder()
            .setBaseUri(baseUri).setSorafsGatewayUri(sorafsGatewayUri).setRequestTimeout(requestTimeout)
            .setDefaultHeaders(defaultHeaders).setObservers(nonTelemetryObservers).setRetryPolicy(retryPolicy)
            .setPendingQueue(pendingQueue).setExportOptions(exportOptions).setNoritoRpcFlowController(noritoRpcFlowController)
            .setTelemetryOptions(telemetryOptions).setTelemetrySink(TelemetryExportStatusSink.unwrap(telemetrySink))
            .setTelemetryExporterName(telemetryExporterName).setNetworkContextProvider(networkContextProvider)
            .setDeviceProfileProvider(deviceProfileProvider).setCrashTelemetryMetadataProvider(crashMetadataProvider)
            .setCrashTelemetryEnabled(crashTelemetryEnabled)
    }

    fun toNoritoRpcClient(executor: HttpTransportExecutor?): NoritoRpcClient {
        val b = toNoritoRpcClientBuilder()
        if (executor != null) b.setTransportExecutor(executor)
        return b.build()
    }

    fun toNoritoRpcClient(): NoritoRpcClient = toNoritoRpcClient(PlatformHttpTransportExecutor.createDefault())

    private fun toNoritoRpcClientBuilder(): NoritoRpcClient.Builder =
        NoritoRpcClient.builder().setBaseUri(baseUri).setTimeout(requestTimeout).defaultHeaders(defaultHeaders)
            .observers(observers).setTelemetryOptions(telemetryOptions).setTelemetrySink(telemetrySink)
            .setNetworkContextProvider(networkContextProvider).setDeviceProfileProvider(deviceProfileProvider)
            .setFlowController(noritoRpcFlowController)

    fun toOfflineToriiClient(executor: HttpTransportExecutor): OfflineToriiClient =
        OfflineToriiClient.builder().executor(executor).baseUri(baseUri).timeout(requestTimeout).defaultHeaders(defaultHeaders).observers(observers).build()

    fun toSubscriptionToriiClient(executor: HttpTransportExecutor): SubscriptionToriiClient =
        SubscriptionToriiClient.builder().executor(executor).baseUri(baseUri).timeout(requestTimeout).defaultHeaders(defaultHeaders).observers(observers).build()

    fun toSubscriptionToriiClient(): SubscriptionToriiClient = toSubscriptionToriiClient(PlatformHttpTransportExecutor.createDefault())

    private fun maybeInstallCrashTelemetryHandler(builder: Builder, sink: TelemetrySink?): CrashTelemetryHandler? {
        if (!builder.crashTelemetryEnabled) return null
        if (!builder.telemetryOptions.enabled || sink == null) return null
        return CrashTelemetryHandler.install(builder.telemetryOptions, sink, builder.crashMetadataProvider)
    }

    class Builder {
        internal var baseUri: URI = URI.create("http://localhost:8080")
        internal var sorafsGatewayUri: URI? = null
        internal var requestTimeout: Duration = Duration.ofSeconds(10)
        internal val defaultHeaders = LinkedHashMap<String, String>()
        internal val observers = ArrayList<ClientObserver>()
        internal var retryPolicy: RetryPolicy = RetryPolicy.none()
        internal var pendingQueue: PendingTransactionQueue? = null
        internal var exportOptions: ExportOptions? = null
        internal var noritoRpcFlowController: NoritoRpcFlowController? = NoritoRpcFlowController.unlimited()
        internal var telemetryOptions: TelemetryOptions = TelemetryOptions.disabled()
        internal var telemetrySink: TelemetrySink? = null
        internal var telemetryExporterName: String? = null
        internal var networkContextProvider: NetworkContextProvider = NetworkContextProvider.disabled()
        internal var deviceProfileProvider: DeviceProfileProvider = DeviceProfileProvider.disabled()
        internal var crashTelemetryEnabled: Boolean = false
        internal var crashMetadataProvider: MetadataProvider = CrashTelemetryHandler.defaultMetadataProvider()

        fun setBaseUri(baseUri: URI): Builder { this.baseUri = baseUri; return this }
        fun setSorafsGatewayUri(sorafsGatewayUri: URI): Builder { this.sorafsGatewayUri = sorafsGatewayUri; return this }
        fun setRequestTimeout(requestTimeout: Duration?): Builder { if (requestTimeout != null && !requestTimeout.isNegative) this.requestTimeout = requestTimeout; return this }
        fun putDefaultHeader(name: String, value: String): Builder { defaultHeaders[name] = value; return this }
        fun clearDefaultHeaders(): Builder { defaultHeaders.clear(); return this }
        fun setDefaultHeaders(headers: Map<String, String>?): Builder { clearDefaultHeaders(); headers?.forEach { (k, v) -> putDefaultHeader(k, v) }; return this }
        fun addObserver(observer: ClientObserver): Builder { observers.add(observer); return this }
        fun clearObservers(): Builder { observers.clear(); return this }
        fun setObservers(observers: List<ClientObserver>?): Builder { clearObservers(); observers?.forEach { addObserver(it) }; return this }
        fun setRetryPolicy(retryPolicy: RetryPolicy): Builder { this.retryPolicy = retryPolicy; return this }
        fun setPendingQueue(pendingQueue: PendingTransactionQueue?): Builder { this.pendingQueue = pendingQueue; return this }

        fun enableOfflineJournalQueue(journalPath: Path, key: OfflineJournalKey): Builder {
            try { pendingQueue = OfflineJournalPendingTransactionQueue(OfflineJournal(journalPath, key)) }
            catch (ex: OfflineJournalException) { throw IllegalStateException("Failed to initialize offline journal queue", ex) }
            return this
        }
        fun enableOfflineJournalQueue(journalPath: Path, seed: ByteArray): Builder = enableOfflineJournalQueue(journalPath, OfflineJournalKey.derive(seed))
        fun enableOfflineJournalQueue(journalPath: Path, passphrase: CharArray): Builder = enableOfflineJournalQueue(journalPath, OfflineJournalKey.deriveFromPassphrase(passphrase))

        fun enableDirectoryPendingQueue(rootDir: Path): Builder {
            try { pendingQueue = DirectoryPendingTransactionQueue(rootDir) }
            catch (ex: IOException) { throw IllegalStateException("Failed to initialise directory pending queue", ex) }
            return this
        }

        fun enableFilePendingQueue(queueFile: Path): Builder {
            try { pendingQueue = FilePendingTransactionQueue(queueFile) }
            catch (ex: IOException) { throw IllegalStateException("Failed to initialise file pending queue", ex) }
            return this
        }

        fun setExportOptions(exportOptions: ExportOptions?): Builder { this.exportOptions = exportOptions; return this }
        fun setNoritoRpcFlowController(flowController: NoritoRpcFlowController): Builder { this.noritoRpcFlowController = flowController; return this }
        fun setNoritoRpcMaxConcurrentRequests(maxConcurrentRequests: Int): Builder { this.noritoRpcFlowController = NoritoRpcFlowController.semaphore(maxConcurrentRequests); return this }
        fun setTelemetryOptions(telemetryOptions: TelemetryOptions): Builder { this.telemetryOptions = telemetryOptions; return this }
        fun setTelemetrySink(telemetrySink: TelemetrySink?): Builder { this.telemetrySink = telemetrySink; return this }
        fun setTelemetryExporterName(exporterName: String?): Builder { this.telemetryExporterName = exporterName?.trim(); return this }
        fun setNetworkContextProvider(provider: NetworkContextProvider): Builder { this.networkContextProvider = provider; return this }
        fun setDeviceProfileProvider(provider: DeviceProfileProvider): Builder { this.deviceProfileProvider = provider; return this }
        fun enableCrashTelemetryHandler(): Builder { crashTelemetryEnabled = true; return this }
        internal fun setCrashTelemetryEnabled(enabled: Boolean): Builder { crashTelemetryEnabled = enabled; return this }
        fun setCrashTelemetryMetadataProvider(metadataProvider: MetadataProvider): Builder { this.crashMetadataProvider = metadataProvider; return this }
        fun build(): ClientConfig = ClientConfig(this)

        internal fun resolveTelemetryExporterName(): String {
            val candidate = telemetryExporterName?.trim() ?: ""
            if (candidate.isNotEmpty()) return candidate
            if (telemetrySink != null) {
                val simpleName = telemetrySink!!.javaClass.simpleName
                if (!simpleName.isNullOrEmpty()) return simpleName
            }
            return "android_sdk"
        }
    }

    /** Options controlling deterministic key exports for queued transactions. */
    class ExportOptions private constructor(
        private val keyManager: KeyProvider,
        private val passphraseProvider: PassphraseProvider?
    ) {
        fun keyManager(): KeyProvider = keyManager

        internal fun withTelemetry(telemetry: KeystoreTelemetryEmitter?): ExportOptions {
            if (telemetry == null) return this
            return this
        }

        fun passphraseForAlias(alias: String): CharArray {
            if (passphraseProvider == null) return CharArray(0)
            return passphraseProvider.passphraseForAlias(alias) ?: CharArray(0)
        }

        fun interface PassphraseProvider {
            fun passphraseForAlias(alias: String): CharArray?
        }

        class Builder {
            private var keyManager: KeyProvider? = null
            private var passphraseProvider: PassphraseProvider? = null
            fun setKeyManager(keyManager: KeyProvider): Builder { this.keyManager = keyManager; return this }
            fun setPassphrase(passphrase: CharArray?): Builder {
                if (passphrase == null) { passphraseProvider = null } else { val base = passphrase.clone(); passphraseProvider = PassphraseProvider { base.clone() } }
                return this
            }
            fun setPassphraseProvider(provider: PassphraseProvider?): Builder { passphraseProvider = provider; return this }
            fun build(): ExportOptions = ExportOptions(keyManager!!, passphraseProvider)
        }

        companion object {
            @JvmStatic fun builder(): Builder = Builder()
        }
    }

    companion object {
        @JvmStatic fun builder(): Builder = Builder()
    }
}
