package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.Consumer;
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocationQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveRetentionQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ComparatorOp;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactPackageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactReleaseQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.MaintainerDirectoryEntry;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Namespace;
import org.hyperledger.iroha.android.client.MusubiModelsV1.OrderedPrefixQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageName;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackagePageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ProviderBundleAttestationKey;
import org.hyperledger.iroha.android.client.MusubiModelsV1.RegistrySnapshot;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ReleaseId;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexPage;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverReleaseRow;
import org.hyperledger.iroha.android.client.MusubiModelsV1.SearchQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Version;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionComparator;
import org.hyperledger.iroha.android.client.MusubiModelsV1.VersionReq;
import org.hyperledger.iroha.android.client.MusubiModelsV1.WireValue;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.junit.Test;

/** Cross-SDK checks for the Rust-owned Musubi first-release JSON fixture. */
public final class MusubiSdkV1FixtureTests {
  private static final Set<String> EXPECTED_PATHS =
      new LinkedHashSet<>(
          Arrays.asList(
              MusubiToriiClientV1.EXACT_PACKAGE_PATH,
              MusubiToriiClientV1.EXACT_RELEASE_PATH,
              MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH,
              MusubiToriiClientV1.RESOLVER_INDEX_PATH,
              MusubiToriiClientV1.VERSIONS_PATH,
              MusubiToriiClientV1.MAINTAINERS_PATH,
              MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
              MusubiToriiClientV1.ARCHIVE_RETENTION_PATH,
              MusubiToriiClientV1.ALIAS_PATH,
              MusubiToriiClientV1.ALIAS_HISTORY_PATH,
              MusubiToriiClientV1.ORDERED_PREFIX_PATH,
              MusubiToriiClientV1.SEARCH_PATH));

  @Test
  public void canonicalNamesVersionsAndRequirementsMatchRustFixture() throws Exception {
    final Map<String, Object> root = fixture();
    assertEquals("iroha-musubi-sdk-v1", root.get("format"));
    assertEquals(Long.valueOf(1L), root.get("fixture_version"));
    assertEquals("iroha_data_model::musubi", root.get("rust_owner"));

    final Map<String, Object> canonical = object(root.get("canonical"));
    assertWireEquals(canonical.get("namespace"), new Namespace(newtypeText(canonical.get("namespace"))));
    assertWireEquals(
        canonical.get("package_name"), new PackageName(newtypeText(canonical.get("package_name"))));

    final Version version = Version.parse("1.2.3-rc.1");
    assertEquals("1.2.3-rc.1", version.canonicalText());
    assertWireEquals(canonical.get("version"), version);

    for (final Object raw : array(canonical.get("requirements"))) {
      final Map<String, Object> item = object(raw);
      assertWireEquals(item.get("wire"), VersionReq.parse((String) item.get("text")));
    }
    for (final Object raw : array(canonical.get("requirement_aliases"))) {
      final Map<String, Object> item = object(raw);
      final VersionReq requirement = VersionReq.parse((String) item.get("input"));
      assertEquals(item.get("canonical"), requirement.canonicalText());
      assertWireEquals(item.get("wire"), requirement);
    }
    for (final Object raw : array(canonical.get("requirement_matches"))) {
      final Map<String, Object> item = object(raw);
      final VersionReq requirement = VersionReq.parse((String) item.get("requirement"));
      final Version candidate = Version.parse((String) item.get("candidate"));
      assertEquals(item.get("matches"), Boolean.valueOf(requirement.matches(candidate)));
    }
  }

  @Test
  public void decodedComparatorRequirementsRejectNoncanonicalExactForms() {
    final VersionComparator first =
        new VersionComparator(ComparatorOp.EQUAL, Version.parse("1.0.0"));
    final VersionComparator second =
        new VersionComparator(ComparatorOp.EQUAL, Version.parse("2.0.0"));
    expectFailure(
        () ->
            VersionReq.fromWire(
                VersionReq.Kind.COMPARATORS,
                null,
                null,
                null,
                Collections.singletonList(first)));
    expectFailure(
        () ->
            VersionReq.fromWire(
                VersionReq.Kind.COMPARATORS,
                null,
                null,
                null,
                Arrays.asList(first, second)));
  }

  @Test
  public void nameBackedFieldsRejectEveryUnicodeBidiControl() {
    final char[] controls = {
      '\u061c', '\u200e', '\u200f',
      '\u202a', '\u202b', '\u202c', '\u202d', '\u202e',
      '\u2066', '\u2067', '\u2068', '\u2069'
    };
    for (final char control : controls) {
      expectFailure(() -> new Namespace("domain" + control + ".dataspace"));
      expectFailure(() -> MusubiModelsV1.PackageScope.domain("domain" + control));
    }
  }

  @Test
  public void everyTypedRouteRoundTripsExactRequestAndResponseJson() throws Exception {
    final List<Map<String, Object>> routes = routes();
    final Set<String> actualPaths = new LinkedHashSet<>();
    for (final Map<String, Object> route : routes) {
      final String path = (String) route.get("path");
      actualPaths.add(path);
      final WireValue request = MusubiJsonV1.decodeQuery(path, route.get("request"));
      final WireValue response = MusubiJsonV1.decodeResponse(path, route.get("response"));
      assertWireEquals(route.get("request"), request);
      assertWireEquals(route.get("response"), response);
    }
    assertEquals(12, routes.size());
    assertEquals(EXPECTED_PATHS, actualPaths);
  }

  @Test
  public void archiveRetentionIsBoundedTypedAndBindsTheExactRequest() throws Exception {
    final Map<String, Object> route = route(MusubiToriiClientV1.ARCHIVE_RETENTION_PATH);
    final ArchiveRetentionQuery request =
        (ArchiveRetentionQuery) MusubiJsonV1.decodeQuery(
            MusubiToriiClientV1.ARCHIVE_RETENTION_PATH, route.get("request"));
    final ArchiveRetentionPage page = MusubiJsonV1.parseArchiveRetentionPage(
        JsonEncoder.encode(route.get("response")).getBytes(StandardCharsets.UTF_8));
    page.requireMatches(request);
    assertEquals(4, page.items().size());
    assertEquals(new BigInteger("1700000000000"), page.finalizedTimeMs());
    assertTrue(page.items().get(0).mustRetain());
    assertTrue(page.items().get(1).mustRetain());
    assertFalse(page.items().get(2).mustRetain());
    assertFalse(page.items().get(3).mustRetain());

    final Map<String, Object> zeroTime = object(deepMutableCopy(route.get("response")));
    zeroTime.put("finalized_time_ms", Long.valueOf(0L));
    MusubiJsonV1.parseArchiveRetentionPage(
            JsonEncoder.encode(zeroTime).getBytes(StandardCharsets.UTF_8))
        .requireMatches(request);
    final Map<String, Object> negativeTime = object(deepMutableCopy(route.get("response")));
    negativeTime.put("finalized_time_ms", Long.valueOf(-1L));
    expectFailure(
        () ->
            MusubiJsonV1.parseArchiveRetentionPage(
                JsonEncoder.encode(negativeTime).getBytes(StandardCharsets.UTF_8)));
    final Map<String, Object> missingTime = object(deepMutableCopy(route.get("response")));
    missingTime.remove("finalized_time_ms");
    expectFailure(
        () ->
            MusubiJsonV1.parseArchiveRetentionPage(
                JsonEncoder.encode(missingTime).getBytes(StandardCharsets.UTF_8)));

    final Map<String, Object> mismatched = object(deepMutableCopy(route.get("response")));
    final Map<String, Object> first = object(array(mismatched.get("items")).get(0));
    final List<Object> changedBytes = new ArrayList<>();
    for (int index = 0; index < 32; index++) changedBytes.add(Long.valueOf(17L));
    first.put("archive_id", Collections.<Object>singletonList(changedBytes));
    final ArchiveRetentionPage mismatchedPage = MusubiJsonV1.parseArchiveRetentionPage(
        JsonEncoder.encode(mismatched).getBytes(StandardCharsets.UTF_8));
    expectFailure(() -> mismatchedPage.requireMatches(request));
  }

  @Test
  public void archiveLocationPageRejectsNoncurrentOrUnorderedItems() throws Exception {
    final Map<String, Object> valid = populatedArchiveLocationResponse();
    final MusubiModelsV1.ArchiveLocationPage page =
        (MusubiModelsV1.ArchiveLocationPage)
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH, valid);
    assertEquals(2, page.items().size());
    assertEquals(BigInteger.valueOf(49L), page.items().get(0).finalizedHeight());
    assertEquals(1, page.items().get(0).providers().size());
    assertArrayEquals(
        repeatedBytes((byte) 63),
        page.items().get(0).providerAttestationSetDigest().bytes());

    assertArchiveLocationRejected(
        valid,
        response -> {
          final List<Object> items = array(response.get("items"));
          response.put(
              "items", Arrays.asList(deepMutableCopy(items.get(1)), deepMutableCopy(items.get(0))));
        });
    assertArchiveLocationRejected(
        valid,
        response -> {
          final Object first = array(response.get("items")).get(0);
          response.put("items", Arrays.asList(deepMutableCopy(first), deepMutableCopy(first)));
        });
    assertArchiveLocationRejected(
        valid,
        response -> {
          final Object item = array(response.get("items")).get(0);
          final List<Object> excessive = new ArrayList<>();
          for (int index = 0; index < 5; index++) excessive.add(deepMutableCopy(item));
          response.put("items", excessive);
        });
    assertArchiveLocationRejected(
        valid,
        response -> {
          final Map<String, Object> archive = object(response.get("archive"));
          response.put(
              "items",
              Collections.singletonList(
                  archiveLocationWire(3, archive.get("archive_id"), 49L, 1L, "Healthy")));
        });
    assertArchiveLocationRejected(
        valid,
        response ->
            object(array(response.get("items")).get(0)).put("archive_id", digestWire(9)));
    assertArchiveLocationRejected(
        valid,
        response -> {
          final Map<String, Object> first = object(array(response.get("items")).get(0));
          object(first.get("state")).put("kind", "Retired");
        });
    assertArchiveLocationRejected(
        valid,
        response ->
            object(array(response.get("items")).get(0))
                .put("finalized_height", Long.valueOf(51L)));
    assertArchiveLocationRejected(
        valid,
        response ->
            object(array(response.get("items")).get(0)).put("revision", Long.valueOf(3L)));
    assertArchiveLocationRejected(
        valid,
        response -> object(array(response.get("items")).get(0))
            .put("providers", Collections.emptyList()));
    assertArchiveLocationRejected(
        valid,
        response -> object(array(response.get("items")).get(0))
            .put("provider_attestation_set_digest", digestWire(0)));
    assertArchiveLocationRejected(
        valid,
        response -> {
          final Map<String, Object> first = object(array(response.get("items")).get(0));
          first.remove("provider_attestation_set_digest");
          first.put("provider_attestations", Collections.emptyList());
        });
  }

  @Test
  public void maintainerDirectoryDecodesAcceptedAndPendingInvitationVariants() throws Exception {
    final Map<String, Object> route = route(MusubiToriiClientV1.MAINTAINERS_PATH);
    final byte[] response =
        JsonEncoder.encode(route.get("response")).getBytes(StandardCharsets.UTF_8);
    final MusubiModelsV1.Page<MaintainerDirectoryEntry> page =
        MusubiJsonV1.parseMaintainerPage(response);
    assertEquals(2, page.items().size());

    final MaintainerDirectoryEntry accepted = page.items().get(0);
    assertEquals(MaintainerDirectoryEntry.Kind.ACCEPTED, accepted.kind());
    assertEquals("Owner", accepted.acceptedMember().roleKind());

    final MaintainerDirectoryEntry pending = page.items().get(1);
    assertEquals(MaintainerDirectoryEntry.Kind.PENDING_INVITATION, pending.kind());
    assertEquals("Maintainer", pending.pendingInvitation().roleKind());
    assertEquals("Pending", pending.pendingInvitation().stateKind());
    assertEquals(BigInteger.valueOf(2L), pending.pendingInvitation().expectedGovernanceRevision());
    for (final byte value : pending.pendingInvitation().inviteId().bytes()) {
      assertEquals(13, value & 0xff);
    }

    final Map<String, Object> malformed = object(deepMutableCopy(route.get("response")));
    final Map<String, Object> malformedPending =
        object(object(array(malformed.get("items")).get(1)).get("value"));
    object(malformedPending.get("state")).put("kind", "Accepted");
    expectFailure(
        () -> MusubiJsonV1.decodeResponse(MusubiToriiClientV1.MAINTAINERS_PATH, malformed));
  }

  @Test
  public void readOnlyClientPostsEachTypedQueryToItsExactV1Route() throws Exception {
    final List<Map<String, Object>> routes = routes();
    final Map<String, byte[]> responses = new LinkedHashMap<>();
    for (final Map<String, Object> route : routes) {
      responses.put(
          (String) route.get("path"),
          JsonEncoder.encode(route.get("response")).getBytes(StandardCharsets.UTF_8));
    }
    final FixtureExecutor executor = new FixtureExecutor(responses);
    final MusubiToriiClientV1 client =
        MusubiToriiClientV1.builder()
            .baseUri(URI.create("http://localhost:8080"))
            .executor(executor)
            .build();

    for (final Map<String, Object> route : routes) {
      final String path = (String) route.get("path");
      final WireValue request = MusubiJsonV1.decodeQuery(path, route.get("request"));
      invoke(client, path, request);
      final TransportRequest captured = executor.requests.get(executor.requests.size() - 1);
      assertEquals("POST", captured.method());
      assertEquals(path, captured.uri().getPath());
      assertEquals(route.get("request"), parseJson(captured.body()));
    }
    final Set<String> actualPaths = new LinkedHashSet<>();
    for (final TransportRequest request : executor.requests) {
      actualPaths.add(request.uri().getPath());
    }
    assertEquals(EXPECTED_PATHS, actualPaths);
  }

  @Test
  public void readOnlyClientRejectsResponsesThatDoNotMatchRequests() throws Exception {
    assertClientRejected(
        MusubiToriiClientV1.EXACT_PACKAGE_PATH,
        request -> object(request.get("package")).put("name", Collections.singletonList("another-package")));
    assertClientRejected(
        MusubiToriiClientV1.EXACT_RELEASE_PATH,
        request ->
            object(object(request.get("release")).get("package"))
                .put("name", Collections.singletonList("another-package")));
    assertClientRejected(
        MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH,
        request ->
            request.put(
                "provider_id",
                Collections.singletonList(
                    String.join("", Collections.nCopies(32, "FE")))));
    for (final String path :
        Arrays.asList(
            MusubiToriiClientV1.RESOLVER_INDEX_PATH,
            MusubiToriiClientV1.VERSIONS_PATH)) {
      assertClientRejected(
          path,
          request -> {
            final Map<String, Object> cursor = new LinkedHashMap<>();
            cursor.put("snapshot", registrySnapshotWire(49L));
            cursor.put("query_hash", digestWire(19));
            cursor.put("last_key", "fixture-last-key");
            cursor.put("caller", null);
            object(request.get("page")).put("cursor", cursor);
          });
    }
    assertClientRejected(
        MusubiToriiClientV1.MAINTAINERS_PATH,
        request -> object(request.get("package")).put("name", Collections.singletonList("another-package")));
    assertClientRejected(
        MusubiToriiClientV1.MAINTAINERS_PATH,
        request -> object(request.get("page")).put("limit", Long.valueOf(1L)));
    assertClientRejected(
        MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
        request -> request.put("archive_id", digestWire(9)));
    assertClientRejected(
        MusubiToriiClientV1.ALIAS_PATH,
        request -> request.put("alias", Collections.singletonList("another-alias")));
    assertClientRejected(
        MusubiToriiClientV1.ALIAS_HISTORY_PATH,
        request -> request.put("alias", Collections.singletonList("another-alias")));
    assertClientRejected(
        MusubiToriiClientV1.ORDERED_PREFIX_PATH,
        request -> request.put("prefix", Collections.singletonList("another/")));

    final Map<String, Object> searchResponse =
        object(route(MusubiToriiClientV1.SEARCH_PATH).get("response"));
    final Object firstPackage =
        object(array(searchResponse.get("items")).get(0)).get("package");
    assertClientRejected(
        MusubiToriiClientV1.SEARCH_PATH,
        request -> {
          final Map<String, Object> cursor = new LinkedHashMap<>();
          cursor.put("snapshot", deepMutableCopy(searchResponse.get("snapshot")));
          cursor.put("query_hash", digestWire(20));
          cursor.put("last_package", deepMutableCopy(firstPackage));
          object(request.get("page")).put("cursor", cursor);
        });

    assertClientRejected(
        MusubiToriiClientV1.VERSIONS_PATH,
        request -> {
          final Map<String, Object> cursor = new LinkedHashMap<>();
          cursor.put("snapshot", registrySnapshotWire(50L));
          cursor.put("query_hash", digestWire(19));
          cursor.put("last_key", "1.0.0");
          cursor.put("caller", null);
          object(request.get("page")).put("cursor", cursor);
        },
        response -> {
          final Map<String, Object> cursor = new LinkedHashMap<>();
          cursor.put("snapshot", registrySnapshotWire(50L));
          cursor.put("query_hash", digestWire(20));
          cursor.put("last_key", "1.2.3");
          cursor.put("caller", null);
          response.put("next_cursor", cursor);
        });
  }

  @Test
  public void pagedClientsBindEchoedQueriesOnEmptyFirstPages() throws Exception {
    final List<String> paths =
        Arrays.asList(
            MusubiToriiClientV1.RESOLVER_INDEX_PATH,
            MusubiToriiClientV1.VERSIONS_PATH,
            MusubiToriiClientV1.MAINTAINERS_PATH,
            MusubiToriiClientV1.ALIAS_HISTORY_PATH,
            MusubiToriiClientV1.ORDERED_PREFIX_PATH,
            MusubiToriiClientV1.SEARCH_PATH);
    for (final String path : paths) {
      assertClientRejected(
          path,
          ignored -> {},
          response -> {
            response.put("items", Collections.emptyList());
            response.put("next_cursor", null);
            object(object(response.get("query")).get("page"))
                .put("limit", Long.valueOf(49L));
          });
    }
  }

  @Test
  public void echoedPagesRejectTamperedStructuredCursorBoundariesAndContinuations()
      throws Exception {
    final Map<String, Object> versionsRoute = route(MusubiToriiClientV1.VERSIONS_PATH);
    final Map<String, Object> canonical =
        object(deepMutableCopy(versionsRoute.get("response")));
    final Object snapshot = deepMutableCopy(canonical.get("snapshot"));
    final Map<String, Object> versionControls =
        object(object(canonical.get("query")).get("page"));
    versionControls.put("limit", Long.valueOf(1L));
    versionControls.put("cursor", finalizedCursorWire(snapshot, "1.0.0", 19));
    canonical.put("next_cursor", finalizedCursorWire(snapshot, "1.2.3", 19));
    MusubiJsonV1.decodeResponse(MusubiToriiClientV1.VERSIONS_PATH, canonical);

    final Map<String, Object> wrongTail = object(deepMutableCopy(canonical));
    object(wrongTail.get("next_cursor")).put("last_key", "9.9.9");
    expectDecodeFailure(MusubiToriiClientV1.VERSIONS_PATH, wrongTail);

    final Map<String, Object> shortPage = object(deepMutableCopy(canonical));
    object(object(shortPage.get("query")).get("page")).put("limit", Long.valueOf(2L));
    expectDecodeFailure(MusubiToriiClientV1.VERSIONS_PATH, shortPage);

    final Map<String, Object> wrongHash = object(deepMutableCopy(canonical));
    object(wrongHash.get("next_cursor")).put("query_hash", digestWire(20));
    expectDecodeFailure(MusubiToriiClientV1.VERSIONS_PATH, wrongHash);

    final Map<String, Object> wrongSnapshot = object(deepMutableCopy(canonical));
    object(object(wrongSnapshot.get("next_cursor")).get("snapshot"))
        .put("finalized_height", Long.valueOf(49L));
    expectDecodeFailure(MusubiToriiClientV1.VERSIONS_PATH, wrongSnapshot);

    for (final boolean requestCursor : Arrays.asList(Boolean.TRUE, Boolean.FALSE)) {
      final Map<String, Object> callerBound = object(deepMutableCopy(canonical));
      final Map<String, Object> cursor = requestCursor
          ? object(object(object(callerBound.get("query")).get("page")).get("cursor"))
          : object(callerBound.get("next_cursor"));
      cursor.put("caller", "unexpected-caller");
      expectDecodeFailure(MusubiToriiClientV1.VERSIONS_PATH, callerBound);
    }

    for (final String boundary : Arrays.asList("01.0.0", "1.2.3")) {
      final Map<String, Object> badBoundary = object(deepMutableCopy(canonical));
      object(object(object(badBoundary.get("query")).get("page")).get("cursor"))
          .put("last_key", boundary);
      expectDecodeFailure(MusubiToriiClientV1.VERSIONS_PATH, badBoundary);
    }

    final Map<String, String> malformedBoundaries = new LinkedHashMap<>();
    malformedBoundaries.put(MusubiToriiClientV1.RESOLVER_INDEX_PATH, "not-semver");
    malformedBoundaries.put(MusubiToriiClientV1.MAINTAINERS_PATH, "not-a-maintainer-key");
    malformedBoundaries.put(MusubiToriiClientV1.ALIAS_HISTORY_PATH, "math:1");
    malformedBoundaries.put(MusubiToriiClientV1.ORDERED_PREFIX_PATH, "sora/zzzz");
    for (final Map.Entry<String, String> boundary : malformedBoundaries.entrySet()) {
      final Map<String, Object> response =
          object(deepMutableCopy(route(boundary.getKey()).get("response")));
      object(object(response.get("query")).get("page"))
          .put(
              "cursor",
              finalizedCursorWire(response.get("snapshot"), boundary.getValue(), 19));
      expectDecodeFailure(boundary.getKey(), response);
    }

    final Map<String, Object> searchRoute = route(MusubiToriiClientV1.SEARCH_PATH);
    final Map<String, Object> searchBoundary =
        object(deepMutableCopy(searchRoute.get("response")));
    final Object firstSearchPackage =
        object(array(searchBoundary.get("items")).get(0)).get("package");
    final Map<String, Object> searchCursor = new LinkedHashMap<>();
    searchCursor.put("snapshot", deepMutableCopy(searchBoundary.get("snapshot")));
    searchCursor.put("query_hash", digestWire(19));
    searchCursor.put("last_package", deepMutableCopy(firstSearchPackage));
    object(object(searchBoundary.get("query")).get("page")).put("cursor", searchCursor);
    expectDecodeFailure(MusubiToriiClientV1.SEARCH_PATH, searchBoundary);

    final Map<String, Object> searchContinuation =
        object(deepMutableCopy(searchRoute.get("response")));
    object(object(searchContinuation.get("query")).get("page"))
        .put("limit", Long.valueOf(1L));
    final Map<String, Object> nextSearchCursor = new LinkedHashMap<>();
    nextSearchCursor.put("snapshot", deepMutableCopy(searchContinuation.get("snapshot")));
    nextSearchCursor.put("query_hash", digestWire(19));
    nextSearchCursor.put("last_package", deepMutableCopy(firstSearchPackage));
    searchContinuation.put("next_cursor", nextSearchCursor);
    MusubiJsonV1.decodeResponse(MusubiToriiClientV1.SEARCH_PATH, searchContinuation);

    final Map<String, Object> shortSearch = object(deepMutableCopy(searchContinuation));
    object(object(shortSearch.get("query")).get("page")).put("limit", Long.valueOf(2L));
    expectDecodeFailure(MusubiToriiClientV1.SEARCH_PATH, shortSearch);

    final Map<String, Object> wrongSearchTail = object(deepMutableCopy(searchContinuation));
    object(object(wrongSearchTail.get("next_cursor")).get("last_package"))
        .put("name", Collections.singletonList("zzz"));
    expectDecodeFailure(MusubiToriiClientV1.SEARCH_PATH, wrongSearchTail);

    final Map<String, Object> wrongSearchHash = object(deepMutableCopy(searchContinuation));
    final Map<String, Object> previousPackage =
        object(deepMutableCopy(firstSearchPackage));
    previousPackage.put("name", Collections.singletonList("aaa"));
    final Map<String, Object> previousSearchCursor = new LinkedHashMap<>();
    previousSearchCursor.put("snapshot", deepMutableCopy(wrongSearchHash.get("snapshot")));
    previousSearchCursor.put("query_hash", digestWire(20));
    previousSearchCursor.put("last_package", previousPackage);
    object(object(wrongSearchHash.get("query")).get("page"))
        .put("cursor", previousSearchCursor);
    expectDecodeFailure(MusubiToriiClientV1.SEARCH_PATH, wrongSearchHash);
  }

  @Test
  public void versionMaintainerAliasHistoryAndResolverPagesRequireStrictOrder()
      throws Exception {
    final Map<String, Object> versionsRoute = route(MusubiToriiClientV1.VERSIONS_PATH);
    final Map<String, Object> ascending =
        object(deepMutableCopy(versionsRoute.get("response")));
    final Map<String, Object> upper = object(array(ascending.get("items")).get(0));
    final Map<String, Object> lower = object(deepMutableCopy(upper));
    lower.put("patch", Long.valueOf(2L));
    ascending.put("items", Arrays.asList(lower, upper));
    object(object(ascending.get("query")).get("page")).put("limit", Long.valueOf(0L));
    final PackagePageQuery zeroLimitRequest =
        (PackagePageQuery)
            MusubiJsonV1.decodeQuery(
                MusubiToriiClientV1.VERSIONS_PATH, ascending.get("query"));
    MusubiJsonV1.parseVersionPage(
            JsonEncoder.encode(ascending).getBytes(StandardCharsets.UTF_8))
        .requireVersionMatches(zeroLimitRequest);
    expectFailure(() -> new MusubiModelsV1.PageRequest(101L, null));

    final List<List<Object>> invalidVersionItems = new ArrayList<>();
    invalidVersionItems.add(Arrays.asList(deepMutableCopy(upper), deepMutableCopy(lower)));
    invalidVersionItems.add(Arrays.asList(deepMutableCopy(upper), deepMutableCopy(upper)));
    for (final List<Object> items : invalidVersionItems) {
      final Map<String, Object> response =
          object(deepMutableCopy(versionsRoute.get("response")));
      response.put("items", items);
      expectFailure(
          () -> MusubiJsonV1.decodeResponse(MusubiToriiClientV1.VERSIONS_PATH, response));
    }

    for (final String path :
        Arrays.asList(
            MusubiToriiClientV1.MAINTAINERS_PATH,
            MusubiToriiClientV1.ALIAS_HISTORY_PATH)) {
      final Map<String, Object> route = route(path);
      final List<Object> original = array(object(route.get("response")).get("items"));
      final List<Object> malformedItems = new ArrayList<>();
      if (original.size() > 1) {
        for (int index = original.size() - 1; index >= 0; index--) {
          malformedItems.add(deepMutableCopy(original.get(index)));
        }
      } else {
        malformedItems.add(deepMutableCopy(original.get(0)));
        malformedItems.add(deepMutableCopy(original.get(0)));
      }
      final Map<String, Object> malformed =
          object(deepMutableCopy(route.get("response")));
      malformed.put("items", malformedItems);
      expectFailure(() -> MusubiJsonV1.decodeResponse(path, malformed));
    }

    final ResolverIndexQuery resolverRequest =
        (ResolverIndexQuery)
            MusubiJsonV1.decodeQuery(
                MusubiToriiClientV1.RESOLVER_INDEX_PATH,
                route(MusubiToriiClientV1.RESOLVER_INDEX_PATH).get("request"));
    final byte[] snapshotHash = new byte[32];
    Arrays.fill(snapshotHash, (byte) 7);
    final RegistrySnapshot snapshot =
        new RegistrySnapshot(BigInteger.ONE, snapshotHash, BigInteger.ONE);
    final ResolverReleaseRow first =
        new ResolverReleaseRow(
            new ReleaseId(resolverRequest.packageId(), Version.parse("1.0.0")),
            BigInteger.ONE,
            BigInteger.ONE,
            Collections.emptyMap());
    final ResolverReleaseRow second =
        new ResolverReleaseRow(
            new ReleaseId(resolverRequest.packageId(), Version.parse("2.0.0")),
            BigInteger.ONE,
            BigInteger.ONE,
            Collections.emptyMap());
    final byte[] genesisHash = new byte[32];
    Arrays.fill(genesisHash, (byte) 8);
    expectFailure(
        () ->
            new ResolverIndexPage(
                resolverRequest,
                "fixture-chain",
                genesisHash,
                Arrays.asList(second, first),
                null,
                snapshot));
    expectFailure(
        () ->
            new ResolverIndexPage(
                resolverRequest,
                "fixture-chain",
                genesisHash,
                Arrays.asList(first, first),
                null,
                snapshot));
    final ResolverIndexPage page =
        new ResolverIndexPage(
            resolverRequest,
            "fixture-chain",
            genesisHash,
            Collections.singletonList(first),
            null,
            snapshot);
    expectFailure(() -> page.requireMatches(resolverRequest));
    final PackageId differentPackage =
        new PackageId(
            resolverRequest.packageId().homeDataspace(),
            resolverRequest.packageId().scope(),
            new PackageName("another-package"));
    expectFailure(
        () -> page.requireMatches(new ResolverIndexQuery(differentPackage, null, new MusubiModelsV1.PageRequest())));
  }

  @Test
  public void governedTakedownRequiresOnlyAppliedHeight() throws Exception {
    final Map<String, Object> exactRelease = route(MusubiToriiClientV1.EXACT_RELEASE_PATH);
    final Map<String, Object> canonical =
        object(deepMutableCopy(exactRelease.get("response")));
    final Map<String, Object> homeRelease = object(canonical.get("home_release"));
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("action_digest", homeRelease.get("release_digest"));
    payload.put("reason", Collections.singletonList("security response"));
    payload.put("applied_at_height", Long.valueOf(50L));
    final Map<String, Object> governance = new LinkedHashMap<>();
    governance.put("kind", "TakenDown");
    governance.put("value", payload);
    homeRelease.put("artifact_governance", governance);
    object(homeRelease.get("revisions"))
        .put("artifact_governance", Long.valueOf(2L));
    object(object(canonical.get("universal_release")).get("selection"))
        .put("governance", deepMutableCopy(governance));
    MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, canonical);

    final Map<String, Object> legacy = object(deepMutableCopy(canonical));
    final Map<String, Object> legacyPayload =
        object(
            object(object(legacy.get("home_release")).get("artifact_governance"))
                .get("value"));
    legacyPayload.put("enacted_at_height", legacyPayload.remove("applied_at_height"));
    expectFailure(
        () -> MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, legacy));

    final Map<String, Object> zeroHeight = object(deepMutableCopy(canonical));
    final Map<String, Object> zeroPayload =
        object(
            object(object(zeroHeight.get("home_release")).get("artifact_governance"))
                .get("value"));
    zeroPayload.put("applied_at_height", Long.valueOf(0L));
    expectFailure(
        () -> MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, zeroHeight));
  }

  @Test
  public void exactReleaseRejectsSubstitutedProjectionsAndNonfinalAnchors()
      throws Exception {
    final Map<String, Object> exactRelease = route(MusubiToriiClientV1.EXACT_RELEASE_PATH);

    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(response.get("universal_release"))
                .put("release_digest", digestWire(64)));
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Object replacement = digestWire(64);
          object(response.get("home_release"))
              .put("release_digest", deepMutableCopy(replacement));
          object(response.get("universal_release"))
              .put("release_digest", deepMutableCopy(replacement));
        });
    assertExactReleaseRejected(
        exactRelease,
        response -> object(response.get("home_release")).put("published_by", "not-an-account"));
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(response.get("home_release"))
                .put("published_at_height", Long.valueOf(0L)));
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(response.get("universal_release"))
                .put("archive_id", digestWire(65)));
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(response.get("universal_release"))
                .put("interface_digest", digestWire(66)));
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(object(response.get("universal_release")).get("abi"))
                .put("abi_hash", new ArrayList<Object>(Collections.nCopies(32, Long.valueOf(67L)))));
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> home = object(response.get("home_release"));
          final Map<String, Object> release =
              object(object(home.get("manifest")).get("release"));
          final Map<String, Object> dependencyPackage =
              object(deepMutableCopy(release.get("package")));
          dependencyPackage.put("name", Collections.singletonList("dependency"));
          final Map<String, Object> requirement = new LinkedHashMap<>();
          requirement.put("kind", "Any");
          requirement.put("value", null);
          final Map<String, Object> dependency = new LinkedHashMap<>();
          dependency.put("alias", "dependency");
          dependency.put("package", dependencyPackage);
          dependency.put("requirement", requirement);
          object(response.get("universal_release"))
              .put("dependencies", Collections.singletonList(dependency));
        });
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> selection =
              object(object(response.get("universal_release")).get("selection"));
          object(selection.get("yank"))
              .put("reason", Collections.singletonList("substituted state"));
        });
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> home = object(response.get("home_release"));
          final Map<String, Object> takedown = new LinkedHashMap<>();
          takedown.put("action_digest", deepMutableCopy(home.get("release_digest")));
          takedown.put("reason", Collections.singletonList("substituted governance"));
          takedown.put("applied_at_height", Long.valueOf(50L));
          final Map<String, Object> governed = new LinkedHashMap<>();
          governed.put("kind", "TakenDown");
          governed.put("value", takedown);
          object(object(response.get("universal_release")).get("selection"))
              .put("governance", governed);
        });
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> home = object(response.get("home_release"));
          final Map<String, Object> selection =
              object(object(response.get("universal_release")).get("selection"));
          object(home.get("yank")).put("revision", Long.valueOf(10L));
          object(home.get("revisions")).put("yank", Long.valueOf(10L));
          object(selection.get("yank")).put("revision", Long.valueOf(10L));
        });
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> home = object(response.get("home_release"));
          final Map<String, Object> selection =
              object(object(response.get("universal_release")).get("selection"));
          object(home.get("yank")).put("changed_at_height", Long.valueOf(42L));
          object(selection.get("yank")).put("changed_at_height", Long.valueOf(42L));
        });
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> home = object(response.get("home_release"));
          final Map<String, Object> selection =
              object(object(response.get("universal_release")).get("selection"));
          object(home.get("yank")).put("changed_at_height", Long.valueOf(51L));
          object(selection.get("yank")).put("changed_at_height", Long.valueOf(51L));
        });
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(object(response.get("home_release")).get("revisions"))
                .put("artifact_governance", Long.valueOf(10L)));
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> home = object(response.get("home_release"));
          final Map<String, Object> takedown = new LinkedHashMap<>();
          takedown.put("action_digest", deepMutableCopy(home.get("release_digest")));
          takedown.put("reason", Collections.singletonList("premature takedown"));
          takedown.put("applied_at_height", Long.valueOf(42L));
          final Map<String, Object> governed = new LinkedHashMap<>();
          governed.put("kind", "TakenDown");
          governed.put("value", takedown);
          home.put("artifact_governance", governed);
          object(home.get("revisions"))
              .put("artifact_governance", Long.valueOf(2L));
          object(object(response.get("universal_release")).get("selection"))
              .put("governance", deepMutableCopy(governed));
        });
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          final Map<String, Object> home = object(response.get("home_release"));
          final Map<String, Object> takedown = new LinkedHashMap<>();
          takedown.put("action_digest", deepMutableCopy(home.get("release_digest")));
          takedown.put("reason", Collections.singletonList("nonfinal takedown"));
          takedown.put("applied_at_height", Long.valueOf(51L));
          final Map<String, Object> governed = new LinkedHashMap<>();
          governed.put("kind", "TakenDown");
          governed.put("value", takedown);
          home.put("artifact_governance", governed);
          object(home.get("revisions"))
              .put("artifact_governance", Long.valueOf(2L));
          object(object(response.get("universal_release")).get("selection"))
              .put("governance", deepMutableCopy(governed));
        });
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(response.get("home_release"))
                .put("published_at_height", Long.valueOf(51L)));
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(response.get("universal_release"))
                .put("index_revision", Long.valueOf(10L)));
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(
                    object(object(response.get("universal_release")).get("selection"))
                        .get("storage"))
                .put("finalized_height", Long.valueOf(51L)));
    assertExactReleaseRejected(
        exactRelease,
        response ->
            object(
                    object(object(response.get("universal_release")).get("selection"))
                        .get("storage"))
                .put(
                    "finalized_block_hash",
                    new ArrayList<Object>(Collections.nCopies(32, Long.valueOf(6L)))));
    assertExactReleaseRejected(
        exactRelease,
        response -> {
          object(response.get("snapshot")).put("finalized_height", Long.valueOf(1L));
          final Map<String, Object> home = object(response.get("home_release"));
          home.put("published_at_height", Long.valueOf(1L));
          object(home.get("yank")).put("changed_at_height", Long.valueOf(1L));
          final Map<String, Object> selection =
              object(object(response.get("universal_release")).get("selection"));
          object(selection.get("yank")).put("changed_at_height", Long.valueOf(1L));
          object(selection.get("storage")).put("finalized_height", Long.valueOf(1L));
        });
  }

  @Test
  public void rejectsNoncanonicalInputsUnknownFieldsAndUnknownAbiVersions() throws Exception {
    final Map<String, Object> rejected = object(fixture().get("reject"));
    for (final Object name : array(rejected.get("names"))) {
      expectFailure(() -> new PackageName((String) name));
    }
    for (final Object version : array(rejected.get("versions"))) {
      expectFailure(() -> Version.parse((String) version));
    }
    for (final Object requirement : array(rejected.get("requirements"))) {
      expectFailure(() -> VersionReq.parse((String) requirement));
    }
    assertFalse(array(rejected.get("fixture_versions")).contains(Long.valueOf(1L)));

    final Map<String, Object> exactPackage = route(MusubiToriiClientV1.EXACT_PACKAGE_PATH);
    final Map<String, Object> requestWithUnknown =
        new LinkedHashMap<>(object(exactPackage.get("request")));
    requestWithUnknown.put("legacy", Boolean.TRUE);
    expectFailure(
        () ->
            MusubiJsonV1.decodeQuery(
                MusubiToriiClientV1.EXACT_PACKAGE_PATH, requestWithUnknown));

    final Map<String, Object> responseWithUnknown =
        new LinkedHashMap<>(object(exactPackage.get("response")));
    responseWithUnknown.put("legacy", Boolean.TRUE);
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.EXACT_PACKAGE_PATH, responseWithUnknown));

    final Map<String, Object> exactRelease = route(MusubiToriiClientV1.EXACT_RELEASE_PATH);
    final Map<String, Object> response = object(deepMutableCopy(exactRelease.get("response")));
    final Map<String, Object> manifest =
        object(object(response.get("home_release")).get("manifest"));
    final Map<String, Object> abi = object(manifest.get("abi"));
    abi.put("abi_version", Long.valueOf(2L));
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, response));

    final Map<String, Object> futureStorage =
        object(deepMutableCopy(exactRelease.get("response")));
    object(futureStorage.get("universal_release"))
        .put("index_revision", Long.valueOf(8L));
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.EXACT_RELEASE_PATH, futureStorage));

    final Map<String, Object> providerRoute =
        route(MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH);
    final Map<String, Object> substitutedAttestationDigest =
        object(deepMutableCopy(providerRoute.get("response")));
    substitutedAttestationDigest.put("attestation_digest", digestWire(64));
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH,
                substitutedAttestationDigest));

    final Map<String, Object> archiveRoute = route(MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH);
    final Map<String, Object> archiveResponse =
        object(deepMutableCopy(archiveRoute.get("response")));
    final Map<String, Object> archive = object(archiveResponse.get("archive"));
    final Map<String, Object> receipt = object(archive.get("staging_receipt"));
    final Map<String, Object> payload = object(receipt.get("payload"));
    payload.put("version", Long.valueOf(2L));
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH, archiveResponse));
  }

  private static void invoke(
      final MusubiToriiClientV1 client, final String path, final WireValue request) {
    switch (path) {
      case MusubiToriiClientV1.EXACT_PACKAGE_PATH:
        client.findExactPackage((ExactPackageQuery) request).join();
        break;
      case MusubiToriiClientV1.EXACT_RELEASE_PATH:
        client.findExactRelease((ExactReleaseQuery) request).join();
        break;
      case MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH:
        client.findProviderBundleAttestation((ProviderBundleAttestationKey) request).join();
        break;
      case MusubiToriiClientV1.RESOLVER_INDEX_PATH:
        client.findResolverIndex((ResolverIndexQuery) request).join();
        break;
      case MusubiToriiClientV1.VERSIONS_PATH:
        client.findVersions((PackagePageQuery) request).join();
        break;
      case MusubiToriiClientV1.MAINTAINERS_PATH:
        client.findMaintainers((PackagePageQuery) request).join();
        break;
      case MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH:
        client.findArchiveLocations((ArchiveLocationQuery) request).join();
        break;
      case MusubiToriiClientV1.ARCHIVE_RETENTION_PATH:
        client.findArchiveRetention((ArchiveRetentionQuery) request).join();
        break;
      case MusubiToriiClientV1.ALIAS_PATH:
        client.findAlias((AliasQuery) request).join();
        break;
      case MusubiToriiClientV1.ALIAS_HISTORY_PATH:
        client.findAliasHistory((AliasQuery) request).join();
        break;
      case MusubiToriiClientV1.ORDERED_PREFIX_PATH:
        client.findOrderedPrefix((OrderedPrefixQuery) request).join();
        break;
      case MusubiToriiClientV1.SEARCH_PATH:
        client.search((SearchQuery) request).join();
        break;
      default:
        throw new AssertionError("unhandled fixture path " + path);
    }
  }

  private static Map<String, Object> route(final String path) throws Exception {
    for (final Map<String, Object> route : routes()) {
      if (path.equals(route.get("path"))) return route;
    }
    throw new AssertionError("fixture route not found: " + path);
  }

  private static void assertClientRejected(
      final String path, final Consumer<Map<String, Object>> mutation) throws Exception {
    assertClientRejected(path, mutation, ignored -> {});
  }

  private static void assertClientRejected(
      final String path,
      final Consumer<Map<String, Object>> requestMutation,
      final Consumer<Map<String, Object>> responseMutation) throws Exception {
    final Map<String, Object> route = route(path);
    final Map<String, Object> requestValue =
        object(deepMutableCopy(route.get("request")));
    requestMutation.accept(requestValue);
    final WireValue request = MusubiJsonV1.decodeQuery(path, requestValue);
    final Map<String, Object> responseValue =
        object(deepMutableCopy(route.get("response")));
    responseMutation.accept(responseValue);
    final Map<String, byte[]> responses = new LinkedHashMap<>();
    responses.put(
        path,
        JsonEncoder.encode(responseValue).getBytes(StandardCharsets.UTF_8));
    final MusubiToriiClientV1 client =
        MusubiToriiClientV1.builder()
            .baseUri(URI.create("http://localhost:8080"))
            .executor(new FixtureExecutor(responses))
            .build();
    expectAsyncFailure(() -> invoke(client, path, request));
  }

  private static Map<String, Object> registrySnapshotWire(final long finalizedHeight) {
    final List<Object> hash = new ArrayList<>();
    for (int index = 0; index < 32; index++) hash.add(Long.valueOf(7L));
    final Map<String, Object> snapshot = new LinkedHashMap<>();
    snapshot.put("finalized_height", Long.valueOf(finalizedHeight));
    snapshot.put("finalized_block_hash", hash);
    snapshot.put("index_revision", Long.valueOf(9L));
    return snapshot;
  }

  private static Map<String, Object> populatedArchiveLocationResponse() throws Exception {
    final Map<String, Object> route = route(MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH);
    final Map<String, Object> response = object(deepMutableCopy(route.get("response")));
    final Map<String, Object> archive = object(response.get("archive"));
    final Map<String, Object> first =
        archiveLocationWire(1, archive.get("archive_id"), 49L, 1L, "Healthy");
    final Map<String, Object> second =
        archiveLocationWire(2, archive.get("archive_id"), 50L, 2L, "Degraded");
    archive.put("location_revision", Long.valueOf(2L));
    archive.put(
        "location_ids",
        Arrays.asList(
            deepMutableCopy(first.get("location_id")),
            deepMutableCopy(second.get("location_id"))));
    response.put("items", Arrays.asList(first, second));
    return response;
  }

  private static Map<String, Object> archiveLocationWire(
      final int locationByte,
      final Object archiveId,
      final long finalizedHeight,
      final long revision,
      final String state) {
    final Map<String, Object> taggedState = new LinkedHashMap<>();
    taggedState.put("kind", state);
    taggedState.put("value", null);
    final Map<String, Object> location = new LinkedHashMap<>();
    location.put("location_id", digestWire(locationByte));
    location.put("archive_id", deepMutableCopy(archiveId));
    location.put("pin_manifest", digestWire(61));
    location.put("replication_order", digestWire(62));
    location.put(
        "providers",
        Collections.<Object>singletonList(
            Collections.<Object>singletonList(
                String.join("", Collections.nCopies(32, "3F")))));
    location.put("provider_attestation_set_digest", digestWire(63));
    location.put("renew_after_epoch", Long.valueOf(1L));
    location.put("expires_at_epoch", Long.valueOf(2L));
    location.put("finalized_height", Long.valueOf(finalizedHeight));
    location.put("revision", Long.valueOf(revision));
    location.put("state", taggedState);
    return location;
  }

  private static List<Object> digestWire(final int fill) {
    final List<Object> bytes = new ArrayList<>();
    for (int index = 0; index < 32; index++) bytes.add(Long.valueOf(fill));
    return Collections.singletonList(bytes);
  }

  private static byte[] repeatedBytes(final byte value) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, value);
    return bytes;
  }

  private static Map<String, Object> finalizedCursorWire(
      final Object snapshot, final String lastKey, final int queryHashByte) {
    final Map<String, Object> cursor = new LinkedHashMap<>();
    cursor.put("snapshot", deepMutableCopy(snapshot));
    cursor.put("query_hash", digestWire(queryHashByte));
    cursor.put("last_key", lastKey);
    cursor.put("caller", null);
    return cursor;
  }

  private static void expectDecodeFailure(
      final String path, final Map<String, Object> response) {
    expectFailure(() -> MusubiJsonV1.decodeResponse(path, response));
  }

  private static void assertArchiveLocationRejected(
      final Map<String, Object> valid,
      final Consumer<Map<String, Object>> mutation) {
    final Map<String, Object> response = object(deepMutableCopy(valid));
    mutation.accept(response);
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH, response));
  }

  private static void assertExactReleaseRejected(
      final Map<String, Object> route,
      final Consumer<Map<String, Object>> mutation) {
    final Map<String, Object> response = object(deepMutableCopy(route.get("response")));
    mutation.accept(response);
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.EXACT_RELEASE_PATH, response));
  }

  private static List<Map<String, Object>> routes() throws Exception {
    final List<Map<String, Object>> values = new ArrayList<>();
    for (final Object route : array(fixture().get("routes"))) values.add(object(route));
    return values;
  }

  private static Map<String, Object> fixture() throws Exception {
    return object(parseJson(Files.readAllBytes(findFixture())));
  }

  private static Path findFixture() {
    Path current = Paths.get("").toAbsolutePath().normalize();
    for (int index = 0; index < 8 && current != null; index++) {
      final Path candidate = current.resolve("fixtures/musubi/sdk_v1.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("fixtures/musubi/sdk_v1.json was not found");
  }

  private static void assertWireEquals(final Object expected, final WireValue value) {
    assertEquals(expected, parseJson(value.toJsonBytes()));
  }

  private static Object parseJson(final byte[] bytes) {
    return JsonParser.parse(new String(bytes, StandardCharsets.UTF_8));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value) {
    if (!(value instanceof Map)) throw new AssertionError("fixture value must be an object");
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> array(final Object value) {
    if (!(value instanceof List)) throw new AssertionError("fixture value must be an array");
    return (List<Object>) value;
  }

  private static String newtypeText(final Object value) {
    final List<Object> values = array(value);
    assertEquals(1, values.size());
    return (String) values.get(0);
  }

  private static Object deepMutableCopy(final Object value) {
    if (value instanceof Map) {
      final Map<String, Object> copy = new LinkedHashMap<>();
      for (final Map.Entry<String, Object> entry : object(value).entrySet()) {
        copy.put(entry.getKey(), deepMutableCopy(entry.getValue()));
      }
      return copy;
    }
    if (value instanceof List) {
      final List<Object> copy = new ArrayList<>();
      for (final Object item : array(value)) copy.add(deepMutableCopy(item));
      return copy;
    }
    return value;
  }

  private static void expectFailure(final Runnable operation) {
    boolean failed = false;
    try {
      operation.run();
    } catch (final IllegalArgumentException | IllegalStateException expected) {
      failed = true;
    }
    assertTrue("operation must reject non-V1 or noncanonical input", failed);
  }

  private static void expectAsyncFailure(final Runnable operation) {
    boolean failed = false;
    try {
      operation.run();
    } catch (final CompletionException expected) {
      Throwable cause = expected;
      while (cause != null && !failed) {
        failed = cause instanceof IllegalArgumentException;
        cause = cause.getCause();
      }
    } catch (final IllegalArgumentException expected) {
      failed = true;
    }
    assertTrue("client must reject a response that is not bound to its request", failed);
  }

  private static final class FixtureExecutor implements HttpTransportExecutor {
    private final Map<String, byte[]> responses;
    private final List<TransportRequest> requests = new ArrayList<>();

    private FixtureExecutor(final Map<String, byte[]> responses) {
      this.responses = responses;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      final byte[] body = responses.get(request.uri().getPath());
      if (body == null) throw new AssertionError("unexpected route " + request.uri().getPath());
      return CompletableFuture.completedFuture(
          new TransportResponse(
              200,
              body,
              "OK",
              Collections.singletonMap("Content-Type", Collections.singletonList("application/json"))));
    }
  }
}
