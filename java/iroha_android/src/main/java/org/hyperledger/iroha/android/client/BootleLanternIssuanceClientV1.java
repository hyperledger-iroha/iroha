package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.Varint;

/**
 * Exact, no-retry client for native Bootle/Lantern blind issuance.
 *
 * <p>This client deliberately has no observer or generic-header surface: bearer material cannot
 * enter SDK telemetry, and callers cannot weaken the canonical media type or identity-only
 * encoding.
 */
public final class BootleLanternIssuanceClientV1 {
  /** Canonical authorization route. */
  public static final String AUTHORIZE_PATH =
      "/v1/privacy/bootle-lantern/issuance/authorize";

  /** Canonical one-shot issuance route. */
  public static final String ISSUE_PATH = "/v1/privacy/bootle-lantern/issuance/issue";

  /** Sole request and successful-response media type. */
  public static final String NORITO_MEDIA_TYPE = "application/x-norito";

  /** Exact authorize response length. */
  public static final int AUTHORIZATION_RESPONSE_BYTES = 320;

  /** Exact {@code ILA1 || ILQ1} issue request length. */
  public static final int ISSUE_REQUEST_BYTES = 71_896;

  /** Exact issue response length. */
  public static final int ISSUE_RESPONSE_BYTES = 3_176;

  /** Maximum accepted structured issuance-error body length. */
  public static final int ERROR_RESPONSE_MAX_BYTES = 512;

  private static final String JSON_MEDIA_TYPE = "application/json";
  private static final byte[] AUTHORIZATION_MAGIC =
      "ILA1".getBytes(StandardCharsets.US_ASCII);
  private static final byte[] BLIND_REQUEST_MAGIC =
      "ILQ1".getBytes(StandardCharsets.US_ASCII);
  private static final byte[] RESPONSE_MAGIC = "ILR1".getBytes(StandardCharsets.US_ASCII);
  private static final String WWW_AUTHENTICATE_VALUE =
      "Bearer realm=\"iroha-bootle-lantern-issuance\"";
  private static final String ERROR_ENVELOPE_TYPE_NAME =
      "iroha_torii_shared::ErrorEnvelope";

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;

  private BootleLanternIssuanceClientV1(final Builder builder) {
    this.executor =
        builder.executor == null
            ? PlatformHttpTransportExecutor.createDefault()
            : builder.executor;
    this.baseUri = validateBaseUri(builder.baseUri);
    this.timeout = builder.timeout;
  }

  /** Creates a client builder. */
  public static Builder builder() {
    return new Builder();
  }

  /**
   * Mints one exact 320-byte {@code ILA1} authorization with an exact empty request body.
   *
   * <p>The operation is submitted exactly once. Transport and HTTP failures are never retried.
   */
  public CompletableFuture<byte[]> authorize(
      final BootleLanternIssuanceCredentialV1 credential) {
    return executeExact(
        "Bootle/Lantern issuance authorization",
        AUTHORIZE_PATH,
        credential,
        new byte[0],
        AUTHORIZATION_RESPONSE_BYTES,
        AUTHORIZATION_MAGIC);
  }

  /**
   * Submits exactly {@code ILA1 || ILQ1} and returns one exact 3,176-byte {@code ILR1} response.
   *
   * <p>The operation is submitted exactly once. Transport and HTTP failures are never retried.
   */
  public CompletableFuture<byte[]> issue(
      final BootleLanternIssuanceCredentialV1 credential,
      final byte[] canonicalRequest) {
    Objects.requireNonNull(canonicalRequest, "canonicalRequest");
    if (canonicalRequest.length != ISSUE_REQUEST_BYTES) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issue request must be exactly " + ISSUE_REQUEST_BYTES + " bytes");
    }
    if (!hasExactMagic(canonicalRequest, AUTHORIZATION_MAGIC, 0)
        || !hasExactMagic(
            canonicalRequest, BLIND_REQUEST_MAGIC, AUTHORIZATION_RESPONSE_BYTES)) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issue request must contain canonical ILA1 || ILQ1 magics");
    }
    return executeExact(
        "Bootle/Lantern blind issuance",
        ISSUE_PATH,
        credential,
        canonicalRequest,
        ISSUE_RESPONSE_BYTES,
        RESPONSE_MAGIC);
  }

  private CompletableFuture<byte[]> executeExact(
      final String operation,
      final String path,
      final BootleLanternIssuanceCredentialV1 credential,
      final byte[] body,
      final int expectedResponseBytes,
      final byte[] expectedResponseMagic) {
    final TransportRequest request =
        buildRequest(path, credential, body, expectedResponseBytes);
    final CompletableFuture<byte[]> result = new CompletableFuture<>();
    final CompletableFuture<TransportResponse> execution;
    try {
      execution = executor.execute(request);
    } catch (final RuntimeException ignored) {
      result.completeExceptionally(
          new BootleLanternIssuanceClientExceptionV1(operation + " request failed"));
      return result;
    }
    execution.whenComplete(
        (response, throwable) -> {
          if (throwable != null) {
            result.completeExceptionally(
                new BootleLanternIssuanceClientExceptionV1(operation + " request failed"));
            return;
          }
          try {
            result.complete(
                validateResponse(
                    response, operation, expectedResponseBytes, expectedResponseMagic));
          } catch (final RuntimeException error) {
            result.completeExceptionally(
                error instanceof BootleLanternIssuanceClientExceptionV1
                    ? error
                    : new BootleLanternIssuanceClientExceptionV1(
                        operation + " response is invalid"));
          }
        });
    return result;
  }

  private TransportRequest buildRequest(
      final String path,
      final BootleLanternIssuanceCredentialV1 credential,
      final byte[] body,
      final int expectedResponseBytes) {
    final URI target = resolvePath(path);
    final Map<String, String> headers = new LinkedHashMap<>();
    headers.put(
        "Authorization",
        Objects.requireNonNull(credential, "credential").authorizationHeaderValue());
    headers.put("Content-Type", NORITO_MEDIA_TYPE);
    headers.put("Accept", NORITO_MEDIA_TYPE);
    headers.put("Accept-Encoding", "identity");
    headers.put("Cache-Control", "no-store");
    headers.put("Pragma", "no-cache");
    TransportSecurity.requireHttpRequestAllowed(
        "BootleLanternIssuanceClientV1", baseUri, target, headers, body);
    final TransportRequest.Builder request =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .setBody(body)
            .setTimeout(timeout)
            .setMaximumResponseBytes((long) Math.max(expectedResponseBytes, ERROR_RESPONSE_MAX_BYTES));
    for (final Map.Entry<String, String> header : headers.entrySet()) {
      request.addHeader(header.getKey(), header.getValue());
    }
    return request.build();
  }

  private byte[] validateResponse(
      final TransportResponse response,
      final String operation,
      final int expectedResponseBytes,
      final byte[] expectedResponseMagic) {
    Objects.requireNonNull(response, "response");
    if (response.statusCode() != 200) {
      throw decodeErrorResponse(response, operation);
    }
    requireExactHeader(response.headers(), "Content-Type", NORITO_MEDIA_TYPE, operation);
    requireNoHeader(response.headers(), "Content-Encoding", operation);
    requireNoHeader(response.headers(), "WWW-Authenticate", operation);
    final byte[] body = response.body();
    if (body.length != expectedResponseBytes) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response must be exactly " + expectedResponseBytes + " bytes");
    }
    if (!hasExactMagic(body, expectedResponseMagic, 0)) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response wire magic is invalid");
    }
    requireExactOptionalContentLength(response.headers(), body.length, operation);
    return body.clone();
  }

  private BootleLanternIssuanceClientExceptionV1 decodeErrorResponse(
      final TransportResponse response, final String operation) {
    final ErrorContract contract = errorContract(response.statusCode());
    if (contract == null) {
      return new BootleLanternIssuanceClientExceptionV1(
          operation + " returned an unsupported error response");
    }
    try {
      final byte[] body = response.body();
      if (body.length == 0 || body.length > ERROR_RESPONSE_MAX_BYTES) {
        throw new IllegalArgumentException("error response body has an invalid length");
      }
      requireExactHeader(
          response.headers(), "Content-Type", contract.mediaType, operation);
      requireNoHeader(response.headers(), "Content-Encoding", operation);
      requireExactOptionalContentLength(response.headers(), body.length, operation);
      final List<String> retryAfter = headerValues(response.headers(), "Retry-After");
      if (Long.valueOf(1L).equals(contract.retryAfterSeconds)) {
        if (retryAfter.size() != 1 || !"1".equals(retryAfter.get(0))) {
          throw new IllegalArgumentException("invalid Retry-After");
        }
      } else if (!retryAfter.isEmpty()) {
        throw new IllegalArgumentException("unexpected Retry-After");
      }
      final List<String> wwwAuthenticate =
          headerValues(response.headers(), "WWW-Authenticate");
      if (response.statusCode() == 401) {
        if (wwwAuthenticate.size() != 1
            || !WWW_AUTHENTICATE_VALUE.equals(wwwAuthenticate.get(0))) {
          throw new IllegalArgumentException("invalid WWW-Authenticate");
        }
      } else if (!wwwAuthenticate.isEmpty()) {
        throw new IllegalArgumentException("unexpected WWW-Authenticate");
      }

      final ErrorEnvelope envelope;
      if (response.statusCode() == 406) {
        final byte[] expected =
            ("{\"code\":\""
                    + contract.code
                    + "\",\"message\":\""
                    + contract.code
                    + "\"}")
                .getBytes(StandardCharsets.UTF_8);
        if (!java.util.Arrays.equals(body, expected)) {
          throw new IllegalArgumentException("non-canonical JSON error envelope");
        }
        envelope = new ErrorEnvelope(contract.code, contract.code);
      } else {
        envelope = decodeNoritoErrorEnvelope(body);
      }
      if (!contract.code.equals(envelope.code)
          || !contract.code.equals(envelope.message)) {
        throw new IllegalArgumentException(
            "error envelope does not match its HTTP status");
      }
      return new BootleLanternIssuanceClientExceptionV1(
          operation + " returned HTTP " + response.statusCode() + ": " + contract.code,
          response.statusCode(),
          contract.code,
          contract.retryAfterSeconds);
    } catch (final RuntimeException ignored) {
      return new BootleLanternIssuanceClientExceptionV1(
          operation + " returned an invalid error response");
    }
  }

  private static ErrorContract errorContract(final int status) {
    switch (status) {
      case 400:
        return new ErrorContract("privacy_issuance_invalid_request", NORITO_MEDIA_TYPE, null);
      case 401:
        return new ErrorContract("privacy_issuance_unauthorized", NORITO_MEDIA_TYPE, null);
      case 406:
        return new ErrorContract("privacy_issuance_not_acceptable", JSON_MEDIA_TYPE, null);
      case 409:
        return new ErrorContract("privacy_issuance_state_conflict", NORITO_MEDIA_TYPE, null);
      case 413:
        return new ErrorContract("privacy_issuance_payload_too_large", NORITO_MEDIA_TYPE, null);
      case 415:
        return new ErrorContract(
            "privacy_issuance_unsupported_media_type", NORITO_MEDIA_TYPE, null);
      case 429:
        return new ErrorContract(
            "privacy_issuance_capacity_exhausted", NORITO_MEDIA_TYPE, 1L);
      case 503:
        return new ErrorContract("privacy_issuance_unavailable", NORITO_MEDIA_TYPE, null);
      default:
        return null;
    }
  }

  private static ErrorEnvelope decodeNoritoErrorEnvelope(final byte[] body) {
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(body, SchemaHash.hash16(ERROR_ENVELOPE_TYPE_NAME));
    final NoritoHeader header = decoded.header();
    if (header.compression() != NoritoHeader.COMPRESSION_NONE
        || header.flags() != NoritoHeader.COMPACT_LEN
        || body.length != NoritoHeader.HEADER_LENGTH + header.payloadLength()) {
      throw new IllegalArgumentException("non-canonical error-envelope framing");
    }
    header.validateChecksum(decoded.payload());
    int offset = 0;
    final DecodedString code = readStrictNoritoStringField(decoded.payload(), offset);
    offset = code.nextOffset;
    final DecodedString message = readStrictNoritoStringField(decoded.payload(), offset);
    offset = message.nextOffset;
    final DecodedField details = readStrictNoritoField(decoded.payload(), offset);
    offset = details.nextOffset;
    if (details.value.length != 1 || details.value[0] != 0) {
      throw new IllegalArgumentException("error details must be absent");
    }
    if (offset != decoded.payload().length) {
      throw new IllegalArgumentException("trailing error-envelope payload");
    }
    return new ErrorEnvelope(code.value, message.value);
  }

  private static DecodedField readStrictNoritoField(
      final byte[] payload, final int offset) {
    final Varint.DecodeResult length = Varint.decode(payload, offset);
    if (length.value() < 0 || length.value() > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("field length overflow");
    }
    final int start = length.nextOffset();
    final int fieldLength = (int) length.value();
    if (start < 0 || start > payload.length - fieldLength) {
      throw new IllegalArgumentException("truncated error-envelope field");
    }
    final int end = start + fieldLength;
    return new DecodedField(Arrays.copyOfRange(payload, start, end), end);
  }

  private static DecodedString readStrictNoritoStringField(
      final byte[] payload, final int offset) {
    final DecodedField field = readStrictNoritoField(payload, offset);
    final DecodedString decoded = readStrictNoritoString(field.value, 0);
    if (decoded.nextOffset != field.value.length) {
      throw new IllegalArgumentException(
          "trailing bytes in error-envelope string field");
    }
    return new DecodedString(decoded.value, field.nextOffset);
  }

  private static DecodedString readStrictNoritoString(
      final byte[] payload, final int offset) {
    final Varint.DecodeResult length = Varint.decode(payload, offset);
    if (length.value() < 0 || length.value() > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("string length overflow");
    }
    final int start = length.nextOffset();
    final int end = start + (int) length.value();
    if (end < start || end > payload.length) {
      throw new IllegalArgumentException("truncated string");
    }
    try {
      final String value =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(payload, start, end - start))
              .toString();
      return new DecodedString(value, end);
    } catch (final CharacterCodingException error) {
      throw new IllegalArgumentException("invalid UTF-8", error);
    }
  }

  private static final class ErrorContract {
    private final String code;
    private final String mediaType;
    private final Long retryAfterSeconds;

    private ErrorContract(
        final String code, final String mediaType, final Long retryAfterSeconds) {
      this.code = code;
      this.mediaType = mediaType;
      this.retryAfterSeconds = retryAfterSeconds;
    }
  }

  private static final class ErrorEnvelope {
    private final String code;
    private final String message;

    private ErrorEnvelope(final String code, final String message) {
      this.code = code;
      this.message = message;
    }
  }

  private static final class DecodedString {
    private final String value;
    private final int nextOffset;

    private DecodedString(final String value, final int nextOffset) {
      this.value = value;
      this.nextOffset = nextOffset;
    }
  }

  private static final class DecodedField {
    private final byte[] value;
    private final int nextOffset;

    private DecodedField(final byte[] value, final int nextOffset) {
      this.value = value;
      this.nextOffset = nextOffset;
    }
  }

  private URI resolvePath(final String path) {
    final String base = baseUri.toString();
    return URI.create((base.endsWith("/") ? base.substring(0, base.length() - 1) : base) + path);
  }

  private static URI validateBaseUri(final URI baseUri) {
    Objects.requireNonNull(baseUri, "baseUri");
    if (!baseUri.isAbsolute() || !"https".equalsIgnoreCase(baseUri.getScheme())) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance requires an absolute HTTPS base URI");
    }
    if (baseUri.getHost() == null || baseUri.getHost().isEmpty()) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance base URI must contain a host");
    }
    if (baseUri.getRawUserInfo() != null) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance base URI must not contain user info");
    }
    if (baseUri.getRawQuery() != null || baseUri.getRawFragment() != null) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance base URI must not contain a query or fragment");
    }
    final String path = baseUri.getRawPath();
    if (path != null && !path.isEmpty() && !"/".equals(path)) {
      throw new IllegalArgumentException(
          "Bootle/Lantern issuance base URI must be origin-only");
    }
    return baseUri;
  }

  private static List<String> headerValues(
      final Map<String, List<String>> headers, final String name) {
    final List<String> values = new ArrayList<>();
    for (final Map.Entry<String, List<String>> header : headers.entrySet()) {
      if (header.getKey().equalsIgnoreCase(name) && header.getValue() != null) {
        values.addAll(header.getValue());
      }
    }
    return values;
  }

  private static void requireExactHeader(
      final Map<String, List<String>> headers,
      final String name,
      final String expected,
      final String operation) {
    final List<String> values = headerValues(headers, name);
    if (values.size() != 1 || !expected.equals(values.get(0))) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response " + name + " must be exactly " + expected);
    }
  }

  private static void requireNoHeader(
      final Map<String, List<String>> headers,
      final String name,
      final String operation) {
    if (!headerValues(headers, name).isEmpty()) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response must not contain " + name);
    }
  }

  private static void requireExactOptionalContentLength(
      final Map<String, List<String>> headers,
      final int actualBytes,
      final String operation) {
    final List<String> values = headerValues(headers, "Content-Length");
    if (values.isEmpty()) {
      return;
    }
    if (values.size() != 1) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response has ambiguous Content-Length");
    }
    final String value = values.get(0);
    if (!isCanonicalDecimal(value)) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response Content-Length is not canonical and exact");
    }
    final long parsed;
    try {
      parsed = Long.parseLong(value);
    } catch (final NumberFormatException ignored) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response Content-Length is not canonical and exact");
    }
    if (parsed != actualBytes) {
      throw new BootleLanternIssuanceClientExceptionV1(
          operation + " response Content-Length is not canonical and exact");
    }
  }

  private static boolean isCanonicalDecimal(final String value) {
    if ("0".equals(value)) {
      return true;
    }
    if (value.isEmpty() || value.charAt(0) < '1' || value.charAt(0) > '9') {
      return false;
    }
    for (int index = 1; index < value.length(); index++) {
      if (value.charAt(index) < '0' || value.charAt(index) > '9') {
        return false;
      }
    }
    return true;
  }

  private static boolean hasExactMagic(
      final byte[] body, final byte[] magic, final int offset) {
    if (offset < 0 || body.length < offset + magic.length) {
      return false;
    }
    for (int index = 0; index < magic.length; index++) {
      if (body[offset + index] != magic[index]) {
        return false;
      }
    }
    return true;
  }

  /** Builder for an exact issuance client. */
  public static final class Builder {
    private HttpTransportExecutor executor;
    private URI baseUri = URI.create("https://localhost:8080");
    private Duration timeout = Duration.ofSeconds(15);

    private Builder() {}

    /** Injects the single-attempt transport executor. */
    public Builder executor(final HttpTransportExecutor executor) {
      this.executor = Objects.requireNonNull(executor, "executor");
      return this;
    }

    /** Sets an origin-only HTTPS Torii base URI. */
    public Builder baseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    /** Sets the per-request timeout, or {@code null} to use executor defaults. */
    public Builder timeout(final Duration timeout) {
      if (timeout != null && timeout.isNegative()) {
        throw new IllegalArgumentException("timeout must be non-negative");
      }
      this.timeout = timeout;
      return this;
    }

    /** Builds the issuance client. */
    public BootleLanternIssuanceClientV1 build() {
      return new BootleLanternIssuanceClientV1(this);
    }
  }

}
