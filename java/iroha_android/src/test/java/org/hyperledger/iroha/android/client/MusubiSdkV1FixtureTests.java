package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

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
import org.hyperledger.iroha.android.client.MusubiModelsV1.AliasQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ArchiveLocationQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactPackageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ExactReleaseQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Namespace;
import org.hyperledger.iroha.android.client.MusubiModelsV1.OrderedPrefixQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackageName;
import org.hyperledger.iroha.android.client.MusubiModelsV1.PackagePageQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.ResolverIndexQuery;
import org.hyperledger.iroha.android.client.MusubiModelsV1.Version;
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
              MusubiToriiClientV1.RESOLVER_INDEX_PATH,
              MusubiToriiClientV1.VERSIONS_PATH,
              MusubiToriiClientV1.MAINTAINERS_PATH,
              MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
              MusubiToriiClientV1.ALIAS_PATH,
              MusubiToriiClientV1.ALIAS_HISTORY_PATH,
              MusubiToriiClientV1.ORDERED_PREFIX_PATH));

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
    assertEquals(9, routes.size());
    assertEquals(EXPECTED_PATHS, actualPaths);
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
    final Map<String, Object> manifest = object(response.get("manifest"));
    final Map<String, Object> abi = object(manifest.get("abi"));
    abi.put("abi_version", Long.valueOf(2L));
    expectFailure(
        () ->
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, response));
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
      case MusubiToriiClientV1.ALIAS_PATH:
        client.findAlias((AliasQuery) request).join();
        break;
      case MusubiToriiClientV1.ALIAS_HISTORY_PATH:
        client.findAliasHistory((AliasQuery) request).join();
        break;
      case MusubiToriiClientV1.ORDERED_PREFIX_PATH:
        client.findOrderedPrefix((OrderedPrefixQuery) request).join();
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
