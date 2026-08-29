package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.net.URI;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.junit.Test;

/** Adversarial transport-contract tests for the native Bootle/Lantern issuance client. */
public final class BootleLanternIssuanceClientV1Tests {
  /** Binds every public client to the shared synthetic wire-contract fixture. */
  @Test
  public void sharedClientContractFixtureBindsExactWireBytes() throws Exception {
    final Map<String, Object> fixture =
        object(
            JsonParser.parse(
                new String(
                    Files.readAllBytes(clientContractFixture()), StandardCharsets.UTF_8)));
    assertEquals(
        "iroha.bootle_lantern.issuance_client_contract", fixture.get("schema"));
    assertEquals(1, JsonNumbers.asInt(fixture.get("version"), "version"));
    assertEquals("public-synthetic-test-data", fixture.get("classification"));

    final Map<String, Object> transport = object(fixture.get("transport"));
    assertEquals("POST", transport.get("method"));
    assertEquals(BootleLanternIssuanceClientV1.AUTHORIZE_PATH, transport.get("authorize_path"));
    assertEquals(BootleLanternIssuanceClientV1.ISSUE_PATH, transport.get("issue_path"));
    assertEquals(
        BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
        transport.get("norito_media_type"));
    assertEquals(
        "Bearer realm=\"iroha-bootle-lantern-issuance\"",
        transport.get("unauthorized_www_authenticate"));

    final Map<String, Object> credentialContract = object(fixture.get("credential"));
    assertEquals("base64url-unpadded-canonical", credentialContract.get("encoding"));
    assertEquals(
        1,
        JsonNumbers.asInt(
            credentialContract.get("minimum_decoded_bytes"), "minimum_decoded_bytes"));
    assertEquals(
        BootleLanternIssuanceCredentialV1.MAX_BYTES,
        JsonNumbers.asInt(
            credentialContract.get("maximum_decoded_bytes"), "maximum_decoded_bytes"));
    final List<Object> examples = array(credentialContract.get("examples"));
    assertEquals(3, examples.size());
    for (final Object value : examples) {
      final Map<String, Object> example = object(value);
      final byte[] decoded = hexBytes((String) example.get("decoded_hex"));
      final String encoded = (String) example.get("encoded");
      assertEquals(
          encoded, Base64.getUrlEncoder().withoutPadding().encodeToString(decoded));
      final BootleLanternIssuanceCredentialV1 admitted =
          BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(encoded);
      assertEquals("Bearer " + encoded, admitted.authorizationHeaderValue());
      admitted.close();
    }

    final Map<String, Object> bodies = object(fixture.get("bodies"));
    assertEquals(
        "byte-at-index-equals-index-modulo-256-with-canonical-wire-magics",
        bodies.get("pattern"));
    final Object[][] bodyContracts =
        new Object[][] {
          {
            "authorization_response",
            "ILA1",
            BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES
          },
          {
            "issue_request",
            "ILA1+ILQ1",
            BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES
          },
          {
            "issue_response",
            "ILR1",
            BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES
          }
        };
    for (final Object[] bodyContract : bodyContracts) {
      final String name = (String) bodyContract[0];
      final String wire = (String) bodyContract[1];
      final int length = (Integer) bodyContract[2];
      final Map<String, Object> body = object(bodies.get(name));
      assertEquals(wire, body.get("wire"));
      assertEquals(length, JsonNumbers.asInt(body.get("length_bytes"), name + ".length_bytes"));
      assertEquals(body.get("pattern_sha256_hex"), sha256Hex(patterned(length)));
    }
    assertArrayEquals("ILA1".getBytes(StandardCharsets.US_ASCII),
        java.util.Arrays.copyOfRange(patterned(320), 0, 4));
    assertArrayEquals("ILA1".getBytes(StandardCharsets.US_ASCII),
        java.util.Arrays.copyOfRange(patterned(71_896), 0, 4));
    assertArrayEquals("ILQ1".getBytes(StandardCharsets.US_ASCII),
        java.util.Arrays.copyOfRange(patterned(71_896), 320, 324));
    assertArrayEquals("ILR1".getBytes(StandardCharsets.US_ASCII),
        java.util.Arrays.copyOfRange(patterned(3_176), 0, 4));
    final List<Object> componentValues =
        array(object(bodies.get("issue_request")).get("component_lengths_bytes"));
    assertEquals(2, componentValues.size());
    final int authorizationLength =
        JsonNumbers.asInt(componentValues.get(0), "component_lengths_bytes[0]");
    final int blindRequestLength =
        JsonNumbers.asInt(componentValues.get(1), "component_lengths_bytes[1]");
    assertEquals(320, authorizationLength);
    assertEquals(71_576, blindRequestLength);
    assertEquals(
        BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES,
        authorizationLength + blindRequestLength);

    final Map<String, Object> errors = object(fixture.get("errors"));
    assertEquals(
        BootleLanternIssuanceClientV1.ERROR_RESPONSE_MAX_BYTES,
        JsonNumbers.asInt(errors.get("maximum_body_bytes"), "maximum_body_bytes"));
    final Map<String, Object> envelope = object(errors.get("norito_envelope"));
    assertEquals("iroha_torii_shared::ErrorEnvelope", envelope.get("schema_type_name"));
    assertEquals("793f11768076bfe270a17aeb86752cd9", envelope.get("schema_hash_hex"));
    assertEquals("02", envelope.get("flags_hex"));
    final List<Object> errorResponses = array(errors.get("responses"));
    assertEquals(8, errorResponses.size());
    for (final Object value : errorResponses) {
      final Map<String, Object> contract = object(value);
      assertEquals(
          JsonNumbers.asInt(contract.get("status"), "status") == 401
              ? transport.get("unauthorized_www_authenticate")
              : null,
          contract.get("www_authenticate"));
      final BootleLanternIssuanceClientExceptionV1 failure =
          assertClientFailure(
              client(new ScriptedExecutor(errorResponse(contract))).authorize(credential()));
      assertEquals(
          Integer.valueOf(JsonNumbers.asInt(contract.get("status"), "status")),
          failure.statusCode());
      assertEquals(contract.get("code"), failure.code());
      assertEquals(retryAfter(contract), failure.retryAfterSeconds());
    }
  }

  /** Verifies the exact empty authorization request and one-attempt contract. */
  @Test
  public void authorizeUsesCanonicalExactEmptySingleAttemptRequest() {
    final byte[] responseBytes =
        patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES);
    final ScriptedExecutor executor = new ScriptedExecutor(success(responseBytes));
    final BootleLanternIssuanceCredentialV1 credential =
        BootleLanternIssuanceCredentialV1.fromOpaqueBytes(new byte[] {0x61});

    final byte[] result = client(executor).authorize(credential).join();

    assertArrayEquals(responseBytes, result);
    assertEquals(1, executor.calls);
    final TransportRequest request = executor.lastRequest;
    assertEquals("POST", request.method());
    assertEquals(BootleLanternIssuanceClientV1.AUTHORIZE_PATH, request.uri().getRawPath());
    assertArrayEquals(new byte[0], request.body());
    assertEquals(
        Long.valueOf(BootleLanternIssuanceClientV1.ERROR_RESPONSE_MAX_BYTES),
        request.maximumResponseBytes());
    assertEquals("Bearer YQ", exactHeader(request, "Authorization"));
    assertEquals(
        BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
        exactHeader(request, "Content-Type"));
    assertEquals(
        BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
        exactHeader(request, "Accept"));
    assertEquals("identity", exactHeader(request, "Accept-Encoding"));
    assertTrue(headerValues(request, "Content-Encoding").isEmpty());
    assertEquals("no-store", exactHeader(request, "Cache-Control"));
    assertEquals("no-cache", exactHeader(request, "Pragma"));
  }

  /** Verifies exact issue lengths and defensive request copies. */
  @Test
  public void issueUsesExactDefensiveBodyAndResponseLimits() {
    final byte[] requestBytes = patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES);
    final byte[] original = requestBytes.clone();
    final byte[] responseBytes = patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES);
    final ScriptedExecutor executor = new ScriptedExecutor(success(responseBytes));
    final BootleLanternIssuanceCredentialV1 credential =
        BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url("AQID");

    final CompletableFuture<byte[]> future = client(executor).issue(credential, requestBytes);
    java.util.Arrays.fill(requestBytes, (byte) 0);
    final byte[] result = future.join();

    assertArrayEquals(responseBytes, result);
    assertEquals(1, executor.calls);
    assertEquals(BootleLanternIssuanceClientV1.ISSUE_PATH, executor.lastRequest.uri().getRawPath());
    assertArrayEquals(original, executor.lastRequest.body());
    assertEquals("Bearer AQID", exactHeader(executor.lastRequest, "Authorization"));
    assertEquals(
        Long.valueOf(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES),
        executor.lastRequest.maximumResponseBytes());
  }

  /** Rejects all non-exact issue request sizes before the executor can observe them. */
  @Test
  public void issueRejectsZeroTruncatedExtendedAndOversizedBodiesBeforeExecution() {
    final ScriptedExecutor executor =
        new ScriptedExecutor(success(patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES)));
    final BootleLanternIssuanceCredentialV1 credential = credential();
    final int[] invalidSizes =
        new int[] {
          0,
          1,
          BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES - 1,
          BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES + 1,
          BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES * 2
        };

    for (final int size : invalidSizes) {
      assertThrows(
          IllegalArgumentException.class,
          () -> client(executor).issue(credential, new byte[size]));
    }
    assertEquals(0, executor.calls);
  }

  /** Rejects correct-length issue requests with wrong, truncated, or shifted ILA1 magic. */
  @Test
  public void issueRejectsSameLengthWrongTruncatedShiftedAndSubstitutedIla1Magic() {
    final ScriptedExecutor executor =
        new ScriptedExecutor(
            success(patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES)));
    final byte[][] prefixes =
        new byte[][] {
          new byte[] {0, 0, 0, 0},
          "ILA0".getBytes(StandardCharsets.US_ASCII),
          new byte[] {0x49, 0x4c, 0x41, 0},
          "XLA1".getBytes(StandardCharsets.US_ASCII)
        };
    for (final byte[] prefix : prefixes) {
      final byte[] request = patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES);
      System.arraycopy(prefix, 0, request, 0, prefix.length);
      assertThrows(
          IllegalArgumentException.class,
          () -> client(executor).issue(credential(), request));
    }
    final byte[][] blindRequestPrefixes =
        new byte[][] {
          new byte[] {0, 0, 0, 0},
          "ILQ0".getBytes(StandardCharsets.US_ASCII),
          new byte[] {0x49, 0x4c, 0x51, 0},
          "XLQ1".getBytes(StandardCharsets.US_ASCII)
        };
    for (final byte[] prefix : blindRequestPrefixes) {
      final byte[] request = patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES);
      System.arraycopy(
          prefix,
          0,
          request,
          BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES,
          prefix.length);
      assertThrows(
          IllegalArgumentException.class,
          () -> client(executor).issue(credential(), request));
    }
    assertEquals(0, executor.calls);
  }

  /** Rejects malformed or overlong credentials and redacts retained secret material. */
  @Test
  public void credentialAdmissionIsCanonicalBoundedDefensiveAndRedacted() {
    assertThrows(
        IllegalArgumentException.class,
        () -> BootleLanternIssuanceCredentialV1.fromOpaqueBytes(new byte[0]));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            BootleLanternIssuanceCredentialV1.fromOpaqueBytes(
                new byte[BootleLanternIssuanceCredentialV1.MAX_BYTES + 1]));
    final List<String> malformed = new ArrayList<>();
    Collections.addAll(
        malformed, "", "A", "YQ==", "YR", "Y Q", "YQ\n", "Bearer YQ", "+w");
    malformed.add(
        Base64.getUrlEncoder()
            .withoutPadding()
            .encodeToString(new byte[BootleLanternIssuanceCredentialV1.MAX_BYTES + 1]));
    malformed.add(
        repeat(
            'A',
            ((BootleLanternIssuanceCredentialV1.MAX_BYTES + 2) / 3) * 4 + 1));
    for (final String encoded : malformed) {
      assertThrows(
          "credential must fail: " + encoded,
          IllegalArgumentException.class,
          () -> BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(encoded));
    }

    final byte[] source = new byte[] {0x61};
    final BootleLanternIssuanceCredentialV1 credential =
        BootleLanternIssuanceCredentialV1.fromOpaqueBytes(source);
    source[0] = 0x62;
    final ScriptedExecutor executor =
        new ScriptedExecutor(
            success(patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)));
    client(executor).authorize(credential).join();
    assertEquals("Bearer YQ", exactHeader(executor.lastRequest, "Authorization"));
    assertFalse(credential.toString().contains("YQ"));
    assertFalse(credential.toString().contains("61"));
    assertTrue(credential.toString().contains("REDACTED"));

    final byte[] exactMaximum = new byte[BootleLanternIssuanceCredentialV1.MAX_BYTES];
    java.util.Arrays.fill(exactMaximum, (byte) 0xff);
    final String exactMaximumEncoded =
        Base64.getUrlEncoder().withoutPadding().encodeToString(exactMaximum);
    final BootleLanternIssuanceCredentialV1 maximumCredential =
        BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(exactMaximumEncoded);
    assertEquals("Bearer " + exactMaximumEncoded, maximumCredential.authorizationHeaderValue());
    maximumCredential.close();

    credential.close();
    credential.close();
    assertThrows(IllegalStateException.class, () -> client(executor).authorize(credential));
    assertEquals(1, executor.calls);
  }

  /** Rejects zero, truncated, and extended authorization response bodies. */
  @Test
  public void authorizeRejectsZeroTruncatedAndExtendedResponses() {
    final int[] sizes =
        new int[] {
          0,
          1,
          BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES - 1,
          BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES + 1
        };
    for (final int size : sizes) {
      final ScriptedExecutor executor = new ScriptedExecutor(success(new byte[size], false));
      assertClientFailure(client(executor).authorize(credential()));
      assertEquals(1, executor.calls);
    }
  }

  /** Rejects zero, truncated, and extended issue response bodies. */
  @Test
  public void issueRejectsZeroTruncatedAndExtendedResponses() {
    final int[] sizes =
        new int[] {
          0,
          1,
          BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES - 1,
          BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES + 1
        };
    for (final int size : sizes) {
      final ScriptedExecutor executor = new ScriptedExecutor(success(new byte[size], false));
      assertClientFailure(
          client(executor)
              .issue(
                  credential(),
                  patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES)));
      assertEquals(1, executor.calls);
    }
  }

  /** Rejects exact-length successful responses without exact ILA1 or ILR1 magic. */
  @Test
  public void successfulResponsesRequireExactIla1AndIlr1Magic() {
    final byte[][] authorizationPrefixes =
        new byte[][] {
          new byte[] {0, 0, 0, 0},
          "ILA0".getBytes(StandardCharsets.US_ASCII),
          new byte[] {0x49, 0x4c, 0x41, 0},
          "XLA1".getBytes(StandardCharsets.US_ASCII)
        };
    for (final byte[] prefix : authorizationPrefixes) {
      final byte[] body =
          patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES);
      System.arraycopy(prefix, 0, body, 0, prefix.length);
      assertClientFailure(client(new ScriptedExecutor(success(body))).authorize(credential()));
    }
    final byte[][] responsePrefixes =
        new byte[][] {
          new byte[] {0, 0, 0, 0},
          "ILR0".getBytes(StandardCharsets.US_ASCII),
          new byte[] {0x49, 0x4c, 0x52, 0},
          "XLR1".getBytes(StandardCharsets.US_ASCII)
        };
    for (final byte[] prefix : responsePrefixes) {
      final byte[] body = patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES);
      System.arraycopy(prefix, 0, body, 0, prefix.length);
      assertClientFailure(
          client(new ScriptedExecutor(success(body)))
              .issue(
                  credential(),
                  patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES)));
    }
  }

  /** Rejects redirects and every non-200 status without retries. */
  @Test
  public void responsesRequireExactStatusAndNeverRetryRedirectOrFailure() {
    final int[] statuses =
        new int[] {201, 204, 301, 302, 307, 308, 418, 500};
    for (final int status : statuses) {
      final Map<String, List<String>> headers = new LinkedHashMap<>();
      headers.put(
          "Content-Type",
          Collections.singletonList(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE));
      final ScriptedExecutor executor =
          new ScriptedExecutor(
              response(
                  status,
                  patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES),
                  headers));
      assertClientFailure(client(executor).authorize(credential()));
      assertEquals("HTTP " + status + " must not be retried", 1, executor.calls);
    }

    final ScriptedExecutor asynchronousFailure =
        ScriptedExecutor.failure(new IllegalStateException("network down"));
    assertClientFailure(client(asynchronousFailure).authorize(credential()));
    assertEquals(1, asynchronousFailure.calls);

    final ScriptedExecutor synchronousFailure = ScriptedExecutor.synchronousFailure();
    assertClientFailure(client(synchronousFailure).authorize(credential()));
    assertEquals(1, synchronousFailure.calls);
  }

  /** Discards secret-bearing synchronous and asynchronous transport exception causes. */
  @Test
  public void transportFailuresAfterAuthorizationExposureDiscardSecretBearingCauses() {
    final String leaked = "Bearer secret-that-must-not-survive";
    final ScriptedExecutor asynchronous =
        ScriptedExecutor.failure(new IllegalStateException(leaked));
    final BootleLanternIssuanceClientExceptionV1 asynchronousFailure =
        assertClientFailure(client(asynchronous).authorize(credential()));
    assertEquals(
        "Bootle/Lantern issuance authorization request failed",
        asynchronousFailure.getMessage());
    assertEquals(null, asynchronousFailure.getCause());
    assertFalse(asynchronousFailure.toString().contains(leaked));

    final ScriptedExecutor synchronous =
        ScriptedExecutor.synchronousFailure(new IllegalStateException(leaked));
    final BootleLanternIssuanceClientExceptionV1 synchronousFailure =
        assertClientFailure(client(synchronous).authorize(credential()));
    assertEquals(
        "Bootle/Lantern issuance authorization request failed",
        synchronousFailure.getMessage());
    assertEquals(null, synchronousFailure.getCause());
    assertFalse(synchronousFailure.toString().contains(leaked));
  }

  /** Decodes every canonical first-release structured error contract. */
  @Test
  public void structuredErrorsBindStatusMediaCodeAndRetryHint() throws Exception {
    for (final Map<String, Object> contract : errorContracts()) {
      final ScriptedExecutor executor = new ScriptedExecutor(errorResponse(contract));
      final BootleLanternIssuanceClientExceptionV1 failure =
          assertClientFailure(client(executor).authorize(credential()));
      assertEquals(
          Integer.valueOf(JsonNumbers.asInt(contract.get("status"), "status")),
          failure.statusCode());
      assertEquals(contract.get("code"), failure.code());
      assertEquals(retryAfter(contract), failure.retryAfterSeconds());
      assertEquals(1, executor.calls);
    }
  }

  /** Rejects every obsolete or non-canonical framing form for all Norito error statuses. */
  @Test
  public void allSevenNoritoErrorsRejectLegacyMalformedTruncatedAndTrailingFrames()
      throws Exception {
    int noritoContracts = 0;
    for (final Map<String, Object> contract : errorContracts()) {
      if (!BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE.equals(
          contract.get("media_type"))) {
        continue;
      }
      noritoContracts++;
      final byte[] canonical = errorBody(contract);
      final List<byte[]> variants =
          java.util.Arrays.asList(
              rejectedLegacyNoritoErrorFrame(canonical, (String) contract.get("code")),
              malformedNoritoFieldFrame(canonical),
              java.util.Arrays.copyOf(canonical, canonical.length - 1),
              java.util.Arrays.copyOf(canonical, canonical.length + 1));
      for (final byte[] body : variants) {
        final BootleLanternIssuanceClientExceptionV1 failure =
            assertClientFailure(
                client(new ScriptedExecutor(errorResponse(contract, body, null)))
                    .authorize(credential()));
        assertEquals(null, failure.statusCode());
        assertEquals(null, failure.code());
        assertEquals(null, failure.retryAfterSeconds());
      }
    }
    assertEquals(7, noritoContracts);
  }

  /** Rejects corrupt, substituted, overlong, and contradictory structured errors. */
  @Test
  public void structuredErrorsRejectMalformedSubstitutedAndOversizedEnvelopes()
      throws Exception {
    final Map<Integer, Map<String, Object>> contracts = new LinkedHashMap<>();
    for (final Map<String, Object> contract : errorContracts()) {
      contracts.put(JsonNumbers.asInt(contract.get("status"), "status"), contract);
    }
    final Map<String, Object> badRequest = contracts.get(400);
    final Map<String, Object> unauthorized = contracts.get(401);
    final Map<String, Object> notAcceptable = contracts.get(406);
    final Map<String, Object> capacity = contracts.get(429);
    final Map<String, Object> unavailable = contracts.get(503);
    final byte[] corrupted = errorBody(badRequest);
    corrupted[0] ^= 1;

    final List<TransportResponse> adversarial = new ArrayList<>();
    adversarial.add(errorResponse(badRequest, corrupted, null));
    adversarial.add(
        errorResponse(
            badRequest,
            null,
            headers(
                "Content-Type",
                "application/json",
                "Content-Length",
                Integer.toString(errorBody(badRequest).length))));
    adversarial.add(
        errorResponse(
            badRequest,
            null,
            headers(
                "Content-Type",
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
                "Content-Encoding",
                "identity")));
    adversarial.add(
        errorResponse(
            badRequest,
            null,
            headers(
                "Content-Type",
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
                "Content-Length",
                "0107")));
    adversarial.add(errorResponse(badRequest, errorBody(unauthorized), null));
    adversarial.add(
        errorResponse(
            notAcceptable,
            (((String) notAcceptable.get("body_utf8")) + " ")
                .getBytes(StandardCharsets.UTF_8),
            null));
    adversarial.add(
        errorResponse(
            capacity,
            null,
            headers(
                "Content-Type",
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
                "Retry-After",
                "2")));
    adversarial.add(
        errorResponse(
            unavailable,
            null,
            headers(
                "Content-Type",
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
                "Retry-After",
                "1")));
    adversarial.add(
        errorResponse(
            unauthorized,
            null,
            headers(
                "Content-Type",
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
                "Content-Length",
                Integer.toString(errorBody(unauthorized).length))));
    final String challenge = (String) unauthorized.get("www_authenticate");
    final Map<String, List<String>> duplicateChallenge =
        headers(
            "Content-Type",
            BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
            "Content-Length",
            Integer.toString(errorBody(unauthorized).length));
    duplicateChallenge.put("WWW-Authenticate", java.util.Arrays.asList(challenge, challenge));
    adversarial.add(errorResponse(unauthorized, null, duplicateChallenge));
    adversarial.add(
        errorResponse(
            unauthorized,
            null,
            headers(
                "Content-Type",
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
                "Content-Length",
                Integer.toString(errorBody(unauthorized).length),
                "WWW-Authenticate",
                "Bearer realm=\"attacker\"")));
    adversarial.add(
        errorResponse(
            badRequest,
            null,
            headers(
                "Content-Type",
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
                "Content-Length",
                Integer.toString(errorBody(badRequest).length),
                "WWW-Authenticate",
                challenge)));
    adversarial.add(
        errorResponse(
            badRequest,
            new byte[BootleLanternIssuanceClientV1.ERROR_RESPONSE_MAX_BYTES + 1],
            null));
    for (final TransportResponse response : adversarial) {
      final BootleLanternIssuanceClientExceptionV1 failure =
          assertClientFailure(client(new ScriptedExecutor(response)).authorize(credential()));
      assertEquals(null, failure.statusCode());
      assertEquals(null, failure.code());
      assertEquals(null, failure.retryAfterSeconds());
    }
  }

  /** Requires one byte-for-byte canonical Norito content type. */
  @Test
  public void responsesRequireOneExactNoritoContentType() {
    final byte[] bytes =
        patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES);
    final List<List<String>> variants = new ArrayList<>();
    variants.add(Collections.emptyList());
    variants.add(Collections.singletonList("application/json"));
    variants.add(Collections.singletonList("application/x-norito; charset=binary"));
    variants.add(Collections.singletonList("Application/X-Norito"));
    variants.add(
        java.util.Arrays.asList("application/x-norito", "application/x-norito"));
    for (final List<String> values : variants) {
      final Map<String, List<String>> headers = new LinkedHashMap<>();
      if (!values.isEmpty()) {
        headers.put("Content-Type", values);
      }
      final ScriptedExecutor executor =
          new ScriptedExecutor(response(200, bytes, headers));
      assertClientFailure(client(executor).authorize(credential()));
      assertEquals(1, executor.calls);
    }
  }

  /** Rejects all response content encodings, including a redundant identity encoding. */
  @Test
  public void responsesRejectCompressionIncludingIdentityEncoding() {
    for (final String encoding : new String[] {"gzip", "br", "deflate", "identity"}) {
      final Map<String, List<String>> headers = new LinkedHashMap<>();
      headers.put(
          "Content-Type",
          Collections.singletonList(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE));
      headers.put("Content-Encoding", Collections.singletonList(encoding));
      final ScriptedExecutor executor =
          new ScriptedExecutor(
              response(
                  200,
                  patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES),
                  headers));
      assertClientFailure(client(executor).authorize(credential()));
      assertEquals(1, executor.calls);
    }

    final Map<String, List<String>> challengedHeaders = new LinkedHashMap<>();
    challengedHeaders.put(
        "Content-Type",
        Collections.singletonList(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE));
    challengedHeaders.put(
        "WWW-Authenticate",
        Collections.singletonList("Bearer realm=\"iroha-bootle-lantern-issuance\""));
    assertClientFailure(
        client(
                new ScriptedExecutor(
                    response(
                        200,
                        patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES),
                        challengedHeaders)))
            .authorize(credential()));
  }

  /** Requires an optional Content-Length to be unique, canonical, and body-exact. */
  @Test
  public void responseContentLengthMustBeUniqueCanonicalAndExactWhenPresent() {
    final byte[] bytes =
        patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES);
    final List<List<String>> variants =
        java.util.Arrays.asList(
            Collections.singletonList("319"),
            Collections.singletonList("321"),
            Collections.singletonList("0320"),
            Collections.singletonList("+320"),
            Collections.singletonList("320 "),
            java.util.Arrays.asList("320", "320"));
    for (final List<String> values : variants) {
      final Map<String, List<String>> headers = new LinkedHashMap<>();
      headers.put(
          "Content-Type",
          Collections.singletonList(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE));
      headers.put("Content-Length", values);
      final ScriptedExecutor executor =
          new ScriptedExecutor(response(200, bytes, headers));
      assertClientFailure(client(executor).authorize(credential()));
    }

    assertArrayEquals(
        bytes,
        client(new ScriptedExecutor(success(bytes, false))).authorize(credential()).join());
    assertArrayEquals(
        bytes,
        client(new ScriptedExecutor(success(bytes, true))).authorize(credential()).join());
  }

  /** Ensures failures do not render credentials or echoed response bodies. */
  @Test
  public void errorsNeverRenderCredentialOrResponseBody() {
    final String secret = "credential-secret";
    final String encoded =
        Base64.getUrlEncoder()
            .withoutPadding()
            .encodeToString(secret.getBytes(StandardCharsets.UTF_8));
    final BootleLanternIssuanceCredentialV1 credential =
        BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(encoded);
    final ScriptedExecutor executor =
        new ScriptedExecutor(
            response(
                401,
                ("server-echo:" + secret + ":" + encoded).getBytes(StandardCharsets.UTF_8),
                Collections.emptyMap()));

    final BootleLanternIssuanceClientExceptionV1 error =
        assertClientFailure(client(executor).authorize(credential));
    final String rendered = error.toString();
    assertFalse(rendered.contains(secret));
    assertFalse(rendered.contains(encoded));
    assertFalse(rendered.contains("server-echo"));
  }

  /** Rejects insecure and path-bearing Torii roots before a credential is submitted. */
  @Test
  public void clientRejectsInsecureOrNonOriginBaseUrisBeforeSendingCredentials() {
    final String[] invalidUris =
        new String[] {
          "http://taira.sora.org",
          "https://user@taira.sora.org",
          "https://taira.sora.org/proxy",
          "https://taira.sora.org?route=privacy",
          "https://taira.sora.org#privacy",
          "/relative"
        };
    for (final String uri : invalidUris) {
      final ScriptedExecutor executor =
          new ScriptedExecutor(
              success(patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)));
      assertThrows(
          uri,
          IllegalArgumentException.class,
          () ->
              BootleLanternIssuanceClientV1.builder()
                  .baseUri(URI.create(uri))
                  .executor(executor)
                  .build());
      assertEquals(0, executor.calls);
    }
  }

  private static BootleLanternIssuanceClientV1 client(final HttpTransportExecutor executor) {
    return BootleLanternIssuanceClientV1.builder()
        .baseUri(URI.create("https://taira.sora.org"))
        .executor(executor)
        .build();
  }

  private static BootleLanternIssuanceCredentialV1 credential() {
    return BootleLanternIssuanceCredentialV1.fromOpaqueBytes(new byte[] {1, 2, 3});
  }

  private static String exactHeader(final TransportRequest request, final String name) {
    final List<String> values = headerValues(request, name);
    assertEquals(name + " must occur exactly once", 1, values.size());
    return values.get(0);
  }

  private static List<String> headerValues(
      final TransportRequest request, final String name) {
    final List<String> values = new ArrayList<>();
    for (final Map.Entry<String, List<String>> header : request.headers().entrySet()) {
      if (header.getKey().equalsIgnoreCase(name)) {
        values.addAll(header.getValue());
      }
    }
    return values;
  }

  private static BootleLanternIssuanceClientExceptionV1 assertClientFailure(
      final CompletableFuture<byte[]> future) {
    try {
      future.join();
      fail("expected Bootle/Lantern issuance client failure");
      throw new AssertionError("unreachable");
    } catch (final CompletionException failure) {
      assertTrue(failure.getCause() instanceof BootleLanternIssuanceClientExceptionV1);
      return (BootleLanternIssuanceClientExceptionV1) failure.getCause();
    }
  }

  private static TransportResponse success(final byte[] body) {
    return success(body, true);
  }

  private static TransportResponse success(final byte[] body, final boolean includeLength) {
    final Map<String, List<String>> headers = new LinkedHashMap<>();
    headers.put(
        "Content-Type",
        Collections.singletonList(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE));
    if (includeLength) {
      headers.put("Content-Length", Collections.singletonList(Integer.toString(body.length)));
    }
    return response(200, body, headers);
  }

  private static List<Map<String, Object>> errorContracts() throws Exception {
    final Map<String, Object> fixture =
        object(
            JsonParser.parse(
                new String(
                    Files.readAllBytes(clientContractFixture()), StandardCharsets.UTF_8)));
    final List<Map<String, Object>> contracts = new ArrayList<>();
    for (final Object value : array(object(fixture.get("errors")).get("responses"))) {
      contracts.add(object(value));
    }
    return contracts;
  }

  private static byte[] errorBody(final Map<String, Object> contract) {
    final Object bodyHex = contract.get("body_hex");
    return bodyHex == null
        ? ((String) contract.get("body_utf8")).getBytes(StandardCharsets.UTF_8)
        : hexBytes((String) bodyHex);
  }

  private static byte[] malformedNoritoFieldFrame(final byte[] body) {
    final byte[] malformed = java.util.Arrays.copyOf(body, body.length);
    assertArrayEquals(
        "NRT0".getBytes(StandardCharsets.US_ASCII),
        java.util.Arrays.copyOfRange(malformed, 0, 4));
    final ByteBuffer frame = ByteBuffer.wrap(malformed).order(ByteOrder.LITTLE_ENDIAN);
    final long payloadLength = frame.getLong(23);
    assertEquals(40L + payloadLength, malformed.length);
    assertTrue((malformed[40] & 0xFF) < 0x7F);
    malformed[40]++;
    frame.putLong(31, crc64(java.util.Arrays.copyOfRange(malformed, 40, malformed.length)));
    return malformed;
  }

  private static byte[] rejectedLegacyNoritoErrorFrame(
      final byte[] template, final String code) {
    final byte[] encoded = code.getBytes(StandardCharsets.UTF_8);
    assertTrue(encoded.length < 0x80);
    final byte[] payload = new byte[encoded.length * 2 + 3];
    int offset = 0;
    payload[offset++] = (byte) encoded.length;
    System.arraycopy(encoded, 0, payload, offset, encoded.length);
    offset += encoded.length;
    payload[offset++] = (byte) encoded.length;
    System.arraycopy(encoded, 0, payload, offset, encoded.length);
    offset += encoded.length;
    payload[offset] = 0;
    return noritoFrameWithPayload(template, payload);
  }

  private static byte[] noritoFrameWithPayload(
      final byte[] template, final byte[] payload) {
    final byte[] frameBytes = java.util.Arrays.copyOf(template, 40 + payload.length);
    System.arraycopy(payload, 0, frameBytes, 40, payload.length);
    final ByteBuffer frame = ByteBuffer.wrap(frameBytes).order(ByteOrder.LITTLE_ENDIAN);
    frame.putLong(23, payload.length);
    frame.putLong(31, crc64(payload));
    return frameBytes;
  }

  private static long crc64(final byte[] payload) {
    final long polynomial = 0xC96C5795D7870F42L;
    long value = 0xFFFFFFFFFFFFFFFFL;
    for (final byte raw : payload) {
      value ^= raw & 0xFFL;
      for (int bit = 0; bit < 8; bit++) {
        value = (value & 1L) == 0L ? value >>> 1 : polynomial ^ (value >>> 1);
      }
    }
    return value ^ 0xFFFFFFFFFFFFFFFFL;
  }

  private static Long retryAfter(final Map<String, Object> contract) {
    return contract.containsKey("retry_after_seconds")
        ? Long.valueOf(
            JsonNumbers.asInt(
                contract.get("retry_after_seconds"), "retry_after_seconds"))
        : null;
  }

  private static TransportResponse errorResponse(final Map<String, Object> contract) {
    return errorResponse(contract, null, null);
  }

  private static TransportResponse errorResponse(
      final Map<String, Object> contract,
      final byte[] replacementBody,
      final Map<String, List<String>> replacementHeaders) {
    final byte[] body = replacementBody == null ? errorBody(contract) : replacementBody;
    final Map<String, List<String>> canonicalHeaders = new LinkedHashMap<>();
    canonicalHeaders.put(
        "Content-Type", Collections.singletonList((String) contract.get("media_type")));
    canonicalHeaders.put(
        "Content-Length", Collections.singletonList(Integer.toString(body.length)));
    final Long retryAfter = retryAfter(contract);
    if (retryAfter != null) {
      canonicalHeaders.put(
          "Retry-After", Collections.singletonList(Long.toString(retryAfter)));
    }
    if (contract.containsKey("www_authenticate")) {
      canonicalHeaders.put(
          "WWW-Authenticate",
          Collections.singletonList((String) contract.get("www_authenticate")));
    }
    return response(
        JsonNumbers.asInt(contract.get("status"), "status"),
        body,
        replacementHeaders == null ? canonicalHeaders : replacementHeaders);
  }

  private static Map<String, List<String>> headers(final String... entries) {
    if ((entries.length & 1) != 0) {
      throw new AssertionError("header fixtures must be name/value pairs");
    }
    final Map<String, List<String>> headers = new LinkedHashMap<>();
    for (int index = 0; index < entries.length; index += 2) {
      headers.put(entries[index], Collections.singletonList(entries[index + 1]));
    }
    return headers;
  }

  private static TransportResponse response(
      final int status, final byte[] body, final Map<String, List<String>> headers) {
    return new TransportResponse(status, body, "scripted", headers, null, false);
  }

  private static byte[] patterned(final int size) {
    final byte[] output = new byte[size];
    for (int index = 0; index < size; index++) {
      output[index] = (byte) index;
    }
    if (size == BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES) {
      System.arraycopy("ILA1".getBytes(StandardCharsets.US_ASCII), 0, output, 0, 4);
    } else if (size == BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES) {
      System.arraycopy("ILA1".getBytes(StandardCharsets.US_ASCII), 0, output, 0, 4);
      System.arraycopy("ILQ1".getBytes(StandardCharsets.US_ASCII), 0, output, 320, 4);
    } else if (size == BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES) {
      System.arraycopy("ILR1".getBytes(StandardCharsets.US_ASCII), 0, output, 0, 4);
    }
    return output;
  }

  private static Path clientContractFixture() {
    Path current = Paths.get("").toAbsolutePath().normalize();
    while (current != null) {
      final Path candidate =
          current.resolve("fixtures/privacy/bootle_lantern_issuance_client_v1.json");
      if (Files.isRegularFile(candidate)) {
        return candidate;
      }
      current = current.getParent();
    }
    throw new AssertionError("shared Bootle/Lantern issuance client fixture was not found");
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value) {
    if (!(value instanceof Map)) {
      throw new AssertionError("fixture value must be an object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> array(final Object value) {
    if (!(value instanceof List)) {
      throw new AssertionError("fixture value must be an array");
    }
    return (List<Object>) value;
  }

  private static byte[] hexBytes(final String encoded) {
    if ((encoded.length() & 1) != 0) {
      throw new AssertionError("fixture hex must have an even length");
    }
    final byte[] decoded = new byte[encoded.length() / 2];
    for (int index = 0; index < decoded.length; index++) {
      final int high = Character.digit(encoded.charAt(index * 2), 16);
      final int low = Character.digit(encoded.charAt(index * 2 + 1), 16);
      if (high < 0 || low < 0) {
        throw new AssertionError("fixture hex must be canonical hexadecimal");
      }
      decoded[index] = (byte) ((high << 4) | low);
    }
    return decoded;
  }

  private static String sha256Hex(final byte[] bytes) {
    final byte[] digest;
    try {
      digest = MessageDigest.getInstance("SHA-256").digest(bytes);
    } catch (final NoSuchAlgorithmException error) {
      throw new AssertionError("SHA-256 must be available", error);
    }
    final StringBuilder encoded = new StringBuilder(digest.length * 2);
    for (final byte value : digest) {
      encoded.append(String.format("%02x", value & 0xff));
    }
    return encoded.toString();
  }

  private static String repeat(final char character, final int count) {
    final char[] output = new char[count];
    java.util.Arrays.fill(output, character);
    return new String(output);
  }

  private static final class ScriptedExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private final Throwable failure;
    private final boolean throwSynchronously;
    private int calls;
    private TransportRequest lastRequest;

    private ScriptedExecutor(final TransportResponse response) {
      this(response, null, false);
    }

    private ScriptedExecutor(
        final TransportResponse response,
        final Throwable failure,
        final boolean throwSynchronously) {
      this.response = response;
      this.failure = failure;
      this.throwSynchronously = throwSynchronously;
    }

    private static ScriptedExecutor failure(final Throwable failure) {
      return new ScriptedExecutor(null, failure, false);
    }

    private static ScriptedExecutor synchronousFailure() {
      return new ScriptedExecutor(null, null, true);
    }

    private static ScriptedExecutor synchronousFailure(final Throwable failure) {
      return new ScriptedExecutor(null, failure, true);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      calls += 1;
      lastRequest = request;
      if (throwSynchronously) {
        if (failure instanceof RuntimeException) {
          throw (RuntimeException) failure;
        }
        throw new IllegalStateException("synchronous transport failure");
      }
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      if (failure != null) {
        future.completeExceptionally(failure);
      } else {
        future.complete(response);
      }
      return future;
    }
  }
}
