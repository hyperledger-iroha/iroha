package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.validationfee.ValidationFeeHijiriQuoteRequestV1;
import org.hyperledger.iroha.android.validationfee.ValidationFeeHijiriQuoteV1;

/** Focused exact-transport contract tests for live Hijiri validation-fee quotes. */
public final class HttpClientTransportHijiriQuoteTests {
  private HttpClientTransportHijiriQuoteTests() {}

  public static void main(final String[] args) throws Exception {
    transportResponseExposesNetworkProvenance();
    exactUnredirectedProvenanceReachesNativeVerification();
    missingChangedAndRedirectedProvenanceFailClosed();
    parameterizedPublicCacheDirectivesFailClosed();
    quotedCommaDecoysAndMalformedSyntaxFailClosed();
    validQuotedExtensionReachesNativeVerification();
    System.out.println("[IrohaAndroid] Hijiri quote HTTP transport tests passed.");
  }

  private static void transportResponseExposesNetworkProvenance() {
    final URI finalUri =
        URI.create("https://torii.example/v1/validation-fee/hijiri/quote");
    final TransportResponse response =
        TransportResponse.builder()
            .setStatusCode(200)
            .setNetworkProvenance(finalUri, true)
            .build();
    assert finalUri.equals(response.finalUri()) : "final response URI provenance was lost";
    assert response.redirected() : "redirect provenance was lost";

    final TransportResponse withoutProvenance =
        new TransportResponse(200, new byte[0], "ok", Map.of(), null, false);
    assert withoutProvenance.finalUri() == null : "absent provenance must remain explicit";
    assert !withoutProvenance.redirected() : "absent provenance must not imply a redirect";
  }

  private static void exactUnredirectedProvenanceReachesNativeVerification() {
    final class VerificationObserved extends RuntimeException {}
    final HttpClientTransport.ValidationFeeHijiriQuoteCodec codec =
        new CodecThatMustReachVerification() {
          @Override
          public ValidationFeeHijiriQuoteV1 verify(
              final byte[] responseNorito, final byte[] requestNorito) {
            throw new VerificationObserved();
          }
        };
    boolean verifierReached = false;
    try {
      transport(new ExactResponseExecutor(null, false, true, exactHeaders()))
          .postValidationFeeHijiriQuote(request(), auth(), codec)
          .join();
    } catch (final CompletionException error) {
      verifierReached = error.getCause() instanceof VerificationObserved;
    }
    assert verifierReached : "exact unredirected response provenance must reach verification";
  }

  private static void missingChangedAndRedirectedProvenanceFailClosed() {
    final ExactResponseExecutor[] hostileExecutors = {
      new ExactResponseExecutor(null, false, false, exactHeaders()),
      new ExactResponseExecutor(
          URI.create("https://redirect.example/hijiri/quote"), false, true, exactHeaders()),
      new ExactResponseExecutor(null, true, true, exactHeaders())
    };
    for (final ExactResponseExecutor executor : hostileExecutors) {
      assertFailureContains(
          executor,
          "exact signed URL without redirects",
          "missing, changed, or redirected provenance must fail closed");
    }
  }

  private static void parameterizedPublicCacheDirectivesFailClosed() {
    for (final String publicDirective :
        List.of("public=max-age", "PUBLIC = \"Set-Cookie\"")) {
      assertFailureContains(
          new ExactResponseExecutor(
              null,
              false,
              true,
              Map.of(
                  "Content-Type", List.of("application/x-norito"),
                  "Content-Encoding", List.of("identity"),
                  "Cache-Control", List.of("private, no-store, " + publicDirective))),
          "private and no-store",
          "parameterized public cache directives must fail closed");
    }
  }

  private static void quotedCommaDecoysAndMalformedSyntaxFailClosed() {
    final String[] invalidCacheControls = {
      "private, x=\"a,no-store,b\"",
      "no-store, x=\"a,private,b\"",
      "private=\"Set-Cookie\", no-store",
      "private, no-store=extension",
      "private, no-store, x=\"unterminated",
      "private, no-store, x=\"dangling\\",
      "private, no-store, x=\"closed\"junk",
      "private, no-store, x=bad\\escape"
    };
    for (final String cacheControl : invalidCacheControls) {
      assertFailureContains(
          new ExactResponseExecutor(
              null,
              false,
              true,
              Map.of(
                  "Content-Type", List.of("application/x-norito"),
                  "Content-Encoding", List.of("identity"),
                  "Cache-Control", List.of(cacheControl))),
          "private and no-store",
          "quoted decoys and malformed Cache-Control syntax must fail closed");
    }
  }

  private static void validQuotedExtensionReachesNativeVerification() {
    final class VerificationObserved extends RuntimeException {}
    final HttpClientTransport.ValidationFeeHijiriQuoteCodec codec =
        new CodecThatMustReachVerification() {
          @Override
          public ValidationFeeHijiriQuoteV1 verify(
              final byte[] responseNorito, final byte[] requestNorito) {
            throw new VerificationObserved();
          }
        };
    boolean verifierReached = false;
    try {
      transport(
              new ExactResponseExecutor(
                  null,
                  false,
                  true,
                  Map.of(
                      "Content-Type", List.of("application/x-norito"),
                      "Content-Encoding", List.of("identity"),
                      "Cache-Control",
                      List.of("private, no-store, extension=\"a,public,b\""))))
          .postValidationFeeHijiriQuote(request(), auth(), codec)
          .join();
    } catch (final CompletionException error) {
      verifierReached = error.getCause() instanceof VerificationObserved;
    }
    assert verifierReached
        : "a valid quoted extension containing commas must preserve the private policy";
  }

  private static void assertFailureContains(
      final HttpTransportExecutor executor,
      final String expectedMessage,
      final String assertionMessage) {
    boolean failedClosed = false;
    try {
      transport(executor)
          .postValidationFeeHijiriQuote(request(), auth(), new CodecThatMustReachVerification())
          .join();
    } catch (final CompletionException error) {
      failedClosed =
          error.getCause() instanceof IllegalStateException
              && error.getCause().getMessage().contains(expectedMessage);
    }
    assert failedClosed : assertionMessage;
  }

  private static HttpClientTransport transport(final HttpTransportExecutor executor) {
    return HttpClientTransport.withExecutor(
        executor, CanonicalRequestSigningTestSupport.signedClientConfig("https://torii.example"));
  }

  private static ValidationFeeHijiriQuoteRequestV1 request() {
    return new ValidationFeeHijiriQuoteRequestV1(TestAccountIds.ed25519Authority(0x51), 2);
  }

  private static ToriiCanonicalRequestAuth auth() {
    return new ToriiCanonicalRequestAuth(
        TestAccountIds.ed25519Authority(0x52),
        message -> new byte[] {1},
        1_700_000_000_123L,
        "hijiri-quote-focused");
  }

  private static Map<String, List<String>> exactHeaders() {
    return Map.of(
        "Content-Type", List.of("application/x-norito"),
        "Content-Encoding", List.of("identity"),
        "Cache-Control", List.of("private, no-store"));
  }

  private static class CodecThatMustReachVerification
      implements HttpClientTransport.ValidationFeeHijiriQuoteCodec {
    @Override
    public byte[] encode(final ValidationFeeHijiriQuoteRequestV1 request) {
      return new byte[] {1};
    }

    @Override
    public ValidationFeeHijiriQuoteV1 verify(
        final byte[] responseNorito, final byte[] requestNorito) {
      throw new AssertionError("hostile transport metadata reached native verification");
    }
  }

  private static final class ExactResponseExecutor implements HttpTransportExecutor {
    private final URI finalUriOverride;
    private final boolean redirected;
    private final boolean includeProvenance;
    private final Map<String, List<String>> headers;

    private ExactResponseExecutor(
        final URI finalUriOverride,
        final boolean redirected,
        final boolean includeProvenance,
        final Map<String, List<String>> headers) {
      this.finalUriOverride = finalUriOverride;
      this.redirected = redirected;
      this.includeProvenance = includeProvenance;
      this.headers = headers;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if (!includeProvenance) {
        return CompletableFuture.completedFuture(
            new TransportResponse(200, new byte[] {9}, "ok", headers, null, false));
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(
              200,
              new byte[] {9},
              "ok",
              headers,
              finalUriOverride == null ? request.uri() : finalUriOverride,
              redirected));
    }
  }
}
