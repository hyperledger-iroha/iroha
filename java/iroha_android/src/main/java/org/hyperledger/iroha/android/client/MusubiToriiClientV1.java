package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.Function;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasHistoryEntry;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocationPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocationQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactPackageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactReleaseSnapshot;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactReleaseQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.MaintainerDirectoryEntry;
import org.hyperledger.iroha.android.client.MusubiModelsV1.OrderedPrefixQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.OrderedPrefixPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackagePageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Page;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationKey;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationRecord;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Version;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Exact-network authenticated Torii client for the twelve typed Musubi queries. */
public final class MusubiToriiClientV1 {
  public static final String EXACT_PACKAGE_PATH = "/v1/musubi/queries/exact-package";
  public static final String EXACT_RELEASE_PATH = "/v1/musubi/queries/exact-release";
  public static final String PROVIDER_BUNDLE_ATTESTATION_PATH =
      "/v1/musubi/queries/provider-bundle-attestation";
  public static final String RESOLVER_INDEX_PATH = "/v1/musubi/queries/resolver-index";
  public static final String VERSIONS_PATH = "/v1/musubi/queries/versions";
  public static final String MAINTAINERS_PATH = "/v1/musubi/queries/maintainers";
  public static final String ARCHIVE_LOCATIONS_PATH = "/v1/musubi/queries/archive-locations";
  public static final String ARCHIVE_RETENTION_PATH = "/v1/musubi/queries/archive-retention";
  public static final String ALIAS_PATH = "/v1/musubi/queries/alias";
  public static final String ALIAS_HISTORY_PATH = "/v1/musubi/queries/alias-history";
  public static final String ORDERED_PREFIX_PATH = "/v1/musubi/queries/ordered-prefix";
  public static final String SEARCH_PATH = "/v1/musubi/queries/search";

  private static final int REQUEST_MAX_BYTES = 64 * 1024;
  // Exact-release JSON repeats the bounded dependency vector in both registry projections.
  private static final int RESPONSE_MAX_BYTES = 32 * 1024 * 1024;

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final LocalSigningContext localSigningContext;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;

  private MusubiToriiClientV1(final Builder builder) {
    this.executor = Objects.requireNonNull(builder.executor, "executor");
    this.baseUri = Objects.requireNonNull(builder.baseUri, "baseUri");
    if (builder.localSigningContext == null) {
      throw new IllegalStateException(
          "localSigningContext must be configured before building a Musubi client");
    }
    this.localSigningContext = builder.localSigningContext;
    this.timeout = builder.timeout;
    this.defaultHeaders =
        Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    this.observers = Collections.unmodifiableList(new ArrayList<>(builder.observers));
  }

  public static Builder builder() { return new Builder(); }

  /** Fetches one exact structural package record. */
  public CompletableFuture<PackageRecord> findExactPackage(
      final ExactPackageQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final ExactPackageQuery checked = required(request);
    return executePost(
        EXACT_PACKAGE_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final PackageRecord record = MusubiJsonV1.parseExactPackage(payload);
          record.requireMatches(checked);
          return record;
        });
  }

  /** Fetches paired home and universal projections for one exact release at finality. */
  public CompletableFuture<ExactReleaseSnapshot> findExactRelease(
      final ExactReleaseQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final ExactReleaseQuery checked = required(request);
    return executePost(
        EXACT_RELEASE_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final ExactReleaseSnapshot snapshot = MusubiJsonV1.parseExactRelease(payload);
          snapshot.requireMatches(checked);
          return snapshot;
        });
  }

  /** Fetches one immutable provider proof by its archive/order/provider identity. */
  public CompletableFuture<ProviderBundleAttestationRecord> findProviderBundleAttestation(
      final ProviderBundleAttestationKey request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final ProviderBundleAttestationKey checked = required(request);
    return executePost(
        PROVIDER_BUNDLE_ATTESTATION_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final ProviderBundleAttestationRecord record =
              MusubiJsonV1.parseProviderBundleAttestation(payload);
          record.requireMatches(checked);
          return record;
        });
  }

  /** Reads the finalized universal sparse resolver index. */
  public CompletableFuture<ResolverIndexPage> findResolverIndex(
      final ResolverIndexQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final ResolverIndexQuery checked = required(request);
    return executePost(
        RESOLVER_INDEX_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final ResolverIndexPage page = MusubiJsonV1.parseResolverPage(payload);
          page.requireMatches(checked);
          return page;
        });
  }

  /** Lists exact structured versions for a package. */
  public CompletableFuture<Page<Version>> findVersions(
      final PackagePageQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final PackagePageQuery checked = required(request);
    return executePost(
        VERSIONS_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final Page<Version> page = MusubiJsonV1.parseVersionPage(payload);
          page.requireVersionMatches(checked);
          return page;
        });
  }

  /** Lists accepted owners/maintainers and pending invitations for a package. */
  public CompletableFuture<Page<MaintainerDirectoryEntry>> findMaintainers(
      final PackagePageQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final PackagePageQuery checked = required(request);
    return executePost(
        MAINTAINERS_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final Page<MaintainerDirectoryEntry> page = MusubiJsonV1.parseMaintainerPage(payload);
          page.requireMaintainerMatches(checked);
          return page;
        });
  }

  /** Lists renewable SoraFS locations for an archive. */
  public CompletableFuture<ArchiveLocationPage> findArchiveLocations(
      final ArchiveLocationQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final ArchiveLocationQuery checked = required(request);
    return executePost(
        ARCHIVE_LOCATIONS_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final ArchiveLocationPage page = MusubiJsonV1.parseArchiveLocationPage(payload);
          page.requireMatches(checked);
          return page;
        });
  }

  /** Classifies a bounded exact archive batch for fail-closed cache retention. */
  public CompletableFuture<ArchiveRetentionPage> findArchiveRetention(
      final ArchiveRetentionQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final ArchiveRetentionQuery checked = required(request);
    return executePost(
        ARCHIVE_RETENTION_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final ArchiveRetentionPage page = MusubiJsonV1.parseArchiveRetentionPage(payload);
          page.requireMatches(checked);
          return page;
        });
  }

  /** Resolves one paid permanent global alias. */
  public CompletableFuture<AliasRecord> findAlias(
      final AliasQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final AliasQuery checked = required(request);
    return executePost(
        ALIAS_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final AliasRecord record = MusubiJsonV1.parseAlias(payload);
          record.requireMatches(checked);
          return record;
        });
  }

  /** Lists immutable history for one permanent global alias. */
  public CompletableFuture<Page<AliasHistoryEntry>> findAliasHistory(
      final AliasQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final AliasQuery checked = required(request);
    return executePost(
        ALIAS_HISTORY_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final Page<AliasHistoryEntry> page = MusubiJsonV1.parseAliasHistoryPage(payload);
          page.requireAliasHistoryMatches(checked);
          return page;
        });
  }

  /** Scans the deterministic public package directory by byte prefix. */
  public CompletableFuture<OrderedPrefixPage> findOrderedPrefix(
      final OrderedPrefixQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final OrderedPrefixQuery checked = required(request);
    return executePost(
        ORDERED_PREFIX_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final OrderedPrefixPage page = MusubiJsonV1.parseOrderedPackagePage(payload);
          page.requireMatches(checked);
          return page;
        });
  }

  /** Searches the rebuildable finalized-event package metadata projection. */
  public CompletableFuture<SearchPage> search(
      final SearchQuery request, final ToriiCanonicalRequestAuth canonicalAuth) {
    final SearchQuery checked = required(request);
    return executePost(
        SEARCH_PATH,
        checked.toJsonBytes(),
        requiredAuth(canonicalAuth),
        payload -> {
          final SearchPage page = MusubiJsonV1.parseSearchPage(payload);
          page.requireMatches(checked);
          return page;
        });
  }

  public HttpTransportExecutor executor() { return executor; }

  private static <T> T required(final T value) { return Objects.requireNonNull(value, "request"); }
  private static ToriiCanonicalRequestAuth requiredAuth(
      final ToriiCanonicalRequestAuth value) {
    return Objects.requireNonNull(value, "canonicalAuth");
  }

  private <T> CompletableFuture<T> executePost(
      final String path,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final Function<byte[], T> parser) {
    if (body.length > REQUEST_MAX_BYTES) {
      throw new IllegalArgumentException(
          "Musubi request exceeds the " + REQUEST_MAX_BYTES + "-byte route limit");
    }
    final TransportRequest request = buildRequest(path, body, canonicalAuth);
    notifyRequest(request);
    return executor.execute(request).handle(
        (response, throwable) -> {
          if (throwable != null) {
            final Throwable cause = throwable instanceof CompletionException
                ? throwable.getCause() : throwable;
            final MusubiToriiException error =
                new MusubiToriiException("Musubi V1 request failed", cause == null ? throwable : cause);
            notifyFailure(request, error);
            throw new CompletionException(error);
          }
          try {
            return parseResponse(request, response, parser);
          } catch (final RuntimeException error) {
            final MusubiToriiException wrapped = error instanceof MusubiToriiException
                ? (MusubiToriiException) error
                : new MusubiToriiException("Failed to decode strict Musubi V1 response", error);
            notifyFailure(request, wrapped);
            throw new CompletionException(wrapped);
          }
        });
  }

  private <T> T parseResponse(
      final TransportRequest request,
      final TransportResponse response,
      final Function<byte[], T> parser) {
    final byte[] body = response.body();
    final String rejectCode = HttpErrorMessageExtractor.extractRejectCode(
        response.headers(), "x-iroha-reject-code", body);
    final ClientResponse clientResponse =
        new ClientResponse(response.statusCode(), body, response.message(), null, rejectCode);
    if (response.statusCode() < 200 || response.statusCode() >= 300) {
      final String extracted = HttpErrorMessageExtractor.extractMessage(body);
      throw new MusubiToriiException(
          "Musubi V1 request failed with HTTP " + response.statusCode() + ": "
              + (extracted == null ? response.message() : extracted));
    }
    if (body.length > RESPONSE_MAX_BYTES) {
      throw new MusubiToriiException(
          "Musubi response exceeds the " + RESPONSE_MAX_BYTES + "-byte client limit");
    }
    if (!hasJsonContentType(response.headers())) {
      throw new MusubiToriiException("Musubi response must use application/json");
    }
    final T parsed = parser.apply(body);
    notifyResponse(request, clientResponse);
    return parsed;
  }

  private TransportRequest buildRequest(
      final String path,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final String normalized = path.startsWith("/") ? path.substring(1) : path;
    final String base = baseUri.toString();
    final URI target = URI.create(base.endsWith("/") ? base + normalized : base + "/" + normalized);
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    ensureHeader(headers, "Accept", "application/json");
    ensureHeader(headers, "Content-Type", "application/json");
    requireCanonicalHeadersUnset(headers);
    headers.putAll(buildCanonicalHeaders(target, body, canonicalAuth));
    TransportSecurity.requireHttpRequestAllowed(
        "MusubiToriiClientV1", baseUri, target, headers, body);
    final TransportRequest.Builder builder = TransportRequest.builder()
        .setUri(target)
        .setMethod("POST")
        .setBody(body)
        .setTimeout(timeout)
        .setMaximumResponseBytes(Long.valueOf(RESPONSE_MAX_BYTES));
    for (final Map.Entry<String, String> header : headers.entrySet()) {
      builder.addHeader(header.getKey(), header.getValue());
    }
    return builder.build();
  }

  private Map<String, String> buildCanonicalHeaders(
      final URI target,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final Long timestampMs = canonicalAuth.timestampMs();
    final String nonce = canonicalAuth.nonce();
    if ((timestampMs == null) != (nonce == null)) {
      throw new IllegalArgumentException("timestampMs and nonce must be provided together");
    }
    if (timestampMs == null) {
      return CanonicalRequestSigner.buildHeaders(
          localSigningContext.networkId(), "POST", target, body, canonicalAuth);
    }
    return CanonicalRequestSigner.buildHeaders(
        localSigningContext.networkId(),
        "POST",
        target,
        body,
        canonicalAuth,
        timestampMs.longValue(),
        nonce);
  }

  private static void requireCanonicalHeadersUnset(final Map<String, String> headers) {
    for (final String candidate : headers.keySet()) {
      if (candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_ACCOUNT)
          || candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_SIGNATURE)
          || candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_TIMESTAMP_MS)
          || candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_NONCE)
          || candidate.equalsIgnoreCase("X-Iroha-Witness")) {
        throw new IllegalArgumentException(
            "canonical request headers must be supplied only through canonicalAuth");
      }
    }
  }

  private void notifyRequest(final TransportRequest request) {
    for (final ClientObserver observer : observers) observer.onRequest(request);
  }

  private void notifyResponse(final TransportRequest request, final ClientResponse response) {
    for (final ClientObserver observer : observers) observer.onResponse(request, response);
  }

  private void notifyFailure(final TransportRequest request, final Throwable error) {
    for (final ClientObserver observer : observers) observer.onFailure(request, error);
  }

  private static void ensureHeader(
      final Map<String, String> headers, final String name, final String value) {
    String existing = null;
    for (final String key : headers.keySet()) {
      if (key.equalsIgnoreCase(name)) { existing = key; break; }
    }
    headers.put(existing == null ? name : existing, value);
  }

  private static boolean hasJsonContentType(final Map<String, List<String>> headers) {
    for (final Map.Entry<String, List<String>> header : headers.entrySet()) {
      if (!header.getKey().equalsIgnoreCase("Content-Type")) continue;
      for (final String value : header.getValue()) {
        if (value.split(";", 2)[0].trim().equalsIgnoreCase("application/json")) return true;
      }
    }
    return false;
  }

  /** Builder with SDK transport injection for Android/JVM parity. */
  public static final class Builder {
    private HttpTransportExecutor executor;
    private URI baseUri = URI.create("http://localhost:8080");
    private LocalSigningContext localSigningContext;
    private Duration timeout = Duration.ofSeconds(15);
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private final List<ClientObserver> observers = new ArrayList<>();

    public Builder executor(final HttpTransportExecutor executor) {
      this.executor = executor; return this;
    }
    public Builder baseUri(final URI baseUri) { this.baseUri = baseUri; return this; }
    public Builder localSigningContext(final LocalSigningContext localSigningContext) {
      this.localSigningContext = Objects.requireNonNull(localSigningContext, "localSigningContext");
      return this;
    }
    public Builder timeout(final Duration timeout) { this.timeout = timeout; return this; }
    public Builder addHeader(final String name, final String value) {
      defaultHeaders.put(name, value); return this;
    }
    public Builder addObserver(final ClientObserver observer) {
      if (observer != null) observers.add(observer); return this;
    }
    public MusubiToriiClientV1 build() {
      if (executor == null) executor = PlatformHttpTransportExecutor.createDefault();
      return new MusubiToriiClientV1(this);
    }
  }
}

/** Failure returned by {@link MusubiToriiClientV1}. */
final class MusubiToriiException extends RuntimeException {
  MusubiToriiException(final String message) { super(message); }
  MusubiToriiException(final String message, final Throwable cause) { super(message, cause); }
}
