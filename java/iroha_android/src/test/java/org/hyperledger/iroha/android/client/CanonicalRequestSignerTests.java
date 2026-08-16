package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.testing.TestNetworkIds;

public final class CanonicalRequestSignerTests {

  private static final NetworkId NETWORK_ID = TestNetworkIds.canonical();

  private CanonicalRequestSignerTests() {}

  public static void main(final String[] args) throws Exception {
    canonicalQuerySortsPairs();
    canonicalQueryMatchesRustFormEncodingAndUtf8Ordering();
    canonicalQueryMatchesRustLossyUtf8MalformedSequenceBoundaries();
    canonicalQueryEnforcesV1PairAndByteLimits();
    canonicalRequestEnforcesV1MethodLimit();
    canonicalRequestEnforcesV1PathLimit();
    canonicalRequestRejectsNegativeTimestamp();
    canonicalAuthEnforcesV1AccountAndNonceLimits();
    canonicalHeadersUseAsciiHexForI105AndPreserveAlias();
    canonicalAuthRejectsInvalidAliasSpellings();
    canonicalAuthRejectsInvalidCallbackSignatures();
    callbackHeadersReceiveCanonicalMessage();
    unsignedBodyAuthJsonRemovesOnlyTopLevelProofFields();
    callbackBodySignatureReceivesCanonicalMessage();
    canonicalAuthRejectsPaddedFreshnessAndAccountFields();
    canonicalAuthCannotReplayAcrossSameLabelNetworks();
    System.out.println("[IrohaAndroid] Canonical request signer tests passed.");
  }

  private static void canonicalQuerySortsPairs() {
    final String rendered =
        CanonicalRequestSigner.canonicalQueryString("b=2&a=3&b=1&space=a+b");
    assert "a=3&b=1&b=2&space=a+b".equals(rendered)
        : "canonical query mismatch: " + rendered;
  }

  private static void canonicalQueryMatchesRustFormEncodingAndUtf8Ordering() {
    assert "a=1&b=%21*%28%29%7E%27"
            .equals(CanonicalRequestSigner.canonicalQueryString("b=!*()~'&a=1"))
        : "application/x-www-form-urlencoded safe set mismatch";
    assert "x=A%25zz%EF%BF%BD"
            .equals(CanonicalRequestSigner.canonicalQueryString("x=%41%zz%FF"))
        : "mixed valid and malformed percent decoding mismatch";
    assert "%EE%80%80=bmp&%F0%90%80%80=supplementary"
            .equals(
                CanonicalRequestSigner.canonicalQueryString(
                    "\uE000=bmp&\uD800\uDC00=supplementary"))
        : "decoded query pairs must sort by UTF-8 bytes";
    assert "k=%EE%80%80&k=%F0%90%80%80"
            .equals(
                CanonicalRequestSigner.canonicalQueryString("k=\uD800\uDC00&k=\uE000"))
        : "decoded query values must sort by UTF-8 bytes";
  }

  private static void canonicalQueryMatchesRustLossyUtf8MalformedSequenceBoundaries() {
    assert "x=%EF%BF%BD%EF%BF%BD%EF%BF%BD"
            .equals(CanonicalRequestSigner.canonicalQueryString("x=%ED%A0%80"))
        : "encoded surrogate bytes must each decode to one replacement character";
    assert "x=%EF%BF%BDA".equals(
            CanonicalRequestSigner.canonicalQueryString("x=%E2%82%41"))
        : "a malformed third byte must replace the valid two-byte prefix";
    assert "x=%EF%BF%BD".equals(
            CanonicalRequestSigner.canonicalQueryString("x=%F0%9F%92"))
        : "an incomplete trailing sequence must decode to one replacement character";
    assert "x=%EF%BF%BDA%EF%BF%BD".equals(
            CanonicalRequestSigner.canonicalQueryString("x=%F0%9F%41%80"))
        : "a malformed four-byte sequence must retain Rust replacement boundaries";
  }

  private static void canonicalQueryEnforcesV1PairAndByteLimits() {
    final StringBuilder exactPairs = new StringBuilder();
    for (int index = 0;
        index < CanonicalRequestSigner.CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1;
        index++) {
      if (index > 0) {
        exactPairs.append('&');
      }
      exactPairs.append('k').append(index).append("=v");
    }
    CanonicalRequestSigner.canonicalQueryString(exactPairs.toString());
    assertIllegalArgument(
        () -> CanonicalRequestSigner.canonicalQueryString(exactPairs + "&overflow=v"),
        "query pair limit");

    final String exactBytes =
        "k="
            + repeat(
                'x', CanonicalRequestSigner.CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 - 2);
    assert exactBytes.getBytes(StandardCharsets.UTF_8).length
        == CanonicalRequestSigner.CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1;
    CanonicalRequestSigner.canonicalQueryString(exactBytes);
    assertIllegalArgument(
        () -> CanonicalRequestSigner.canonicalQueryString(exactBytes + "x"),
        "raw query byte limit");

    assert "a=1&b=2".equals(CanonicalRequestSigner.canonicalQueryString("&&b=2&&a=1&"))
        : "empty query components must be ignored";
  }

  private static void canonicalRequestEnforcesV1MethodLimit() throws Exception {
    final URI uri = new URI("https://torii.example/v1/test");
    CanonicalRequestSigner.canonicalRequestMessage(
        repeat('A', CanonicalRequestSigner.CANONICAL_REQUEST_MAX_METHOD_BYTES_V1),
        uri,
        new byte[0]);
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.canonicalRequestMessage(
                repeat('A', CanonicalRequestSigner.CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 + 1),
                uri,
                new byte[0]),
        "method byte limit");
    CanonicalRequestSigner.canonicalRequestMessage("M-SEARCH", uri, new byte[0]);
    for (final String method : new String[] {"", "GET POST", "GET\n", "GÉT"}) {
      assertIllegalArgument(
          () -> CanonicalRequestSigner.canonicalRequestMessage(method, uri, new byte[0]),
          "invalid HTTP method token");
    }
  }

  private static void canonicalRequestEnforcesV1PathLimit() throws Exception {
    final byte[] root =
        CanonicalRequestSigner.canonicalRequestMessage("GET", new URI("/"), new byte[0]);
    final byte[] originWithoutPath =
        CanonicalRequestSigner.canonicalRequestMessage(
            "GET", new URI("https://torii.example"), new byte[0]);
    assert Arrays.equals(root, originWithoutPath)
        : "an origin URL without a path must sign the HTTP root path";

    final URI exact =
        new URI("/" + repeat('x', CanonicalRequestSigner.CANONICAL_REQUEST_MAX_PATH_BYTES_V1 - 1));
    CanonicalRequestSigner.canonicalRequestMessage("GET", exact, new byte[0]);
    final URI excessive =
        new URI("/" + repeat('x', CanonicalRequestSigner.CANONICAL_REQUEST_MAX_PATH_BYTES_V1));
    assertIllegalArgument(
        () -> CanonicalRequestSigner.canonicalRequestMessage("GET", excessive, new byte[0]),
        "path byte limit");
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.canonicalRequestMessage(
                "GET", URI.create("v1/relative"), new byte[0]),
        "relative path");
    for (final URI invalidUri :
        new URI[] {
          URI.create("?a=1"),
          URI.create("mailto:alice@example.com"),
          URI.create("//torii.example/v1/test"),
          URI.create("https:/v1/test"),
          URI.create("/v1/test#fragment")
        }) {
      assertIllegalArgument(
          () ->
              CanonicalRequestSigner.canonicalRequestMessage(
                  "GET", invalidUri, new byte[0]),
          "ambiguous or non-HTTP URI: " + invalidUri);
    }
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.canonicalRequestMessage(
                "GET", URI.create("/café"), new byte[0]),
        "non-ASCII raw path");
    CanonicalRequestSigner.canonicalRequestMessage(
        "GET", URI.create("/caf%C3%A9"), new byte[0]);
    final byte[] structuralEscapes =
        CanonicalRequestSigner.canonicalRequestMessage(
            "GET", URI.create("/v1/%2e%2Fasset/%252e"), new byte[0]);
    assert "/v1/%2e%2Fasset/%252e"
            .equals(new String(structuralEscapes, StandardCharsets.UTF_8).split("\\n")[1])
        : "valid percent escapes must retain their exact raw spelling";
    for (final String invalidPath :
        new String[] {
          "/.",
          "/..",
          "/v1/./asset",
          "/v1/../asset",
          "/v1/%2e/asset",
          "/v1/%2E%2e/asset",
          "/v1/.%2E/asset"
        }) {
      assertIllegalArgument(
          () ->
              CanonicalRequestSigner.canonicalRequestMessage(
                  "GET", URI.create(invalidPath), new byte[0]),
          "raw or percent-decoded dot segment: " + invalidPath);
    }
    for (final String malformedPath : new String[] {"/v1/%", "/v1/%2", "/v1/%GG"}) {
      assertIllegalArgument(() -> URI.create(malformedPath), "malformed percent escape");
    }
  }

  private static void canonicalRequestRejectsNegativeTimestamp() throws Exception {
    final URI uri = new URI("/v1/test");
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                NETWORK_ID, "GET", uri, new byte[0], -1L, "negative-timestamp"),
        "negative timestamp");
  }

  private static void canonicalAuthEnforcesV1AccountAndNonceLimits() throws Exception {
    final URI uri = new URI("https://torii.example/v1/accounts");
    final long timestampMs = 1_717_171_717_005L;
    final String validAlias = "alice@universal";
    CanonicalRequestSigner.buildHeaders(
        NETWORK_ID,
        "get",
        uri,
        new byte[0],
        new ToriiCanonicalRequestAuth(validAlias, CanonicalRequestSignerTests::fakeSignature),
        timestampMs,
        "account-limit");
    final String excessiveAccount =
        repeat('a', CanonicalRequestSigner.CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 + 1);
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.buildHeaders(
                NETWORK_ID,
                "get",
                uri,
                new byte[0],
                new ToriiCanonicalRequestAuth(
                    excessiveAccount, CanonicalRequestSignerTests::fakeSignature),
                timestampMs,
                "account-limit-plus-one"),
        "account literal byte limit");

    final String exactNonce = repeat('n', 256);
    CanonicalRequestSigner.canonicalRequestSignatureMessage(
        NETWORK_ID, "get", uri, new byte[0], timestampMs, exactNonce);
    for (final String nonce :
        new String[] {exactNonce + "n", "internal space", "control\u0001", "nönce"}) {
      assertIllegalArgument(
          () ->
              CanonicalRequestSigner.canonicalRequestSignatureMessage(
                  NETWORK_ID, "get", uri, new byte[0], timestampMs, nonce),
          "nonce limit or alphabet: " + nonce.length());
    }
  }

  private static void canonicalHeadersUseAsciiHexForI105AndPreserveAlias() throws Exception {
    final String i105 =
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final String canonicalHex =
        "0x02000120ce7fa46c9dce7ea4b125e2e36bdb63ea33073e7590ac92816ae1e861b7048b03";
    assert canonicalHex.equals(
            AccountAddress.parseEncodedIgnoringCurveSupport(i105, null).address.canonicalHex())
        : "shared I105 fixture must decode to the cross-SDK canonical bytes";
    final URI uri = new URI("https://torii.example/v1/accounts");
    final long timestampMs = 1_717_171_717_006L;
    final Map<String, String> i105Headers =
        CanonicalRequestSigner.buildHeaders(
            NETWORK_ID,
            "get",
            uri,
            new byte[0],
            new ToriiCanonicalRequestAuth(i105, CanonicalRequestSignerTests::fakeSignature),
            timestampMs,
            "i105-header-hex");

    assert canonicalHex.equals(i105Headers.get(CanonicalRequestSigner.HEADER_ACCOUNT))
        : "I105 account header must use canonical hex";
    assert canonicalHex.matches("0x[0-9a-f]+")
        : "canonical account header must be lowercase ASCII hex";

    final String alias = "alice-1@wonderland";
    final Map<String, String> aliasHeaders =
        CanonicalRequestSigner.buildHeaders(
            NETWORK_ID,
            "get",
            uri,
            new byte[0],
            new ToriiCanonicalRequestAuth(alias, CanonicalRequestSignerTests::fakeSignature),
            timestampMs,
            "alias-header");
    assert alias.equals(aliasHeaders.get(CanonicalRequestSigner.HEADER_ACCOUNT))
        : "ASCII account aliases must remain literal";

    final Map<String, Object> signedBody =
        CanonicalRequestSigner.withBodySignature(
            NETWORK_ID,
            "post",
            uri,
            new LinkedHashMap<>(),
            new ToriiCanonicalRequestAuth(i105, CanonicalRequestSignerTests::fakeSignature),
            timestampMs,
            "i105-body");
    assert i105.equals(signedBody.get(CanonicalRequestSigner.BODY_ACCOUNT_ID))
        : "body authentication must preserve the semantic I105 account id";
  }

  private static void canonicalAuthRejectsInvalidAliasSpellings() throws Exception {
    final URI uri = new URI("https://torii.example/v1/accounts");
    final long timestampMs = 1_717_171_717_007L;
    for (final String alias :
        new String[] {
          "alice",
          "0x1234",
          "0xalice@universal",
          "Alice@universal",
          "alice@@universal",
          "alice@a.b.c",
          "-alice@universal",
          "alice-@universal",
          "ab--cd@universal",
          "alice@Universal",
          "alice@universal.",
          "xn--@universal",
          repeat('a', 64) + "@universal"
        }) {
      assertIllegalArgument(
          () ->
              CanonicalRequestSigner.buildHeaders(
                  NETWORK_ID,
                  "get",
                  uri,
                  new byte[0],
                  new ToriiCanonicalRequestAuth(alias, CanonicalRequestSignerTests::fakeSignature),
                  timestampMs,
                  "invalid-alias"),
          "invalid header alias: " + alias);
      assertIllegalArgument(
          () ->
              CanonicalRequestSigner.withBodySignature(
                  NETWORK_ID,
                  "post",
                  uri,
                  new LinkedHashMap<>(),
                  new ToriiCanonicalRequestAuth(alias, CanonicalRequestSignerTests::fakeSignature),
                  timestampMs,
                  "invalid-body-alias"),
          "invalid body alias: " + alias);
    }

    for (final String alias :
        new String[] {
          "alice_1@universal",
          "merchant@banka.paynet",
          "xn--bcher-kva@paynet",
          "alice@xn--fa-hia",
          "alice@xn--3xa",
          "alice@xn--nxa6a",
          "alice@xn--11b2ezcw70k",
          "alice@xn--mgba3gch31f060k",
          "alice@xn--ngba7iz95i",
          "alice@xn--ab-0ea",
          "alice@xn--a-jib",
          "alice@xn--ab-3n4a",
          "xn--alice@universal",
          "xn--a@universal",
          "alice@xn--ab-j1t",
          "alice@xn--mgba000r",
          "alice@xn--ngba000r",
          "alice@xn--ab-uuba211bca8057b",
          "alice@xn--4u8c",
          "alice@xn--pq1d",
          "alice@xn--kx7e",
          "alice@xn--5h0f",
          "alice@xn--zo5h",
          "alice@xn--fi3d",
          "alice@xn--d4f"
        }) {
      final Map<String, String> headers =
          CanonicalRequestSigner.buildHeaders(
              NETWORK_ID,
              "get",
              uri,
              new byte[0],
              new ToriiCanonicalRequestAuth(alias, CanonicalRequestSignerTests::fakeSignature),
              timestampMs,
              "valid-alias");
      assert alias.equals(headers.get(CanonicalRequestSigner.HEADER_ACCOUNT))
          : "structurally valid alias header spelling must remain exact";
    }
  }

  private static void canonicalAuthRejectsInvalidCallbackSignatures() throws Exception {
    final URI uri = new URI("https://torii.example/v1/accounts");
    final long timestampMs = 1_717_171_717_008L;
    final String alias = "alice@universal";
    final byte[] maximum =
        new byte[CanonicalRequestSigner.CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1];
    Arrays.fill(maximum, (byte) 1);
    CanonicalRequestSigner.buildHeaders(
        NETWORK_ID,
        "get",
        uri,
        new byte[0],
        new ToriiCanonicalRequestAuth(alias, ignored -> maximum.clone()),
        timestampMs,
        "maximum-signature");

    assertIllegalState(
        () ->
            CanonicalRequestSigner.buildHeaders(
                NETWORK_ID,
                "get",
                uri,
                new byte[0],
                new ToriiCanonicalRequestAuth(alias, ignored -> new byte[0]),
                timestampMs,
                "empty-signature"),
        "empty callback signature");
    assertIllegalState(
        () ->
            CanonicalRequestSigner.buildHeaders(
                NETWORK_ID,
                "get",
                uri,
                new byte[0],
                new ToriiCanonicalRequestAuth(alias, ignored -> new byte[64]),
                timestampMs,
                "zero-signature"),
        "all-zero callback signature");
    assertIllegalState(
        () ->
            CanonicalRequestSigner.buildHeaders(
                NETWORK_ID,
                "get",
                uri,
                new byte[0],
                new ToriiCanonicalRequestAuth(
                    alias,
                    ignored ->
                        new byte[
                            CanonicalRequestSigner.CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1 + 1]),
                timestampMs,
                "oversized-signature"),
        "oversized callback signature");
  }

  private static void callbackHeadersReceiveCanonicalMessage() throws Exception {
    final URI uri = new URI("https://torii.example/v1/offline/top-up?b=2&a=1");
    final byte[] body = "{\"operation_id\":\"operation-1\"}".getBytes(StandardCharsets.UTF_8);
    final long timestampMs = 1_717_171_717_001L;
    final String nonce = "callback-header-nonce";
    final byte[] expectedMessage =
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            NETWORK_ID, "post", uri, body, timestampMs, nonce);
    final byte[] expectedSignature = fakeSignature(expectedMessage);
    final AtomicReference<byte[]> signedMessage = new AtomicReference<>();
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "alice@universal",
            message -> {
              signedMessage.set(Arrays.copyOf(message, message.length));
              return fakeSignature(message);
            },
            timestampMs,
            nonce);

    final Map<String, String> headers =
        CanonicalRequestSigner.buildHeaders(
            NETWORK_ID, "post", uri, body, auth, timestampMs, nonce);

    assert Arrays.equals(expectedMessage, signedMessage.get()) : "callback header message mismatch";
    assert "alice@universal".equals(headers.get(CanonicalRequestSigner.HEADER_ACCOUNT))
        : "callback account header mismatch";
    assert Long.toString(timestampMs)
            .equals(headers.get(CanonicalRequestSigner.HEADER_TIMESTAMP_MS))
        : "callback timestamp header mismatch";
    assert nonce.equals(headers.get(CanonicalRequestSigner.HEADER_NONCE))
        : "callback nonce header mismatch";
    assert Base64.getEncoder()
            .encodeToString(expectedSignature)
            .equals(headers.get(CanonicalRequestSigner.HEADER_SIGNATURE))
        : "callback signature header mismatch";
  }

  private static void unsignedBodyAuthJsonRemovesOnlyTopLevelProofFields() {
    final Map<String, Object> nested = new LinkedHashMap<>();
    nested.put(CanonicalRequestSigner.BODY_SIGNATURE_BASE64, "keep");
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("z", "last");
    body.put(CanonicalRequestSigner.BODY_SIGNATURE_BASE64, "remove");
    body.put("nested", nested);
    body.put("witness_base64", "remove-too");
    body.put(CanonicalRequestSigner.BODY_ACCOUNT_ID, "alice");
    body.put(CanonicalRequestSigner.BODY_TIMESTAMP_MS, 7L);
    body.put(CanonicalRequestSigner.BODY_NONCE, "n");

    final String unsigned =
        new String(CanonicalRequestSigner.unsignedBodyAuthJson(body), StandardCharsets.UTF_8);

    assert "{\"account_id\":\"alice\",\"nested\":{\"signature_base64\":\"keep\"},\"nonce\":\"n\",\"timestamp_ms\":7,\"z\":\"last\"}"
            .equals(unsigned)
        : "unsigned body auth JSON mismatch: " + unsigned;
  }

  private static void callbackBodySignatureReceivesCanonicalMessage() throws Exception {
    final URI uri = new URI("https://torii.example/v1/transactions");
    final long timestampMs = 1_717_171_717_002L;
    final String nonce = "callback-body-nonce";
    final AtomicReference<byte[]> signedMessage = new AtomicReference<>();
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "alice@universal",
            message -> {
              signedMessage.set(Arrays.copyOf(message, message.length));
              return fakeSignature(message);
            },
            timestampMs,
            nonce);
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("operation_id", "operation-2");

    final Map<String, Object> signed =
        CanonicalRequestSigner.withBodySignature(
            NETWORK_ID, "post", uri, body, auth, timestampMs, nonce);
    final byte[] expectedMessage =
        CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            NETWORK_ID, "post", uri, signed, timestampMs, nonce);

    assert Arrays.equals(expectedMessage, signedMessage.get()) : "callback body message mismatch";
    assert "alice@universal".equals(signed.get(CanonicalRequestSigner.BODY_ACCOUNT_ID))
        : "callback body account_id mismatch";
    assert Long.valueOf(timestampMs).equals(signed.get(CanonicalRequestSigner.BODY_TIMESTAMP_MS))
        : "callback body timestamp mismatch";
    assert nonce.equals(signed.get(CanonicalRequestSigner.BODY_NONCE))
        : "callback body nonce mismatch";
    assert Base64.getEncoder()
            .encodeToString(fakeSignature(expectedMessage))
            .equals(signed.get(CanonicalRequestSigner.BODY_SIGNATURE_BASE64))
        : "callback body signature mismatch";
    assert !signed.containsKey("witness_base64")
        : "callback body auth must not include witness";
  }

  private static void canonicalAuthRejectsPaddedFreshnessAndAccountFields() throws Exception {
    final URI uri = new URI("https://torii.example/v1/offline/top-up");
    final byte[] bodyBytes = "{\"operation_id\":\"operation-1\"}".getBytes(StandardCharsets.UTF_8);
    final long timestampMs = 1_717_171_717_003L;
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("operation_id", "operation-1");

    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                NETWORK_ID, "post", uri, bodyBytes, timestampMs, " nonce"),
        "padded signature nonce");
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.buildHeaders(
                NETWORK_ID,
                "post",
                uri,
                bodyBytes,
                new ToriiCanonicalRequestAuth(
                    "alice@universal ", CanonicalRequestSignerTests::fakeSignature),
                timestampMs,
                "nonce"),
        "padded header account");
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.buildHeaders(
                NETWORK_ID,
                "post",
                uri,
                bodyBytes,
                new ToriiCanonicalRequestAuth(
                    "alice@universal", CanonicalRequestSignerTests::fakeSignature),
                timestampMs,
                "\nnonce"),
        "padded header nonce");
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.withBodySignature(
                NETWORK_ID,
                "post",
                uri,
                body,
                new ToriiCanonicalRequestAuth(
                    " alice@universal", CanonicalRequestSignerTests::fakeSignature),
                timestampMs,
                "nonce"),
        "padded body account");
    assertIllegalArgument(
        () ->
            CanonicalRequestSigner.withBodySignature(
                NETWORK_ID,
                "post",
                uri,
                body,
                new ToriiCanonicalRequestAuth(
                    "alice@universal", CanonicalRequestSignerTests::fakeSignature),
                timestampMs,
                "nonce "),
        "padded body nonce");
  }

  private static void canonicalAuthCannotReplayAcrossSameLabelNetworks() throws Exception {
    final URI uri = new URI("https://torii.example/v1/accounts?label=same");
    final byte[] canonical =
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            NETWORK_ID,
            "GET",
            uri,
            new byte[0],
            1_717_171_717_003L,
            "network-bound-nonce");
    final byte[] foreign =
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            TestNetworkIds.fromSeed(7L),
            "GET",
            uri,
            new byte[0],
            1_717_171_717_003L,
            "network-bound-nonce");

    assert !Arrays.equals(canonical, foreign)
        : "same-label requests on different genesis networks must not share signing bytes";
  }

  private static void assertIllegalArgument(final Runnable body, final String label) {
    try {
      body.run();
    } catch (IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(label + " should reject");
  }

  private static void assertIllegalState(final Runnable body, final String label) {
    try {
      body.run();
    } catch (IllegalStateException expected) {
      return;
    }
    throw new AssertionError(label + " should reject");
  }

  private static byte[] fakeSignature(final byte[] message) {
    final byte[] signature = new byte[64];
    for (int index = 0; index < signature.length; index++) {
      signature[index] = (byte) (message[index % message.length] ^ index);
    }
    return signature;
  }

  private static String repeat(final char value, final int count) {
    final char[] chars = new char[count];
    Arrays.fill(chars, value);
    return new String(chars);
  }
}
