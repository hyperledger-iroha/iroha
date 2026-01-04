package org.hyperledger.iroha.android.client;

import java.io.IOException;
import java.net.URI;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.client.queue.DirectoryPendingTransactionQueue;
import org.hyperledger.iroha.android.client.queue.FilePendingTransactionQueue;
import org.hyperledger.iroha.android.client.queue.OfflineJournalPendingTransactionQueue;
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue;
import org.hyperledger.iroha.android.offline.OfflineJournal;
import org.hyperledger.iroha.android.offline.OfflineJournalException;
import org.hyperledger.iroha.android.offline.OfflineJournalKey;
import org.hyperledger.iroha.android.telemetry.AndroidDeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.AndroidNetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.CrashTelemetryHandler;
import org.hyperledger.iroha.android.telemetry.CrashTelemetryReporter;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.NetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryExportStatusSink;
import org.hyperledger.iroha.android.telemetry.KeystoreTelemetryEmitter;
import org.hyperledger.iroha.android.telemetry.TelemetryObserver;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;

/** Configuration options for {@link IrohaClient} implementations. */
public final class ClientConfig {
  private final URI baseUri;
  private final URI sorafsGatewayUri;
  private final Duration requestTimeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;
  private final RetryPolicy retryPolicy;
  private final PendingTransactionQueue pendingQueue;
  private final ExportOptions exportOptions;
  private final NoritoRpcFlowController noritoRpcFlowController;
  private final TelemetryOptions telemetryOptions;
  private final TelemetrySink telemetrySink;
  private final NetworkContextProvider networkContextProvider;
  private final DeviceProfileProvider deviceProfileProvider;
  private final String telemetryExporterName;
  private final boolean crashTelemetryEnabled;
  private final CrashTelemetryHandler.MetadataProvider crashMetadataProvider;
  private final CrashTelemetryHandler crashTelemetryHandler;

  private ClientConfig(final Builder builder) {
    this.baseUri = builder.baseUri;
    this.sorafsGatewayUri =
        builder.sorafsGatewayUri != null ? builder.sorafsGatewayUri : builder.baseUri;
    this.requestTimeout = builder.requestTimeout;
    this.defaultHeaders = Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    final List<ClientObserver> observerList = new ArrayList<>(builder.observers);
    final String resolvedExporterName = builder.resolveTelemetryExporterName();
    final TelemetrySink instrumentedSink =
        builder.telemetrySink == null
            ? null
            : TelemetryExportStatusSink.wrap(builder.telemetrySink, resolvedExporterName);
    if (builder.telemetryOptions.enabled() && instrumentedSink != null) {
      observerList.add(new TelemetryObserver(builder.telemetryOptions, instrumentedSink));
    }
    this.observers = List.copyOf(observerList);
    this.retryPolicy = builder.retryPolicy;
    this.pendingQueue = builder.pendingQueue;
    final KeystoreTelemetryEmitter keystoreTelemetry =
        KeystoreTelemetryEmitter.from(
            builder.telemetryOptions, instrumentedSink, builder.deviceProfileProvider);
    this.exportOptions =
        builder.exportOptions == null
            ? null
            : builder.exportOptions.withTelemetry(keystoreTelemetry);
    this.noritoRpcFlowController =
        builder.noritoRpcFlowController != null
            ? builder.noritoRpcFlowController
            : NoritoRpcFlowController.unlimited();
    this.telemetryOptions = builder.telemetryOptions;
    this.telemetrySink = instrumentedSink;
    this.telemetryExporterName = resolvedExporterName;
    this.networkContextProvider = builder.networkContextProvider;
    this.deviceProfileProvider = builder.deviceProfileProvider;
    this.crashTelemetryEnabled = builder.crashTelemetryEnabled;
    this.crashMetadataProvider = builder.crashMetadataProvider;
    this.crashTelemetryHandler = maybeInstallCrashTelemetryHandler(builder, instrumentedSink);
  }

  public URI baseUri() {
    return baseUri;
  }

  /** Base URI used for SoraFS gateway requests. Defaults to {@link #baseUri()} when unset. */
  public URI sorafsGatewayUri() {
    return sorafsGatewayUri;
  }

  public Duration requestTimeout() {
    return requestTimeout;
  }

  public Builder toBuilder() {
    final List<ClientObserver> nonTelemetryObservers = new ArrayList<>();
    for (final ClientObserver observer : observers) {
      if (!(observer instanceof TelemetryObserver)) {
        nonTelemetryObservers.add(observer);
      }
    }
    return new Builder()
        .setBaseUri(baseUri)
        .setSorafsGatewayUri(sorafsGatewayUri)
        .setRequestTimeout(requestTimeout)
        .setDefaultHeaders(defaultHeaders)
        .setObservers(nonTelemetryObservers)
        .setRetryPolicy(retryPolicy)
        .setPendingQueue(pendingQueue)
        .setExportOptions(exportOptions)
        .setNoritoRpcFlowController(noritoRpcFlowController)
        .setTelemetryOptions(telemetryOptions)
        .setTelemetrySink(TelemetryExportStatusSink.unwrap(telemetrySink))
        .setTelemetryExporterName(telemetryExporterName)
        .setNetworkContextProvider(networkContextProvider)
        .setDeviceProfileProvider(deviceProfileProvider)
        .setCrashTelemetryMetadataProvider(crashMetadataProvider)
        .setCrashTelemetryEnabled(crashTelemetryEnabled);
  }

  public static Builder builder() {
    return new Builder();
  }

  /** Headers that will be applied to every Torii request. */
  public Map<String, String> defaultHeaders() {
    return defaultHeaders;
  }

  /** Registered observers that receive request lifecycle callbacks. */
  public List<ClientObserver> observers() {
    return observers;
  }

  public RetryPolicy retryPolicy() {
    return retryPolicy;
  }

  public PendingTransactionQueue pendingQueue() {
    return pendingQueue;
  }

  public ExportOptions exportOptions() {
    return exportOptions;
  }

  public NoritoRpcFlowController noritoRpcFlowController() {
    return noritoRpcFlowController;
  }

  /**
   * Creates a {@link NoritoRpcClient} backed by the provided {@link HttpTransportExecutor}, ensuring
   * the RPC calls share the same telemetry/interceptor stack as {@link HttpClientTransport}.
   */
  public NoritoRpcClient toNoritoRpcClient(final HttpTransportExecutor executor) {
    final NoritoRpcClient.Builder builder = toNoritoRpcClientBuilder();
    if (executor != null) {
      builder.setTransportExecutor(executor);
    }
    return builder.build();
  }

  /** Convenience overload that creates a Norito RPC client with the default HTTP implementation. */
  public NoritoRpcClient toNoritoRpcClient() {
    return toNoritoRpcClient(PlatformHttpTransportExecutor.createDefault());
  }

  private NoritoRpcClient.Builder toNoritoRpcClientBuilder() {
    final NoritoRpcClient.Builder builder =
        NoritoRpcClient.builder()
            .setBaseUri(baseUri)
            .setTimeout(requestTimeout)
            .defaultHeaders(defaultHeaders)
            .observers(observers)
            .setTelemetryOptions(telemetryOptions)
            .setTelemetrySink(telemetrySink)
            .setNetworkContextProvider(networkContextProvider)
            .setDeviceProfileProvider(deviceProfileProvider)
            .setFlowController(noritoRpcFlowController);
    return builder;
  }

  /** Returns the telemetry configuration (may be disabled). */
  public TelemetryOptions telemetryOptions() {
    return telemetryOptions;
  }

  /** Returns the configured telemetry sink when provided. */
  public Optional<TelemetrySink> telemetrySink() {
    return Optional.ofNullable(telemetrySink);
  }

  /** Returns the configured exporter name used by {@code android.telemetry.export.status}. */
  public String telemetryExporterName() {
    return telemetryExporterName;
  }

  /** Returns the provider used to capture {@code android.telemetry.network_context}. */
  public NetworkContextProvider networkContextProvider() {
    return networkContextProvider;
  }

  /** Returns the provider used to emit {@code android.telemetry.device_profile}. */
  public DeviceProfileProvider deviceProfileProvider() {
    return deviceProfileProvider;
  }

  /** Returns the crash telemetry handler when one is installed. */
  public Optional<CrashTelemetryHandler> crashTelemetryHandler() {
    return Optional.ofNullable(crashTelemetryHandler);
  }

  /**
   * Returns a {@link CrashTelemetryReporter} that shares this config's telemetry configuration so
   * crash upload code can emit {@code android.crash.report.upload}.
   */
  public Optional<CrashTelemetryReporter> crashTelemetryReporter() {
    if (!telemetryOptions.enabled() || telemetrySink == null) {
      return Optional.empty();
    }
    return Optional.of(new CrashTelemetryReporter(telemetryOptions, telemetrySink));
  }

  /**
   * Creates an {@link OfflineToriiClient} that reuses this config's base URI, timeout, headers, and
   * observers. Callers must provide the executor so transports can share the same HTTP stack.
   */
  public OfflineToriiClient toOfflineToriiClient(final HttpTransportExecutor executor) {
    Objects.requireNonNull(executor, "executor");
    return OfflineToriiClient.builder()
        .executor(executor)
        .baseUri(baseUri)
        .timeout(requestTimeout)
        .defaultHeaders(defaultHeaders)
        .observers(observers)
        .build();
  }

  private CrashTelemetryHandler maybeInstallCrashTelemetryHandler(
      final Builder builder, final TelemetrySink sink) {
    if (!builder.crashTelemetryEnabled) {
      return null;
    }
    if (!builder.telemetryOptions.enabled() || sink == null) {
      return null;
    }
    return CrashTelemetryHandler.install(
        builder.telemetryOptions, sink, builder.crashMetadataProvider);
  }

  public static final class Builder {
    private URI baseUri = URI.create("http://localhost:8080");
    private URI sorafsGatewayUri;
    private Duration requestTimeout = Duration.ofSeconds(10);
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private final List<ClientObserver> observers = new ArrayList<>();
    private RetryPolicy retryPolicy = RetryPolicy.none();
    private PendingTransactionQueue pendingQueue;
    private ExportOptions exportOptions;
    private NoritoRpcFlowController noritoRpcFlowController =
        NoritoRpcFlowController.unlimited();
    private TelemetryOptions telemetryOptions = TelemetryOptions.disabled();
    private TelemetrySink telemetrySink;
    private String telemetryExporterName;
    private NetworkContextProvider networkContextProvider = NetworkContextProvider.disabled();
    private DeviceProfileProvider deviceProfileProvider = DeviceProfileProvider.disabled();
    private boolean crashTelemetryEnabled;
    private CrashTelemetryHandler.MetadataProvider crashMetadataProvider =
        CrashTelemetryHandler.defaultMetadataProvider();

    public Builder setBaseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    public Builder setSorafsGatewayUri(final URI sorafsGatewayUri) {
      this.sorafsGatewayUri = Objects.requireNonNull(sorafsGatewayUri, "sorafsGatewayUri");
      return this;
    }

    public Builder setRequestTimeout(final Duration requestTimeout) {
      if (requestTimeout != null && !requestTimeout.isNegative()) {
        this.requestTimeout = requestTimeout;
      }
      return this;
    }

    public Builder putDefaultHeader(final String name, final String value) {
      defaultHeaders.put(
          Objects.requireNonNull(name, "name"),
          Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder clearDefaultHeaders() {
      defaultHeaders.clear();
      return this;
    }

    public Builder setDefaultHeaders(final Map<String, String> headers) {
      clearDefaultHeaders();
      if (headers != null) {
        headers.forEach(this::putDefaultHeader);
      }
      return this;
    }

    public Builder addObserver(final ClientObserver observer) {
      observers.add(Objects.requireNonNull(observer, "observer"));
      return this;
    }

    public Builder clearObservers() {
      observers.clear();
      return this;
    }

    public Builder setObservers(final List<ClientObserver> observers) {
      clearObservers();
      if (observers != null) {
        observers.forEach(this::addObserver);
      }
      return this;
    }

    public Builder setRetryPolicy(final RetryPolicy retryPolicy) {
      this.retryPolicy = Objects.requireNonNull(retryPolicy, "retryPolicy");
      return this;
    }

    public Builder setPendingQueue(final PendingTransactionQueue pendingQueue) {
      this.pendingQueue = pendingQueue;
      return this;
    }

    public Builder enableOfflineJournalQueue(
        final Path journalPath, final OfflineJournalKey key) {
      Objects.requireNonNull(journalPath, "journalPath");
      Objects.requireNonNull(key, "key");
      try {
        final OfflineJournal journal = new OfflineJournal(journalPath, key);
        this.pendingQueue = new OfflineJournalPendingTransactionQueue(journal);
      } catch (final OfflineJournalException ex) {
        throw new IllegalStateException("Failed to initialize offline journal queue", ex);
      }
      return this;
    }

    public Builder enableOfflineJournalQueue(final Path journalPath, final byte[] seed) {
      return enableOfflineJournalQueue(journalPath, OfflineJournalKey.derive(seed));
    }

    public Builder enableOfflineJournalQueue(final Path journalPath, final char[] passphrase) {
      return enableOfflineJournalQueue(
          journalPath, OfflineJournalKey.deriveFromPassphrase(passphrase));
    }

    /**
     * Enables a directory-backed pending queue that persists each transaction as its own envelope
     * file under {@code rootDir}, retaining insertion order across process restarts for OEM/MDM
     * storage policies.
     */
    public Builder enableDirectoryPendingQueue(final Path rootDir) {
      Objects.requireNonNull(rootDir, "rootDir");
      try {
        this.pendingQueue = new DirectoryPendingTransactionQueue(rootDir);
      } catch (final IOException ex) {
        throw new IllegalStateException("Failed to initialise directory pending queue", ex);
      }
      return this;
    }

    /**
     * Enables a file-backed pending queue that persists signed transactions across restarts. The
     * queue stores one Base64-encoded envelope per line and mirrors the Swift/CLI layout so
     * operators can inspect or replay entries.
     */
    public Builder enableFilePendingQueue(final Path queueFile) {
      Objects.requireNonNull(queueFile, "queueFile");
      try {
        this.pendingQueue = new FilePendingTransactionQueue(queueFile);
      } catch (final IOException ex) {
        throw new IllegalStateException("Failed to initialise file pending queue", ex);
      }
      return this;
    }

    public Builder setExportOptions(final ExportOptions exportOptions) {
      this.exportOptions = exportOptions;
      return this;
    }

    public Builder setNoritoRpcFlowController(final NoritoRpcFlowController flowController) {
      this.noritoRpcFlowController = Objects.requireNonNull(flowController, "flowController");
      return this;
    }

    public Builder setNoritoRpcMaxConcurrentRequests(final int maxConcurrentRequests) {
      this.noritoRpcFlowController =
          NoritoRpcFlowController.semaphore(maxConcurrentRequests);
      return this;
    }

    public Builder setTelemetryOptions(final TelemetryOptions telemetryOptions) {
      this.telemetryOptions = Objects.requireNonNull(telemetryOptions, "telemetryOptions");
      return this;
    }

    /**
     * Registers a {@link TelemetrySink}. When paired with an enabled {@link TelemetryOptions}
     * instance the sink receives hashed authority events for every Torii request.
     */
    public Builder setTelemetrySink(final TelemetrySink telemetrySink) {
      this.telemetrySink = telemetrySink;
      return this;
    }

    /** Labels the exporter recorded in {@code android.telemetry.export.status}. */
    public Builder setTelemetryExporterName(final String exporterName) {
      this.telemetryExporterName = exporterName == null ? null : exporterName.trim();
      return this;
    }

    public Builder setNetworkContextProvider(final NetworkContextProvider provider) {
      this.networkContextProvider = Objects.requireNonNull(provider, "networkContextProvider");
      return this;
    }

    public Builder setDeviceProfileProvider(final DeviceProfileProvider provider) {
      this.deviceProfileProvider = Objects.requireNonNull(provider, "deviceProfileProvider");
      return this;
    }

    public Builder enableAndroidDeviceProfileProvider() {
      return setDeviceProfileProvider(AndroidDeviceProfileProvider.create());
    }

    /**
     * Convenience helper that registers the built-in Android {@link NetworkContextProvider}.
     *
     * <p>The {@code androidContext} parameter must be an instance of
     * {@code android.content.Context}; the provider reflects into Android's connectivity APIs so the
     * SDK can emit `android.telemetry.network_context` without introducing hard dependencies on the
     * Android SDK during library builds.</p>
     */
    public Builder enableAndroidNetworkContext(final Object androidContext) {
      return setNetworkContextProvider(AndroidNetworkContextProvider.fromContext(androidContext));
    }

    /**
     * Enables crash telemetry by installing the default handler that records {@code
     * android.crash.report.capture}.
     */
    public Builder enableCrashTelemetryHandler() {
      this.crashTelemetryEnabled = true;
      return this;
    }

    Builder setCrashTelemetryEnabled(final boolean enabled) {
      this.crashTelemetryEnabled = enabled;
      return this;
    }

    /** Overrides the metadata provider used by the installed crash telemetry handler. */
    public Builder setCrashTelemetryMetadataProvider(
        final CrashTelemetryHandler.MetadataProvider metadataProvider) {
      this.crashMetadataProvider =
          Objects.requireNonNull(metadataProvider, "metadataProvider");
      return this;
    }

    public ClientConfig build() {
      return new ClientConfig(this);
    }

    private String resolveTelemetryExporterName() {
      final String candidate = telemetryExporterName == null ? "" : telemetryExporterName.trim();
      if (!candidate.isEmpty()) {
        return candidate;
      }
      if (telemetrySink != null) {
        final String simpleName = telemetrySink.getClass().getSimpleName();
        if (simpleName != null && !simpleName.isEmpty()) {
          return simpleName;
        }
      }
      return "android_sdk";
    }
  }

  /** Options controlling deterministic key exports for queued transactions. */
  public static final class ExportOptions {
    private final IrohaKeyManager keyManager;
    private final PassphraseProvider passphraseProvider;

    private ExportOptions(final Builder builder) {
      this(builder.keyManager, builder.passphraseProvider);
    }

    private ExportOptions(
        final IrohaKeyManager keyManager, final PassphraseProvider passphraseProvider) {
      this.keyManager = Objects.requireNonNull(keyManager, "keyManager");
      this.passphraseProvider = passphraseProvider;
    }

    public IrohaKeyManager keyManager() {
      return keyManager;
    }

    ExportOptions withTelemetry(final KeystoreTelemetryEmitter telemetry) {
      if (telemetry == null) {
        return this;
      }
      return new ExportOptions(keyManager.withTelemetry(telemetry), passphraseProvider);
    }

    public char[] passphraseForAlias(final String alias) {
      if (passphraseProvider == null) {
        return new char[0];
      }
      final char[] value = passphraseProvider.passphraseForAlias(alias);
      return value == null ? new char[0] : value;
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private IrohaKeyManager keyManager;
      private PassphraseProvider passphraseProvider;

      public Builder setKeyManager(final IrohaKeyManager keyManager) {
        this.keyManager = keyManager;
        return this;
      }

      public Builder setPassphrase(final char[] passphrase) {
        if (passphrase == null) {
          this.passphraseProvider = null;
        } else {
          final char[] base = passphrase.clone();
          this.passphraseProvider = alias -> base.clone();
        }
        return this;
      }

      public Builder setPassphraseProvider(final PassphraseProvider provider) {
        this.passphraseProvider = provider;
        return this;
      }

      public ExportOptions build() {
        return new ExportOptions(this);
      }
    }

    @FunctionalInterface
    public interface PassphraseProvider {
      char[] passphraseForAlias(String alias);
    }
  }
}
