package org.hyperledger.iroha.android.client;

import static org.hyperledger.iroha.android.client.HttpClientTransportSubmissionContractTests.compatibleCapabilitiesResponse;
import static org.hyperledger.iroha.android.client.HttpClientTransportSubmissionContractTests.isCapabilitiesRequest;

import java.net.URI;
import java.net.URLEncoder;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.Signature;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference;
import org.hyperledger.iroha.android.alias.AccountAliasName;
import org.hyperledger.iroha.android.alias.AliasQuoteGuardV1;
import org.hyperledger.iroha.android.alias.AliasSetupModels;
import org.hyperledger.iroha.android.alias.AliasSetupPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasTransactionPlanV1;
import org.hyperledger.iroha.android.alias.EnsureAlias;
import org.hyperledger.iroha.android.alias.ResolvedAccountAliasV1;
import org.hyperledger.iroha.android.client.queue.FilePendingTransactionQueue;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.nexus.UaidBindingsQuery;
import org.hyperledger.iroha.android.nexus.UaidBindingsResponse;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery.UaidManifestStatusFilter;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestRecord;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestStatus;
import org.hyperledger.iroha.android.nexus.UaidPortfolioQuery;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.TransactionBuilder;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.sorafs.AnonymityPolicy;
import org.hyperledger.iroha.android.sorafs.GatewayFetchOptions;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.TransportPolicy;
import org.hyperledger.iroha.android.sorafs.WriteModeHint;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.NetworkContext;
import org.hyperledger.iroha.android.telemetry.NetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class HttpClientTransportTests {
  private static final String VPN_HELPER_TICKET_HEX = "5356504e48543100" + "00".repeat(656);
  private static final String VALID_ED25519_PUBLIC_KEY_HEX = TestEd25519Keys.publicKeyHex(0x22);
  private static final String ED25519_IDENTITY_KEY_HEX = "01" + "00".repeat(31);
  private static final String VERIFYING_KEY_CHAIN_ID = "test-chain";

  private static FeePaymentIntent feePayment(final Long gasLimit) {
    return FeePaymentIntent.authority(Collections.emptyList(), gasLimit);
  }

  private HttpClientTransportTests() {}

  private static String noncanonicalStandardBase64PadBitAlias(final String encoded) {
    if (!encoded.endsWith("==")) {
      throw new AssertionError("64-byte signatures encode with == padding");
    }
    final String alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    final char[] chars = encoded.toCharArray();
    final int index = chars.length - 3;
    final int value = alphabet.indexOf(chars[index]);
    if (value < 0) {
      throw new AssertionError("standard base64 alphabet");
    }
    chars[index] = alphabet.charAt(value ^ 0x01);
    return new String(chars);
  }

  private static String canonicalSignatureBase64Fixture() {
    final byte[] signature = new byte[64];
    for (int i = 0; i < signature.length; i++) {
      signature[i] = 0x01;
    }
    return Base64.getEncoder().encodeToString(signature);
  }

  public static void main(final String[] args) throws Exception {
    submitBuildsToriiRequest();
    submitPropagatesExecutorFailure();
    submitSkipsRetryWhenNetworkRetriesDisabled();
    submitRetriesOnServerError();
    retryPolicyRecognizesRetryableStatus();
    ledgerExecutedBlockWireIsExactBoundedAndFailClosed();
    privacyCapabilitiesAreTypedAndExact();
    submitQueuesTransactionsWhenOffline();
    submitQueuesTransactionsWithExportedKey();
    submitReplaysPendingTransactions();
    submitQueuesTransactionsSkipsExportWhenProviderDeclines();
    sorafsGatewayFetchUsesConfig();
    submitEmitsPendingQueueGauge();
    submitEmitsNetworkContextTelemetry();
    submitEmitsDeviceProfileTelemetry();
    submitEmitsRetryTelemetry();
    waitForTransactionStatusEmitsTelemetrySignals();
    pipelineStatusRedactionFailureUsesSignalId();
    uaidPortfolioRequestParsesResponse();
    uaidPortfolioRequestSupportsQuery();
    uaidPortfolioQueryRejectsPaddedSelectorsBeforeDispatch();
    uaidPathLiteralRejectsPaddedInputBeforeDispatch();
    uaidRequestsRespectBasePath();
    uaidBindingsRequestParsesResponse();
    uaidManifestsRequestSupportsQuery();
    identifierPoliciesRequestParsesResponse();
    identifierPolicyParserRejectsNonExactPolicyAndProofVerifierFields();
    ramLfeProgramPoliciesRequestParsesResponse();
    ramLfeProgramPolicyParserRejectsNonExactFields();
    identifierResolveRequestParsesResponse();
    identifierResolveRequestParsesProgrammedReceiptResponse();
    identifierResolveRequestAllowsNotFound();
    identifierHiddenFunctionRequestsRejectMalformedCiphertextEnvelopeFields();
    identifierClaimLookupAllowsNotFound();
    identifierClaimRecordParserRejectsNonExactClaimFields();
    identifierClaimReceiptUsesAccountPath();
    ramLfeExecuteRequestParsesResponse();
    ramLfeExecuteRequestAllowsNotFound();
    ramLfeReceiptVerifyUsesRawReceipt();
    ramLfeResponseParsersRejectNonExactFields();
    vpnProfileRequestParsesNativeLeaseFields();
    vpnProfileRequiresHttpsAndValidatesAvailabilityBoundTrustTuple();
    vpnSessionParserRejectsNonCanonicalHelperTicketHex();
    vpnResponseParsersRejectNonCanonicalIdsHashesAndUnknownFields();
    vpnResponseParsersRejectMissingRequiredFieldsAndSchemaBounds();
    vpnRoutesRejectWrongSuccessfulStatusCodes();
    vpnQuoteRequestSignsCanonicalBodyAndParsesOpenLeaseInstruction();
    ed25519KeyRoutesRejectSmallOrderIdentityPoint();
    feeQuoteRequestSignsExactUnsignedPayloadAndPreservesPayer();
    feeQuoteRejectsPayerRevisionAndGasSubstitution();
    feeSponsorProgramRequestSignsExactSelectorAndParsesLifecycle();
    feeSponsorProgramRejectsSubstitutedResponseId();
    vpnSessionAndReceiptRequestsUseNativeLeaseDtos();
    verifierKeyRegisterAndUpdateReturnUnsignedDrafts();
    verifierKeyRequestsRejectMalformedInputsBeforeRequest();
    verifierKeyDraftCanonicalInstructionUsesU8StatusDiscriminant();
    verifierKeyDraftParserRejectsNonExactOrTamperedResponses();
    verifierKeyDraftRejectsSemanticSubstitutionBeforeSigning();
    verifierKeyDraftRequiresLocalSigningContextBeforeRequest();
    callContractRequestParsesResponse();
    contractCallBoundaryConsumesSharedRustArgumentRecordFixture();
    callContractRejectsInvalidEntrypointOrGas();
    callContractResponseRequiresOperationReceipt();
    proposeMultisigRequestParsesResponse();
    proposeMultisigRejectsAdversarialRequestShapes();
    multisigResponseParserRejectsMalformedFields();
    callContractRejectsAmbiguousTarget();
    governanceContractRequestParsesResponse();
    resolveAccountAliasRequestParsesResponse();
    resolveRestrictedAccountAliasUsesCanonicalAuthentication();
    aliasSetupPlanningIsCanonicalSignedReadOnlyAndParsesTypedPlan();
    resolveAccountAliasRequestParsesResponseWithoutIndex();
    resolveAccountAliasAllowsNotFound();
    resolveAccountAliasRejectsNonIntegerIndex();
    accountAliasParserRejectsNonExactResponseFields();
    resolveAccountAliasFailsOnMalformedJson();
    identifierNormalizationCanonicalizesInputs();
    identifierBfvEnvelopeBuilderMatchesSharedSoracloudVectors();
    identifierBfvEnvelopeBuilderMatchesSharedSoracloudOperationInputVectors();
    sharedSoracloudBfvKeyBundleComponentVectorsAreComplete();
    sharedSoracloudBfvKeyBundleComponentVectorsRejectAdversarialDrift();
    identifierBfvEnvelopeBuilderProducesDeterministicCiphertext();
    identifierBfvEnvelopeBuilderRejectsAdversarialPublicParameters();
    identifierReceiptVerifierAcceptsEd25519Receipt();
    identifierReceiptVerifierRejectsAdversarialReceipts();
    identifierResolutionReceiptParserRejectsNonExactReceiptTags();
    identifierReceiptVerifierMatchesSharedReceiptVectors();
    invalidateAndCancelDelegatesToExecutor();
    System.out.println("[IrohaAndroid] HTTP client transport tests passed.");
  }

  private static void submitBuildsToriiRequest() throws Exception {
    final CapturingExecutor executor = new CapturingExecutor();
    final RecordingObserver observer = new RecordingObserver();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://127.0.0.1:8080"))
            .setRequestTimeout(Duration.ofSeconds(15))
            .putDefaultHeader("Authorization", "Bearer token")
            .addObserver(observer)
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final SignedTransaction transaction = transactionWithPayload((byte) 0x01);

    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Expected stub executor to return 202";
    assert "accepted".equals(response.message()) : "Executor message should propagate";
    final String expectedHash =
        SignedTransactionHasher.hashHex(transaction);
    final String actualHash = response.hashHex().orElse(null);
    assert actualHash != null : "ClientResponse must expose canonical hash";
    assert expectedHash.equals(actualHash)
        : "Canonical hash must match SignedTransactionHasher output";

    final TransportRequest request = executor.lastRequest;
    assert "POST".equals(request.method()) : "Submit must use POST";
    assert request.timeout() != null && request.timeout().equals(config.requestTimeout())
        : "Request timeout must match config";
    final List<String> contentTypes = request.headers().get("Content-Type");
    assert contentTypes != null && contentTypes.contains("application/x-norito")
        : "Content-Type header must be Norito";
    final List<String> acceptHeaders = request.headers().get("Accept");
    assert acceptHeaders != null
        && acceptHeaders.contains(WireFormatPreference.NORITO_PREFERRED.acceptHeader())
        : "Accept header must include Norito";
    assert request.uri().toString().equals("https://127.0.0.1:8080/v1/pipeline/transactions")
        : "Submit endpoint must target Torii pipeline route";
    final List<String> authHeaders = request.headers().get("Authorization");
    assert authHeaders != null && authHeaders.contains("Bearer token")
        : "Custom headers from config must be applied";

    final byte[] body = request.body();
    final byte[] expected = SignedTransactionEncoder.encodeVersioned(transaction);
    assert java.util.Arrays.equals(body, expected)
        : "Body must include Norito-encoded signed transaction";

    assert observer.requestCount.get() == 2
        : "Observer must see capability GET and submit POST";
    assert observer.responseCount.get() == 2
        : "Observer must see capability and submit responses";
    assert observer.failureCount.get() == 0 : "Observer must not see failure";
  }

  private static void submitPropagatesExecutorFailure() {
    final RuntimeException transportError = new RuntimeException("network down");
    final FailingExecutor executor = new FailingExecutor(transportError);
    final RecordingObserver observer = new RecordingObserver();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://localhost:8080"))
                .addObserver(observer)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x02);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
      Throwable cause = ex;
      while (cause != transportError && cause.getCause() != null) {
        cause = cause.getCause();
      }
      assert cause == transportError
          : "Capability probe failure must retain the original transport error";
    }
    assert threw : "Expected submit to rethrow executor error";
    assert observer.requestCount.get() == 1 : "Observer must see request";
    assert observer.responseCount.get() == 0 : "No response should be recorded";
    assert observer.failureCount.get() == 1 : "Observer must see failure";
  }

  private static void submitSkipsRetryWhenNetworkRetriesDisabled() {
    final CountingFailingExecutor executor =
        new CountingFailingExecutor(new RuntimeException("network down"));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://localhost:8080"))
            .setRetryPolicy(
                RetryPolicy.builder()
                    .setMaxAttempts(3)
                    .setBaseDelay(Duration.ZERO)
                    .setRetryOnNetworkError(false)
                    .build())
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final SignedTransaction transaction = transactionWithPayload((byte) 0x03);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when network retries are disabled";
    assert executor.callCount == 1 : "Transport must not retry on network failures when disabled";
  }

  private static void submitRetriesOnServerError() {
    final SequencedExecutor executor = new SequencedExecutor();
    final RecordingObserver observer = new RecordingObserver();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://localhost:8080"))
            .addObserver(observer)
            .setRetryPolicy(
                RetryPolicy.builder()
                    .setMaxAttempts(2)
                    .setBaseDelay(Duration.ZERO)
                    .build())
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final SignedTransaction transaction = transactionWithPayload((byte) 0x04);

    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Final attempt should succeed";
    assert executor.callCount == 2 : "Transport should retry once";
    assert observer.requestCount.get() == 4
        : "Observer should see capability and submit requests for both attempts";
    assert observer.responseCount.get() == 3
        : "Observer should see both capabilities and the successful submit response";
    assert observer.failureCount.get() == 0 : "Retries on responses should not trigger failure callback";
    final String expectedHash = SignedTransactionHasher.hashHex(transaction);
    assert expectedHash.equals(response.hashHex().orElse(null))
        : "Canonical hash must match SignedTransactionHasher output after retries";
  }

  private static void retryPolicyRecognizesRetryableStatus() {
    final RetryPolicy defaultPolicy = RetryPolicy.builder().setMaxAttempts(1).build();
    assert defaultPolicy.isRetryableStatus(503) : "Server errors should be retryable by default";
    assert defaultPolicy.isRetryableStatus(429) : "Too many requests should be retryable by default";
    assert !defaultPolicy.isRetryableStatus(400) : "Client errors should not be retryable";

    final RetryPolicy custom =
        RetryPolicy.builder()
            .setMaxAttempts(1)
            .setRetryOnServerError(false)
            .setRetryOnTooManyRequests(false)
            .addRetryStatusCode(418)
            .build();
    assert !custom.isRetryableStatus(503) : "Server errors must be disabled by policy";
    assert !custom.isRetryableStatus(429) : "429 must be disabled by policy";
    assert custom.isRetryableStatus(418) : "Custom retry codes must be honored";
  }

  private static void ledgerExecutedBlockWireIsExactBoundedAndFailClosed() {
    final byte[] canonical = new byte[] {0x4e, 0x52, 0x54, 0x30};
    final OneResponseExecutor success =
        new OneResponseExecutor(
            new TransportResponse(
                200,
                canonical,
                "ok",
                Map.of(
                    "Content-Type", List.of("application/x-norito"),
                    "Content-Length", List.of(Integer.toString(canonical.length)))));
    final HttpClientTransport client =
        HttpClientTransport.withExecutor(
            success,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build());
    final byte[] received =
        client
            .getLedgerExecutedBlockWire(new BigInteger("18446744073709551615"))
            .join();
    assert Arrays.equals(canonical, received);
    assert success.requestCount == 1;
    assert "GET".equals(success.lastRequest.method());
    assert "/v1/ledger/block/18446744073709551615".equals(success.lastRequest.uri().getRawPath());
    assert List.of("application/x-norito").equals(success.lastRequest.headers().get("Accept"));
    assert Long.valueOf(32L * 1024L * 1024L).equals(success.lastRequest.maximumResponseBytes());

    for (final BigInteger height :
        List.of(BigInteger.ZERO, BigInteger.valueOf(-1L), BigInteger.ONE.shiftLeft(64))) {
      boolean rejected = false;
      try {
        client.getLedgerExecutedBlockWire(height);
      } catch (final IllegalArgumentException expected) {
        rejected = true;
      }
      assert rejected : "invalid ledger height must fail before dispatch";
    }
    assert success.requestCount == 1 : "invalid heights must not dispatch";

    final List<TransportResponse> hostile =
        List.of(
            new TransportResponse(
                201, canonical, "", Map.of("Content-Type", List.of("application/x-norito"))),
            new TransportResponse(
                200,
                canonical,
                "",
                Map.of("Content-Type", List.of("application/x-norito; charset=binary"))),
            new TransportResponse(
                200,
                canonical,
                "",
                Map.of("Content-Type", List.of("application/x-norito", "application/x-norito"))),
            new TransportResponse(
                200,
                canonical,
                "",
                Map.of(
                    "Content-Type", List.of("application/x-norito"),
                    "Content-Length", List.of("04"))),
            new TransportResponse(
                200,
                canonical,
                "",
                Map.of(
                    "Content-Type", List.of("application/x-norito"),
                    "Content-Length", List.of("5"))),
            new TransportResponse(
                200,
                new byte[0],
                "",
                Map.of("Content-Type", List.of("application/x-norito"))));
    for (final TransportResponse response : hostile) {
      boolean rejected = false;
      try {
        HttpClientTransport.withExecutor(
                new OneResponseExecutor(response),
                ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example"))
                    .build())
            .getLedgerExecutedBlockWire(1L)
            .join();
      } catch (final CompletionException expected) {
        rejected = true;
      }
      assert rejected : "hostile executed-block response must fail closed";
    }

    final byte[] oversized = new byte[32 * 1024 * 1024 + 1];
    boolean oversizedRejected = false;
    try {
      HttpClientTransport.withExecutor(
              new OneResponseExecutor(
                  new TransportResponse(
                      200,
                      oversized,
                      "",
                      Map.of("Content-Type", List.of("application/x-norito")))),
              ClientConfig.builder()
                  .setBaseUri(URI.create("https://torii.example"))
                  .build())
          .getLedgerExecutedBlockWire(1L)
          .join();
    } catch (final CompletionException expected) {
      oversizedRejected = true;
    }
    assert oversizedRejected : "oversized executed-block wire must fail closed";
  }

  private static void privacyCapabilitiesAreTypedAndExact() {
    final byte[] body = privacyCapabilitySnapshotJson().getBytes(StandardCharsets.UTF_8);
    final OneResponseExecutor executor =
        new OneResponseExecutor(
            new TransportResponse(
                200,
                body,
                "ok",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of(Integer.toString(body.length)))));
    final HttpClientTransport client =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build());
    final org.hyperledger.iroha.sdk.privacy.PrivacyCapabilitySnapshotV1 snapshot =
        client.getPrivacyCapabilities().join();
    assert snapshot.committedHeight.equals(BigInteger.valueOf(42L));
    assert snapshot.protocols.size() == 12;
    assert "GET".equals(executor.lastRequest.method());
    assert "/v1/privacy/capabilities".equals(executor.lastRequest.uri().getRawPath());
    final List<String> requestAccepts = new ArrayList<>();
    for (final Map.Entry<String, List<String>> entry :
        executor.lastRequest.headers().entrySet()) {
      if (entry.getKey().equalsIgnoreCase("Accept")) {
        requestAccepts.addAll(entry.getValue());
      }
    }
    assert List.of("application/json").equals(requestAccepts)
        : "privacy capability request must contain exactly one canonical Accept value";
    assert Long.valueOf(256L * 1024L).equals(executor.lastRequest.maximumResponseBytes());

    final org.hyperledger.iroha.sdk.privacy.PrivacyCapabilitySnapshotV1 withoutContentLength =
        HttpClientTransport.withExecutor(
                new OneResponseExecutor(
                    new TransportResponse(
                        200,
                        body,
                        "ok",
                        Map.of("Content-Type", List.of("application/json")))),
                ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example"))
                    .build())
            .getPrivacyCapabilities()
            .join();
    assert withoutContentLength.committedHeight.equals(BigInteger.valueOf(42L))
        : "Content-Length is optional when the remaining response is exact";

    for (final String defaultAcceptName : List.of("Accept", "aCcEpT")) {
      final OneResponseExecutor blockedExecutor =
          new OneResponseExecutor(
              new TransportResponse(
                  200,
                  body,
                  "ok",
                  Map.of("Content-Type", List.of("application/json"))));
      boolean overrideRejected = false;
      try {
        HttpClientTransport.withExecutor(
                blockedExecutor,
                ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example"))
                    .putDefaultHeader(defaultAcceptName, "application/json")
                    .build())
            .getPrivacyCapabilities();
      } catch (final IllegalArgumentException expected) {
        overrideRejected = true;
      }
      assert overrideRejected : "default Accept must be rejected case-insensitively";
      assert blockedExecutor.requestCount == 0 : "invalid default Accept must not dispatch";
    }

    final String retired =
        privacyCapabilitySnapshotJson()
            .replace("zk-ace-pq-authorization-v0", "sis-with-hints");
    boolean retiredRejected = false;
    try {
      HttpClientTransport.withExecutor(
              new OneResponseExecutor(
                  new TransportResponse(
                      200,
                      retired.getBytes(StandardCharsets.UTF_8),
                      "",
                      Map.of("Content-Type", List.of("application/json")))),
              ClientConfig.builder()
                  .setBaseUri(URI.create("https://torii.example"))
                  .build())
          .getPrivacyCapabilities()
          .join();
    } catch (final CompletionException expected) {
      retiredRejected = true;
    }
    assert retiredRejected : "retired privacy labels must fail closed";

    final String bodyLength = Integer.toString(body.length);
    final Map<String, List<String>> caseFoldedDuplicateContentType = new LinkedHashMap<>();
    caseFoldedDuplicateContentType.put("Content-Type", List.of("application/json"));
    caseFoldedDuplicateContentType.put("content-type", List.of("application/json"));
    final Map<String, List<String>> caseFoldedDuplicateContentLength = new LinkedHashMap<>();
    caseFoldedDuplicateContentLength.put("Content-Type", List.of("application/json"));
    caseFoldedDuplicateContentLength.put("Content-Length", List.of(bodyLength));
    caseFoldedDuplicateContentLength.put("content-length", List.of(bodyLength));
    final List<TransportResponse> hostileResponses =
        List.of(
            new TransportResponse(
                201, body, "", Map.of("Content-Type", List.of("application/json"))),
            new TransportResponse(200, body, "", Map.of()),
            new TransportResponse(
                200,
                body,
                "",
                Map.of("Content-Type", List.of("application/json; charset=utf-8"))),
            new TransportResponse(
                200,
                body,
                "",
                Map.of("Content-Type", List.of("Application/Json"))),
            new TransportResponse(
                200,
                body,
                "",
                Map.of(
                    "Content-Type", List.of("application/json", "application/json"))),
            new TransportResponse(200, body, "", caseFoldedDuplicateContentType),
            new TransportResponse(
                200,
                body,
                "",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of(bodyLength, bodyLength))),
            new TransportResponse(200, body, "", caseFoldedDuplicateContentLength),
            new TransportResponse(
                200,
                body,
                "",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of("0"))),
            new TransportResponse(
                200,
                body,
                "",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of("0" + bodyLength))),
            new TransportResponse(
                200,
                body,
                "",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of("+" + bodyLength))),
            new TransportResponse(
                200,
                body,
                "",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of(bodyLength + " "))),
            new TransportResponse(
                200,
                body,
                "",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of("9".repeat(4096)))),
            new TransportResponse(
                200,
                new byte[0],
                "",
                Map.of(
                    "Content-Type", List.of("application/json"),
                    "Content-Length", List.of("0"))));
    for (final TransportResponse hostileResponse : hostileResponses) {
      assertPrivacyCapabilitiesResponseRejected(hostileResponse);
    }

    assertPrivacyCapabilitiesResponseRejected(
        new TransportResponse(
            200,
            new byte[256 * 1024 + 1],
            "",
            Map.of("Content-Type", List.of("application/json"))));
  }

  private static void assertPrivacyCapabilitiesResponseRejected(
      final TransportResponse response) {
    boolean rejected = false;
    try {
      HttpClientTransport.withExecutor(
              new OneResponseExecutor(response),
              ClientConfig.builder()
                  .setBaseUri(URI.create("https://torii.example"))
                  .build())
          .getPrivacyCapabilities()
          .join();
    } catch (final CompletionException expected) {
      rejected = true;
    }
    assert rejected : "hostile privacy capability response must fail closed";
  }

  private static String privacyCapabilitySnapshotJson() {
    final StringBuilder rows = new StringBuilder();
    for (final org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1 protocol :
        org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1.values()) {
      if (rows.length() != 0) {
        rows.append(',');
      }
      rows.append("{\"protocol_id\":{\"protocol\":\"")
          .append(protocol.getCanonicalLabel())
          .append(
              "\",\"value\":null},\"compiled_profile\":{\"status\":\"unavailable\","
                  + "\"value\":{\"reason\":\"engine-unavailable\",\"detail\":null}},"
                  + "\"activation\":null}");
    }
    return "{\"version\":1,\"committed_height\":42,\"consensus_policy\":{"
        + "\"current_limits\":{"
        + "\"max_actions_per_transaction\":1,\"max_actions_per_block\":2,"
        + "\"max_proof_bytes_per_action\":9437184,\"max_action_bytes\":9437184,"
        + "\"max_privacy_bytes_per_transaction\":9437184,"
        + "\"max_privacy_bytes_per_block\":18874368,"
        + "\"max_statement_and_encrypted_output_bytes_per_transaction\":262144,"
        + "\"max_nullifiers_per_action\":8,\"max_commitments_per_action\":8,"
        + "\"retained_root_count\":2048},\"pending_tightening\":null},"
        + "\"protocols\":["
        + rows
        + "]}";
  }

  private static void submitQueuesTransactionsWhenOffline() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-offline-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));
    final SignedTransaction transaction = transactionWithPayload((byte) 0x11);

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://localhost:8080"))
                .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
                .setPendingQueue(queue)
                .build());

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";
    assert queue.size() == 1 : "Transaction must be queued";
    final List<SignedTransaction> persisted = queue.drain();
    assert persisted.size() == 1 : "Drain must return queued transaction";
    assert payloadEquals(transaction, persisted.get(0)) : "Queued payload must match original";
  }

  private static void submitReplaysPendingTransactions() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-replay-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));
    final SignedTransaction first = transactionWithPayload((byte) 0x21);
    final SignedTransaction second = transactionWithPayload((byte) 0x22);
    queue.enqueue(first);
    queue.enqueue(second);

    final RecordingExecutor executor = new RecordingExecutor();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://localhost:8080"))
                .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
                .setPendingQueue(queue)
                .build());

    final SignedTransaction live = transactionWithPayload((byte) 0x23);
    transport.submitTransaction(live).join();
    assert queue.size() == 0 : "Pending queue must be empty after replay";
    final List<byte[]> payloadOrder = executor.payloads;
    assert payloadOrder.size() == 3 : "Executor should receive queued transactions plus live submission";
    assert java.util.Arrays.equals(
            payloadOrder.get(0), SignedTransactionEncoder.encodeVersioned(first))
        : "First queued transaction must be sent first";
    assert java.util.Arrays.equals(
            payloadOrder.get(1), SignedTransactionEncoder.encodeVersioned(second))
        : "Second queued transaction must be sent second";
    assert java.util.Arrays.equals(
            payloadOrder.get(2), SignedTransactionEncoder.encodeVersioned(live))
        : "Live transaction must be sent last";
  }

  private static void submitEmitsPendingQueueGauge() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-telemetry-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("01020304")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://localhost:8080"))
                .setRetryPolicy(RetryPolicy.none())
                .setPendingQueue(queue)
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x33);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.pending_queue.depth");
    assert event != null : "Telemetry sink should capture queue depth emission";
    assert "android.pending_queue.depth".equals(event.signalId())
        : "Gauge must use the pending queue signal id";
    assert "file".equals(event.fields().get("queue"))
        : "Queue label should describe the implementation";
    assert Long.valueOf(1L).equals(event.fields().get("depth"))
        : "Queue depth gauge must report pending entry count";
  }

  private static void submitEmitsNetworkContextTelemetry() throws Exception {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0a0b0c0d")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();
    final NetworkContextProvider provider =
        () -> Optional.of(NetworkContext.of("wifi", false));

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new CapturingExecutor(),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://localhost:8080"))
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .setNetworkContextProvider(provider)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x44);
    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Executor should accept submission";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.telemetry.network_context");
    assert event != null : "Telemetry sink should capture network context emission";
    assert "android.telemetry.network_context".equals(event.signalId())
        : "Signal id must match android.telemetry.network_context";
    assert "wifi".equals(event.fields().get("network_type"))
        : "Network type should reflect provider snapshot";
    assert Boolean.FALSE.equals(event.fields().get("roaming"))
        : "Roaming flag should be forwarded as-is";
  }

  private static void submitEmitsDeviceProfileTelemetry() throws Exception {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0d0c0b0a")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();
    final DeviceProfileProvider provider = () -> Optional.of(DeviceProfile.of("enterprise"));

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new CapturingExecutor(),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://localhost:8080"))
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .setDeviceProfileProvider(provider)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x66);
    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Executor should accept submission";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.telemetry.device_profile");
    assert event != null : "Telemetry sink should capture device profile emission";
    assert "enterprise".equals(event.fields().get("profile_bucket"))
        : "Profile bucket should match provider snapshot";
  }

  private static void submitEmitsRetryTelemetry() throws Exception {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0f0e0d0c")
                    .setSaltVersion("2026Q1")
                    .build())
            .build();
    final RetryPolicy retryPolicy =
        RetryPolicy.builder().setMaxAttempts(2).setBaseDelay(Duration.ofMillis(250)).build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new SequencedExecutor(),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://retry.test:8080"))
                .setRetryPolicy(retryPolicy)
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    final SignedTransaction transaction = transactionWithPayload((byte) 0x55);
    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert response.statusCode() == 202 : "Submission should succeed after retry";

    final RecordingTelemetrySink.GaugeEvent event =
        telemetrySink.lastEvent("android.torii.http.retry");
    assert event != null : "Retry telemetry signal must be emitted";
    final Map<String, Object> fields = event.fields();
    final String expectedHash =
        telemetryOptions
            .redaction()
            .hashAuthority("retry.test:8080")
            .orElseThrow(() -> new IllegalStateException("Hash must be present"));
    assert expectedHash.equals(fields.get("authority_hash"))
        : "Retry signal should carry hashed authority";
    assert "/v1/pipeline/transactions".equals(fields.get("route"))
        : "Route must describe the Torii submit endpoint";
    assert Integer.valueOf(1).equals(fields.get("retry_count"))
        : "First retry should report attempt #1";
    assert "503".equals(fields.get("error_code"))
        : "Error code must reflect the HTTP status";
    assert Long.valueOf(250L).equals(fields.get("backoff_ms"))
        : "Backoff must follow the retry policy";
  }

  private static void waitForTransactionStatusEmitsTelemetrySignals() {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0a0b0c0d")
                    .setSaltVersion("2026Q2")
                    .build())
            .build();
    final String hashHex =
        "deadbeefcafefeeddeadbeefcafefeeddeadbeefcafefeeddeadbeefcafefeed";
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new ScriptedExecutor(
                new TransportResponse(202, statusPayload(hashHex, "Queued"), "", Map.of()),
                new TransportResponse(200, statusPayload(hashHex, "Applied"), "", Map.of())),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://status-telemetry.test:8080"))
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    final Map<String, Object> payload =
        transport
            .waitForTransactionStatus(
                hashHex, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();
    assert "Applied".equals(PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        : "Expected authoritative applied status";

    final List<Map<String, Object>> signals =
        telemetrySink.eventsBySignal("android.torii.pipeline.status");
    assert signals.size() == 2 : "Expected pending and success telemetry events";
    final Map<String, Object> pending = signals.get(0);
    final Map<String, Object> success = signals.get(1);
    final String expectedAuthorityHash =
        telemetryOptions
            .redaction()
            .hashAuthority("status-telemetry.test:8080")
            .orElseThrow(() -> new IllegalStateException("authority hash missing"));

    assert expectedAuthorityHash.equals(pending.get("authority_hash"))
        : "Pending signal must carry hashed authority";
    assert hashHex.equals(pending.get("tx_hash"))
        : "Pending signal must carry transaction hash";
    assert "Queued".equals(pending.get("status_kind"))
        : "Pending signal must record the canonical queued status kind";
    assert "pending".equals(pending.get("outcome"))
        : "Pending signal must use pending outcome";
    assert ((Number) pending.get("attempts")).intValue() == 1
        : "Pending signal must record first attempt";

    assert expectedAuthorityHash.equals(success.get("authority_hash"))
        : "Success signal must carry hashed authority";
    assert hashHex.equals(success.get("tx_hash"))
        : "Success signal must carry transaction hash";
    assert "Applied".equals(success.get("status_kind"))
        : "Success signal must record applied status";
    assert "success".equals(success.get("outcome"))
        : "Success signal must use success outcome";
    assert ((Number) success.get("attempts")).intValue() == 2
        : "Success signal must reflect attempt count";
  }

  private static void pipelineStatusRedactionFailureUsesSignalId() {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final TelemetryOptions telemetryOptions =
        TelemetryOptions.builder()
            .setTelemetryRedaction(
                TelemetryOptions.Redaction.builder()
                    .setSaltHex("0e0f1011")
                    .setSaltVersion("2026Q3")
                    .build())
            .build();
    final String hashHex =
        "beadfeedbeadfeedbeadfeedbeadfeedbeadfeedbeadfeedbeadfeedbeadfeed";
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new ScriptedExecutor(
                new TransportResponse(200, statusPayload(hashHex, "Applied"), "", Map.of())),
            ClientConfig.builder()
                .setBaseUri(URI.create("http:/")) // No authority -> redaction failure path.
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    transport
        .waitForTransactionStatus(
            hashHex, PipelineStatusOptions.builder().intervalMillis(0L).build())
        .join();

    final List<Map<String, Object>> failures =
        telemetrySink.eventsBySignal("android.telemetry.redaction.failure");
    boolean found = false;
    for (final Map<String, Object> fields : failures) {
      if ("android.torii.pipeline.status".equals(fields.get("signal_id"))) {
        assert "blank_authority".equals(fields.get("reason"))
            : "Redaction failure must report the blank authority reason";
        found = true;
        break;
      }
    }
    assert found : "Pipeline status redaction failures must reference the pipeline status signal";
  }

  private static void submitQueuesTransactionsWithExportedKey() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(provider));
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT), keyManager);
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
            .setChainId("00000000")
            .setAuthority(TestAccountIds.ed25519Authority(0x27))
            .setInstructions(Collections.emptyList())
            .build();
    final SignedTransaction transaction =
        builder.encodeAndSign(payload, "queued-alias", KeySecurityPreference.SOFTWARE_ONLY);

    final char[] passphrase = "queue-passphrase".toCharArray();
    final Path tempDir = Files.createTempDirectory("iroha-queue-export-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));

    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://localhost:8080"))
            .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
            .setPendingQueue(queue)
            .setExportOptions(
                ClientConfig.ExportOptions.builder()
                    .setKeyManager(keyManager)
                    .setPassphrase(passphrase)
                    .build())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")), config);

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";

    final List<SignedTransaction> drained = queue.drain();
    assert drained.size() == 1 : "Queued transaction expected";
    final SignedTransaction queued = drained.get(0);
    assert queued.keyAlias().orElse("?").equals("queued-alias") : "Alias must be preserved";
    assert queued.exportedKeyBundle().isPresent() : "Exported key bundle must be attached";
    java.util.Arrays.fill(passphrase, '\0');
  }

  private static void submitQueuesTransactionsSkipsExportWhenProviderDeclines() throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(provider));
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT), keyManager);
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
            .setChainId("00000000")
            .setAuthority(TestAccountIds.ed25519Authority(0x28))
            .setInstructions(Collections.emptyList())
            .build();
    final SignedTransaction transaction =
        builder.encodeAndSign(payload, "skip-alias", KeySecurityPreference.SOFTWARE_ONLY);

    final Path tempDir = Files.createTempDirectory("iroha-queue-skip-export-");
    final FilePendingTransactionQueue queue =
        new FilePendingTransactionQueue(tempDir.resolve("pending.queue"));

    final char[] passphrase = "skip-passphrase".toCharArray();
    final ClientConfig.ExportOptions exportOptions =
        ClientConfig.ExportOptions.builder()
            .setKeyManager(keyManager)
            .setPassphraseProvider(
                alias -> "skip-alias".equals(alias) ? null : passphrase.clone())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new FailingExecutor(new RuntimeException("offline")),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://localhost:8080"))
                .setRetryPolicy(RetryPolicy.builder().setMaxAttempts(1).build())
                .setPendingQueue(queue)
                .setExportOptions(exportOptions)
                .build());

    boolean threw = false;
    try {
      transport.submitTransaction(transaction).join();
    } catch (final RuntimeException ex) {
      threw = true;
    }
    assert threw : "Submission should fail when executor errors";

    final List<SignedTransaction> drained = queue.drain();
    assert drained.size() == 1 : "Queued transaction expected";
    final SignedTransaction queued = drained.get(0);
    assert queued.exportedKeyBundle().isEmpty() : "Export bundle should be omitted when provider declines";
    java.util.Arrays.fill(passphrase, '\0');
  }

  private static void sorafsGatewayFetchUsesConfig() throws Exception {
    final CapturingExecutor executor = new CapturingExecutor();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example:8080"))
            .setSorafsGatewayUri(URI.create("https://gateway.example/"))
            .setRequestTimeout(Duration.ofSeconds(12))
            .putDefaultHeader("X-Trace", "android-client")
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("primary")
            .setProviderIdHex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
            .setGatewayPublicKeyHex(VALID_ED25519_PUBLIC_KEY_HEX)
            .setBaseUrl("https://storage.example/")
            .setStreamTokenBase64("dG9rZW4=")
            .build();

    final GatewayFetchRequest request =
        GatewayFetchRequest.builder()
            .setManifestIdHex("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef")
            .setChunkerHandle("sorafs.sf1@1.0.0")
            .setOptions(
                GatewayFetchOptions.builder()
                    .setTelemetryRegion("us-east-1")
                    .setTransportPolicy(TransportPolicy.DIRECT_ONLY)
                    .setAnonymityPolicy(AnonymityPolicy.ANON_STRICT_PQ)
                    .setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)
                    .build())
            .addProvider(provider)
            .build();

    final ClientResponse response = transport.sorafsGatewayFetch(request).join();
    assert response.statusCode() == 202 : "Gateway fetch should surface executor status";

    final TransportRequest requestSent = executor.lastRequest;
    assert requestSent != null : "Executor should capture request";
    assert requestSent.uri().toString().equals(
            "https://gateway.example/v1/sorafs/gateway/fetch")
        : "Gateway URI must use the canonical origin and fixed fetch endpoint";
    assert requestSent.headers().getOrDefault("Content-Type", List.of()).contains("application/json")
        : "Gateway fetch must set JSON content type";
    assert requestSent.headers().getOrDefault("X-Trace", List.of()).contains("android-client")
        : "Custom headers should propagate";

    final String body = readBody(requestSent);
    assert body.equals(request.toJsonString()) : "Request body must serialise fetch payload";
  }

  private static void uaidPortfolioRequestParsesResponse() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String assetDefinitionId = TestAssetDefinitionIds.TERTIARY;
    final String json =
        ("{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"totals\":{\"accounts\":2,\"positions\":3},"
            + "\"dataspaces\":[{"
            + "\"dataspace_id\":42,"
            + "\"dataspace_alias\":\"sandbox\","
            + "\"accounts\":[{"
            + "\"account_id\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\","
            + "\"label\":\"Primary\","
            + "\"assets\":[{"
            + "\"asset_id\":\""
            + assetDefinitionId
            + "#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
            + "\",\"asset_definition_id\":\""
            + assetDefinitionId
            + "\",\"quantity\":\"42\""
            + "}]"
            + "}]"
            + "}]"
            + "}");
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .putDefaultHeader("X-Test", "uaid")
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final UaidPortfolioResponse response =
        transport.getUaidPortfolio("UAID:" + hex.toUpperCase()).join();
    assert response.uaid().equals("uaid:" + hex)
        : "UAID literal must be normalised";
    assert response.totals().accounts() == 2 : "Accounts total should parse";
    assert response.totals().positions() == 3 : "Positions total should parse";
    assert response.dataspaces().size() == 1 : "Expected one dataspace entry";
    final UaidPortfolioResponse.UaidPortfolioDataspace dataspace = response.dataspaces().get(0);
    assert dataspace.dataspaceId() == 42 : "Dataspace ID mismatch";
    assert "sandbox".equals(dataspace.dataspaceAlias())
        : "Dataspace alias mismatch";
    assert dataspace.accounts().size() == 1 : "Expected single account entry";
    final UaidPortfolioResponse.UaidPortfolioAccount account =
        dataspace.accounts().get(0);
    assert "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".equals(account.accountId())
        : "Account ID mismatch";
    assert "Primary".equals(account.label()) : "Account label mismatch";
    assert account.assets().size() == 1 : "Expected single asset entry";
    final UaidPortfolioResponse.UaidPortfolioAsset asset = account.assets().get(0);
    assert (assetDefinitionId + "#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV").equals(asset.assetId())
        : "Asset ID mismatch";
    assert assetDefinitionId.equals(asset.assetDefinitionId()) : "Asset definition mismatch";
    assert assetDefinitionId.equals(asset.asset()) : "Legacy asset accessor mismatch";
    assert asset.scope() == null : "Modern portfolio payload must not require legacy scope";
    assert "42".equals(asset.quantity()) : "Asset quantity mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "UAID request must be captured";
    assert "GET".equals(request.method()) : "UAID portfolio must use GET";
    assert request.headers().getOrDefault("Accept", List.of()).contains("application/json")
        : "Accept header must request JSON";
    assert request.headers().getOrDefault("X-Test", List.of()).contains("uaid")
        : "Custom headers must propagate";
    assert request.uri()
        .toString()
        .equals("https://torii.example/v1/accounts/uaid%3A" + hex + "/portfolio")
        : "Request URI must percent-encode UAID literal";
  }

  private static void uaidPortfolioRequestSupportsQuery() {
    final String hex =
        "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff0102030405060708090a0b0c0d0e0f11";
    final String assetDefinitionId = TestAssetDefinitionIds.TERTIARY;
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"totals\":{\"accounts\":0,\"positions\":0},"
            + "\"dataspaces\":[]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final UaidPortfolioQuery query =
        UaidPortfolioQuery.builder().setAsset(assetDefinitionId).setScope("global").build();
    transport.getUaidPortfolio("uaid:" + hex.toUpperCase(), query).join();

    final TransportRequest request = executor.lastRequest();
    assert request != null : "UAID request must be captured";
    assert request
        .uri()
        .toString()
        .equals(
            "https://torii.example/v1/accounts/uaid%3A"
                + hex
                + "/portfolio?asset="
                + assetDefinitionId
                + "&scope=global")
        : "UAID portfolio query must include asset and scope filters";
  }

  private static void uaidPortfolioQueryRejectsPaddedSelectorsBeforeDispatch() {
    final String hex =
        "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff0102030405060708090a0b0c0d0e0f11";
    final String assetDefinitionId = TestAssetDefinitionIds.TERTIARY;
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"totals\":{\"accounts\":0,\"positions\":0},"
            + "\"dataspaces\":[]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    transport
        .getUaidPortfolio(
            "uaid:" + hex,
            UaidPortfolioQuery.builder().setAsset(assetDefinitionId).setScope("global").build())
        .join();
    final TransportRequest before = executor.lastRequest();

    expectIllegalArgument(
        () ->
            transport.getUaidPortfolio(
                "uaid:" + hex,
                UaidPortfolioQuery.builder().setAsset(" " + assetDefinitionId).build()),
        "UAID portfolio asset selector must reject leading whitespace");
    assert executor.lastRequest() == before
        : "Padded asset selector must fail before sending an HTTP request";

    expectIllegalArgument(
        () ->
            transport.getUaidPortfolio(
                "uaid:" + hex,
                UaidPortfolioQuery.builder().setAsset(assetDefinitionId + " ").build()),
        "UAID portfolio asset selector must reject trailing whitespace");
    assert executor.lastRequest() == before
        : "Padded asset selector must fail before sending an HTTP request";

    expectIllegalArgument(
        () ->
            transport.getUaidPortfolio(
                "uaid:" + hex,
                UaidPortfolioQuery.builder().setAsset(assetDefinitionId).setScope(" global").build()),
        "UAID portfolio scope selector must reject leading whitespace");
    assert executor.lastRequest() == before
        : "Padded scope selector must fail before sending an HTTP request";
  }

  private static void uaidPathLiteralRejectsPaddedInputBeforeDispatch() {
    final String hex =
        "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff0102030405060708090a0b0c0d0e0f11";
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"totals\":{\"accounts\":0,\"positions\":0},"
            + "\"dataspaces\":[]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    transport.getUaidPortfolio("UAID:" + hex.toUpperCase()).join();
    final TransportRequest before = executor.lastRequest();
    assert before != null : "UAID positive request must be captured";

    for (final String uaid : new String[] {" uaid:" + hex, "uaid:" + hex + " ", "uaid: " + hex}) {
      expectIllegalArgument(
          () -> transport.getUaidPortfolio(uaid),
          "UAID path literal must reject surrounding whitespace before dispatch");
      assert executor.lastRequest() == before
          : "Padded UAID path literal must fail before sending an HTTP request";
    }
  }

  private static void uaidRequestsRespectBasePath() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String json = "{\"uaid\":\"uaid:" + hex + "\"}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example/api"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    transport.getUaidPortfolio("uaid:" + hex).join();

    final TransportRequest request = executor.lastRequest();
    assert request != null : "UAID request should be captured";
    assert request
        .uri()
        .toString()
        .equals("https://torii.example/api/v1/accounts/uaid%3A" + hex + "/portfolio")
        : "UAID endpoints must preserve baseUri path segments";
  }

  private static void uaidBindingsRequestParsesResponse() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"dataspaces\":[{"
            + "\"dataspace_id\":7,"
            + "\"dataspace_alias\":null,"
            + "\"accounts\":[\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\",\"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D\"]"
            + "}]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final UaidBindingsQuery query = UaidBindingsQuery.builder().build();
    final UaidBindingsResponse response =
        transport.getUaidBindings("uaid:" + hex.toUpperCase(), query).join();
    assert response.dataspaces().size() == 1 : "Expected bindings entry";
    final UaidBindingsResponse.UaidBindingsDataspace dataspace = response.dataspaces().get(0);
    assert dataspace.dataspaceId() == 7 : "Dataspace ID mismatch";
    assert dataspace.accounts().size() == 2 : "Account bindings mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request.uri()
        .toString()
        .equals(
            "https://torii.example/v1/space-directory/uaids/uaid%3A" + hex)
        : "Bindings URI must encode UAID literal and query";
  }

  private static void uaidManifestsRequestSupportsQuery() {
    final String hex =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    final String json =
        "{"
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"total\":1,"
            + "\"manifests\":[{"
            + "\"dataspace_id\":9,"
            + "\"dataspace_alias\":\"pilot\","
            + "\"manifest_hash\":\"deadbeef\","
            + "\"status\":\"Revoked\","
            + "\"lifecycle\":{"
            + "\"activated_epoch\":10,"
            + "\"expired_epoch\":null,"
            + "\"revocation\":{\"epoch\":15,\"reason\":\"policy\"}"
            + "},"
            + "\"accounts\":[\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"],"
            + "\"manifest\":{"
            + "\"version\":\"1\","
            + "\"uaid\":\"uaid:"
            + hex
            + "\","
            + "\"dataspace\":9,"
            + "\"entries\":[]"
            + "}"
            + "}]"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    final UaidManifestQuery query =
        UaidManifestQuery.builder()
            .setDataspaceId(9L)
            .setStatus(UaidManifestStatusFilter.INACTIVE)
            .setLimit(25L)
            .setOffset(5L)
            .build();

    final UaidManifestsResponse response =
        transport.getUaidManifests("uaid:" + hex, query).join();
    assert response.total() == 1 : "Total manifests must parse";
    assert response.manifests().size() == 1 : "Expected manifest record";
    final UaidManifestRecord record = response.manifests().get(0);
    assert record.dataspaceId() == 9 : "Dataspace ID mismatch";
    assert "pilot".equals(record.dataspaceAlias()) : "Dataspace alias mismatch";
    assert "deadbeef".equals(record.manifestHash()) : "Manifest hash mismatch";
    assert record.status() == UaidManifestStatus.REVOKED : "Status parsing mismatch";
    assert record.lifecycle().activatedEpoch() == 10L : "Activated epoch mismatch";
    assert record.lifecycle().expiredEpoch() == null : "Expired epoch should be null";
    assert record.lifecycle().revocation() != null : "Revocation should be present";
    assert record.lifecycle().revocation().epoch() == 15L : "Revocation epoch mismatch";
    assert "policy".equals(record.lifecycle().revocation().reason()) : "Revocation reason mismatch";
    assert record.accounts().contains("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV") : "Accounts must surface";
    assert record.manifestJson().contains("\"version\":\"1\"") : "Manifest JSON should be stored";
    final Map<String, Object> manifestMap = record.manifestAsMap();
    assert "1".equals(manifestMap.get("version")) : "Manifest map mismatch";
    assert manifestMap.get("dataspace") instanceof Number
        && ((Number) manifestMap.get("dataspace")).longValue() == 9L
        : "Manifest dataspace mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request
        .uri()
        .toString()
        .equals(
            "https://torii.example/v1/space-directory/uaids/uaid%3A"
                + hex
                + "/manifests?dataspace=9&status=inactive&limit=25&offset=5")
        : "Manifest URI must include encoded query parameters";
  }

  private static void identifierPoliciesRequestParsesResponse() {
    final String json = identifierPoliciesJson();
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final IdentifierPolicyListResponse response = transport.listIdentifierPolicies().join();
    assert response.total() == 1L : "Policy list total mismatch";
    assert response.items().size() == 1 : "Expected one identifier policy";
    final IdentifierPolicySummary item = response.items().get(0);
    assert "phone#retail".equals(item.policyId()) : "Policy id mismatch";
    assert "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".equals(item.owner()) : "Owner mismatch";
    assert item.active() : "Policy should be active";
    assert item.normalization() == IdentifierNormalization.PHONE_E164
        : "Normalization mismatch";
    assert "bfv-v1".equals(item.inputEncryption()) : "Input encryption mismatch";
    assert "ABCD".equals(item.inputEncryptionPublicParameters())
        : "Input encryption params mismatch";
    assert item.inputEncryptionPublicParametersDecoded() != null
        : "Decoded BFV parameters should be present";
    assert item.inputEncryptionPublicParametersDecoded().parameters().polynomialDegree() == 64L
        : "Decoded BFV polynomial degree mismatch";
    assert item.inputEncryptionPublicParametersDecoded().parameters().decompositionBaseLog() == 12
        : "Decoded BFV decomposition-base-log mismatch";
    assert "u64-v1".equals(item.inputEncryptionPublicParametersDecoded().noritoLengthEncoding())
        : "Decoded BFV Norito length encoding mismatch";
    assert item.proofVerifier() != null : "Proof verifier metadata should be parsed";
    assert "halo2-ipa".equals(item.proofVerifier().proofBackend())
        : "Proof verifier backend mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier policy request must be captured";
    assert "GET".equals(request.method()) : "Identifier policy list must use GET";
    assert request.uri().toString().equals("https://torii.example/v1/identifier-policies")
        : "Identifier policy URI mismatch";
    assert request.headers().getOrDefault("Accept", List.of()).contains("application/json")
        : "Identifier policy request must accept JSON";
  }

  private static void identifierPolicyParserRejectsNonExactPolicyAndProofVerifierFields() {
    final String canonical = identifierPoliciesJson();
    final String[][] cases = {
      {
        "identifier policy list.items[0].owner",
        canonical.replace(
            "\"owner\":\"sorauﾛ1Pﾉ",
            "\"owner\":\" sorauﾛ1Pﾉ")
      },
      {
        "identifier policy list.items[0].normalization",
        canonical.replace(
            "\"normalization\":\"phone_e164\"",
            "\"normalization\":\"Phone_E164\"")
      },
      {
        "identifier policy list.items[0].backend",
        canonical.replace(
            "\"backend\":\"bfv-affine-sha3-256-v1\"",
            "\"backend\":\"bfv-affine-sha3-256-v1 \"")
      },
      {
        "identifier policy list.items[0].input_encryption",
        canonical.replace(
            "\"input_encryption\":\"bfv-v1\"",
            "\"input_encryption\":\"BFV-v1\"")
      },
      {
        "identifier policy list.items[0].input_encryption_public_parameters",
        canonical.replace(
            "\"input_encryption_public_parameters\":\"ABCD\"",
            "\"input_encryption_public_parameters\":\" ABCD\"")
      },
      {
        "identifier policy list.items[0].input_encryption_public_parameters_decoded.norito_length_encoding",
        canonical.replace(
            "\"norito_length_encoding\":\"u64-v1\"",
            "\"norito_length_encoding\":\" u64-v1\"")
      },
      {
        "identifier policy list.items[0].note",
        canonical.replace(
            "\"note\":\"retail phone policy\"",
            "\"note\":\"retail phone policy \"")
      },
      {
        "identifier policy list.items[0].proof_verifier.proof_backend",
        canonical.replace("\"proof_backend\":\"halo2-ipa\"", "\"proof_backend\":\" halo2-ipa\"")
      },
      {
        "identifier policy list.items[0].proof_verifier.circuit_id",
        canonical.replace("\"circuit_id\":\"identifier-ram-lfe-v1\"", "\"circuit_id\":\"identifier-ram-lfe-v1 \"")
      },
      {
        "identifier policy list.items[0].proof_verifier.public_inputs_schema_hash",
        canonical.replace(
            "\"public_inputs_schema_hash\":\"" + "66".repeat(32) + "\"",
            "\"public_inputs_schema_hash\":\" " + "66".repeat(32) + "\"")
      },
      {
        "identifier policy list.items[0].proof_verifier.verifying_key_bytes_b64",
        canonical.replace(
            "\"verifying_key_bytes_b64\":\"AQID\"",
            "\"verifying_key_bytes_b64\":\"AQID \"")
      }
    };
    for (final String[] testCase : cases) {
      assertRamLfeParseFails(
          testCase[0],
          () -> IdentifierJsonParser.parsePolicyList(testCase[1].getBytes(StandardCharsets.UTF_8)));
    }
  }

  private static String identifierPoliciesJson() {
    return "{"
        + "\"total\":1,"
        + "\"items\":[{"
        + "\"policy_id\":\"phone#retail\","
        + "\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\","
        + "\"active\":true,"
        + "\"normalization\":\"phone_e164\","
        + "\"resolver_public_key\":\"ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29\","
        + "\"backend\":\"bfv-affine-sha3-256-v1\","
        + "\"input_encryption\":\"bfv-v1\","
        + "\"input_encryption_public_parameters\":\"ABCD\","
        + "\"input_encryption_public_parameters_decoded\":{"
        + "\"parameters\":{\"polynomial_degree\":64,\"plaintext_modulus\":257,\"ciphertext_modulus\":1099511627776,\"decomposition_base_log\":12},"
        + "\"public_key\":{\"b\":[1,2,3],\"a\":[4,5,6]},"
        + "\"max_input_bytes\":32,"
        + "\"norito_length_encoding\":\"u64-v1\""
        + "},"
        + "\"proof_verifier\":{"
        + "\"proof_backend\":\"halo2-ipa\","
        + "\"circuit_id\":\"identifier-ram-lfe-v1\","
        + "\"public_inputs_schema_hash\":\""
        + "66".repeat(32)
        + "\","
        + "\"verifying_key_bytes_b64\":\"AQID\""
        + "},"
        + "\"note\":\"retail phone policy\""
        + "}]"
        + "}";
  }

  private static void ramLfeProgramPoliciesRequestParsesResponse() {
    final String json = ramLfeProgramPoliciesJson();
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final RamLfeProgramPolicyListResponse response = transport.listRamLfeProgramPolicies().join();
    assert response.total() == 1L : "RAM-LFE policy list total mismatch";
    assert response.items().size() == 1 : "Expected one RAM-LFE program policy";
    final RamLfeProgramPolicySummary item = response.items().get(0);
    assert "identifier_lookup_retail".equals(item.programId()) : "Program id mismatch";
    assert "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".equals(item.owner()) : "Owner mismatch";
    assert item.active() : "Program policy should be active";
    assert "signed".equals(item.verificationMode()) : "Verification mode mismatch";
    assert "bfv-v1".equals(item.inputEncryption()) : "Input encryption mismatch";
    assert item.inputEncryptionPublicParametersDecoded() != null
        : "Decoded BFV parameters should be present";
    assert item.inputEncryptionPublicParametersDecoded().parameters().polynomialDegree() == 64L
        : "Decoded BFV polynomial degree mismatch";
    assert item.proofVerifier() != null : "Proof verifier metadata should be parsed";
    assert "halo2-ipa".equals(item.proofVerifier().proofBackend())
        : "Proof verifier backend mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "RAM-LFE policy request must be captured";
    assert "GET".equals(request.method()) : "RAM-LFE policy list must use GET";
    assert request.uri().toString().equals("https://torii.example/v1/ram-lfe/program-policies")
        : "RAM-LFE policy URI mismatch";
    assert request.headers().getOrDefault("Accept", List.of()).contains("application/json")
        : "RAM-LFE policy request must accept JSON";
  }

  private static void ramLfeProgramPolicyParserRejectsNonExactFields() {
    final String canonical = ramLfeProgramPoliciesJson();
    final String[][] cases = {
      {
        "ram-lfe program policy list.items[0].program_id",
        canonical.replace(
            "\"program_id\":\"identifier_lookup_retail\"",
            "\"program_id\":\" identifier_lookup_retail\"")
      },
      {
        "ram-lfe program policy list.items[0].owner",
        canonical.replace(
            "\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"",
            "\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV \"")
      },
      {
        "ram-lfe program policy list.items[0].resolver_public_key",
        canonical.replace(
            "\"resolver_public_key\":\"ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29\"",
            "\"resolver_public_key\":\" ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29\"")
      },
      {
        "ram-lfe program policy list.items[0].backend",
        canonical.replace(
            "\"backend\":\"bfv-programmed-sha3-256-v1\"",
            "\"backend\":\"BFV-programmed-sha3-256-v1\"")
      },
      {
        "ram-lfe program policy list.items[0].verification_mode",
        canonical.replace("\"verification_mode\":\"signed\"", "\"verification_mode\":\" signed\"")
      },
      {
        "ram-lfe program policy list.items[0].input_encryption",
        canonical.replace("\"input_encryption\":\"bfv-v1\"", "\"input_encryption\":\"bfv-v1 \"")
      },
      {
        "ram-lfe program policy list.items[0].input_encryption_public_parameters",
        canonical.replace(
            "\"input_encryption_public_parameters\":\"ABCD\"",
            "\"input_encryption_public_parameters\":\" ABCD\"")
      },
      {
        "ram-lfe program policy list.items[0].proof_verifier.proof_backend",
        canonical.replace("\"proof_backend\":\"halo2-ipa\"", "\"proof_backend\":\" halo2-ipa\"")
      },
      {
        "ram-lfe program policy list.items[0].proof_verifier.circuit_id",
        canonical.replace("\"circuit_id\":\"ram-lfe-v1\"", "\"circuit_id\":\"ram-lfe-v1 \"")
      },
      {
        "ram-lfe program policy list.items[0].proof_verifier.public_inputs_schema_hash",
        canonical.replace(
            "\"public_inputs_schema_hash\":\"" + "44".repeat(32) + "\"",
            "\"public_inputs_schema_hash\":\" " + "44".repeat(32) + "\"")
      },
      {
        "ram-lfe program policy list.items[0].proof_verifier.verifying_key_bytes_b64",
        canonical.replace(
            "\"verifying_key_bytes_b64\":\"AQID\"",
            "\"verifying_key_bytes_b64\":\"AQID \"")
      }
    };
    for (final String[] testCase : cases) {
      assertRamLfeParseFails(
          testCase[0],
          () -> RamLfeJsonParser.parsePolicyList(testCase[1].getBytes(StandardCharsets.UTF_8)));
    }
  }

  private static String ramLfeProgramPoliciesJson() {
    return "{"
        + "\"total\":1,"
        + "\"items\":[{"
        + "\"program_id\":\"identifier_lookup_retail\","
        + "\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\","
        + "\"active\":true,"
        + "\"resolver_public_key\":\"ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29\","
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":\"signed\","
        + "\"input_encryption\":\"bfv-v1\","
        + "\"input_encryption_public_parameters\":\"ABCD\","
        + "\"input_encryption_public_parameters_decoded\":{"
        + "\"parameters\":{\"polynomial_degree\":64,\"plaintext_modulus\":257,\"ciphertext_modulus\":1099511627776,\"decomposition_base_log\":12},"
        + "\"public_key\":{\"b\":[1,2,3],\"a\":[4,5,6]},"
        + "\"max_input_bytes\":32"
        + "},"
        + "\"proof_verifier\":{"
        + "\"proof_backend\":\"halo2-ipa\","
        + "\"circuit_id\":\"ram-lfe-v1\","
        + "\"public_inputs_schema_hash\":\""
        + "44".repeat(32)
        + "\","
        + "\"verifying_key_bytes_b64\":\"AQID\""
        + "},"
        + "\"note\":\"retail programmed policy\""
        + "}]"
        + "}";
  }

  private static IdentifierReceiptFixture signedIdentifierReceiptFixture(
      final IdentifierResolutionPayload payload) {
    try {
      final KeyPairGenerator generator = KeyPairGenerator.getInstance("Ed25519");
      final KeyPair keyPair = generator.generateKeyPair();
      final byte[] publicKeyBytes = keyPair.getPublic().getEncoded();
      final byte[] rawPublicKey =
          java.util.Arrays.copyOfRange(
              publicKeyBytes, publicKeyBytes.length - 32, publicKeyBytes.length);
      final byte[] payloadBytes = IdentifierReceiptCanonicalEncoder.encodePayload(payload);
      final byte[] message = IrohaHash.prehash(payloadBytes);
      final Signature signer = Signature.getInstance("Ed25519");
      signer.initSign(keyPair.getPrivate());
      signer.update(message);
      final byte[] signature = signer.sign();
      return new IdentifierReceiptFixture(
          "ed25519:" + PublicKeyCodec.encodePublicKeyMultihash(0x01, rawPublicKey),
          hex(signature));
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to build signed identifier receipt fixture", ex);
    }
  }

  private static String identifierReceiptJson(
      final IdentifierResolutionPayload payload, final String signatureHex) {
    return identifierReceiptJson(
        payload,
        signatureHex,
        payload.execution().backend(),
        payload.execution().verificationMode(),
        null);
  }

  private static String identifierReceiptJson(
      final IdentifierResolutionPayload payload,
      final String signatureHex,
      final String backend,
      final String verificationMode,
      final String attestationJsonOverride) {
    final String attestationJson =
        attestationJsonOverride != null
            ? attestationJsonOverride
            : "{\"kind\":\"signed\",\"signature\":" + jsonString(signatureHex) + "}";
    return "{"
        + "\"payload\":"
        + identifierPayloadJson(payload, backend, verificationMode)
        + ",\"attestation\":"
        + attestationJson
        + "}";
  }

  private static String identifierPayloadJson(final IdentifierResolutionPayload payload) {
    return identifierPayloadJson(
        payload,
        payload.execution().backend(),
        payload.execution().verificationMode());
  }

  private static String identifierPayloadJson(
      final IdentifierResolutionPayload payload,
      final String backend,
      final String verificationMode) {
    return "{"
        + "\"policy_id\":"
        + jsonString(payload.policyId())
        + ",\"execution\":"
        + identifierExecutionJson(payload.execution(), backend, verificationMode)
        + ",\"opening\":"
        + identifierOpeningJson(payload.opening())
        + ",\"opaque_id\":"
        + jsonString(payload.opaqueId())
        + ",\"receipt_hash\":"
        + jsonString(payload.receiptHash())
        + ",\"uaid\":"
        + jsonString(payload.uaid())
        + ",\"account_id\":"
        + jsonString(payload.accountId())
        + "}";
  }

  private static String identifierExecutionJson(
      final IdentifierResolutionExecutionPayload execution) {
    return identifierExecutionJson(
        execution,
        execution.backend(),
        execution.verificationMode());
  }

  private static String identifierExecutionJson(
      final IdentifierResolutionExecutionPayload execution,
      final String backend,
      final String verificationMode) {
    final String expires =
        execution.expiresAtMs() == null ? "" : ",\"expires_at_ms\":" + execution.expiresAtMs();
    return "{"
        + "\"program_id\":"
        + jsonString(execution.programId())
        + ",\"program_digest\":"
        + jsonString(execution.programDigest())
        + ",\"backend\":"
        + jsonString(backend)
        + ",\"verification_mode\":"
        + jsonString(verificationMode)
        + ",\"input_ciphertext_hash\":"
        + jsonString(execution.inputCiphertextHash())
        + ",\"output_ciphertext_hash\":"
        + jsonString(execution.outputCiphertextHash())
        + ",\"parameter_digest\":"
        + jsonString(execution.parameterDigest())
        + ",\"evaluation_key_digest\":"
        + jsonString(execution.evaluationKeyDigest())
        + ",\"output_hash\":"
        + jsonString(execution.outputHash())
        + ",\"associated_data_hash\":"
        + jsonString(execution.associatedDataHash())
        + ",\"executed_at_ms\":"
        + execution.executedAtMs()
        + expires
        + "}";
  }

  private static String identifierOpeningJson(final RamLfeOutputOpening opening) {
    final RamLfeOutputOpeningPayload payload = opening.payload();
    final String expires =
        payload.expiresAtMs() == null ? "" : ",\"expires_at_ms\":" + payload.expiresAtMs();
    return "{"
        + "\"payload\":{"
        + "\"program_id\":"
        + jsonString(payload.programId())
        + ",\"input_ciphertext_hash\":"
        + jsonString(payload.inputCiphertextHash())
        + ",\"output_ciphertext_hash\":"
        + jsonString(payload.outputCiphertextHash())
        + ",\"parameter_digest\":"
        + jsonString(payload.parameterDigest())
        + ",\"evaluation_key_digest\":"
        + jsonString(payload.evaluationKeyDigest())
        + ",\"opened_output_hash\":"
        + jsonString(payload.openedOutputHash())
        + ",\"opened_at_ms\":"
        + payload.openedAtMs()
        + expires
        + "},\"signature\":"
        + jsonString(opening.signature())
        + "}";
  }

  private static String jsonString(final String value) {
    return "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
  }

  private static RamLfeOutputOpening sampleOpening(final String programId) {
    return new RamLfeOutputOpening(
        new RamLfeOutputOpeningPayload(
            programId,
            "aa".repeat(32),
            "bb".repeat(32),
            "cc".repeat(32),
            "dd".repeat(32),
            "ee".repeat(32),
            42L,
            142L),
        "ff".repeat(64));
  }

  private static void identifierResolveRequestParsesResponse() {
    final String accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final IdentifierResolutionPayload payload =
        new IdentifierResolutionPayload(
            "phone#retail",
            new IdentifierResolutionExecutionPayload(
                "identifier_lookup_retail",
                "11".repeat(32),
                "bfv-affine-sha3-256-v1",
                "signed",
                "aa".repeat(32),
                "bb".repeat(32),
                "cc".repeat(32),
                "dd".repeat(32),
                "22".repeat(32),
                "33".repeat(32),
                42L,
                142L),
            sampleOpening("identifier_lookup_retail"),
            "opaque:" + "11".repeat(32),
            "22".repeat(32),
            "uaid:" + "33".repeat(31) + "35",
            accountId);
    final IdentifierReceiptFixture signed = signedIdentifierReceiptFixture(payload);
    final String json = identifierReceiptJson(payload, signed.signatureHex());
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<IdentifierResolutionReceipt> response =
        transport.resolveIdentifier(" phone#retail ", "0xABCD", payload.opening()).join();
    assert response.isPresent() : "Expected identifier resolution receipt";
    final IdentifierResolutionReceipt receipt = response.orElseThrow();
    assert "phone#retail".equals(receipt.policyId()) : "Policy id mismatch";
    assert ("opaque:" + "11".repeat(32)).equals(receipt.opaqueId()) : "Opaque id mismatch";
    assert "22".repeat(32).equals(receipt.receiptHash()) : "Receipt hash mismatch";
    assert ("uaid:" + "33".repeat(31) + "35").equals(receipt.uaid()) : "UAID mismatch";
    assert accountId.equals(receipt.accountId()) : "Account id mismatch";
    assert receipt.resolvedAtMs() == 42L : "Resolved timestamp mismatch";
    assert Long.valueOf(142L).equals(receipt.expiresAtMs()) : "Expiry mismatch";
    final IdentifierPolicySummary policy =
        new IdentifierPolicySummary(
            "phone#retail",
            accountId,
            true,
            IdentifierNormalization.PHONE_E164,
            signed.resolverPublicKey(),
            "bfv-affine-sha3-256-v1",
            "bfv-v1",
            null,
            null,
            null);
    assert receipt.verifyAttestation(policy) : "Receipt signature verification must succeed";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier resolve request must be captured";
    assert "POST".equals(request.method()) : "Identifier resolve must use POST";
    assert request.uri().toString().equals("https://torii.example/v1/identifiers/resolve")
        : "Identifier resolve URI mismatch";
    assert request.headers().getOrDefault("Content-Type", List.of()).contains("application/json")
        : "Identifier resolve must send JSON";
    final String requestBody = readBody(request);
    assert requestBody.contains("\"policy_id\":\"phone#retail\"")
        : "Identifier resolve payload must include policy id";
    assert requestBody.contains("\"encrypted_input\":\"abcd\"")
        : "Identifier resolve payload must include encrypted input";
    assert requestBody.contains("\"output_opening\":")
        : "Identifier resolve payload must include output opening";
  }

  private static void identifierResolveRequestParsesProgrammedReceiptResponse() {
    final String accountId =
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final IdentifierResolutionPayload payload =
        new IdentifierResolutionPayload(
            "email#retail",
            new IdentifierResolutionExecutionPayload(
                "email_retail",
                "44".repeat(32),
                "bfv-programmed-sha3-256-v1",
                "signed",
                "aa".repeat(32),
                "bb".repeat(32),
                "cc".repeat(32),
                "dd".repeat(32),
                "55".repeat(32),
                "66".repeat(32),
                42L,
                142L),
            sampleOpening("email_retail"),
            "opaque:" + "11".repeat(32),
            "22".repeat(32),
            "uaid:" + "33".repeat(31) + "35",
            accountId);
    final IdentifierReceiptFixture signed = signedIdentifierReceiptFixture(payload);
    final String json = identifierReceiptJson(payload, signed.signatureHex());
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<IdentifierResolutionReceipt> response =
        transport.resolveIdentifier("email#retail", "ABCD", payload.opening()).join();
    assert response.isPresent() : "Expected structured identifier resolution receipt";
    final IdentifierResolutionReceipt receipt = response.orElseThrow();
    assert "email#retail".equals(receipt.policyId()) : "Structured policy id mismatch";
    assert ("opaque:" + "11".repeat(32)).equals(receipt.opaqueId())
        : "Structured opaque id mismatch";
    assert "22".repeat(32).equals(receipt.receiptHash())
        : "Structured receipt hash mismatch";
    assert ("uaid:" + "33".repeat(31) + "35").equals(receipt.uaid())
        : "Structured UAID mismatch";
    assert accountId.equals(receipt.accountId()) : "Structured account id mismatch";
    assert receipt.resolvedAtMs() == 42L : "Structured resolved timestamp mismatch";
    assert Long.valueOf(142L).equals(receipt.expiresAtMs())
        : "Structured expiry mismatch";
    final IdentifierPolicySummary policy =
        new IdentifierPolicySummary(
            "email#retail",
            accountId,
            true,
            IdentifierNormalization.EMAIL_ADDRESS,
            signed.resolverPublicKey(),
            "bfv-programmed-sha3-256-v1",
            "bfv-v1",
            null,
            null,
            null);
    assert receipt.verifyAttestation(policy)
        : "Structured receipt signature verification must succeed";
  }

  private static void identifierResolveRequestAllowsNotFound() {
    final StubResponseExecutor executor = new StubResponseExecutor(404, new byte[0], "not found");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<IdentifierResolutionReceipt> response =
        transport.resolveIdentifier("phone#retail", "0xABCD", sampleOpening("identifier_lookup_retail")).join();
    assert response.isEmpty() : "404 identifier resolution should return Optional.empty";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier resolve request must be captured";
    assert readBody(request).contains("\"output_opening\":")
        : "Encrypted identifier resolve payload must include output opening";
  }

  private static void identifierHiddenFunctionRequestsRejectMalformedCiphertextEnvelopeFields() {
    expectIllegalArgument(
        () ->
            IdentifierResolveRequest.encrypted(
                "phone#retail", "abc", sampleOpening("identifier_lookup_retail")),
        "odd-length identifier ciphertext hex must be rejected");
    expectIllegalArgument(
        () ->
            IdentifierResolveRequest.encrypted(
                " ", "abcd", sampleOpening("identifier_lookup_retail")),
        "blank identifier policy ids must be rejected");
    expectIllegalArgument(
        () -> RamLfeExecuteRequest.encrypted("abc"),
        "odd-length RAM-LFE ciphertext hex must be rejected");
    expectIllegalArgument(
        () -> RamLfeExecuteRequest.encrypted("zz"),
        "non-hex RAM-LFE ciphertext must be rejected");
    expectIllegalArgument(
        () ->
            IdentifierResolveRequest.encrypted(
                samplePlaintextOnlyIdentifierPolicy(),
                "abcd",
                sampleOpening("identifier_lookup_retail")),
        "identifier policies without BFV input encryption must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildIdentifierResolvePayload(
                "phone#retail", "abc", sampleOpening("identifier_lookup_retail")),
        "identifier resolve payloads must reject malformed encrypted input");
  }

  private static void identifierClaimLookupAllowsNotFound() {
    final StubResponseExecutor executor = new StubResponseExecutor(404, new byte[0], "not found");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<IdentifierClaimRecord> response =
        transport.getIdentifierClaimByReceiptHash("55".repeat(32)).join();
    assert response.isEmpty() : "404 identifier claim lookup should return Optional.empty";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier claim lookup request must be captured";
    assert request.uri().toString().equals("https://torii.example/v1/identifiers/receipts/" + "55".repeat(32))
        : "Identifier claim lookup URI mismatch";
  }

  private static void identifierClaimRecordParserRejectsNonExactClaimFields() {
    final String accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final IdentifierResolutionPayload payload = sampleIdentifierResolutionPayload(accountId, "66");

    final String canonical = identifierClaimRecordJson(
        payload.policyId(),
        payload.opaqueId(),
        payload.receiptHash(),
        payload.uaid(),
        payload.accountId());
    final IdentifierClaimRecord claim =
        IdentifierJsonParser.parseClaimRecord(canonical.getBytes(StandardCharsets.UTF_8));
    assert payload.policyId().equals(claim.policyId()) : "claim policy id mismatch";
    assert payload.opaqueId().equals(claim.opaqueId()) : "claim opaque id mismatch";
    assert payload.receiptHash().equals(claim.receiptHash()) : "claim receipt hash mismatch";
    assert payload.uaid().equals(claim.uaid()) : "claim uaid mismatch";
    assert payload.accountId().equals(claim.accountId()) : "claim account id mismatch";
    assert claim.verifiedAtMs() == 42L : "claim verified_at_ms mismatch";
    assert Long.valueOf(142L).equals(claim.expiresAtMs()) : "claim expires_at_ms mismatch";

    final String[][] adversarial =
        new String[][] {
          {
            "policy_id",
            identifierClaimRecordJson(
                " " + payload.policyId(),
                payload.opaqueId(),
                payload.receiptHash(),
                payload.uaid(),
                payload.accountId())
          },
          {
            "opaque_id",
            identifierClaimRecordJson(
                payload.policyId(),
                payload.opaqueId() + " ",
                payload.receiptHash(),
                payload.uaid(),
                payload.accountId())
          },
          {
            "receipt_hash",
            identifierClaimRecordJson(
                payload.policyId(),
                payload.opaqueId(),
                " " + payload.receiptHash(),
                payload.uaid(),
                payload.accountId())
          },
          {
            "uaid",
            identifierClaimRecordJson(
                payload.policyId(),
                payload.opaqueId(),
                payload.receiptHash(),
                payload.uaid() + " ",
                payload.accountId())
          },
          {
            "account_id",
            identifierClaimRecordJson(
                payload.policyId(),
                payload.opaqueId(),
                payload.receiptHash(),
                payload.uaid(),
                " " + payload.accountId())
          },
        };
    for (final String[] item : adversarial) {
      expectRuntimeException(
          () -> IdentifierJsonParser.parseClaimRecord(item[1].getBytes(StandardCharsets.UTF_8)),
          "identifier claim record parser must reject non-exact " + item[0]);
    }
  }

  private static String identifierClaimRecordJson(
      final String policyId,
      final String opaqueId,
      final String receiptHash,
      final String uaid,
      final String accountId) {
    return "{"
        + "\"policy_id\":"
        + jsonString(policyId)
        + ",\"opaque_id\":"
        + jsonString(opaqueId)
        + ",\"receipt_hash\":"
        + jsonString(receiptHash)
        + ",\"uaid\":"
        + jsonString(uaid)
        + ",\"account_id\":"
        + jsonString(accountId)
        + ",\"verified_at_ms\":42"
        + ",\"expires_at_ms\":142"
        + "}";
  }

  private static void identifierClaimReceiptUsesAccountPath() {
    final String accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final IdentifierResolutionPayload payload =
        new IdentifierResolutionPayload(
            "phone#retail",
            new IdentifierResolutionExecutionPayload(
                "identifier_lookup_retail",
                "44".repeat(32),
                "bfv-affine-sha3-256-v1",
                "signed",
                "aa".repeat(32),
                "bb".repeat(32),
                "cc".repeat(32),
                "dd".repeat(32),
                "55".repeat(32),
                "66".repeat(32),
                7L,
                null),
            sampleOpening("identifier_lookup_retail"),
            "opaque:" + "44".repeat(32),
            "55".repeat(32),
            "uaid:" + "66".repeat(31) + "67",
            accountId);
    final IdentifierReceiptFixture signed = signedIdentifierReceiptFixture(payload);
    final String json = identifierReceiptJson(payload, signed.signatureHex());
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final Optional<IdentifierResolutionReceipt> response =
        transport.issueIdentifierClaimReceipt(accountId, "phone#retail", "ABCD", payload.opening()).join();
    assert response.isPresent() : "Claim receipt should parse";
    assert ("opaque:" + "44".repeat(32)).equals(response.orElseThrow().opaqueId())
        : "Opaque id mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier claim request must be captured";
    final String encodedAccountId =
        URLEncoder.encode(accountId, StandardCharsets.UTF_8).replace("+", "%20");
    assert request
        .uri()
        .toString()
        .equals(
            "https://torii.example/api/v1/accounts/"
                + encodedAccountId
                + "/identifiers/claim-receipt")
        : "Identifier claim receipt path must encode account id";
    assert readBody(request).contains("\"output_opening\":")
        : "Identifier claim payload must include output opening";
  }

  private static void ramLfeExecuteRequestParsesResponse() {
    final String outputHash = "44".repeat(32);
    final String json = ramLfeExecuteResponseJson();
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<RamLfeExecuteResponse> response =
        transport.executeRamLfeProgram("identifier_lookup_retail", "0xABCD").join();
    assert response.isPresent() : "Expected RAM-LFE execute response";
    final RamLfeExecuteResponse execute = response.orElseThrow();
    assert "identifier_lookup_retail".equals(execute.programId()) : "Program id mismatch";
    assert outputHash.equals(execute.outputHash()) : "Output hash mismatch";
    assert "abcd".equals(execute.outputCiphertext()) : "Output ciphertext mismatch";
    assert "signed".equals(execute.verificationMode()) : "Verification mode mismatch";
    assert execute.receipt().containsKey("payload") : "Raw receipt payload must be preserved";
    assert "identifier_lookup_retail".equals(execute.outputOpening().payload().programId())
        : "Output opening must be parsed";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "RAM-LFE execute request must be captured";
    assert "POST".equals(request.method()) : "RAM-LFE execute must use POST";
    assert request
        .uri()
        .toString()
        .equals("https://torii.example/v1/ram-lfe/programs/identifier_lookup_retail/execute")
        : "RAM-LFE execute URI mismatch";
    assert readBody(request).equals("{\"encrypted_input\":\"abcd\"}")
        : "RAM-LFE execute payload mismatch";
  }

  private static void ramLfeExecuteRequestAllowsNotFound() {
    final StubResponseExecutor executor = new StubResponseExecutor(404, new byte[0], "not found");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<RamLfeExecuteResponse> response =
        transport.executeRamLfeProgram("identifier_lookup_retail", "ABCD").join();
    assert response.isEmpty() : "404 RAM-LFE execute should return Optional.empty";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "RAM-LFE execute request must be captured";
    assert readBody(request).equals("{\"encrypted_input\":\"abcd\"}")
        : "Encrypted RAM-LFE execute payload mismatch";
  }

  private static void ramLfeReceiptVerifyUsesRawReceipt() {
    final String json = ramLfeReceiptVerifyResponseJson();
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());
    final Map<String, Object> verificationMode = new LinkedHashMap<>();
    verificationMode.put("mode", "Signed");
    verificationMode.put("value", null);
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("program_id", Map.of("name", "identifier_lookup_retail"));
    payload.put("backend", "bfv-programmed-sha3-256-v1");
    payload.put("verification_mode", verificationMode);
    payload.put("program_digest", "hash:" + "11".repeat(32).toUpperCase() + "#ABCD");
    payload.put("output_hash", "hash:" + "22".repeat(32).toUpperCase() + "#BCDE");
    payload.put(
        "associated_data_hash", "hash:" + "33".repeat(32).toUpperCase() + "#CDEF");
    payload.put("executed_at_ms", 42L);
    payload.put("expires_at_ms", 142L);
    final Map<String, Object> receipt = new LinkedHashMap<>();
    receipt.put("payload", payload);
    receipt.put("signature", "aa".repeat(64));

    final RamLfeReceiptVerifyResponse response =
        transport.verifyRamLfeReceipt(receipt, "C0FFEE").join();
    assert response.valid() : "RAM-LFE verify response should be valid";
    assert "identifier_lookup_retail".equals(response.programId()) : "Program id mismatch";
    assert Boolean.TRUE.equals(response.outputHashMatches()) : "Output-hash match mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "RAM-LFE verify request must be captured";
    assert request.uri().toString().equals("https://torii.example/api/v1/ram-lfe/receipts/verify")
        : "RAM-LFE verify URI mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> requestPayload =
        (Map<String, Object>) JsonParser.parse(readBody(request));
    assert "c0ffee".equals(requestPayload.get("output_hex")) : "Verify output_hex mismatch";
    assert requestPayload.get("receipt") instanceof Map<?, ?>
        : "Verify request must preserve raw receipt";
  }

  private static void ramLfeResponseParsersRejectNonExactFields() {
    final String canonicalExecute = ramLfeExecuteResponseJson();
    final String[][] executeCases = {
      {
        "program_id",
        canonicalExecute.replace(
            "\"program_id\":\"identifier_lookup_retail\",\"opaque_hash\"",
            "\"program_id\":\" identifier_lookup_retail\",\"opaque_hash\"")
      },
      {
        "opaque_hash",
        canonicalExecute.replace(
            "\"opaque_hash\":\"" + "11".repeat(32) + "\"",
            "\"opaque_hash\":\" " + "11".repeat(32) + "\"")
      },
      {
        "receipt_hash",
        canonicalExecute.replace(
            "\"receipt_hash\":\"" + "22".repeat(32) + "\"",
            "\"receipt_hash\":\"" + "22".repeat(32) + " \"")
      },
      {
        "output_ciphertext",
        canonicalExecute.replace(
            "\"output_ciphertext\":\"abcd\"",
            "\"output_ciphertext\":\" abcd\"")
      },
      {
        "output_hash",
        canonicalExecute.replace(
            "\"output_hash\":\"" + "44".repeat(32) + "\"",
            "\"output_hash\":\" " + "44".repeat(32) + "\"")
      },
      {
        "associated_data_hash",
        canonicalExecute.replace(
            "\"associated_data_hash\":\"" + "55".repeat(32) + "\"",
            "\"associated_data_hash\":\"" + "55".repeat(32) + " \"")
      },
      {
        "backend",
        canonicalExecute.replace(
            "\"backend\":\"bfv-programmed-sha3-256-v1\",\"verification_mode\"",
            "\"backend\":\" bfv-programmed-sha3-256-v1\",\"verification_mode\"")
      },
      {
        "verification_mode",
        canonicalExecute.replace(
            "\"verification_mode\":\"signed\",\"receipt\"",
            "\"verification_mode\":\"Signed\",\"receipt\"")
      }
    };
    for (final String[] testCase : executeCases) {
      assertRamLfeParseFails(
          "ram-lfe execute response." + testCase[0],
          () -> RamLfeJsonParser.parseExecuteResponse(testCase[1].getBytes(StandardCharsets.UTF_8)));
    }

    final String canonicalVerify = ramLfeReceiptVerifyResponseJson();
    final String[][] verifyCases = {
      {
        "program_id",
        canonicalVerify.replace(
            "\"program_id\":\"identifier_lookup_retail\"",
            "\"program_id\":\"identifier_lookup_retail \"")
      },
      {
        "backend",
        canonicalVerify.replace(
            "\"backend\":\"bfv-programmed-sha3-256-v1\"",
            "\"backend\":\"BFV-programmed-sha3-256-v1\"")
      },
      {
        "verification_mode",
        canonicalVerify.replace("\"verification_mode\":\"signed\"", "\"verification_mode\":\" signed\"")
      },
      {
        "output_hash",
        canonicalVerify.replace(
            "\"output_hash\":\"" + "44".repeat(32) + "\"",
            "\"output_hash\":\"" + "44".repeat(32) + " \"")
      },
      {
        "associated_data_hash",
        canonicalVerify.replace(
            "\"associated_data_hash\":\"" + "55".repeat(32) + "\"",
            "\"associated_data_hash\":\" " + "55".repeat(32) + "\"")
      }
    };
    for (final String[] testCase : verifyCases) {
      assertRamLfeParseFails(
          "ram-lfe receipt verify response." + testCase[0],
          () ->
              RamLfeJsonParser.parseReceiptVerifyResponse(
                  testCase[1].getBytes(StandardCharsets.UTF_8)));
    }
  }

  private static void assertRamLfeParseFails(final String label, final Runnable parse) {
    try {
      parse.run();
      assert false : "expected non-exact " + label + " to fail";
    } catch (final RuntimeException expected) {
      assert expected.getMessage() == null || expected.getMessage().contains(label)
          : label + " failure should mention field, got " + expected;
    }
  }

  private static String ramLfeExecuteResponseJson() {
    return "{"
        + "\"program_id\":\"identifier_lookup_retail\","
        + "\"opaque_hash\":\""
        + "11".repeat(32)
        + "\","
        + "\"receipt_hash\":\""
        + "22".repeat(32)
        + "\","
        + "\"output_ciphertext\":\"abcd\","
        + "\"output_hash\":\""
        + "44".repeat(32)
        + "\","
        + "\"associated_data_hash\":\""
        + "55".repeat(32)
        + "\","
        + "\"executed_at_ms\":42,"
        + "\"expires_at_ms\":142,"
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":\"signed\","
        + "\"receipt\":{"
        + "\"payload\":{"
        + "\"program_id\":{\"name\":\"identifier_lookup_retail\"},"
        + "\"program_digest\":\"hash:"
        + "11".repeat(32).toUpperCase()
        + "#ABCD\","
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":{\"mode\":\"Signed\",\"value\":null},"
        + "\"output_hash\":\"hash:"
        + "22".repeat(32).toUpperCase()
        + "#BCDE\","
        + "\"associated_data_hash\":\"hash:"
        + "33".repeat(32).toUpperCase()
        + "#CDEF\","
        + "\"executed_at_ms\":42,"
        + "\"expires_at_ms\":142"
        + "},"
        + "\"signature\":\""
        + "aa".repeat(64)
        + "\""
        + "},"
        + "\"output_opening\":"
        + identifierOpeningJson(sampleOpening("identifier_lookup_retail"))
        + "}";
  }

  private static String ramLfeReceiptVerifyResponseJson() {
    return "{"
        + "\"valid\":true,"
        + "\"program_id\":\"identifier_lookup_retail\","
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":\"signed\","
        + "\"output_hash\":\""
        + "44".repeat(32)
        + "\","
        + "\"associated_data_hash\":\""
        + "55".repeat(32)
        + "\","
        + "\"output_hash_matches\":true"
        + "}";
  }

  private static void vpnProfileRequestParsesNativeLeaseFields() {
    final String json =
        "{"
            + "\"available\":true,"
            + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
            + "\"supported_exit_classes\":[\"standard\",\"low-latency\",\"high-security\"],"
            + "\"default_exit_class\":\"standard\","
            + "\"lease_secs\":600,"
            + "\"dns_push_interval_secs\":60,"
            + "\"meter_family\":\"soranet.vpn.standard\","
            + "\"route_pushes\":[\"0.0.0.0/0\"],"
            + "\"excluded_routes\":[\"10.0.0.0/8\"],"
            + "\"dns_servers\":[\"1.1.1.1\"],"
            + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
            + "\"mtu_bytes\":1280,"
            + "\"display_billing_label\":\"standard XOR\","
            + "\"operator_account_id\":\"sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT\","
            + "\"lease_fee\":\"1000000.25\","
            + "\"settlement_grace_secs\":120,"
            + "\"flow_label_bits\":24,"
            + "\"padding_budget_ms\":15,"
            + "\"relay_id_hex\":\""
            + VALID_ED25519_PUBLIC_KEY_HEX
            + "\","
            + "\"descriptor_commit_hex\":\""
            + "cd".repeat(32)
            + "\","
            + "\"tls_server_name\":\"relay.example\","
            + "\"relay_tls_spki_sha256_hex\":\""
            + "ab".repeat(32)
            + "\","
            + "\"relay_certificate_sha256_hex\":\""
            + "ef".repeat(32)
            + "\","
            + "\"directory_snapshot_digest_hex\":\""
            + "42".repeat(32)
            + "\""
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final VpnProfile profile = transport.getVpnProfile().join();

    assert profile.available() : "VPN profile should be available";
    assert "sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT".equals(profile.operatorAccountId()) : "VPN operator account mismatch";
    assert "1000000.25".equals(profile.leaseFee()) : "VPN lease fee mismatch";
    assert profile.dnsPushIntervalSecs() == 60L : "VPN DNS push interval mismatch";
    assert profile.settlementGraceSecs() == 120L : "VPN settlement grace mismatch";
    assert VALID_ED25519_PUBLIC_KEY_HEX.equals(profile.relayIdHex()) : "VPN relay id mismatch";
    assert "cd".repeat(32).equals(profile.descriptorCommitHex())
        : "VPN descriptor digest mismatch";
    assert "relay.example".equals(profile.tlsServerName()) : "VPN TLS SNI mismatch";
    assert "ab".repeat(32).equals(profile.relayTlsSpkiSha256Hex()) : "VPN TLS pin mismatch";
    assert "ef".repeat(32).equals(profile.relayCertificateSha256Hex())
        : "VPN certificate digest mismatch";
    assert "42".repeat(32).equals(profile.directorySnapshotDigestHex())
        : "VPN directory snapshot digest mismatch";
    assert "GET".equals(executor.lastRequest().method()) : "VPN profile must use GET";
    assert executor.lastRequest().uri().toString().equals("https://torii.example/v1/vpn/profile")
        : "VPN profile URI mismatch";

    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replace("\"dns_push_interval_secs\":60", "\"dns_push_interval_secs\":29")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must reject DNS push intervals below 30 seconds");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replace("\"dns_push_interval_secs\":60,", "")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must require dns_push_interval_secs");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must reject unknown fields");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseProfile(
                json.replace("ab".repeat(32), "AB".repeat(32))
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN profile parser must reject uppercase TLS pins");
  }

  private static void vpnProfileRequiresHttpsAndValidatesAvailabilityBoundTrustTuple() {
    final StubResponseExecutor insecureExecutor =
        new StubResponseExecutor(200, vpnProfileJson().getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport insecureTransport =
        HttpClientTransport.withExecutor(
            insecureExecutor,
            ClientConfig.builder().setBaseUri(URI.create("http://torii.example")).build());
    expectRuntimeException(
        insecureTransport::getVpnProfile,
        "VPN profile must reject insecure transport before dispatch");
    assert insecureExecutor.lastRequest() == null : "insecure VPN request reached dispatch";

    final Map<String, Object> unavailable = vpnJsonObject(vpnProfileJson());
    unavailable.put("available", Boolean.FALSE);
    for (final String field :
        List.of(
            "relay_endpoint",
            "relay_id_hex",
            "descriptor_commit_hex",
            "tls_server_name",
            "relay_tls_spki_sha256_hex",
            "relay_certificate_sha256_hex",
            "directory_snapshot_digest_hex")) {
      unavailable.put(field, "");
    }
    final VpnProfile unavailableProfile =
        VpnJsonParser.parseProfile(
            JsonEncoder.encode(unavailable).getBytes(StandardCharsets.UTF_8));
    assert !unavailableProfile.available();
    assert unavailableProfile.relayEndpoint().isEmpty();
    assert unavailableProfile.relayIdHex().isEmpty();

    final Object[][] invalidValues = {
      {"relay_id_hex", "00".repeat(32)},
      {"descriptor_commit_hex", "00".repeat(32)},
      {"descriptor_commit_hex", "0x" + "cd".repeat(32)},
      {"tls_server_name", "Relay.Example"},
      {"tls_server_name", "-relay.example"},
      {"relay_endpoint", "/dns4/Relay.Example/udp/443/quic"},
      {"relay_endpoint", "/dns4/relay.example/udp/0443/quic"},
      {"relay_endpoint", "/dns4/relay.example/tcp/443/quic"}
    };
    for (final Object[] invalid : invalidValues) {
      expectRuntimeException(
          () ->
              VpnJsonParser.parseProfile(
                  vpnJsonWithField(vpnProfileJson(), (String) invalid[0], invalid[1])),
          "VPN profile parser accepted malformed trust tuple field " + invalid[0]);
    }
  }

  private static void vpnSessionParserRejectsNonCanonicalHelperTicketHex() {
    final String sessionId = "33".repeat(32);
    final String paymentTxHash = "44".repeat(32);
    final String[] invalidValues = {
      "0x" + VPN_HELPER_TICKET_HEX,
      VPN_HELPER_TICKET_HEX.toUpperCase(Locale.ROOT),
      VPN_HELPER_TICKET_HEX.substring(0, VPN_HELPER_TICKET_HEX.length() - 2)
    };
    for (final String invalid : invalidValues) {
      final byte[] payload =
          vpnSessionJson(sessionId, paymentTxHash)
              .replace(VPN_HELPER_TICKET_HEX, invalid)
              .getBytes(StandardCharsets.UTF_8);
      expectRuntimeException(
          () -> VpnJsonParser.parseSession(payload),
          "VPN session parser must reject non-canonical helper_ticket_hex");
    }
  }

  private static void vpnResponseParsersRejectNonCanonicalIdsHashesAndUnknownFields() {
    final String identifier = "ab".repeat(32);
    final String paymentTxHash = "cd".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final String quote = vpnQuoteJson(identifier, meteringKey);

    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote
                    .replace(
                        "\"quote_id\":\"" + identifier + "\"",
                        "\"quote_id\":\"0x" + identifier + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN quote parser must reject prefixed quote ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote.replace("aa".repeat(16), "AA".repeat(16))
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN quote parser must reject uppercase session ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN quote parser must reject unknown fields");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                quote
                    .replaceFirst(
                        "\"payload_hex\":\"cafe\"",
                        "\"payload_hex\":\"cafe\",\"unexpected\":true")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN transaction instruction parser must reject unknown fields");

    final String session = vpnSessionJson(identifier, paymentTxHash);
    expectRuntimeException(
        () ->
            VpnJsonParser.parseSession(
                session
                    .replace(
                        "\"session_id\":\"" + identifier + "\"",
                        "\"session_id\":\"" + identifier.toUpperCase(Locale.ROOT) + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN session parser must reject uppercase session ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseSession(
                session
                    .replace(
                        "\"payment_tx_hash\":\"" + paymentTxHash + "\"",
                        "\"payment_tx_hash\":\"0x" + paymentTxHash + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN session parser must reject prefixed payment hashes");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseSession(
                session.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN session parser must reject unknown fields");

    final String receipt = vpnReceiptJson(identifier, paymentTxHash, true);
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                receipt
                    .replace(
                        "\"lease_id_hex\":\"" + identifier + "\"",
                        "\"lease_id_hex\":\"" + identifier.toUpperCase(Locale.ROOT) + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt parser must reject uppercase lease ids");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                receipt
                    .replace(
                        "\"payment_tx_hash\":\"" + paymentTxHash + "\"",
                        "\"payment_tx_hash\":\"0x" + paymentTxHash + "\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt parser must reject prefixed payment hashes");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                receipt.replaceFirst("\\{", "{\"unexpected\":true,")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt parser must reject unknown fields");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceiptList(
                ("{\"items\":[" + receipt + "],\"total\":1,\"unexpected\":true}")
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN receipt-list parser must reject unknown fields");
  }

  private static void vpnResponseParsersRejectMissingRequiredFieldsAndSchemaBounds() {
    final String identifier = "ab".repeat(32);
    final String paymentTxHash = "cd".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final String profile = vpnProfileJson();
    final String quote = vpnQuoteJson(identifier, meteringKey);
    final String session = vpnSessionJson(identifier, paymentTxHash);
    final String receipt = vpnReceiptJson(identifier, paymentTxHash, true);
    final String receiptList = "{\"items\":[" + receipt + "],\"total\":1}";

    final List<Runnable> missingCases =
        List.of(
            () ->
                VpnJsonParser.parseProfile(
                    vpnJsonWithoutField(profile, "relay_tls_spki_sha256_hex")),
            () ->
                VpnJsonParser.parseQuote(vpnJsonWithoutField(quote, "open_lease_instruction")),
            () -> VpnJsonParser.parseSession(vpnJsonWithoutField(session, "route_pushes")),
            () ->
                VpnJsonParser.parseReceipt(
                    vpnJsonWithoutField(receipt, "settle_lease_instruction")),
            () -> VpnJsonParser.parseReceiptList(vpnJsonWithoutField(receiptList, "items")));
    for (final Runnable decode : missingCases) {
      expectRuntimeException(decode, "VPN response parser must reject a missing required field");
    }
    expectRuntimeException(
        () -> VpnJsonParser.parseSession(vpnJsonWithField(session, "route_pushes", null)),
        "VPN response parser must reject null required arrays");

    final Object[][] profileViolations = {
      {"supported_exit_classes", List.of("standard", "low-latency")},
      {"supported_exit_classes", List.of("standard", "standard", "high-security")},
      {"default_exit_class", "unsupported"},
      {"lease_secs", 0L},
      {"lease_secs", 4_294_967_296L},
      {"mtu_bytes", 1279L},
      {"settlement_grace_secs", 0L},
      {"flow_label_bits", 23L},
      {"padding_budget_ms", 0L}
    };
    for (final Object[] violation : profileViolations) {
      expectRuntimeException(
          () ->
              VpnJsonParser.parseProfile(
                  vpnJsonWithField(profile, (String) violation[0], violation[1])),
          "VPN profile parser must reject invalid " + violation[0]);
    }

    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                vpnJsonWithField(quote, "tx_instructions", Collections.emptyList())),
        "VPN quote parser must reject the retired transaction instruction array");
    expectRuntimeException(
        () -> VpnJsonParser.parseSession(vpnJsonWithField(session, "status", "settled")),
        "VPN session parser must require active status");
    expectRuntimeException(
        () -> VpnJsonParser.parseReceipt(vpnJsonWithField(receipt, "status", "active")),
        "VPN receipt parser must reject active status");
    expectRuntimeException(
        () -> VpnJsonParser.parseReceipt(vpnJsonWithField(receipt, "receipt_source", "operator")),
        "VPN receipt parser must reject unknown receipt sources");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceipt(
                vpnJsonWithField(receipt, "tx_instructions", Collections.emptyList())),
        "VPN receipt parser must reject the retired transaction instruction array");

    final Map<String, Object> receiptObject = vpnJsonObject(receipt);
    expectRuntimeException(
        () ->
            VpnJsonParser.parseReceiptList(
                vpnJsonWithField(receiptList, "items", Collections.nCopies(25, receiptObject))),
        "VPN receipt-list parser must allow at most 24 items");
    expectRuntimeException(
        () -> VpnJsonParser.parseReceiptList(vpnJsonWithField(receiptList, "total", 25L)),
        "VPN receipt-list parser must cap total at 24");
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> vpnJsonObject(final String json) {
    return new LinkedHashMap<>((Map<String, Object>) JsonParser.parse(json));
  }

  private static byte[] vpnJsonWithField(
      final String json, final String field, final Object value) {
    final Map<String, Object> root = vpnJsonObject(json);
    root.put(field, value);
    return JsonEncoder.encode(root).getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] vpnJsonWithoutField(final String json, final String field) {
    final Map<String, Object> root = vpnJsonObject(json);
    root.remove(field);
    return JsonEncoder.encode(root).getBytes(StandardCharsets.UTF_8);
  }

  private static void vpnRoutesRejectWrongSuccessfulStatusCodes() throws Exception {
    final String identifier = "33".repeat(32);
    final String paymentTxHash = "44".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth("alice", keyPair, 1_700_000_000_050L, "vpn-status-nonce");

    assertVpnWrongStatusRejected(201, vpnProfileJson(), transport -> transport.getVpnProfile().join());
    assertVpnWrongStatusRejected(
        200,
        vpnQuoteJson(identifier, meteringKey),
        transport ->
            transport
                .createVpnQuote(new VpnQuoteCreateRequest("standard", "0x" + meteringKey), auth)
                .join());
    assertVpnWrongStatusRejected(
        200,
        vpnSessionJson(identifier, paymentTxHash),
        transport ->
            transport
                .createVpnSession(
                    new VpnSessionCreateRequest(
                        "standard", identifier, "0x" + paymentTxHash, meteringKey),
                    auth)
                .join());
    assertVpnWrongStatusRejected(
        201,
        vpnSessionJson(identifier, paymentTxHash),
        transport -> transport.getVpnSession(identifier, auth).join());
    assertVpnWrongStatusRejected(
        201,
        vpnReceiptJson(identifier, paymentTxHash, false),
        transport -> transport.deleteVpnSession(identifier, auth).join());
    assertVpnWrongStatusRejected(
        200,
        vpnReceiptJson(identifier, paymentTxHash, true),
        transport ->
            transport
                .submitVpnReceipt(
                    new VpnReceiptSubmitRequest("0xCAFE", "BEEF", "0x" + identifier), auth)
                .join());
    final String receipt = vpnReceiptJson(identifier, paymentTxHash, true);
    assertVpnWrongStatusRejected(
        201,
        "{\"items\":[" + receipt + "],\"total\":1}",
        transport -> transport.listVpnReceipts(auth).join());
  }

  @FunctionalInterface
  private interface VpnTransportCall {
    void invoke(HttpClientTransport transport);
  }

  private static void assertVpnWrongStatusRejected(
      final int status, final String body, final VpnTransportCall call) {
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new StubResponseExecutor(status, body.getBytes(StandardCharsets.UTF_8)),
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());
    expectRuntimeException(
        () -> call.invoke(transport),
        "VPN route must reject unexpected successful status " + status);
  }

  private static void vpnQuoteRequestSignsCanonicalBodyAndParsesOpenLeaseInstruction()
      throws Exception {
    final String quoteId = "11".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            201, vpnQuoteJson(quoteId, meteringKey).getBytes(StandardCharsets.UTF_8));
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth("alice", keyPair, 1_700_000_000_000L, "vpn-nonce-1");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final VpnQuote quote =
        transport.createVpnQuote(new VpnQuoteCreateRequest("low-latency", "0x" + meteringKey), auth)
            .join();

    assert quoteId.equals(quote.quoteId()) : "VPN quote id mismatch";
    assert quoteId.equals(quote.leaseIdHex()) : "VPN lease id mismatch";
    assert meteringKey.equals(quote.meteringPublicKeyHex()) : "VPN metering key mismatch";
    assert quote.openLeaseInstruction() != null : "VPN quote must include open lease instruction";
    assert "iroha_data_model::isi::vpn::OpenVpnLeaseEscrow"
        .equals(quote.openLeaseInstruction().wireId()) : "Open lease wire id mismatch";

    final TransportRequest request = executor.lastRequest();
    assert "POST".equals(request.method()) : "VPN quote must use POST";
    assert request.uri().toString().equals("https://torii.example/api/v1/vpn/quotes")
        : "VPN quote URI mismatch";
    assert readBody(request)
        .equals("{\"exit_class\":\"low-latency\",\"metering_public_key_hex\":\"" + meteringKey + "\"}")
        : "VPN quote body mismatch";
    assert "alice".equals(request.headers().get(CanonicalRequestSigner.HEADER_ACCOUNT).get(0))
        : "VPN quote account header mismatch";
    assert "1700000000000"
        .equals(request.headers().get(CanonicalRequestSigner.HEADER_TIMESTAMP_MS).get(0))
        : "VPN quote timestamp header mismatch";
    assert "vpn-nonce-1".equals(request.headers().get(CanonicalRequestSigner.HEADER_NONCE).get(0))
        : "VPN quote nonce header mismatch";
    assertCanonicalSignature(request, keyPair.getPublic(), 1_700_000_000_000L, "vpn-nonce-1");
  }

  private static void ed25519KeyRoutesRejectSmallOrderIdentityPoint() {
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildVpnQuoteCreatePayload(
                "standard", ED25519_IDENTITY_KEY_HEX),
        "VPN quote creation must reject a small-order Ed25519 metering key");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildVpnSessionCreatePayload(
                "standard", "11".repeat(32), "22".repeat(32), ED25519_IDENTITY_KEY_HEX),
        "VPN session creation must reject a small-order Ed25519 metering key");
    expectRuntimeException(
        () ->
            VpnJsonParser.parseQuote(
                vpnQuoteJson("11".repeat(32), ED25519_IDENTITY_KEY_HEX)
                    .getBytes(StandardCharsets.UTF_8)),
        "VPN quote parsing must reject a small-order Ed25519 metering key");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder()
                    .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(new byte[] {1})
                    .setPublicKeyHex(ED25519_IDENTITY_KEY_HEX)
                    .build()),
        "multisig proposal creation must reject a small-order Ed25519 public key");
  }

  private static void feeQuoteRequestSignsExactUnsignedPayloadAndPreservesPayer()
      throws Exception {
    final String responseBody =
        "{\"intent\":{\"payer\":\"authority\",\"value\":{\"charge_limits\":[],"
            + "\"gas_limit\":9000}},\"observation\":{\"schedule_revision\":4},"
            + "\"components\":[],\"capacities\":[],\"decision\":{\"accepted\":true}}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, responseBody.getBytes(StandardCharsets.UTF_8));
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth("alice", keyPair, 1_700_000_000_020L, "fee-quote-1");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());
    final Map<String, Object> unsignedPayload = new LinkedHashMap<>();
    unsignedPayload.put("chain", "test-chain");
    unsignedPayload.put("authority", "alice");
    unsignedPayload.put("fee_payment", feePayment(9_000L).toJsonMap());

    final FeeQuoteResponse quote = transport.quoteFees(unsignedPayload, auth).join();

    assert quote.intent() instanceof FeePaymentIntent.Authority
        : "fee quote payer must remain authority";
    assert Long.valueOf(9_000L).equals(quote.intent().gasLimit())
        : "fee quote gas bound mismatch";
    assert ((Number) quote.observation().get("schedule_revision")).longValue() == 4L
        : "fee quote observation mismatch";
    final TransportRequest request = executor.lastRequest();
    assert "POST".equals(request.method()) : "fee quote must use POST";
    assert "https://torii.example/api/v1/fees/quote".equals(request.uri().toString())
        : "fee quote URI mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> requestBody =
        (Map<String, Object>) JsonParser.parse(readBody(request));
    assert unsignedPayload.equals(requestBody.get("payload")) : "unsigned payload changed";
    assertCanonicalSignature(request, keyPair.getPublic(), 1_700_000_000_020L, "fee-quote-1");

    expectIllegalArgument(
        () ->
            transport.quoteFees(
                unsignedPayload, canonicalAuth("bob", keyPair, null, null)),
        "fee quote must reject an auth payer mismatch");
    assert executor.lastRequest() == request : "payer mismatch must fail before dispatch";
  }

  private static void feeSponsorProgramRequestSignsExactSelectorAndParsesLifecycle()
      throws Exception {
    final String sponsor = TestAccountIds.ed25519Authority(0x37);
    final String responseBody =
        "{\"id\":{\"sponsor\":\""
            + sponsor
            + "\",\"name\":\"wallet_fx\"},"
            + "\"lifecycle\":{\"state\":\"active\",\"value\":null},"
            + "\"active_revision\":3,\"staged_revision\":4,"
            + "\"scheduled_activation\":{\"revision\":4,\"activate_at_height\":100}}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, responseBody.getBytes(StandardCharsets.UTF_8));
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth("alice", keyPair, 1_700_000_000_021L, "fee-program-1");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final FeeSponsorProgramResponse program =
        transport
            .getFeeSponsorProgram(new FeeSponsorProgramId(sponsor, "wallet_fx"), auth)
            .join();

    assert sponsor.equals(program.id().sponsor()) : "fee sponsor account mismatch";
    assert "wallet_fx".equals(program.id().name()) : "fee sponsor program name mismatch";
    assert program.lifecycle() == FeeSponsorProgramLifecycle.ACTIVE
        : "fee sponsor lifecycle mismatch";
    assert Long.valueOf(3L).equals(program.activeRevision())
        : "fee sponsor active revision mismatch";
    assert Long.valueOf(4L).equals(program.stagedRevision())
        : "fee sponsor staged revision mismatch";
    assert program.scheduledActivation() != null : "missing scheduled activation";
    assert program.scheduledActivation().revision() == 4L
        : "scheduled activation revision mismatch";
    assert program.scheduledActivation().activateAtHeight() == 100L
        : "scheduled activation height mismatch";
    final TransportRequest request = executor.lastRequest();
    assert "POST".equals(request.method()) : "fee sponsor lookup must use POST";
    assert "https://torii.example/api/v1/fee-sponsor-programs/by-id"
        .equals(request.uri().toString()) : "fee sponsor lookup URI mismatch";
    assert ("{\"program_id\":\"" + sponsor + "/wallet_fx\"}").equals(readBody(request))
        : "fee sponsor lookup body mismatch";
    assertCanonicalSignature(request, keyPair.getPublic(), 1_700_000_000_021L, "fee-program-1");
  }

  private static void feeQuoteRejectsPayerRevisionAndGasSubstitution() throws Exception {
    final String sponsor = TestAccountIds.ed25519Authority(0x37);
    final FeePaymentIntent sponsorIntent =
        FeePaymentIntent.sponsor(
            new FeeSponsorProgramId(sponsor, "wallet_fx"),
            3L,
            Collections.emptyList(),
            9_000L);
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth = canonicalAuth("alice", keyPair, null, null);

    final Object[][] cases = {
      {
        feePayment(9_000L),
        "{\"payer\":\"authority\",\"value\":{\"charge_limits\":[],\"gas_limit\":9001}}"
      },
      {
        sponsorIntent,
        "{\"payer\":\"authority\",\"value\":{\"charge_limits\":[],\"gas_limit\":9000}}"
      },
      {
        sponsorIntent,
        "{\"payer\":\"sponsor\",\"value\":{\"program_id\":{\"sponsor\":\""
            + sponsor
            + "\",\"name\":\"wallet_fx\"},\"program_revision\":4,"
            + "\"charge_limits\":[],\"gas_limit\":9000}}"
      }
    };
    for (final Object[] entry : cases) {
      final FeePaymentIntent requested = (FeePaymentIntent) entry[0];
      final String responseBody =
          "{\"intent\":"
              + entry[1]
              + ",\"observation\":{},\"components\":[],\"capacities\":[],"
              + "\"decision\":{}}";
      final StubResponseExecutor executor =
          new StubResponseExecutor(200, responseBody.getBytes(StandardCharsets.UTF_8));
      final HttpClientTransport transport =
          HttpClientTransport.withExecutor(
              executor,
              ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());
      final Map<String, Object> unsignedPayload = new LinkedHashMap<>();
      unsignedPayload.put("authority", "alice");
      unsignedPayload.put("fee_payment", requested.toJsonMap());

      expectCompletionIllegalArgument(
          transport.quoteFees(unsignedPayload, auth),
          "fee quote must reject a substituted payer, sponsor revision, or gas bound");
    }
  }

  private static void feeSponsorProgramRejectsSubstitutedResponseId() throws Exception {
    final String sponsor = TestAccountIds.ed25519Authority(0x37);
    final String responseBody =
        "{\"id\":{\"sponsor\":\""
            + sponsor
            + "\",\"name\":\"other\"},"
            + "\"lifecycle\":{\"state\":\"active\",\"value\":null}}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, responseBody.getBytes(StandardCharsets.UTF_8));
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    expectCompletionIllegalArgument(
        transport.getFeeSponsorProgram(
            new FeeSponsorProgramId(sponsor, "wallet_fx"),
            canonicalAuth("alice", keyPair, null, null)),
        "fee sponsor lookup must reject a substituted response id");
  }

  private static void vpnSessionAndReceiptRequestsUseNativeLeaseDtos() throws Exception {
    final String sessionId = "33".repeat(32);
    final String paymentTxHash = "44".repeat(32);
    final String meteringKey = VALID_ED25519_PUBLIC_KEY_HEX;
    final String settledReceipt = vpnReceiptJson(sessionId, paymentTxHash, true);
    final QueueResponseExecutor executor =
        new QueueResponseExecutor(
            List.of(
                new QueuedResponse(201, vpnSessionJson(sessionId, paymentTxHash)),
                new QueuedResponse(200, vpnSessionJson(sessionId, paymentTxHash)),
                new QueuedResponse(200, vpnReceiptJson(sessionId, paymentTxHash, false)),
                new QueuedResponse(201, settledReceipt),
                new QueuedResponse(200, "{\"items\":[" + settledReceipt + "],\"total\":1}")));
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth("alice", keyPair, 1_700_000_000_001L, "vpn-nonce-2");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final VpnSession session =
        transport
            .createVpnSession(
                new VpnSessionCreateRequest("standard", sessionId, "0x" + paymentTxHash, meteringKey),
                auth)
            .join();
    final Optional<VpnSession> fetched = transport.getVpnSession(sessionId, auth).join();
    final Optional<VpnReceipt> deleted = transport.deleteVpnSession("0x" + sessionId, auth).join();
    final VpnReceipt submitted =
        transport
            .submitVpnReceipt(new VpnReceiptSubmitRequest("0xCAFE", "BEEF", "0x" + sessionId), auth)
            .join();
    final VpnReceiptListResponse receipts = transport.listVpnReceipts(auth).join();

    assert sessionId.equals(session.sessionId()) : "VPN session id mismatch";
    assert VPN_HELPER_TICKET_HEX.equals(session.helperTicketHex()) : "VPN helper ticket length mismatch";
    assert session.helperTicketHex().length() == 1328 : "VPN helper ticket must be 664 bytes";
    assert fetched.isPresent() : "VPN session lookup should be present";
    assert deleted.isPresent() : "VPN delete receipt should be present";
    assert "disconnected".equals(deleted.get().status()) : "VPN delete status mismatch";
    assert "settled".equals(submitted.status()) : "VPN settled receipt status mismatch";
    assert "750000.125".equals(submitted.earnedFee()) : "VPN earned fee mismatch";
    assert "250000.125".equals(submitted.refundedFee()) : "VPN refund mismatch";
    assert submitted.settleLeaseInstruction() != null : "VPN settled receipt must include settle instruction";
    assert "iroha_data_model::isi::vpn::SettleVpnLease"
        .equals(submitted.settleLeaseInstruction().wireId()) : "VPN settle wire id mismatch";
    assert receipts.total() == 1L : "VPN receipt list total mismatch";
    assert sessionId.equals(receipts.items().get(0).leaseIdHex()) : "VPN receipt lease id mismatch";

    assert readBody(executor.requests().get(0))
        .equals(
            "{\"exit_class\":\"standard\",\"metering_public_key_hex\":\""
                + meteringKey
                + "\",\"payment_tx_hash\":\""
                + paymentTxHash
                + "\",\"quote_id\":\""
                + sessionId
                + "\"}")
        : "VPN session create body mismatch";
    assert "GET".equals(executor.requests().get(1).method()) : "VPN session lookup method mismatch";
    assert executor.requests().get(1).uri().toString()
        .equals("https://torii.example/v1/vpn/sessions/" + sessionId)
        : "VPN session lookup URI mismatch";
    assert "DELETE".equals(executor.requests().get(2).method()) : "VPN delete method mismatch";
    assert readBody(executor.requests().get(3))
        .equals(
            "{\"client_voucher_hex\":\"beef\",\"lease_id_hex\":\""
                + sessionId
                + "\",\"relay_receipt_hex\":\"cafe\"}")
        : "VPN receipt submit body mismatch";
    assert executor.requests().get(4).uri().toString().equals("https://torii.example/v1/vpn/receipts")
        : "VPN receipt list URI mismatch";
  }

  private static void verifierKeyRegisterAndUpdateReturnUnsignedDrafts() throws Exception {
    final String backend = "halo2/ipa";
    final byte[] registerBytes = new byte[] {1, 2, 3};
    final byte[] updateBytes = new byte[] {10};
    final String authority = TestAccountIds.ed25519Authority(0x37);
    final VerifyingKeyRegisterRequest registerRequestBody =
        verifierKeyRegisterRequestBuilder()
            .authority(" " + authority + " ")
            .backend(backend)
            .name(" transfer_vk ")
            .publicInputsSchemaHashHex("0x" + "AA".repeat(32))
            .gasScheduleId(" halo2-default ")
            .activationHeight(10L)
            .withdrawHeight(10L)
            .commitmentHex(
                verifierKeyCommitment(backend, registerBytes).toUpperCase(Locale.ROOT))
            .verifyingKeyBytes(registerBytes)
            .status("active")
            .build();
    final VerifyingKeyUpdateRequest updateRequestBody =
        verifierKeyUpdateRequestBuilder()
            .authority(authority)
            .backend(backend)
            .name("transfer_vk")
            .version(2L)
            .commitmentHex(verifierKeyCommitment(backend, updateBytes))
            .verifyingKeyBytes(updateBytes)
            .verifyingKeyLength(1L)
            .status("withdrawn")
            .build();
    final Map<String, Object> expectedRegisterPayload =
        HttpClientTransport.buildVerifyingKeyRegisterPayload(registerRequestBody);
    final Map<String, Object> expectedUpdatePayload =
        HttpClientTransport.buildVerifyingKeyUpdatePayload(updateRequestBody);
    final byte[] registerTransactionPayload =
        verifyingKeyTransactionPayload(
            expectedRegisterPayload, VerifyingKeyDraftBinding.Operation.REGISTER);
    final byte[] updateTransactionPayload =
        verifyingKeyTransactionPayload(
            expectedUpdatePayload, VerifyingKeyDraftBinding.Operation.UPDATE);
    final QueueResponseExecutor executor =
        new QueueResponseExecutor(
            List.of(
                new QueuedResponse(200, verifyingKeyDraftJson(registerTransactionPayload)),
                new QueuedResponse(200, verifyingKeyDraftJson(updateTransactionPayload))));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setLocalSigningContext(new LocalSigningContext(VERIFYING_KEY_CHAIN_ID))
                .setBaseUri(URI.create("https://torii.example/api"))
                .build());

    final VerifyingKeyTransactionDraft registerResponse =
        transport.registerVerifyingKey(registerRequestBody).join();
    final VerifyingKeyTransactionDraft updateResponse =
        transport.updateVerifyingKey(updateRequestBody).join();

    assert !registerResponse.submitted() : "VK register draft must not be submitted";
    assert java.util.Arrays.equals(
            registerTransactionPayload, registerResponse.transactionPayloadBytes())
        : "VK register transaction payload mismatch";
    assert java.util.Arrays.equals(
            IrohaHash.prehash(registerTransactionPayload), registerResponse.signingMessageBytes())
        : "VK register signing message mismatch";
    assert !updateResponse.submitted() : "VK update draft must not be submitted";
    assert java.util.Arrays.equals(
            updateTransactionPayload, updateResponse.transactionPayloadBytes())
        : "VK update transaction payload mismatch";
    assert java.util.Arrays.equals(
            IrohaHash.prehash(updateTransactionPayload), updateResponse.signingMessageBytes())
        : "VK update signing message mismatch";
    assert executor.requests().size() == 2 : "VK request count mismatch";

    final TransportRequest registerRequest = executor.requests().get(0);
    assert "POST".equals(registerRequest.method()) : "VK register must use POST";
    assert registerRequest.uri().toString().equals("https://torii.example/api/v1/zk/vk/register")
        : "VK register URI mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> registerPayload =
        (Map<String, Object>) JsonParser.parse(readBody(registerRequest));
    assert authority.equals(registerPayload.get("authority")) : "VK register authority mismatch";
    assert !registerPayload.containsKey("private_key")
        : "VK register request must not contain private signing material";
    assert backend.equals(registerPayload.get("backend")) : "VK register backend mismatch";
    assert "transfer_vk".equals(registerPayload.get("name")) : "VK register name mismatch";
    assert Long.valueOf(1L).equals(((Number) registerPayload.get("version")).longValue())
        : "VK register version mismatch";
    assert "transfer-v1".equals(registerPayload.get("circuit_id")) : "VK register circuit mismatch";
    assert "aa".repeat(32).equals(registerPayload.get("public_inputs_schema_hash_hex"))
        : "VK register schema hash mismatch";
    assert "halo2-default".equals(registerPayload.get("gas_schedule_id"))
        : "VK register gas schedule mismatch";
    assert Long.valueOf(10L).equals(((Number) registerPayload.get("activation_height")).longValue())
        : "VK register activation height mismatch";
    assert Long.valueOf(10L).equals(((Number) registerPayload.get("withdraw_height")).longValue())
        : "VK register withdraw height mismatch";
    assert verifierKeyCommitment(backend, registerBytes).equals(registerPayload.get("commitment_hex"))
        : "VK register commitment mismatch";
    assert Base64.getEncoder().encodeToString(registerBytes).equals(registerPayload.get("vk_bytes"))
        : "VK register inline bytes mismatch";
    assert Long.valueOf(3L).equals(((Number) registerPayload.get("vk_len")).longValue())
        : "VK register vk_len mismatch";
    assert "Active".equals(registerPayload.get("status")) : "VK register status mismatch";

    final TransportRequest updateRequest = executor.requests().get(1);
    assert "POST".equals(updateRequest.method()) : "VK update must use POST";
    assert updateRequest.uri().toString().equals("https://torii.example/api/v1/zk/vk/update")
        : "VK update URI mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> updatePayload =
        (Map<String, Object>) JsonParser.parse(readBody(updateRequest));
    assert authority.equals(updatePayload.get("authority")) : "VK update authority mismatch";
    assert !updatePayload.containsKey("private_key")
        : "VK update request must not contain private signing material";
    assert backend.equals(updatePayload.get("backend")) : "VK update backend mismatch";
    assert "transfer_vk".equals(updatePayload.get("name")) : "VK update name mismatch";
    assert Long.valueOf(2L).equals(((Number) updatePayload.get("version")).longValue())
        : "VK update version mismatch";
    assert "transfer-v1".equals(updatePayload.get("circuit_id")) : "VK update circuit mismatch";
    assert "aa".repeat(32).equals(updatePayload.get("public_inputs_schema_hash_hex"))
        : "VK update schema hash mismatch";
    assert !updatePayload.containsKey("gas_schedule_id")
        : "VK update gas_schedule_id should be optional";
    assert verifierKeyCommitment(backend, updateBytes).equals(updatePayload.get("commitment_hex"))
        : "VK update commitment mismatch";
    assert Base64.getEncoder().encodeToString(updateBytes).equals(updatePayload.get("vk_bytes"))
        : "VK update inline bytes mismatch";
    assert Long.valueOf(1L).equals(((Number) updatePayload.get("vk_len")).longValue())
        : "VK update vk_len mismatch";
    assert "Withdrawn".equals(updatePayload.get("status")) : "VK update status mismatch";
  }

  private static void verifierKeyRequestsRejectMalformedInputsBeforeRequest() throws Exception {
    final String backend = "halo2/ipa";
    final byte[] bytes = new byte[] {1, 2, 3};
    final String commitment = verifierKeyCommitment(backend, bytes);
    final CapturingExecutor executor = new CapturingExecutor();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setLocalSigningContext(new LocalSigningContext(VERIFYING_KEY_CHAIN_ID))
                .setBaseUri(URI.create("https://torii.example/api"))
                .build());

    expectVerifierReject(
        () -> transport.registerVerifyingKey(verifierKeyRegisterRequestBuilder().backend("mock/dev").build()),
        executor,
        "VK register must reject a backend outside the verifier registry");
    expectVerifierReject(
        () -> transport.registerVerifyingKey(verifierKeyRegisterRequestBuilder().authority(" ").build()),
        executor,
        "VK register must reject blank authority");
    expectVerifierReject(
        () -> transport.registerVerifyingKey(verifierKeyRegisterRequestBuilder().name("scope:vk").build()),
        executor,
        "VK register must reject names with separators");
    expectVerifierReject(
        () -> transport.registerVerifyingKey(verifierKeyRegisterRequestBuilder().version(0L).build()),
        executor,
        "VK register must reject version zero");
    expectVerifierReject(
        () ->
            transport.registerVerifyingKey(
                verifierKeyRegisterRequestBuilder().publicInputsSchemaHashHex("abc").build()),
        executor,
        "VK register must reject malformed schema hashes");
    expectVerifierReject(
        () -> transport.registerVerifyingKey(verifierKeyRegisterRequestBuilder().gasScheduleId(" ").build()),
        executor,
        "VK register must reject blank gas schedules");
    expectVerifierReject(
        () ->
            transport.registerVerifyingKey(
                verifierKeyRegisterRequestBuilder().verifyingKeyBytes(new byte[0]).build()),
        executor,
        "VK register must reject empty inline verifier bytes");
    expectVerifierReject(
        () ->
            transport.registerVerifyingKey(
                verifierKeyRegisterRequestBuilder()
                    .verifyingKeyBytes(bytes)
                    .verifyingKeyLength(2L)
                    .build()),
        executor,
        "VK register must reject inline verifier length drift");
    expectVerifierReject(
        () ->
            transport.registerVerifyingKey(
                verifierKeyRegisterRequestBuilder()
                    .backend(backend)
                    .verifyingKeyBytes(bytes)
                    .commitmentHex("00".repeat(32))
                    .build()),
        executor,
        "VK register must reject mismatched commitments");
    expectVerifierReject(
        () ->
            transport.updateVerifyingKey(
                verifierKeyUpdateRequestBuilder().activationHeight(8L).withdrawHeight(7L).build()),
        executor,
        "VK update must reject withdrawn-before-active heights");
    expectVerifierReject(
        () -> transport.updateVerifyingKey(verifierKeyUpdateRequestBuilder().status("retired").build()),
        executor,
        "VK update must reject unknown status values");
    expectVerifierReject(
        () ->
            transport.registerVerifyingKey(
                verifierKeyRegisterRequestBuilder()
                    .verifyingKeyBytes(null)
                    .verifyingKeyLength(3L)
                    .commitmentHex(null)
                    .build()),
        executor,
        "VK register must reject length-only verifier material");
    expectVerifierReject(
        () ->
            transport.registerVerifyingKey(
                verifierKeyRegisterRequestBuilder()
                    .backend(backend)
                    .verifyingKeyBytes(bytes)
                    .commitmentHex(commitment)
                    .maxProofBytes(4_294_967_296L)
                    .build()),
        executor,
        "VK register must reject u32 overflow proof limits");
  }

  private static void verifierKeyDraftCanonicalInstructionUsesU8StatusDiscriminant()
      throws Exception {
    final Map<String, Object> fixture =
        loadSharedFixture("fixtures/zk/verifying_key_record_v1.json");
    final Map<String, Object> request = object(fixture, "request");
    final InstructionBox.WirePayload payload =
        (InstructionBox.WirePayload)
            VerifyingKeyDraftBinding.expectedInstruction(
                    request, VerifyingKeyDraftBinding.Operation.REGISTER)
                .payload();
    final byte[] bytes = payload.payloadBytes();
    final String actualHex = hex(bytes);
    final Map<String, Object> backendTagFrame = object(fixture, "backend_tag_frame");
    final Map<String, Object> statusBoundary = object(fixture, "status_boundary");
    final int backendTagOffset = number(backendTagFrame, "offset").intValue();
    final String backendTagHex = string(backendTagFrame, "hex");
    final int statusOffset = number(statusBoundary, "offset").intValue();
    final String statusHex = string(statusBoundary, "hex");

    assert string(fixture, "expected_inner_frame_hex").equals(actualHex)
        : "canonical VK instruction mismatch: " + actualHex;
    assert bytes.length == number(fixture, "expected_inner_frame_bytes").intValue()
        : "canonical VK instruction length mismatch: " + bytes.length;
    assert backendTagHex.equals(
            hex(
                Arrays.copyOfRange(
                    bytes, backendTagOffset, backendTagOffset + backendTagHex.length() / 2)))
        : "backend tag must remain a four-byte u32 field at the canonical offset";
    assert statusHex.equals(
            hex(
                Arrays.copyOfRange(
                    bytes, statusOffset, statusOffset + statusHex.length() / 2)))
        : "absent inline key must end immediately before the one-byte status field";
  }

  private static void verifierKeyDraftParserRejectsNonExactOrTamperedResponses() throws Exception {
    final Map<String, Object> request =
        HttpClientTransport.buildVerifyingKeyRegisterPayload(
            verifierKeyRegisterRequestBuilder().build());
    final byte[] transactionPayload =
        verifyingKeyTransactionPayload(
            request, VerifyingKeyDraftBinding.Operation.REGISTER);
    final String valid = verifyingKeyDraftJson(transactionPayload);
    final VerifyingKeyTransactionDraft parsed =
        VerifyingKeyTransactionDraft.parseRegister(
            valid.getBytes(StandardCharsets.UTF_8), VERIFYING_KEY_CHAIN_ID, request);
    assert !parsed.submitted() : "parsed VK draft must not be submitted";
    assert java.util.Arrays.equals(transactionPayload, parsed.transactionPayloadBytes())
        : "parsed VK draft transaction payload mismatch";

    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                valid
                    .replace(
                        "\"submitted\":false",
                        "\"submitted\":false,\"retired_private_key\":\"secret\"")
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        "VK draft parser must reject retired or unknown fields");
    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                valid
                    .replace("\"submitted\":false", "\"submitted\":true")
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        "VK draft parser must reject submitted responses");
    final String transactionPayloadB64 =
        Base64.getEncoder().encodeToString(transactionPayload);
    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                valid
                    .replace(transactionPayloadB64, transactionPayloadB64 + "=")
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        "VK draft parser must reject non-canonical base64");
    final String signingMessageB64 =
        Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionPayload));
    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                valid
                    .replace(
                        signingMessageB64,
                        Base64.getEncoder().encodeToString(new byte[31]))
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        "VK draft parser must require a 32-byte signing message");
    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                valid
                    .replace(
                        signingMessageB64,
                        Base64.getEncoder().encodeToString(new byte[32]))
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        "VK draft parser must reject a signing-message substitution");
    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                verifyingKeyDraftJson(new byte[] {1, 2, 3, 4})
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        "VK draft parser must reject a non-Norito transaction payload");
    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                valid
                    .replace("\"transaction_payload_b64\":", "\"payload_b64\":")
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        "VK draft parser must reject a missing transaction payload");

    final HttpClientTransport wrongStatusTransport =
        HttpClientTransport.withExecutor(
            new StubResponseExecutor(202, valid.getBytes(StandardCharsets.UTF_8)),
            ClientConfig.builder()
                .setLocalSigningContext(new LocalSigningContext(VERIFYING_KEY_CHAIN_ID))
                .setBaseUri(URI.create("https://torii.example"))
                .build());
    try {
      final byte[] verifyingKeyBytes = new byte[] {9};
      wrongStatusTransport
          .registerVerifyingKey(
              verifierKeyRegisterRequestBuilder()
                  .verifyingKeyBytes(verifyingKeyBytes)
                  .commitmentHex(verifierKeyCommitment("halo2/ipa", verifyingKeyBytes))
                  .build())
          .join();
      throw new AssertionError("VK draft route must reject HTTP 202");
    } catch (final java.util.concurrent.CompletionException error) {
      assert error.getCause() != null && error.getCause().getMessage().contains("status 202")
          : "VK draft wrong-status rejection must identify HTTP 202";
    }
  }

  private static void verifierKeyDraftRejectsSemanticSubstitutionBeforeSigning()
      throws Exception {
    final Map<String, Object> request =
        HttpClientTransport.buildVerifyingKeyRegisterPayload(
            verifierKeyRegisterRequestBuilder()
                .verifyingKeyBytes(new byte[] {1, 2, 3})
                .build());
    final InstructionBox expectedInstruction =
        VerifyingKeyDraftBinding.expectedInstruction(
            request, VerifyingKeyDraftBinding.Operation.REGISTER);

    expectVerifierDraftReject(
        verifyingKeyTransactionPayload(
            request, VerifyingKeyDraftBinding.Operation.UPDATE),
        request,
        "VK draft must reject register/update substitution");
    expectVerifierDraftReject(
        verifyingKeyTransactionPayload(
            request,
            VerifyingKeyDraftBinding.Operation.REGISTER,
            VERIFYING_KEY_CHAIN_ID,
            (String) request.get("authority"),
            List.of(expectedInstruction, expectedInstruction)),
        request,
        "VK draft must reject extra instructions");
    expectVerifierDraftReject(
        verifyingKeyTransactionPayload(
            request,
            VerifyingKeyDraftBinding.Operation.REGISTER,
            "other-chain",
            (String) request.get("authority"),
            null),
        request,
        "VK draft must reject another chain");
    expectVerifierDraftReject(
        verifyingKeyTransactionPayload(
            request,
            VerifyingKeyDraftBinding.Operation.REGISTER,
            VERIFYING_KEY_CHAIN_ID,
            TestAccountIds.ed25519Authority(0x59),
            null),
        request,
        "VK draft must reject another authority");

    final Map<String, Object> changedRecord = new LinkedHashMap<>(request);
    changedRecord.put("curve", "pasta");
    expectVerifierDraftReject(
        verifyingKeyTransactionPayload(
            changedRecord, VerifyingKeyDraftBinding.Operation.REGISTER),
        request,
        "VK draft must reject a record-field substitution");

    final byte[] canonical =
        verifyingKeyTransactionPayload(
            request, VerifyingKeyDraftBinding.Operation.REGISTER);
    if ((canonical[0] & 0x80) != 0) {
      throw new AssertionError("fixture requires a one-byte first field length");
    }
    final byte[] noncanonical = new byte[canonical.length + 1];
    noncanonical[0] = (byte) (canonical[0] | 0x80);
    noncanonical[1] = 0;
    System.arraycopy(canonical, 1, noncanonical, 2, canonical.length - 1);
    expectVerifierDraftReject(
        noncanonical, request, "VK draft must reject non-canonical Norito");
  }

  private static void verifierKeyDraftRequiresLocalSigningContextBeforeRequest() {
    final CapturingExecutor executor = new CapturingExecutor();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build());

    expectIllegalState(
        () -> transport.registerVerifyingKey(verifierKeyRegisterRequestBuilder().build()),
        "VK register must require a local signing context");
    expectIllegalState(
        () -> transport.updateVerifyingKey(verifierKeyUpdateRequestBuilder().build()),
        "VK update must require a local signing context");
    assert executor.lastRequest == null
        : "missing local signing context must fail before network I/O";
  }

  private static void callContractRequestParsesResponse() {
    final String contractAddress =
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
    final String signingMessageB64 = Base64.getEncoder().encodeToString(new byte[32]);
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            200,
            ("{"
                    + "\"ok\":true,"
                    + "\"submitted\":false,"
                    + "\"dataspace\":\"router\","
                    + "\"code_hash_hex\":\""
                    + "44".repeat(32)
                    + "\","
                    + "\"abi_hash_hex\":\""
                    + "55".repeat(32)
                    + "\","
                    + "\"creation_time_ms\":1712345678901,"
                    + "\"contract_address\":\""
                    + contractAddress
                    + "\","
                    + "\"entrypoint\":\"contribute\","
                    + "\"transaction_ttl_ms\":60000,"
                    + "\"entrypoint_hash_hex\":\""
                    + "77".repeat(32)
                    + "\","
                    + "\"transaction_scaffold_b64\":\"AQID\","
                    + "\"signed_transaction_b64\":\"AQID\","
                    + "\"signing_message_b64\":\""
                    + signingMessageB64
                    + "\","
                    + "\"operation_receipt\":{"
                    + "\"operation_kind\":\"contract_call\","
                    + "\"status\":\"pending_signature\","
                    + "\"transport\":\"torii\","
                    + "\"dataspace\":\"router\","
                    + "\"contract_alias\":\"router::universal\","
                    + "\"contract_address\":\""
                    + contractAddress
                    + "\","
                    + "\"code_hash_hex\":\""
                    + "44".repeat(32)
                    + "\","
                    + "\"abi_hash_hex\":\""
                    + "55".repeat(32)
                    + "\","
                    + "\"entrypoint\":\"contribute\","
                    + "\"entrypoint_hash_hex\":\""
                    + "77".repeat(32)
                    + "\","
                    + "\"gas_limit\":5000,\"gas_used\":17,"
                    + "\"fee_payment\":{\"payer\":\"authority\","
                    + "\"value\":{\"charge_limits\":[],\"gas_limit\":5000}},"
                    + "\"payload_digest_hex\":\""
                    + "88".repeat(32)
                    + "\"}}")
                .getBytes(StandardCharsets.UTF_8),
            "ok");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());
    final Map<String, Object> contractPayload = new LinkedHashMap<>();
    contractPayload.put("buyer", "alice");
    contractPayload.put("payment_amount", 1L);

    final ContractCallResponse response =
        transport
            .prepareContractCall(
                "alice",
                feePayment(5000L),
                null,
                "router::universal",
                "contribute",
                contractPayload)
            .join();

    assert response.ok() : "Call response should be successful";
    assert !response.submitted() : "Call draft must not be submitted";
    assert "router".equals(response.dataspace()) : "Call dataspace mismatch";
    assert "contribute".equals(response.entrypoint()) : "Entrypoint mismatch";
    assert Long.valueOf(60_000L).equals(response.transactionTtlMs())
        : "transaction_ttl_ms mismatch";
    assert "77".repeat(32).equals(response.entrypointHashHex())
        : "entrypoint_hash_hex mismatch";
    assert response.pipelineStatus() == null : "draft must not include pipeline status";
    assert "contract_call".equals(response.operationReceipt().operationKind())
        : "operation kind mismatch";
    assert Long.valueOf(5_000L).equals(response.operationReceipt().gasLimit())
        : "operation gas limit mismatch";
    assert "88".repeat(32).equals(response.operationReceipt().payloadDigestHex())
        : "payload digest mismatch";
    assert "AQID".equals(response.transactionScaffoldB64())
        : "transaction_scaffold_b64 mismatch";
    assert "AQID".equals(response.signedTransactionB64()) : "signed_transaction_b64 mismatch";
    assert signingMessageB64.equals(response.signingMessageB64())
        : "signing_message_b64 mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Contract call request must be captured";
    assert request.uri().toString().equals("https://torii.example/api/v1/contracts/call")
        : "Call URI mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> payload =
        (Map<String, Object>) JsonParser.parse(readBody(request));
    assert "alice".equals(payload.get("authority")) : "Call authority mismatch";
    assert !payload.containsKey("private_key")
        : "contract call preparation must not contain private signing material";
    assert !payload.containsKey("gas_limit") : "legacy gas_limit must be absent";
    assert "router::universal".equals(payload.get("contract_alias"))
        : "contract_alias mismatch";
    assert !payload.containsKey("contract_address") : "contract_address should be absent";
    assert "contribute".equals(payload.get("entrypoint")) : "Call entrypoint mismatch";
    assert !payload.containsKey("gas_asset_id") : "legacy gas_asset_id must be absent";
    @SuppressWarnings("unchecked")
    final Map<String, Object> feeValue =
        (Map<String, Object>) ((Map<String, Object>) payload.get("fee_payment")).get("value");
    assert ((Number) feeValue.get("gas_limit")).longValue() == 5000L
        : "fee_payment gas limit mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> requestPayload = (Map<String, Object>) payload.get("payload");
    assert "alice".equals(requestPayload.get("buyer")) : "Nested buyer mismatch";
    assert Long.valueOf(1L).equals(((Number) requestPayload.get("payment_amount")).longValue())
        : "Nested payment_amount mismatch";
  }

  private static void contractCallBoundaryConsumesSharedRustArgumentRecordFixture()
      throws Exception {
    final Map<String, Object> fixture =
        loadSharedFixture("fixtures/kotodama/entrypoint_argument_record_v1.json");
    assert "EntrypointArgumentRecordV1".equals(fixture.get("codec"))
        : "Argument-record fixture codec mismatch";
    assert "ivm::encode_argument_record_from_json".equals(fixture.get("generator"))
        : "Argument-record fixture generator mismatch";
    final Map<String, Object> schema = object(fixture, "entrypoint_argument_schema_v1");
    assert string(schema, "schema_hash_hex").matches("[0-9a-f]{64}")
        : "Argument schema hash must be canonical lowercase hex";
    final Map<String, Object> record = object(fixture, "entrypoint_argument_record_v1");
    assert string(record, "norito_hex").matches("(?:[0-9a-f]{2})+")
        : "Argument record must be canonical lowercase hex bytes";

    final Map<String, Object> boundary = object(fixture, "torii_boundary");
    final org.hyperledger.iroha.android.model.FeePaymentIntent boundaryFeePayment =
        FeePaymentJson.parse(boundary.get("fee_payment"), "torii_boundary.fee_payment");
    final Map<String, Object> request =
        HttpClientTransport.buildContractCallDraftPayload(
            string(boundary, "authority"),
            boundaryFeePayment,
            null,
            string(boundary, "contract_alias"),
            string(boundary, "entrypoint"),
            boundary.get("payload"));

    assert string(boundary, "authority").equals(request.get("authority"))
        : "Shared call authority mismatch";
    assert !request.containsKey("private_key")
        : "Shared contract-call draft must not contain private signing material";
    assert string(boundary, "contract_alias").equals(request.get("contract_alias"))
        : "Shared call alias mismatch";
    assert !request.containsKey("contract_address") : "Shared call must select only the alias";
    assert string(boundary, "entrypoint").equals(request.get("entrypoint"))
        : "Shared call entrypoint mismatch";
    assert boundary.get("payload").equals(request.get("payload"))
        : "Shared call payload mismatch";
    assert boundaryFeePayment.toJsonMap().equals(request.get("fee_payment"))
        : "Shared call fee payment mismatch";
    assert !request.containsKey("argument_record")
        : "Java must leave canonical argument-record encoding to Rust";
    assert !request.containsKey("argument_record_norito_hex")
        : "Java must not expose a parallel argument-record encoder";
  }

  private static void proposeMultisigRequestParsesResponse() {
    final byte[] instructionBytes = new byte[] {1, 2, 3, 4};
    final String proposalId = "aa".repeat(32);
    final String multisigAccountId = TestAccountIds.ed25519Authority(0x37);
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            200,
            ("{"
                    + "\"ok\":true,"
                    + "\"resolved_multisig_account_id\":\""
                    + multisigAccountId
                    + "\","
                    + "\"submitted\":false,"
                    + "\"proposal_id\":\""
                    + proposalId
                    + "\","
                    + "\"instructions_hash\":\""
                    + proposalId
                    + "\","
                    + "\"tx_hash_hex\":null,"
                    + "\"executed_tx_hash_hex\":null,"
                    + "\"creation_time_ms\":123,"
                    + "\"signing_message_b64\":\"AQID\"}")
                .getBytes(StandardCharsets.UTF_8),
            "ok");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final MultisigResponse response =
        transport
            .proposeMultisig(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instructionBytes)
                    .setPublicKeyHex(
                        "0X" + VALID_ED25519_PUBLIC_KEY_HEX.toUpperCase(Locale.ROOT))
                    .setCreationTimeMs(123L)
                    .setMemo("QR invoice 42")
                    .setValidationFeePolicyVersion(7L)
                    .setValidationFeePolicyHash("AB".repeat(32))
                    .setValidationFeeInstructionIndex(1L)
                    .setValidationFeeTransferEntryIndex(2L)
                    .build())
            .join();

    assert response.ok() : "Multisig response should be successful";
    assert multisigAccountId.equals(response.resolvedMultisigAccountId())
        : "resolved multisig account mismatch";
    assert Boolean.FALSE.equals(response.submitted()) : "submitted mismatch";
    assert proposalId.equals(response.instructionsHash()) : "instructions_hash mismatch";
    assert "AQID".equals(response.signingMessageB64()) : "signing_message_b64 mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Multisig request must be captured";
    assert request.uri().toString().equals("https://torii.example/api/v1/multisig/propose")
        : "Multisig URI mismatch";
    assert "application/json".equals(request.headers().get("Content-Type").get(0))
        : "Content-Type mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> payload =
        (Map<String, Object>) JsonParser.parse(readBody(request));
    assert "cbdc@banka".equals(payload.get("multisig_account_alias"))
        : "multisig_account_alias mismatch";
    assert "alice".equals(payload.get("signer_account_id")) : "signer_account_id mismatch";
    assert VALID_ED25519_PUBLIC_KEY_HEX.equals(payload.get("public_key_hex"))
        : "public_key_hex mismatch";
    assert !payload.containsKey("fee_sponsor") : "legacy fee_sponsor must be absent";
    assert payload.containsKey("fee_payment") : "typed fee payment must be present";
    assert "QR invoice 42".equals(payload.get("memo")) : "memo mismatch";
    assert "7".equals(payload.get("validation_fee_policy_version"))
        : "validation_fee_policy_version mismatch";
    assert "ab".repeat(32).equals(payload.get("validation_fee_policy_hash"))
        : "validation_fee_policy_hash mismatch";
    assert "1".equals(payload.get("validation_fee_instruction_index"))
        : "validation_fee_instruction_index mismatch";
    assert "2".equals(payload.get("validation_fee_transfer_entry_index"))
        : "validation_fee_transfer_entry_index mismatch";
    assert Long.valueOf(123L).equals(((Number) payload.get("creation_time_ms")).longValue())
        : "creation_time_ms mismatch";
    @SuppressWarnings("unchecked")
    final List<String> instructions = (List<String>) payload.get("instructions");
    assert instructions.size() == 1 : "instructions length mismatch";
    assert Base64.getEncoder().encodeToString(instructionBytes).equals(instructions.get(0))
        : "instruction base64 mismatch";

    boolean failed = false;
    try {
      HttpClientTransport.buildMultisigProposePayload(
          MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
              .setMultisigAccountAlias("cbdc@banka")
              .setSignerAccountId("alice")
              .addInstructionBytes(new byte[0])
              .build());
    } catch (final IllegalArgumentException ex) {
      failed = true;
    }
    assert failed : "Empty instruction bytes should be rejected";
  }

  private static void proposeMultisigRejectsAdversarialRequestShapes() {
    final byte[] instruction = new byte[] {1};
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountId("aid:multisig")
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .build()),
        "ambiguous multisig selector must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .build()),
        "missing multisig selector must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setSignatureB64("not base64")
                    .build()),
        "malformed detached signature must be rejected");
    final String canonicalSignature = canonicalSignatureBase64Fixture();
    for (final String signatureB64 :
        List.of(" " + canonicalSignature, noncanonicalStandardBase64PadBitAlias(canonicalSignature))) {
      expectIllegalArgument(
          () ->
              HttpClientTransport.buildMultisigProposePayload(
                  MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                      .setMultisigAccountAlias("cbdc@banka")
                      .setSignerAccountId("alice")
                      .addInstructionBytes(instruction)
                      .setSignatureB64(signatureB64)
                      .build()),
          "noncanonical detached signature must be rejected");
    }
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setPublicKeyHex("aa")
                    .build()),
        "short detached public key must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setCreationTimeMs(-1L)
                    .build()),
        "negative creation time must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeePolicyVersion(1L)
                    .build()),
        "validation fee policy version without hash must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeePolicyHash("ab".repeat(32))
                    .build()),
        "validation fee policy hash without version must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeePolicyVersion(-1L)
                    .setValidationFeePolicyHash("ab".repeat(32))
                    .build()),
        "negative validation fee policy version must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeePolicyVersion(1L)
                    .setValidationFeePolicyHash("not-hex")
                    .build()),
        "malformed validation fee policy hash must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeeInstructionIndex(1L)
                    .build()),
        "validation fee instruction index without policy metadata must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeeTransferEntryIndex(2L)
                    .build()),
        "validation fee transfer entry index without policy metadata must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeePolicyVersion(1L)
                    .setValidationFeePolicyHash("ab".repeat(32))
                    .setValidationFeeTransferEntryIndex(2L)
                    .build()),
        "validation fee transfer entry index without instruction index must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeePolicyVersion(1L)
                    .setValidationFeePolicyHash("ab".repeat(32))
                    .setValidationFeeInstructionIndex(-1L)
                    .build()),
        "negative validation fee instruction index must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setValidationFeePolicyVersion(1L)
                    .setValidationFeePolicyHash("ab".repeat(32))
                    .setValidationFeeInstructionIndex(1L)
                    .setValidationFeeTransferEntryIndex(-2L)
                    .build()),
        "negative validation fee transfer entry index must be rejected");
  }

  private static void multisigResponseParserRejectsMalformedFields() {
    final String multisigAccountId = TestAccountIds.ed25519Authority(0x37);
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":false,"
                        + "\"resolved_multisig_account_id\":\""
                        + multisigAccountId
                        + "\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "false ok response must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\""
                        + multisigAccountId
                        + " \"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "padded resolved multisig account id must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\"multisig\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "non-canonical resolved multisig account id must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\""
                        + multisigAccountId
                        + "\","
                        + "\"submitted\":\"false\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "string submitted flag must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\""
                        + multisigAccountId
                        + "\","
                        + "\"instructions_hash\":\"aa\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "short instructions hash must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\""
                        + multisigAccountId
                        + "\","
                        + "\"signing_message_b64\":\"not base64\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "malformed signing message must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\""
                        + multisigAccountId
                        + "\","
                        + "\"signing_message_b64\":\"\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "empty signing message must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\""
                        + multisigAccountId
                        + "\","
                        + "\"creation_time_ms\":-1}")
                    .getBytes(StandardCharsets.UTF_8)),
        "negative creation time must be rejected");
  }

  private static void callContractRejectsAmbiguousTarget() {
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new CapturingExecutor(),
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    boolean failed = false;
    try {
      transport.prepareContractCall(
          "alice",
          feePayment(5000L),
          "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
          "router::universal",
          "contribute",
          null);
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("Exactly one");
    }
    assert failed : "expected ambiguous contract target rejection";
  }

  private static void callContractRejectsInvalidEntrypointOrGas() {
    for (final String entrypoint : new String[] {"", "   "}) {
      expectIllegalArgument(
          () ->
              HttpClientTransport.buildContractCallDraftPayload(
                  "alice", feePayment(1L), null, "router::universal", entrypoint, null),
          "blank contract entrypoint must be rejected");
    }
    for (final long gasLimit : new long[] {0L, -1L}) {
      expectIllegalArgument(
          () ->
              feePayment(gasLimit),
          "non-positive contract gas limit must be rejected");
    }
  }

  private static void callContractResponseRequiresOperationReceipt() {
    expectRuntimeException(
        () ->
            ContractJsonParser.parseCallResponse(
                ("{"
                        + "\"ok\":true,\"submitted\":true,\"dataspace\":\"router\","
                        + "\"code_hash_hex\":\""
                        + "44".repeat(32)
                        + "\",\"abi_hash_hex\":\""
                        + "55".repeat(32)
                        + "\",\"creation_time_ms\":1,\"entrypoint\":\"contribute\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "contract call response must require operation_receipt");
  }

  private static void governanceContractRequestParsesResponse() {
    final String contractAddress =
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            200,
            ("{"
                    + "\"found\":true,"
                    + "\"contract_address\":\""
                    + contractAddress
                    + "\","
                    + "\"dataspace\":\"router\","
                    + "\"code_hash_hex\":\""
                    + "77".repeat(32)
                    + "\"}")
                .getBytes(StandardCharsets.UTF_8),
            "ok");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final KeyPair keyPair;
    try {
      keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    } catch (final NoSuchAlgorithmException error) {
      throw new AssertionError(error);
    }
    final GovernanceContractResponse response =
        transport
            .getGovernanceContract(
                contractAddress,
                canonicalAuth("alice", keyPair, 1_700_000_000_100L, "governance-read"))
            .join();

    assert response.found() : "Governance binding should be found";
    assert contractAddress.equals(response.contractAddress()) : "Governance contract address mismatch";
    assert "router".equals(response.dataspace()) : "Governance dataspace mismatch";
    assert "77".repeat(32).equals(response.codeHashHex()) : "Governance code hash mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Governance contract request must be captured";
    assert request.uri().toString().equals("https://torii.example/api/v1/gov/contracts/" + contractAddress)
        : "Governance contract URI mismatch";
    assert "GET".equals(request.method()) : "Governance contract must use GET";
  }

  private static void resolveAccountAliasRequestParsesResponse() {
    final String accountId = TestAccountIds.ed25519Authority(0x11);
    final String json =
        "{"
            + "\"alias\":\"alice@universal\","
            + "\"account_id\":\""
            + accountId
            + "\","
            + "\"index\":7,"
            + "\"source\":\"directory\""
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final Optional<AccountAliasResolution> response =
        transport.resolveAccountAlias("alice@universal").join();

    assert response.isPresent() : "Account alias resolution should be present";
    final AccountAliasResolution resolution = response.orElseThrow();
    assert "alice@universal".equals(resolution.alias()) : "Alias mismatch";
    assert accountId.equals(resolution.accountId()) : "Account id mismatch";
    assert BigInteger.valueOf(7L).equals(resolution.index()) : "Index mismatch";
    assert "directory".equals(resolution.source()) : "Source mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Account alias resolve request must be captured";
    assert "POST".equals(request.method()) : "Account alias resolve must use POST";
    assert request.uri().toString().equals("https://torii.example/api/v1/aliases/resolve")
        : "Account alias resolve URI mismatch";
    assert request.headers().getOrDefault("Content-Type", List.of()).contains("application/json")
        : "Account alias resolve must send JSON";
    assert readBody(request).equals("{\"alias\":\"alice@universal\"}")
        : "Account alias resolve payload mismatch";
  }

  private static void resolveRestrictedAccountAliasUsesCanonicalAuthentication()
      throws Exception {
    final String accountId = TestAccountIds.ed25519Authority(0x42);
    final String json =
        "{"
            + "\"alias\":\"merchant@private\","
            + "\"account_id\":\""
            + accountId
            + "\","
            + "\"source\":\"world_state\""
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth("alice", keyPair, 1_700_000_000_000L, "alias-resolve-nonce-1");

    final Optional<AccountAliasResolution> response =
        transport.resolveAccountAlias("merchant@private", auth).join();

    assert response.isPresent() : "Restricted alias resolution should be present";
    assert accountId.equals(response.orElseThrow().accountId())
        : "Restricted alias target mismatch";
    final TransportRequest request = executor.lastRequest();
    assert request != null : "Restricted alias request must be captured";
    assert "alice".equals(request.headers().get(CanonicalRequestSigner.HEADER_ACCOUNT).get(0))
        : "Canonical account header mismatch";
    assert "1700000000000"
        .equals(request.headers().get(CanonicalRequestSigner.HEADER_TIMESTAMP_MS).get(0))
        : "Canonical timestamp header mismatch";
    assert "alias-resolve-nonce-1"
        .equals(request.headers().get(CanonicalRequestSigner.HEADER_NONCE).get(0))
        : "Canonical nonce header mismatch";
    assertCanonicalSignature(
        request, keyPair.getPublic(), 1_700_000_000_000L, "alias-resolve-nonce-1");
  }

  static void aliasSetupPlanningIsCanonicalSignedReadOnlyAndParsesTypedPlan()
      throws Exception {
    final String authority = TestAccountIds.ed25519Authority(0x41);
    final String asset = TestAssetDefinitionIds.PRIMARY;
    final ResolvedAccountAliasV1 alias =
        new ResolvedAccountAliasV1(AccountAliasName.parse("merchant@banka.paynet"), 7L);
    final AliasQuoteGuardV1 guard =
        new AliasQuoteGuardV1(3, asset, "5", 1_700_000_100_000L);
    final AliasSetupModels.AccountAliasIntent intent =
        new AliasSetupModels.AccountAliasIntent(
            new AliasSetupModels.AliasAccountIntentV1(
                alias,
                authority,
                AliasSetupModels.AccountProvisionV1.CREATE,
                AliasSetupModels.AccountAliasRoleV1.PRIMARY));
    final AliasSetupPlanRequestV1 requestBody =
        new AliasSetupPlanRequestV1(
            Collections.singletonList(
                new EnsureAlias(
                    intent,
                    new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
                    guard)));
    final AliasSetupModels.AliasTransactionPlanBodyV1 planBody =
        new AliasSetupModels.AliasTransactionPlanBodyV1(
            1,
            authority,
            "test-chain",
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            Collections.singletonList(
                new AliasSetupModels.AliasPlanResourceV1(
                    intent,
                    AliasSetupModels.AliasPlanDispositionV1.CREATE,
                    new AliasSetupModels.AliasLeaseQuoteV1(
                        new AliasSetupModels.AccountAliasTarget(alias),
                        1,
                        "3",
                        guard,
                        1_800_000_000_000L,
                        1_800_000_100_000L,
                        1_800_000_200_000L),
                    0L)),
            Collections.singletonList(
                new AliasSetupModels.AliasFramedInstructionV1(
                    EnsureAlias.WIRE_ID, new byte[] {0x4e, 0x52, 0x54, 0x30})),
            Collections.singletonList(new AliasSetupModels.AliasAssetTotalV1(asset, "3")),
            Collections.emptyList(),
            Collections.emptyList(),
            1_700_000_100_000L);
    final AliasTransactionPlanV1 responsePlan =
        new AliasTransactionPlanV1(planBody, "03".repeat(32));
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            200,
            JsonEncoder.encode(responsePlan.toJsonMap()).getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());
    final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth auth =
        canonicalAuth(authority, keyPair, 1_700_000_000_000L, "alias-plan-nonce-1");

    final AliasTransactionPlanV1 plan = transport.planAliasSetup(requestBody, auth).join();

    assert authority.equals(plan.body().authority()) : "Alias plan authority mismatch";
    assert plan.body().resources().get(0).disposition()
        == AliasSetupModels.AliasPlanDispositionV1.CREATE;
    final TransportRequest request = executor.lastRequest();
    assert request != null : "Alias setup plan request must be captured";
    assert "POST".equals(request.method()) : "Alias setup planning must use POST";
    assert "https://torii.example/api/v1/aliases/setup/plan".equals(request.uri().toString())
        : "Alias setup planning must use the read-only planner route";
    assert authority.equals(
        request.headers().get(CanonicalRequestSigner.HEADER_ACCOUNT).get(0));
    @SuppressWarnings("unchecked")
    final Map<String, Object> sent = (Map<String, Object>) JsonParser.parse(readBody(request));
    assert Long.valueOf(1L).equals(sent.get("schema_version"));
    assert ((List<?>) sent.get("intents")).size() == 1;
    assert !sent.containsKey("private_key");
    assert !sent.containsKey("payment_proof");
    assertCanonicalSignature(
        request, keyPair.getPublic(), 1_700_000_000_000L, "alias-plan-nonce-1");
  }

  private static void resolveAccountAliasAllowsNotFound() {
    final StubResponseExecutor executor = new StubResponseExecutor(404, new byte[0], "not found");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Optional<AccountAliasResolution> response =
        transport.resolveAccountAlias("missing@universal").join();
    assert response.isEmpty() : "404 account alias resolution should return Optional.empty";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Account alias resolve request must be captured";
    assert request.uri().toString().equals("https://torii.example/v1/aliases/resolve")
        : "Account alias resolve URI mismatch";
    assert readBody(request).equals("{\"alias\":\"missing@universal\"}")
        : "Account alias resolve payload mismatch";
  }

  private static void resolveAccountAliasRequestParsesResponseWithoutIndex() {
    final String accountId = TestAccountIds.ed25519Authority(0x13);
    final String json =
        "{"
            + "\"alias\":\"banking@centralbank.universal\","
            + "\"account_id\":\""
            + accountId
            + "\","
            + "\"source\":\"rekey_record\""
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final Optional<AccountAliasResolution> response =
        transport.resolveAccountAlias("banking@centralbank.universal").join();

    assert response.isPresent() : "Account alias resolution should be present";
    final AccountAliasResolution resolution = response.orElseThrow();
    assert "banking@centralbank.universal".equals(resolution.alias()) : "Alias mismatch";
    assert accountId.equals(resolution.accountId()) : "Account id mismatch";
    assert resolution.index() == null : "Index should be absent when the payload omits it";
    assert "rekey_record".equals(resolution.source()) : "Source mismatch";
  }

  private static void resolveAccountAliasRejectsNonIntegerIndex() {
    final String accountId = TestAccountIds.ed25519Authority(0x14);
    final String json =
        "{"
            + "\"alias\":\"alice@universal\","
            + "\"account_id\":\""
            + accountId
            + "\","
            + "\"index\":3.5"
            + "}";
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, json.getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final CompletableFuture<Optional<AccountAliasResolution>> future =
        transport.resolveAccountAlias("alice@universal");
    boolean threw = false;
    try {
      future.join();
    } catch (final java.util.concurrent.CompletionException ex) {
      threw = true;
    }
    assert threw : "Non-integer index must complete exceptionally";
    assert future.isCompletedExceptionally()
        : "Future must be completed exceptionally for a non-integer index";
  }

  private static void accountAliasParserRejectsNonExactResponseFields() {
    final String accountId = TestAccountIds.ed25519Authority(0x15);
    final String canonical =
        "{"
            + "\"alias\":\"alice@universal\","
            + "\"account_id\":\""
            + accountId
            + "\","
            + "\"index\":7,"
            + "\"source\":\"directory\""
            + "}";
    final String[][] cases = {
      {
        "account alias resolution.alias",
        canonical.replace("\"alias\":\"alice@universal\"", "\"alias\":\" alice@universal\"")
      },
      {
        "account alias resolution.account_id",
        canonical.replace(
            "\"account_id\":\"" + accountId + "\"",
            "\"account_id\":\"" + accountId + " \"")
      },
      {
        "account alias resolution.source",
        canonical.replace("\"source\":\"directory\"", "\"source\":\" directory\"")
      }
    };
    for (final String[] testCase : cases) {
      assertRamLfeParseFails(
          testCase[0],
          () -> AccountAliasJsonParser.parseResolution(testCase[1].getBytes(StandardCharsets.UTF_8)));
    }
  }

  private static void resolveAccountAliasFailsOnMalformedJson() {
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, "not json".getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final CompletableFuture<Optional<AccountAliasResolution>> future =
        transport.resolveAccountAlias("alice@universal");
    boolean threw = false;
    try {
      future.join();
    } catch (final java.util.concurrent.CompletionException ex) {
      threw = true;
    }
    assert threw : "Malformed JSON must complete exceptionally";
    assert future.isCompletedExceptionally()
        : "Future must be completed exceptionally for malformed JSON";
  }

  private static void identifierNormalizationCanonicalizesInputs() {
    assert "+15551234567".equals(
            IdentifierNormalization.PHONE_E164.normalize(" +1 (555) 123-4567 ", "phone"))
        : "Phone normalization mismatch";
    assert "alice.example@example.com".equals(
            IdentifierNormalization.EMAIL_ADDRESS.normalize(
                " Alice.Example@Example.COM ", "email"))
        : "Email normalization mismatch";
    assert "GB82WEST1234".equals(
            IdentifierNormalization.ACCOUNT_NUMBER.normalize(" gb82-west-1234 ", "account"))
        : "Account normalization mismatch";
    assert "\u0001\u0002".equals(
            IdentifierNormalization.EXACT.normalize("\u0001\u0002", "input"))
        : "Exact normalization must preserve non-whitespace control bytes";
    expectIllegalArgument(
        () -> IdentifierNormalization.EXACT.normalize(" \t\n", "input"),
        "Exact normalization must reject whitespace-only input");
  }

  private static void identifierBfvEnvelopeBuilderProducesDeterministicCiphertext() {
    final IdentifierPolicySummary policy =
        new IdentifierPolicySummary(
            "string#retail",
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            true,
            IdentifierNormalization.EXACT,
            "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
            "bfv-affine-sha3-256-v1",
            "bfv-v1",
            null,
            new IdentifierBfvPublicParameters(
                new IdentifierBfvPublicParameters.Parameters(8L, 257L, 16_842_752L, 12),
                new IdentifierBfvPublicParameters.PublicKey(
                    List.of(11_472_226L, 15_791_131L, 10_301_391L, 6_321_610L, 502_045L, 1_948_157L, 5_332_249L, 12_641_494L),
                    List.of(3_503_246L, 2_379_264L, 12_091_019L, 30_169L, 15_804_162L, 8_155_629L, 2_418_997L, 3_003_107L)),
                3),
            null);
    final byte[] seed = hexToBytes("00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF");
    final String expected =
        "4e52543000001042e5b988077612440e4cd45673596b00b004000000000000dd479e32bf99dbd000a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002dac6c00000000000800000000000000440e92000000000008000000000000005b2600000000000008000000000000004a681100000000000800000000000000bc3d2300000000000800000000000000413e85000000000008000000000000005619f900000000000800000000000000bd73fc0000000000880000000000000008000000000000000800000000000000ee894300000000000800000000000000dd22b000000000000800000000000000fe7c50000000000008000000000000001639a3000000000008000000000000006a969b00000000000800000000000000ddd4410000000000080000000000000051076600000000000800000000000000ef14ae00000000002001000000000000880000000000000008000000000000000800000000000000d86c690000000000080000000000000093070e0000000000080000000000000033067500000000000800000000000000ddc5190000000000080000000000000062ea230000000000080000000000000056f00a00000000000800000000000000ab51d400000000000800000000000000e945790000000000880000000000000008000000000000000800000000000000f2204400000000000800000000000000c9ecd2000000000008000000000000001dfc5b00000000000800000000000000d16d660000000000080000000000000016ec0e000000000008000000000000003def83000000000008000000000000006e7ff900000000000800000000000000c1fabb00000000002001000000000000880000000000000008000000000000000800000000000000c8c6eb00000000000800000000000000c9c14800000000000800000000000000f01f8700000000000800000000000000aed22c000000000008000000000000006122990000000000080000000000000036ad8c00000000000800000000000000d1429300000000000800000000000000891f6d0000000000880000000000000008000000000000000800000000000000417eed00000000000800000000000000d79c34000000000008000000000000009f322c0000000000080000000000000091fe5700000000000800000000000000533ce8000000000008000000000000005db8df00000000000800000000000000a8c313000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d654000000000008000000000000005d884400000000000800000000000000567ab50000000000080000000000000007273100000000000800000000000000ff6d0a00000000000800000000000000077466000000000008000000000000006d1d1a000000000008000000000000007050c200000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a100000000000800000000000000cbfa290000000000080000000000000057477300000000000800000000000000608f9200000000000800000000000000f5f5dd00000000000800000000000000445b3b00000000000800000000000000999e690000000000";

    assert expected.equals(policy.encryptInput("ab", seed))
        : "Deterministic BFV ciphertext mismatch";
    final RamLfeOutputOpening opening = sampleOpening("identifier_lookup_retail");
    final IdentifierResolveRequest request = policy.encryptedRequestFromInput("ab", opening, seed);
    assert "string#retail".equals(request.policyId()) : "Encrypted request policy id mismatch";
    assert expected.equals(request.encryptedInputHex())
        : "Encrypted request ciphertext mismatch";
    assert request.outputOpening() == opening : "Encrypted request must keep output opening";
  }

  private static void identifierBfvEnvelopeBuilderMatchesSharedSoracloudVectors()
      throws Exception {
    final Map<String, Object> fixture = loadSharedBfvFixture();
    assert "soracloud-bfv-identifier-envelope-v1".equals(fixture.get("vector_set"))
        : "shared BFV fixture set mismatch";
    assertBfvOperationKeyComponentVectors(object(fixture, "operation_vectors"));
    final IdentifierPolicySummary policy = identifierPolicyFromBfvFixture(object(fixture, "policy"));
    final List<Map<String, Object>> vectors = objectList(fixture, "vectors");
    final List<String> observedDigests = new ArrayList<>();

    for (final Map<String, Object> vector : vectors) {
      final String ciphertextHex =
          policy.encryptInput(
              string(vector, "input_utf8"),
              hexToBytes(string(vector, "seed_hex")));
      assert number(vector, "expected_ciphertext_bytes").intValue() == ciphertextHex.length() / 2
          : string(vector, "name") + " ciphertext byte length mismatch";
      final String digest = sha256Hex(hexToBytes(ciphertextHex));
      assert string(vector, "expected_ciphertext_sha256").equals(digest)
          : string(vector, "name") + " ciphertext digest mismatch";
      assert !observedDigests.contains(digest)
          : "fixture ciphertext digest must be unique: " + digest;
      observedDigests.add(digest);
    }
  }

  private static void sharedSoracloudBfvKeyBundleComponentVectorsAreComplete()
      throws Exception {
    assertBfvOperationKeyComponentVectors(object(loadSharedBfvFixture(), "operation_vectors"));
  }

  private static void identifierBfvEnvelopeBuilderMatchesSharedSoracloudOperationInputVectors()
      throws Exception {
    final Map<String, Object> fixture = loadSharedBfvFixture();
    final Map<String, Object> operationVectors = object(fixture, "operation_vectors");
    assert "soracloud-bfv-operation-v1".equals(operationVectors.get("vector_set"))
        : "operation vector set mismatch";
    final IdentifierPolicySummary policy =
        new IdentifierPolicySummary(
            "soracloud-operation#fixture",
            "owner",
            true,
            IdentifierNormalization.EXACT,
            "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
            "bfv-programmed-sha3-256-v1",
            "bfv-v1",
            null,
            identifierBfvParametersFromFixture(
                object(operationVectors, "public_parameters_decoded")),
            null);
    final List<String> observedDigests = new ArrayList<>();
    int checkedInputs = 0;
    for (final Map<String, Object> vector : objectList(operationVectors, "vectors")) {
      final String vectorName = string(vector, "name");
      for (final Map<String, Object> input : objectList(vector, "inputs")) {
        if (input.get("packed_slots") != null) {
          continue;
        }
        final String seedUtf8 = string(input, "seed_utf8");
        final byte[] inputBytes = hexToBytes(string(input, "input_hex"));
        final String ciphertextHex =
            policy.encryptInput(
                new String(inputBytes, StandardCharsets.UTF_8),
                seedUtf8.getBytes(StandardCharsets.UTF_8));
        assert number(input, "expected_ciphertext_bytes").intValue() == ciphertextHex.length() / 2
            : vectorName + "/" + seedUtf8 + " ciphertext byte length mismatch";
        final String digest = sha256Hex(hexToBytes(ciphertextHex));
        assert string(input, "expected_ciphertext_sha256").equals(digest)
            : vectorName + "/" + seedUtf8 + " ciphertext digest mismatch";
        assert !observedDigests.contains(digest)
            : "operation input digest must be unique: " + digest;
        observedDigests.add(digest);
        checkedInputs++;
      }
    }
    assert checkedInputs == 8 : "fixture should cover every non-packed operation input";
  }

  private static void sharedSoracloudBfvKeyBundleComponentVectorsRejectAdversarialDrift()
      throws Exception {
    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          final Map<String, Object> evaluationKey =
              object(operationVectors, "evaluation_key_bundle");
          objectList(evaluationKey, "relinearization_entries").get(0).remove("b_sha256");
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "missing relinearization component digest must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          final Map<String, Object> evaluationKey =
              object(operationVectors, "evaluation_key_bundle");
          final List<Map<String, Object>> entries =
              objectList(evaluationKey, "relinearization_entries");
          entries.get(1).put("a_sha256", string(entries.get(0), "b_sha256"));
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "duplicate BFV component digest must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          final Map<String, Object> evaluationKey =
              object(operationVectors, "evaluation_key_bundle");
          final List<Map<String, Object>> entries =
              objectList(evaluationKey, "relinearization_entries");
          entries.get(0).put("b_sha256", string(entries.get(0), "b_sha256").toLowerCase(Locale.ROOT));
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "noncanonical lowercase component digest must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          final Map<String, Object> components =
              object(objectList(operationVectors, "rotation_keys").get(0), "zero_refresh_components");
          components.put("c1_sha256", "0".repeat(64));
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "zero rotation refresh component digest must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          final Map<String, Object> bootstrap = object(operationVectors, "bootstrap_key");
          object(bootstrap, "zero_refresh_components").put("coefficient_count", 63);
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "bootstrap refresh coefficient-count drift must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          object(operationVectors, "evaluation_key_bundle").put("rotation_key_count", 99);
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "rotation key count drift must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          operationVectors.remove("full_bootstrap_material");
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "missing full-bootstrap material fixture must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          final Map<String, Object> material =
              object(operationVectors, "full_bootstrap_material");
          material.put("vk_commitment_hex", string(material, "expected_statement_digest_hex"));
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "full-bootstrap verifier commitment drift must be rejected");

    expectAssertionOrIllegalArgument(
        () -> {
          final Map<String, Object> operationVectors =
              object(loadSharedBfvFixture(), "operation_vectors");
          final Map<String, Object> material =
              object(operationVectors, "full_bootstrap_material");
          material.put(
              "expected_material_digest_hex",
              string(material, "expected_material_digest_hex").toUpperCase(Locale.ROOT));
          assertBfvOperationKeyComponentVectors(operationVectors);
        },
        "noncanonical full-bootstrap material digest must be rejected");
  }

  private static void identifierBfvEnvelopeBuilderRejectsAdversarialPublicParameters() {
    final byte[] seed = hexToBytes("00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF");
    final IdentifierBfvPublicParameters base = sampleIdentifierBfvPublicParameters();

    expectIllegalArgument(
        () -> sampleIdentifierPolicy(base).encryptInput("abcd", seed),
        "input longer than max input byte count must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        new IdentifierBfvPublicParameters.Parameters(8L, 257L, 16_842_753L, 12),
                        base.publicKey(),
                        3))
                .encryptInput("ab", seed),
        "non-divisible ciphertext modulus must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        new IdentifierBfvPublicParameters.Parameters(7L, 257L, 16_842_752L, 12),
                        base.publicKey(),
                        3))
                .encryptInput("ab", seed),
        "non-power-of-two polynomial degree must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        new IdentifierBfvPublicParameters.Parameters(8L, 257L, 16_842_752L, 17),
                        base.publicKey(),
                        3))
                .encryptInput("ab", seed),
        "decomposition base outside the supported range must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        base.parameters(),
                        new IdentifierBfvPublicParameters.PublicKey(
                            withoutFirst(base.publicKey().b()),
                            base.publicKey().a()),
                        3))
                .encryptInput("ab", seed),
        "public-key polynomial length mismatch must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        base.parameters(),
                        base.publicKey(),
                        0))
                .encryptInput("ab", seed),
        "zero max input byte count must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        base.parameters(),
                        base.publicKey(),
                        64))
                .encryptInput("ab", seed),
        "max input byte count above registered RAM-LFE profile must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        base.parameters(),
                        new IdentifierBfvPublicParameters.PublicKey(
                            List.of(16_842_752L, 15_791_131L, 10_301_391L, 6_321_610L, 502_045L, 1_948_157L, 5_332_249L, 12_641_494L),
                            base.publicKey().a()),
                        3))
                .encryptInput("ab", seed),
        "public-key coefficients outside the modulus must be rejected");

    expectIllegalArgument(
        () ->
            sampleIdentifierPolicy(
                    new IdentifierBfvPublicParameters(
                        base.parameters(),
                        base.publicKey(),
                        257))
                .encryptInput("ab", seed),
        "max input byte count outside one plaintext slot must be rejected");
  }

  private static IdentifierPolicySummary sampleIdentifierPolicy(
      final IdentifierBfvPublicParameters parameters) {
    return new IdentifierPolicySummary(
        "string#retail",
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        true,
        IdentifierNormalization.EXACT,
        "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
        "bfv-affine-sha3-256-v1",
        "bfv-v1",
        null,
        parameters,
        null);
  }

  private static IdentifierPolicySummary samplePlaintextOnlyIdentifierPolicy() {
    return new IdentifierPolicySummary(
        "string#retail",
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        true,
        IdentifierNormalization.EXACT,
        "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
        "hkdf-sha3-512-prf-v1",
        null,
        null,
        null,
        null);
  }

  private static IdentifierBfvPublicParameters sampleIdentifierBfvPublicParameters() {
    return new IdentifierBfvPublicParameters(
        new IdentifierBfvPublicParameters.Parameters(8L, 257L, 16_842_752L, 12),
        new IdentifierBfvPublicParameters.PublicKey(
            List.of(11_472_226L, 15_791_131L, 10_301_391L, 6_321_610L, 502_045L, 1_948_157L, 5_332_249L, 12_641_494L),
            List.of(3_503_246L, 2_379_264L, 12_091_019L, 30_169L, 15_804_162L, 8_155_629L, 2_418_997L, 3_003_107L)),
        3);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> loadSharedFixture(final String relativePath) throws Exception {
    Path cursor = Paths.get("").toAbsolutePath();
    while (cursor != null) {
      final Path candidate = cursor.resolve(relativePath);
      if (Files.exists(candidate)) {
        return (Map<String, Object>)
            JsonParser.parse(new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8));
      }
      cursor = cursor.getParent();
    }
    throw new IllegalStateException(relativePath + " was not found");
  }

  private static Map<String, Object> loadSharedBfvFixture() throws Exception {
    return loadSharedFixture("fixtures/soracloud/bfv_identifier_vectors_v1.json");
  }

  private static Map<String, Object> loadSharedReceiptFixture() throws Exception {
    return loadSharedFixture("fixtures/soracloud/identifier_receipt_vectors_v1.json");
  }

  private static IdentifierPolicySummary identifierPolicyFromReceiptFixture(
      final Map<String, Object> policy) {
    return identifierPolicyFromReceiptFixture(policy, null, null);
  }

  private static IdentifierPolicySummary identifierPolicyFromReceiptFixture(
      final Map<String, Object> policy,
      final String policyIdOverride,
      final String resolverPublicKeyOverride) {
    return new IdentifierPolicySummary(
        policyIdOverride != null ? policyIdOverride : string(policy, "policy_id"),
        string(policy, "owner"),
        Boolean.TRUE.equals(policy.get("active")),
        IdentifierNormalization.PHONE_E164,
        resolverPublicKeyOverride != null ? resolverPublicKeyOverride : string(policy, "resolver_public_key"),
        string(policy, "backend"),
        policy.get("input_encryption") instanceof String ? (String) policy.get("input_encryption") : null,
        null,
        null,
        null);
  }

  private static IdentifierResolutionReceipt identifierReceiptFromFixture(
      final Map<String, Object> receipt) {
    return identifierReceiptFromFixture(receipt, null, null, null);
  }

  private static IdentifierResolutionReceipt identifierReceiptFromFixture(
      final Map<String, Object> receipt,
      final String outputCiphertextHashOverride,
      final String signatureOverride,
      final Map<String, Object> attestationOverride) {
    return identifierReceiptFromFixture(
        receipt, outputCiphertextHashOverride, signatureOverride, attestationOverride, null);
  }

  private static IdentifierResolutionReceipt identifierReceiptFromFixture(
      final Map<String, Object> receipt,
      final String outputCiphertextHashOverride,
      final String signatureOverride,
      final Map<String, Object> attestationOverride,
      final String policyIdOverride) {
    return identifierReceiptFromFixture(
        receipt,
        outputCiphertextHashOverride,
        signatureOverride,
        attestationOverride,
        policyIdOverride,
        null,
        null);
  }

  private static IdentifierResolutionReceipt identifierReceiptFromFixture(
      final Map<String, Object> receipt,
      final String outputCiphertextHashOverride,
      final String signatureOverride,
      final Map<String, Object> attestationOverride,
      final String policyIdOverride,
      final String executionProgramIdOverride,
      final String openingProgramIdOverride) {
    return identifierReceiptFromFixture(
        receipt,
        outputCiphertextHashOverride,
        signatureOverride,
        attestationOverride,
        policyIdOverride,
        executionProgramIdOverride,
        openingProgramIdOverride,
        null);
  }

  private static IdentifierResolutionReceipt identifierReceiptFromFixture(
      final Map<String, Object> receipt,
      final String outputCiphertextHashOverride,
      final String signatureOverride,
      final Map<String, Object> attestationOverride,
      final String policyIdOverride,
      final String executionProgramIdOverride,
      final String openingProgramIdOverride,
      final String accountIdOverride) {
    return new IdentifierResolutionReceipt(
        identifierPayloadFromFixture(
            object(receipt, "payload"),
            outputCiphertextHashOverride,
            policyIdOverride,
            executionProgramIdOverride,
            openingProgramIdOverride,
            accountIdOverride),
        identifierAttestationFromFixture(
            attestationOverride != null ? attestationOverride : object(receipt, "attestation"),
            signatureOverride));
  }

  private static IdentifierResolutionPayload identifierPayloadFromFixture(
      final Map<String, Object> payload, final String outputCiphertextHashOverride) {
    return identifierPayloadFromFixture(payload, outputCiphertextHashOverride, null);
  }

  private static IdentifierResolutionPayload identifierPayloadFromFixture(
      final Map<String, Object> payload,
      final String outputCiphertextHashOverride,
      final String policyIdOverride) {
    return identifierPayloadFromFixture(
        payload, outputCiphertextHashOverride, policyIdOverride, null, null);
  }

  private static IdentifierResolutionPayload identifierPayloadFromFixture(
      final Map<String, Object> payload,
      final String outputCiphertextHashOverride,
      final String policyIdOverride,
      final String executionProgramIdOverride,
      final String openingProgramIdOverride) {
    return identifierPayloadFromFixture(
        payload,
        outputCiphertextHashOverride,
        policyIdOverride,
        executionProgramIdOverride,
        openingProgramIdOverride,
        null);
  }

  private static IdentifierResolutionPayload identifierPayloadFromFixture(
      final Map<String, Object> payload,
      final String outputCiphertextHashOverride,
      final String policyIdOverride,
      final String executionProgramIdOverride,
      final String openingProgramIdOverride,
      final String accountIdOverride) {
    return new IdentifierResolutionPayload(
        policyIdOverride != null ? policyIdOverride : string(payload, "policy_id"),
        identifierExecutionFromFixture(
            object(payload, "execution"), outputCiphertextHashOverride, executionProgramIdOverride),
        outputOpeningFromFixture(object(payload, "opening"), openingProgramIdOverride),
        string(payload, "opaque_id"),
        string(payload, "receipt_hash"),
        string(payload, "uaid"),
        accountIdOverride != null ? accountIdOverride : string(payload, "account_id"));
  }

  private static IdentifierResolutionExecutionPayload identifierExecutionFromFixture(
      final Map<String, Object> execution, final String outputCiphertextHashOverride) {
    return identifierExecutionFromFixture(execution, outputCiphertextHashOverride, null);
  }

  private static IdentifierResolutionExecutionPayload identifierExecutionFromFixture(
      final Map<String, Object> execution,
      final String outputCiphertextHashOverride,
      final String programIdOverride) {
    return new IdentifierResolutionExecutionPayload(
        programIdOverride != null ? programIdOverride : string(execution, "program_id"),
        string(execution, "program_digest"),
        string(execution, "backend"),
        string(execution, "verification_mode"),
        string(execution, "input_ciphertext_hash"),
        outputCiphertextHashOverride != null
            ? outputCiphertextHashOverride
            : string(execution, "output_ciphertext_hash"),
        string(execution, "parameter_digest"),
        string(execution, "evaluation_key_digest"),
        string(execution, "output_hash"),
        string(execution, "associated_data_hash"),
        number(execution, "executed_at_ms").longValue(),
        optionalLong(execution, "expires_at_ms"));
  }

  private static RamLfeOutputOpening outputOpeningFromFixture(final Map<String, Object> opening) {
    return outputOpeningFromFixture(opening, null);
  }

  private static RamLfeOutputOpening outputOpeningFromFixture(
      final Map<String, Object> opening, final String programIdOverride) {
    final Map<String, Object> payload = object(opening, "payload");
    return new RamLfeOutputOpening(
        new RamLfeOutputOpeningPayload(
            programIdOverride != null ? programIdOverride : string(payload, "program_id"),
            string(payload, "input_ciphertext_hash"),
            string(payload, "output_ciphertext_hash"),
            string(payload, "parameter_digest"),
            string(payload, "evaluation_key_digest"),
            string(payload, "opened_output_hash"),
            number(payload, "opened_at_ms").longValue(),
            optionalLong(payload, "expires_at_ms")),
        string(opening, "signature"));
  }

  private static IdentifierReceiptAttestation identifierAttestationFromFixture(
      final Map<String, Object> attestation, final String signatureOverride) {
    return new IdentifierReceiptAttestation(
        string(attestation, "kind"),
        signatureOverride != null ? signatureOverride : stringOrNull(attestation, "signature"),
        stringOrNull(attestation, "proof_backend"),
        stringOrNull(attestation, "proof_b64"));
  }

  private static IdentifierPolicySummary identifierPolicyFromBfvFixture(
      final Map<String, Object> policy) {
    return new IdentifierPolicySummary(
        string(policy, "policy_id"),
        string(policy, "owner"),
        Boolean.TRUE.equals(policy.get("active")),
        IdentifierNormalization.EXACT,
        string(policy, "resolver_public_key"),
        string(policy, "backend"),
        string(policy, "input_encryption"),
        null,
        identifierBfvParametersFromFixture(
            object(policy, "input_encryption_public_parameters_decoded")),
        null);
  }

  private static IdentifierBfvPublicParameters identifierBfvParametersFromFixture(
      final Map<String, Object> params) {
    final Map<String, Object> parameters = object(params, "parameters");
    final Map<String, Object> publicKey = object(params, "public_key");
    return new IdentifierBfvPublicParameters(
        new IdentifierBfvPublicParameters.Parameters(
            number(parameters, "polynomial_degree").longValue(),
            number(parameters, "plaintext_modulus").longValue(),
            number(parameters, "ciphertext_modulus").longValue(),
            number(parameters, "decomposition_base_log").intValue()),
        new IdentifierBfvPublicParameters.PublicKey(
            longList(publicKey, "b"),
            longList(publicKey, "a")),
        number(params, "max_input_bytes").intValue(),
        params.get("norito_length_encoding") instanceof String
            ? (String) params.get("norito_length_encoding")
            : null);
  }

  private static String sha256Hex(final byte[] bytes) throws Exception {
    return hex(MessageDigest.getInstance("SHA-256").digest(bytes));
  }

  private static String verifierKeyCommitment(final String backend, final byte[] bytes)
      throws Exception {
    final MessageDigest digest = MessageDigest.getInstance("SHA-256");
    final byte[] backendBytes = backend.getBytes(StandardCharsets.UTF_8);
    digest.update("iroha:zk:v1:vk".getBytes(StandardCharsets.UTF_8));
    updateU64Be(digest, backendBytes.length);
    digest.update(backendBytes);
    updateU64Be(digest, bytes.length);
    digest.update(bytes);
    return hex(digest.digest()).toLowerCase(Locale.ROOT);
  }

  private static String verifyingKeyDraftJson(final byte[] transactionPayload) {
    final String transactionPayloadB64 =
        Base64.getEncoder().encodeToString(transactionPayload);
    final String signingMessageB64 =
        Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionPayload));
    return "{\"submitted\":false,\"transaction_payload_b64\":\""
        + transactionPayloadB64
        + "\",\"signing_message_b64\":\""
        + signingMessageB64
        + "\"}";
  }

  private static byte[] verifyingKeyTransactionPayload(
      final Map<String, Object> request,
      final VerifyingKeyDraftBinding.Operation operation) {
    return verifyingKeyTransactionPayload(
        request,
        operation,
        VERIFYING_KEY_CHAIN_ID,
        (String) request.get("authority"),
        null);
  }

  private static byte[] verifyingKeyTransactionPayload(
      final Map<String, Object> request,
      final VerifyingKeyDraftBinding.Operation operation,
      final String chainId,
      final String authority,
      final List<InstructionBox> instructions) {
    final Integer discriminant =
        org.hyperledger.iroha.android.address.AccountAddress.detectI105Discriminant(authority);
    if (discriminant == null) {
      throw new AssertionError("test authority must be canonical I105");
    }
    final List<InstructionBox> instructionList =
        instructions == null
            ? List.of(VerifyingKeyDraftBinding.expectedInstruction(request, operation))
            : instructions;
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority(authority)
            .setCreationTimeMs(1_700_000_000_000L)
            .setInstructions(instructionList)
            .setTimeToLiveMs(5_000L)
            .setNonce(1L)
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), null))
            .build();
    try {
      return new NoritoJavaCodecAdapter(discriminant).encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to encode verifying-key transaction fixture", ex);
    }
  }

  private static void expectVerifierDraftReject(
      final byte[] transactionPayload,
      final Map<String, Object> request,
      final String message) {
    expectIllegalArgument(
        () ->
            VerifyingKeyTransactionDraft.parseRegister(
                verifyingKeyDraftJson(transactionPayload)
                    .getBytes(StandardCharsets.UTF_8),
                VERIFYING_KEY_CHAIN_ID,
                request),
        message);
  }

  private static void updateU64Be(final MessageDigest digest, final long value) {
    for (int shift = 56; shift >= 0; shift -= 8) {
      digest.update((byte) (value >>> shift));
    }
  }

  private static VerifyingKeyRegisterRequest.Builder verifierKeyRegisterRequestBuilder() {
    return VerifyingKeyRegisterRequest.builder()
        .authority(TestAccountIds.ed25519Authority(0x37))
        .backend("halo2/ipa")
        .name("transfer_vk")
        .version(1L)
        .circuitId("transfer-v1")
        .publicInputsSchemaHashHex("aa".repeat(32))
        .gasScheduleId("halo2-default")
        .verifyingKeyBytes(new byte[] {1});
  }

  private static VerifyingKeyUpdateRequest.Builder verifierKeyUpdateRequestBuilder() {
    return VerifyingKeyUpdateRequest.builder()
        .authority(TestAccountIds.ed25519Authority(0x37))
        .backend("halo2/ipa")
        .name("transfer_vk")
        .version(1L)
        .circuitId("transfer-v1")
        .publicInputsSchemaHashHex("aa".repeat(32))
        .verifyingKeyBytes(new byte[] {1});
  }

  private static void assertBfvOperationKeyComponentVectors(
      final Map<String, Object> operationVectors) {
    assert "soracloud-bfv-operation-v1".equals(operationVectors.get("vector_set"))
        : "operation vector set mismatch";
    final Map<String, Object> publicParameters = object(operationVectors, "public_parameters");
    final long publicDegree = number(publicParameters, "polynomial_degree").longValue();
    assertBfvRnsModulusChainFixture(operationVectors, publicDegree);
    final Map<String, Object> evaluationKey = object(operationVectors, "evaluation_key_bundle");
    assert number(publicParameters, "decomposition_base_log").longValue()
            == number(evaluationKey, "decomposition_base_log").longValue()
        : "evaluation-key decomposition base log mismatch";
    assert number(evaluationKey, "decomposition_digit_count").longValue()
            == number(evaluationKey, "relinearization_entry_count").longValue()
        : "evaluation-key decomposition digit count mismatch";
    final List<Map<String, Object>> entries =
        objectList(evaluationKey, "relinearization_entries");
    assert entries.size() == number(evaluationKey, "relinearization_entry_count").intValue()
        : "relinearization entry count mismatch";
    final List<String> componentDigests = new ArrayList<>();
    for (int index = 0; index < entries.size(); index++) {
      final Map<String, Object> entry = entries.get(index);
      assert number(entry, "index").intValue() == index : "relinearization entry index mismatch";
      assert number(entry, "coefficient_count").longValue() == publicDegree
          : "relinearization entry coefficient count mismatch";
      assertBfvComponentDigest("relinearization entry " + index + " b", string(entry, "b_sha256"), componentDigests);
      assertBfvComponentDigest("relinearization entry " + index + " a", string(entry, "a_sha256"), componentDigests);
    }
    final List<Map<String, Object>> galoisKeys = objectList(operationVectors, "galois_keys");
    assert galoisKeys.size() == number(evaluationKey, "galois_key_count").intValue()
        : "Galois key count mismatch";
    for (final Map<String, Object> key : galoisKeys) {
      final long power = number(key, "automorphism_power").longValue();
      final List<Map<String, Object>> galoisEntries = objectList(key, "entries");
      assert galoisEntries.size() == number(key, "entry_count").intValue()
          : "Galois key entry count mismatch";
      for (int index = 0; index < galoisEntries.size(); index++) {
        final Map<String, Object> entry = galoisEntries.get(index);
        assert number(entry, "index").intValue() == index : "Galois entry index mismatch";
        assert number(entry, "coefficient_count").longValue() == publicDegree
            : "Galois entry coefficient count mismatch";
        assertBfvComponentDigest("Galois key " + power + " entry " + index + " b", string(entry, "b_sha256"), componentDigests);
        assertBfvComponentDigest("Galois key " + power + " entry " + index + " a", string(entry, "a_sha256"), componentDigests);
      }
    }
    final List<Map<String, Object>> galoisSwitchVectors =
        objectList(operationVectors, "galois_switch_vectors");
    assert !galoisSwitchVectors.isEmpty() : "Galois switch vectors must not be empty";
    for (final Map<String, Object> vector : galoisSwitchVectors) {
      final String name = string(vector, "name");
      final long power = number(vector, "automorphism_power").longValue();
      boolean hasMatchingKey = false;
      for (final Map<String, Object> key : galoisKeys) {
        if (number(key, "automorphism_power").longValue() == power) {
          hasMatchingKey = true;
          break;
        }
      }
      assert hasMatchingKey : "Galois switch vector has no matching key: " + name;
      final List<Long> plaintextSlots = longList(vector, "input_plaintext_slots");
      assert !plaintextSlots.isEmpty() : "Galois switch vector plaintext slots empty";
      for (final long slot : plaintextSlots) {
        assert slot >= 0 : "Galois switch vector plaintext slot negative";
      }
      assert number(vector, "expected_input_ciphertext_bytes").longValue() > 0
          : "Galois switch vector input bytes must be positive";
      assert number(vector, "expected_output_ciphertext_bytes").longValue() > 0
          : "Galois switch vector output bytes must be positive";
      assertBfvUpperSha256("Galois switch vector " + name + " input", string(vector, "expected_input_ciphertext_sha256"));
      assertBfvUpperSha256("Galois switch vector " + name + " output", string(vector, "expected_output_ciphertext_sha256"));
      assertBfvUpperSha256("Galois switch vector " + name + " plaintext", string(vector, "expected_plaintext_sha256"));
      final Map<String, Object> components = object(vector, "output_components");
      assert number(components, "coefficient_count").longValue() == publicDegree
          : "Galois switch vector coefficient count mismatch";
      assertBfvComponentDigest("Galois switch vector " + name + " c0", string(components, "c0_sha256"), componentDigests);
      assertBfvComponentDigest("Galois switch vector " + name + " c1", string(components, "c1_sha256"), componentDigests);
    }
    final List<Map<String, Object>> packedGaloisSwitchVectors =
        objectList(operationVectors, "packed_galois_switch_vectors");
    assert !packedGaloisSwitchVectors.isEmpty() : "packed Galois switch vectors must not be empty";
    for (final Map<String, Object> vector : packedGaloisSwitchVectors) {
      final String name = string(vector, "name");
      final long power = number(vector, "automorphism_power").longValue();
      boolean hasMatchingKey = false;
      for (final Map<String, Object> key : galoisKeys) {
        if (number(key, "automorphism_power").longValue() == power) {
          hasMatchingKey = true;
          break;
        }
      }
      assert hasMatchingKey : "packed Galois switch vector has no matching key: " + name;
      final List<Long> inputSlots = longList(vector, "input_packed_slots");
      final List<Long> permutation = longList(vector, "expected_slot_permutation");
      final List<Long> outputSlots = longList(vector, "expected_packed_slots");
      assert inputSlots.size() == publicDegree
          : "packed Galois switch vector input slot count mismatch";
      assert permutation.size() == publicDegree
          : "packed Galois switch vector permutation count mismatch";
      assert outputSlots.size() == publicDegree
          : "packed Galois switch vector output slot count mismatch";
      for (final long slot : inputSlots) {
        assert slot >= 0 : "packed Galois switch vector input slot negative";
      }
      for (final long slot : permutation) {
        assert slot >= 0 : "packed Galois switch vector permutation slot negative";
      }
      for (final long slot : outputSlots) {
        assert slot >= 0 : "packed Galois switch vector output slot negative";
      }
      assertBfvUpperSha256("packed Galois switch vector " + name + " packed plaintext", string(vector, "expected_packed_plaintext_sha256"));
      assertBfvUpperSha256("packed Galois switch vector " + name + " input", string(vector, "expected_input_ciphertext_sha256"));
      assertBfvUpperSha256("packed Galois switch vector " + name + " output", string(vector, "expected_output_ciphertext_sha256"));
      assertBfvUpperSha256("packed Galois switch vector " + name + " plaintext", string(vector, "expected_plaintext_coefficients_sha256"));
      final Map<String, Object> components = object(vector, "output_components");
      assert number(components, "coefficient_count").longValue() == publicDegree
          : "packed Galois switch vector coefficient count mismatch";
      assertBfvComponentDigest("packed Galois switch vector " + name + " c0", string(components, "c0_sha256"), componentDigests);
      assertBfvComponentDigest("packed Galois switch vector " + name + " c1", string(components, "c1_sha256"), componentDigests);
    }
    final List<Map<String, Object>> rotationKeys = objectList(operationVectors, "rotation_keys");
    assert rotationKeys.size() == number(evaluationKey, "rotation_key_count").intValue()
        : "rotation key count mismatch";
    for (final Map<String, Object> key : rotationKeys) {
      final Map<String, Object> components = object(key, "zero_refresh_components");
      final long steps = number(key, "rotation_steps").longValue();
      assert number(components, "coefficient_count").longValue() == publicDegree
          : "rotation key " + steps + " coefficient count mismatch";
      assertBfvComponentDigest("rotation key " + steps + " c0", string(components, "c0_sha256"), componentDigests);
      assertBfvComponentDigest("rotation key " + steps + " c1", string(components, "c1_sha256"), componentDigests);
    }
    final Map<String, Object> bootstrap = object(operationVectors, "bootstrap_key");
    assert string(evaluationKey, "bootstrap_key_id").equals(string(bootstrap, "key_id"))
        : "bootstrap key id mismatch";
    assert number(evaluationKey, "bootstrap_max_refresh_rounds").longValue()
            == number(bootstrap, "max_refresh_rounds").longValue()
        : "bootstrap max refresh rounds mismatch";
    assert number(bootstrap, "max_refresh_rounds").longValue() > 0
        : "bootstrap max refresh rounds must be positive";
    final Map<String, Object> bootstrapComponents = object(bootstrap, "zero_refresh_components");
    assert number(bootstrapComponents, "coefficient_count").longValue() == publicDegree
        : "bootstrap coefficient count mismatch";
    assertBfvComponentDigest("bootstrap c0", string(bootstrapComponents, "c0_sha256"), componentDigests);
    assertBfvComponentDigest("bootstrap c1", string(bootstrapComponents, "c1_sha256"), componentDigests);
    final List<Map<String, Object>> roundRefreshes = objectList(bootstrap, "round_refreshes");
    assert roundRefreshes.size() == number(bootstrap, "max_refresh_rounds").intValue()
        : "bootstrap round refresh count mismatch";
    for (int index = 0; index < roundRefreshes.size(); index++) {
      final Map<String, Object> refresh = roundRefreshes.get(index);
      assert number(refresh, "round_index").longValue() == index
          : "bootstrap round refresh index mismatch";
      assert number(refresh, "expected_refresh_bytes").longValue() > 0
          : "bootstrap round refresh bytes must be positive";
      assertBfvUpperSha256("bootstrap round " + index + " refresh", string(refresh, "expected_refresh_sha256"));
      final Map<String, Object> components = object(refresh, "components");
      assert number(components, "coefficient_count").longValue() == publicDegree
          : "bootstrap round refresh coefficient count mismatch";
      if (index == 0) {
        assert string(bootstrapComponents, "c0_sha256").equals(string(components, "c0_sha256"))
            : "bootstrap round 0 c0 must mirror zero refresh";
        assert string(bootstrapComponents, "c1_sha256").equals(string(components, "c1_sha256"))
            : "bootstrap round 0 c1 must mirror zero refresh";
        assertBfvUpperSha256("bootstrap round 0 c0", string(components, "c0_sha256"));
        assertBfvUpperSha256("bootstrap round 0 c1", string(components, "c1_sha256"));
      } else {
        assertBfvComponentDigest("bootstrap round " + index + " c0", string(components, "c0_sha256"), componentDigests);
        assertBfvComponentDigest("bootstrap round " + index + " c1", string(components, "c1_sha256"), componentDigests);
      }
    }
    assert string(bootstrap, "expected_zero_refresh_sha256")
        .equals(string(roundRefreshes.get(0), "expected_refresh_sha256"))
        : "bootstrap first round must mirror zero refresh";
    if (roundRefreshes.size() > 1) {
      assert !string(roundRefreshes.get(0), "expected_refresh_sha256")
          .equals(string(roundRefreshes.get(1), "expected_refresh_sha256"))
          : "bootstrap round refresh material must be domain separated";
    }
    assertBfvFullBootstrapMaterialFixture(operationVectors);
    final List<Map<String, Object>> bootstrapRefreshVectors =
        objectList(operationVectors, "bootstrap_refresh_vectors");
    assert !bootstrapRefreshVectors.isEmpty() : "bootstrap refresh vectors must not be empty";
    for (final Map<String, Object> vector : bootstrapRefreshVectors) {
      final String name = string(vector, "name");
      assert string(bootstrap, "key_id").equals(string(vector, "key_id"))
          : "bootstrap refresh vector key id mismatch";
      final long refreshRounds = number(vector, "refresh_rounds").longValue();
      assert refreshRounds > 0 : "bootstrap refresh vector rounds must be positive";
      assert refreshRounds <= number(bootstrap, "max_refresh_rounds").longValue()
          : "bootstrap refresh vector rounds exceed key bound";
      final List<Long> plaintextSlots = longList(vector, "input_plaintext_slots");
      assert !plaintextSlots.isEmpty() : "bootstrap refresh vector plaintext slots empty";
      for (final long slot : plaintextSlots) {
        assert slot >= 0 : "bootstrap refresh vector plaintext slot negative";
      }
      assert number(vector, "expected_input_ciphertext_bytes").longValue() > 0
          : "bootstrap refresh vector input bytes must be positive";
      assert number(vector, "expected_output_ciphertext_bytes").longValue() > 0
          : "bootstrap refresh vector output bytes must be positive";
      assertBfvUpperSha256("bootstrap refresh vector " + name + " input", string(vector, "expected_input_ciphertext_sha256"));
      assertBfvUpperSha256("bootstrap refresh vector " + name + " output", string(vector, "expected_output_ciphertext_sha256"));
      assertBfvUpperSha256("bootstrap refresh vector " + name + " plaintext", string(vector, "expected_plaintext_sha256"));
      final Map<String, Object> components = object(vector, "output_components");
      assert number(components, "coefficient_count").longValue() == publicDegree
          : "bootstrap refresh vector coefficient count mismatch";
      assertBfvComponentDigest("bootstrap refresh vector " + name + " c0", string(components, "c0_sha256"), componentDigests);
      assertBfvComponentDigest("bootstrap refresh vector " + name + " c1", string(components, "c1_sha256"), componentDigests);
    }
    final List<Map<String, Object>> runtimeVectors = objectList(operationVectors, "vectors");
    for (final Map<String, Object> vector : runtimeVectors) {
      final int expectedDepth = "Multiply".equals(string(vector, "operation"))
          ? balancedBfvMultiplicationDepth(objectList(vector, "inputs").size())
          : 0;
      assert number(vector, "requested_multiplication_depth").intValue() == expectedDepth
          : string(vector, "name") + " requested multiplication depth mismatch";
    }
    Map<String, Object> packedRotate = null;
    for (final Map<String, Object> vector : runtimeVectors) {
      if ("soracloud-packed-rotate-left-output".equals(string(vector, "name"))) {
        packedRotate = vector;
        break;
      }
    }
    assert packedRotate != null : "packed RotateLeft operation vector must be present";
    assert "RotateLeft".equals(string(packedRotate, "operation")) : "packed RotateLeft operation mismatch";
    assert number(packedRotate, "rotation_steps").longValue() == publicDegree / 2
        : "packed RotateLeft rotation mismatch";
    final long packedRotatePower = number(packedRotate, "automorphism_power").longValue();
    assert packedRotatePower == publicDegree + 1 : "packed RotateLeft Galois power mismatch";
    boolean hasPackedRotateKey = false;
    for (final Map<String, Object> key : galoisKeys) {
      if (number(key, "automorphism_power").longValue() == packedRotatePower) {
        hasPackedRotateKey = true;
        break;
      }
    }
    assert hasPackedRotateKey : "packed RotateLeft vector has no matching Galois key";
    final List<Map<String, Object>> packedRotateInputs = objectList(packedRotate, "inputs");
    assert packedRotateInputs.size() == 1 : "packed RotateLeft input count mismatch";
    final Map<String, Object> packedRotateInput = packedRotateInputs.get(0);
    final List<Long> inputSlots = longList(packedRotateInput, "packed_slots");
    final List<Long> outputSlots = longList(packedRotate, "expected_packed_slots");
    assert inputSlots.size() == publicDegree : "packed RotateLeft input slot count mismatch";
    assert outputSlots.size() == publicDegree : "packed RotateLeft output slot count mismatch";
    for (final long slot : inputSlots) {
      assert slot >= 0 : "packed RotateLeft input slot negative";
    }
    for (final long slot : outputSlots) {
      assert slot >= 0 : "packed RotateLeft output slot negative";
    }
    assert number(packedRotateInput, "expected_ciphertext_bytes").longValue() > 0
        : "packed RotateLeft input bytes must be positive";
    assert number(packedRotate, "expected_output_ciphertext_bytes").longValue() > 0
        : "packed RotateLeft output bytes must be positive";
    assertBfvUpperSha256("packed RotateLeft input plaintext", string(packedRotateInput, "expected_packed_plaintext_sha256"));
    assertBfvUpperSha256("packed RotateLeft input", string(packedRotateInput, "expected_ciphertext_sha256"));
    assertBfvUpperSha256("packed RotateLeft output", string(packedRotate, "expected_output_ciphertext_sha256"));
    assertBfvUpperSha256("packed RotateLeft plaintext", string(packedRotate, "expected_plaintext_coefficients_sha256"));
    final Map<String, Object> packedRotateComponents = object(packedRotate, "output_components");
    assert number(packedRotateComponents, "coefficient_count").longValue() == publicDegree
        : "packed RotateLeft coefficient count mismatch";
    assertBfvComponentDigest("packed RotateLeft c0", string(packedRotateComponents, "c0_sha256"), componentDigests);
    assertBfvComponentDigest("packed RotateLeft c1", string(packedRotateComponents, "c1_sha256"), componentDigests);

    Map<String, Object> packedRotateSchedule = null;
    for (final Map<String, Object> vector : runtimeVectors) {
      if ("soracloud-packed-rotate-left-schedule-output".equals(string(vector, "name"))) {
        packedRotateSchedule = vector;
        break;
      }
    }
    assert packedRotateSchedule != null : "packed RotateLeft schedule vector must be present";
    assert "RotateLeft".equals(string(packedRotateSchedule, "operation"))
        : "packed RotateLeft schedule operation mismatch";
    assert number(packedRotateSchedule, "rotation_steps").longValue() == 1
        : "packed RotateLeft schedule rotation mismatch";
    final List<Long> schedulePowers = longList(packedRotateSchedule, "automorphism_powers");
    assert schedulePowers.size() > 1 : "packed RotateLeft schedule must use multiple powers";
    for (final long power : schedulePowers) {
      assert power > 0 : "packed RotateLeft schedule power must be positive";
      boolean hasSchedulePowerKey = false;
      for (final Map<String, Object> key : galoisKeys) {
        if (number(key, "automorphism_power").longValue() == power) {
          hasSchedulePowerKey = true;
          break;
        }
      }
      assert hasSchedulePowerKey : "packed RotateLeft schedule power has no matching Galois key";
    }
    final List<Map<String, Object>> packedRotateScheduleInputs =
        objectList(packedRotateSchedule, "inputs");
    assert packedRotateScheduleInputs.size() == 1
        : "packed RotateLeft schedule input count mismatch";
    final Map<String, Object> packedRotateScheduleInput = packedRotateScheduleInputs.get(0);
    final List<Long> scheduleInputSlots = longList(packedRotateScheduleInput, "packed_slots");
    final List<Long> scheduleOutputSlots = longList(packedRotateSchedule, "expected_packed_slots");
    assert scheduleInputSlots.size() == publicDegree
        : "packed RotateLeft schedule input slot count mismatch";
    assert scheduleOutputSlots.size() == publicDegree
        : "packed RotateLeft schedule output slot count mismatch";
    final List<Long> expectedScheduleOutputSlots =
        new ArrayList<>(scheduleInputSlots.subList(1, scheduleInputSlots.size()));
    expectedScheduleOutputSlots.add(scheduleInputSlots.get(0));
    assert expectedScheduleOutputSlots.equals(scheduleOutputSlots)
        : "packed RotateLeft schedule output slot mismatch";
    for (final long slot : scheduleInputSlots) {
      assert slot >= 0 : "packed RotateLeft schedule input slot negative";
    }
    for (final long slot : scheduleOutputSlots) {
      assert slot >= 0 : "packed RotateLeft schedule output slot negative";
    }
    assert number(packedRotateScheduleInput, "expected_ciphertext_bytes").longValue() > 0
        : "packed RotateLeft schedule input bytes must be positive";
    assert number(packedRotateSchedule, "expected_output_ciphertext_bytes").longValue() > 0
        : "packed RotateLeft schedule output bytes must be positive";
    assertBfvUpperSha256("packed RotateLeft schedule input plaintext", string(packedRotateScheduleInput, "expected_packed_plaintext_sha256"));
    assertBfvUpperSha256("packed RotateLeft schedule input", string(packedRotateScheduleInput, "expected_ciphertext_sha256"));
    assertBfvUpperSha256("packed RotateLeft schedule output", string(packedRotateSchedule, "expected_output_ciphertext_sha256"));
    assertBfvUpperSha256("packed RotateLeft schedule plaintext", string(packedRotateSchedule, "expected_plaintext_coefficients_sha256"));
    final Map<String, Object> packedRotateScheduleComponents =
        object(packedRotateSchedule, "output_components");
    assert number(packedRotateScheduleComponents, "coefficient_count").longValue() == publicDegree
        : "packed RotateLeft schedule coefficient count mismatch";
    assertBfvComponentDigest("packed RotateLeft schedule c0", string(packedRotateScheduleComponents, "c0_sha256"), componentDigests);
    assertBfvComponentDigest("packed RotateLeft schedule c1", string(packedRotateScheduleComponents, "c1_sha256"), componentDigests);
  }

  private static void assertBfvFullBootstrapMaterialFixture(
      final Map<String, Object> operationVectors) {
    final Map<String, Object> material = object(operationVectors, "full_bootstrap_material");
    assert "iroha_bfv_full_bootstrap_v1".equals(string(material, "circuit_id"))
        : "full-bootstrap circuit id mismatch";
    assert number(material, "max_bootstrap_depth").longValue() == 1
        : "full-bootstrap max depth mismatch";

    final List<String> digestFields =
        java.util.Arrays.asList(
            "parameter_digest_hex",
            "rns_modulus_chain_digest_hex",
            "key_switch_decomposition_chain_digest_hex",
            "coefficient_to_slot_key_digest_hex",
            "slot_to_coefficient_key_digest_hex",
            "blind_rotation_key_digest_hex",
            "sample_extraction_key_digest_hex",
            "accumulator_digest_hex",
            "proof_public_input_schema_digest_hex",
            "prover_key_digest_hex",
            "prover_key_material_commitment_hex",
            "verifier_key_digest_hex",
            "verifier_key_material_commitment_hex",
            "vk_commitment_hex",
            "expected_material_digest_hex",
            "expected_statement_digest_hex");
    final List<String> uniqueDigestValues = new ArrayList<>();
    for (final String field : digestFields) {
      final String value = string(material, field);
      assertBfvLowerDigest("full-bootstrap material " + field, value);
      if (!"vk_commitment_hex".equals(field)) {
        assert !uniqueDigestValues.contains(value)
            : "full-bootstrap material digest roles must be unique";
        uniqueDigestValues.add(value);
      }
    }
    assert string(object(operationVectors, "rns_modulus_chain"), "expected_digest_hex")
            .equals(string(material, "rns_modulus_chain_digest_hex"))
        : "full-bootstrap RNS digest mismatch";
    assert string(material, "verifier_key_material_commitment_hex")
            .equals(string(material, "vk_commitment_hex"))
        : "full-bootstrap verifier-key commitment mismatch";
    assert !string(material, "expected_material_digest_hex")
            .equals(string(material, "expected_statement_digest_hex"))
        : "full-bootstrap material and statement digests must differ";
  }

  private static void assertBfvRnsModulusChainFixture(
      final Map<String, Object> operationVectors, final long publicDegree) {
    final Map<String, Object> rns = object(operationVectors, "rns_modulus_chain");
    final List<Long> moduli = longList(rns, "moduli");
    assert !moduli.isEmpty() : "RNS modulus-chain limbs must not be empty";
    final List<Long> sortedModuli = new ArrayList<>(moduli);
    sortedModuli.sort(Long::compareTo);
    assert sortedModuli.equals(moduli) : "RNS modulus-chain limbs must be sorted";
    for (final long modulus : moduli) {
      assert modulus > 2L && (modulus & 1L) == 1L
          : "RNS modulus-chain limbs must be odd prime candidates";
    }
    assert string(rns, "product").matches("[0-9]+")
        : "RNS modulus-chain product must be decimal";
    assertBfvLowerDigest("RNS modulus-chain digest", string(rns, "expected_digest_hex"));

    final Map<String, Object> samples = object(rns, "sample_polynomials");
    assert longList(samples, "lhs_coefficients").size() == (int) publicDegree
        : "RNS lhs coefficient count mismatch";
    assert longList(samples, "rhs_coefficients").size() == (int) publicDegree
        : "RNS rhs coefficient count mismatch";
    for (final String label : java.util.Arrays.asList("lhs", "rhs", "sum", "negacyclic_product")) {
      assertBfvRnsPolynomialFixture(label, object(samples, label), publicDegree, moduli.size());
    }
  }

  private static void assertBfvRnsPolynomialFixture(
      final String label,
      final Map<String, Object> polynomial,
      final long publicDegree,
      final int limbCount) {
    assert number(polynomial, "coefficient_count").longValue() == publicDegree
        : label + " RNS coefficient count mismatch";
    final List<String> limbHashes = stringList(polynomial, "residue_limb_sha256");
    assert limbHashes.size() == limbCount : label + " RNS residue limb count mismatch";
    assertBfvUpperSha256(label + " RNS reconstructed coefficients", string(polynomial, "reconstructed_sha256"));
    for (int index = 0; index < limbHashes.size(); index++) {
      assertBfvUpperSha256(label + " RNS residue limb " + index, limbHashes.get(index));
    }
  }

  private static void assertBfvComponentDigest(
      final String label, final String value, final List<String> seen) {
    assertBfvUpperSha256(label, value);
    assert !seen.contains(value) : label + " must be unique";
    seen.add(value);
  }

  private static void assertBfvUpperSha256(final String label, final String value) {
    assert value.matches("[0-9A-F]{64}") : label + " must be canonical uppercase SHA-256";
    assert !value.equals("0".repeat(64)) : label + " must not be zero";
  }

  private static int balancedBfvMultiplicationDepth(final int inputCount) {
    assert inputCount > 0 : "BFV multiplication depth requires at least one input";
    int covered = 1;
    int depth = 0;
    while (covered < inputCount) {
      covered *= 2;
      depth += 1;
    }
    return depth;
  }

  private static void assertBfvLowerDigest(final String label, final String value) {
    assert value.matches("[0-9a-f]{64}") : label + " must be canonical lowercase hex";
    assert !value.equals("0".repeat(64)) : label + " must not be zero";
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (!(value instanceof Map)) {
      throw new IllegalArgumentException(key + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Map<String, Object>> objectList(
      final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (!(value instanceof List)) {
      throw new IllegalArgumentException(key + " must be a list");
    }
    return (List<Map<String, Object>>) value;
  }

  private static String string(final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(key + " must be a string");
    }
    return (String) value;
  }

  private static String stringOrNull(final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (value == null) {
      return null;
    }
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(key + " must be a string");
    }
    return (String) value;
  }

  private static List<String> stringList(final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (!(value instanceof List)) {
      throw new IllegalArgumentException(key + " must be a list");
    }
    final List<?> raw = (List<?>) value;
    final List<String> out = new ArrayList<>();
    for (final Object entry : raw) {
      if (!(entry instanceof String)) {
        throw new IllegalArgumentException(key + " entries must be strings");
      }
      out.add((String) entry);
    }
    return out;
  }

  private static Number number(final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (value instanceof Number) {
      return (Number) value;
    }
    if (value instanceof String) {
      return new BigInteger((String) value);
    }
    throw new IllegalArgumentException(key + " must be a number");
  }

  private static Long optionalLong(final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (value == null) {
      return null;
    }
    if (value instanceof Number) {
      return ((Number) value).longValue();
    }
    if (value instanceof String) {
      return new BigInteger((String) value).longValue();
    }
    throw new IllegalArgumentException(key + " must be a number");
  }

  private static List<Long> longList(final Map<String, Object> root, final String key) {
    final Object value = root.get(key);
    if (!(value instanceof List)) {
      throw new IllegalArgumentException(key + " must be a list");
    }
    final List<?> raw = (List<?>) value;
    final List<Long> out = new ArrayList<>();
    for (final Object entry : raw) {
      if (entry instanceof Number) {
        out.add(((Number) entry).longValue());
      } else if (entry instanceof String) {
        out.add(new BigInteger((String) entry).longValue());
      } else {
        throw new IllegalArgumentException(key + " entries must be numbers");
      }
    }
    return out;
  }

  private static List<Long> withoutFirst(final List<Long> values) {
    final ArrayList<Long> truncated = new ArrayList<>(values);
    truncated.remove(0);
    return truncated;
  }

  private static void expectIllegalArgument(final Runnable action, final String message) {
    boolean failed = false;
    try {
      action.run();
    } catch (final IllegalArgumentException expected) {
      failed = true;
    }
    assert failed : message;
  }

  private static void expectIllegalState(final Runnable action, final String message) {
    boolean failed = false;
    try {
      action.run();
    } catch (final IllegalStateException expected) {
      failed = true;
    }
    assert failed : message;
  }

  private static void expectCompletionIllegalArgument(
      final CompletableFuture<?> future, final String message) {
    boolean failed = false;
    try {
      future.join();
    } catch (final java.util.concurrent.CompletionException error) {
      failed = error.getCause() instanceof IllegalArgumentException;
    }
    assert failed : message;
  }

  private static void expectVerifierReject(
      final Runnable action, final CapturingExecutor executor, final String message) {
    final TransportRequest before = executor.lastRequest;
    expectIllegalArgument(action, message);
    assert executor.lastRequest == before : message + " before sending an HTTP request";
  }

  @FunctionalInterface
  private interface CheckedRunnable {
    void run() throws Exception;
  }

  private static void expectAssertionOrIllegalArgument(
      final CheckedRunnable action, final String message)
      throws Exception {
    boolean failed = false;
    try {
      action.run();
    } catch (final AssertionError | IllegalArgumentException expected) {
      failed = true;
    }
    assert failed : message;
  }

  private static void expectRuntimeException(final Runnable action, final String message) {
    boolean failed = false;
    try {
      action.run();
    } catch (final RuntimeException expected) {
      failed = true;
    }
    assert failed : message;
  }

  private static void identifierReceiptVerifierAcceptsEd25519Receipt() {
    final String accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final IdentifierResolutionPayload payload =
        new IdentifierResolutionPayload(
            "phone#retail",
            new IdentifierResolutionExecutionPayload(
                "identifier_lookup_retail",
                "11".repeat(32),
                "bfv-affine-sha3-256-v1",
                "signed",
                "aa".repeat(32),
                "bb".repeat(32),
                "cc".repeat(32),
                "dd".repeat(32),
                "22".repeat(32),
                "33".repeat(32),
                42L,
                142L),
            sampleOpening("identifier_lookup_retail"),
            "opaque:" + "11".repeat(32),
            "22".repeat(32),
            "uaid:" + "33".repeat(31) + "35",
            accountId);
    final IdentifierReceiptFixture signed = signedIdentifierReceiptFixture(payload);
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            payload,
            new IdentifierReceiptAttestation("signed", signed.signatureHex(), null, null));
    final IdentifierPolicySummary policy =
        new IdentifierPolicySummary(
            "phone#retail",
            accountId,
            true,
            IdentifierNormalization.PHONE_E164,
            signed.resolverPublicKey(),
            "bfv-affine-sha3-256-v1",
            "bfv-v1",
            null,
            null,
            null);
    assert IdentifierReceiptVerifier.verify(receipt, policy)
        : "Identifier receipt verification must succeed";
  }

  private static void identifierReceiptVerifierRejectsAdversarialReceipts() {
    final String accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final IdentifierResolutionPayload payload = sampleIdentifierResolutionPayload(accountId, "66");
    final IdentifierReceiptFixture signed = signedIdentifierReceiptFixture(payload);
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            payload,
            new IdentifierReceiptAttestation("signed", signed.signatureHex(), null, null));
    final IdentifierPolicySummary policy =
        sampleIdentifierVerifierPolicy(accountId, signed.resolverPublicKey(), "phone#retail");
    assert IdentifierReceiptVerifier.verify(receipt, policy)
        : "baseline adversarial verifier fixture must be valid before tampering";

    final IdentifierResolutionReceipt tampered =
        new IdentifierResolutionReceipt(
            sampleIdentifierResolutionPayload(accountId, "67"),
            receipt.attestation());
    assert !IdentifierReceiptVerifier.verify(tampered, policy)
        : "tampered ciphertext-bound payload must not verify";

    expectIllegalArgument(
        () ->
            IdentifierReceiptVerifier.verify(
                receipt,
                sampleIdentifierVerifierPolicy(
                    accountId,
                    "ed25519:ed0120" + "11".repeat(32),
                    "phone#retail")),
        "identifier verifier must reject invalid Ed25519 resolver keys");

    expectIllegalArgument(
        () ->
            IdentifierReceiptVerifier.verify(
                new IdentifierResolutionReceipt(
                    payload,
                    new IdentifierReceiptAttestation("proof", null, "halo2/ipa", "AQID")),
                policy),
        "proof-only identifier attestations must be rejected by signature verifier");

    expectIllegalArgument(
        () ->
            IdentifierReceiptVerifier.verify(
                receipt,
                sampleIdentifierVerifierPolicy(accountId, signed.resolverPublicKey(), "email#retail")),
        "identifier verifier must reject policy-id mismatch");

    expectIllegalArgument(
        () ->
            IdentifierReceiptVerifier.verify(
                new IdentifierResolutionReceipt(
                    payload,
                    new IdentifierReceiptAttestation("signed", "abc", null, null)),
                policy),
        "identifier verifier must reject malformed signature hex");
  }

  private static void identifierResolutionReceiptParserRejectsNonExactReceiptTags() {
    final String accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
    final IdentifierResolutionPayload payload = sampleIdentifierResolutionPayload(accountId, "66");
    final IdentifierReceiptFixture signed = signedIdentifierReceiptFixture(payload);

    for (final String backend :
        new String[] {
          " bfv-affine-sha3-256-v1", "bfv-affine-sha3-256-v1 ", "BFV-AFFINE-SHA3-256-V1"
        }) {
      expectRuntimeException(
          () ->
              IdentifierJsonParser.parseResolutionReceipt(
                  identifierReceiptJson(
                          payload,
                          signed.signatureHex(),
                          backend,
                          payload.execution().verificationMode(),
                          null)
                      .getBytes(StandardCharsets.UTF_8)),
          "identifier parser must reject non-exact execution backend tags");
    }

    for (final String mode : new String[] {" signed", "signed ", "Signed"}) {
      expectRuntimeException(
          () ->
              IdentifierJsonParser.parseResolutionReceipt(
                  identifierReceiptJson(
                          payload,
                          signed.signatureHex(),
                          payload.execution().backend(),
                          mode,
                          null)
                      .getBytes(StandardCharsets.UTF_8)),
          "identifier parser must reject non-exact execution verification modes");
    }

    for (final String kind : new String[] {" signed", "signed ", "Signed"}) {
      final String attestationJson =
          "{\"kind\":" + jsonString(kind) + ",\"signature\":\"" + signed.signatureHex() + "\"}";
      expectRuntimeException(
          () ->
              IdentifierJsonParser.parseResolutionReceipt(
                  identifierReceiptJson(
                          payload,
                          signed.signatureHex(),
                          payload.execution().backend(),
                          payload.execution().verificationMode(),
                          attestationJson)
                      .getBytes(StandardCharsets.UTF_8)),
          "identifier parser must reject non-exact attestation kind tags");
    }

    for (final String signature :
        new String[] {" " + signed.signatureHex(), signed.signatureHex() + " "}) {
      final String attestationJson =
          "{\"kind\":\"signed\",\"signature\":" + jsonString(signature) + "}";
      expectRuntimeException(
          () ->
              IdentifierJsonParser.parseResolutionReceipt(
                  identifierReceiptJson(
                          payload,
                          signed.signatureHex(),
                          payload.execution().backend(),
                          payload.execution().verificationMode(),
                          attestationJson)
                      .getBytes(StandardCharsets.UTF_8)),
          "identifier parser must reject non-exact attestation signatures");
    }

    for (final String signature :
        new String[] {" " + payload.opening().signature(), payload.opening().signature() + " "}) {
      final String receiptJson =
          identifierReceiptJson(
                  payload,
                  signed.signatureHex(),
                  payload.execution().backend(),
                  payload.execution().verificationMode(),
                  null)
              .replace(
                  "\"signature\":" + jsonString(payload.opening().signature()),
                  "\"signature\":" + jsonString(signature));
      expectRuntimeException(
          () -> IdentifierJsonParser.parseResolutionReceipt(receiptJson.getBytes(StandardCharsets.UTF_8)),
          "identifier parser must reject non-exact opening signatures");
    }

    final String canonicalReceiptJson =
        identifierReceiptJson(
            payload,
            signed.signatureHex(),
            payload.execution().backend(),
            payload.execution().verificationMode(),
            null);
    for (final String[] hashCase :
        new String[][] {
          {
            "\"opaque_id\":" + jsonString(payload.opaqueId()),
            "\"opaque_id\":" + jsonString(" " + payload.opaqueId()),
            "opaque_id"
          },
          {
            "\"receipt_hash\":" + jsonString(payload.receiptHash()),
            "\"receipt_hash\":" + jsonString(payload.receiptHash() + " "),
            "receipt_hash"
          },
          {
            "\"uaid\":" + jsonString(payload.uaid()),
            "\"uaid\":" + jsonString(" " + payload.uaid()),
            "uaid"
          },
          {
            "\"program_digest\":" + jsonString(payload.execution().programDigest()),
            "\"program_digest\":" + jsonString(" " + payload.execution().programDigest()),
            "program_digest"
          },
          {
            "\"input_ciphertext_hash\":" + jsonString(payload.opening().payload().inputCiphertextHash()),
            "\"input_ciphertext_hash\":"
                + jsonString(payload.opening().payload().inputCiphertextHash() + " "),
            "opening input_ciphertext_hash"
          },
        }) {
      final String receiptJson = canonicalReceiptJson.replace(hashCase[0], hashCase[1]);
      expectRuntimeException(
          () -> IdentifierJsonParser.parseResolutionReceipt(receiptJson.getBytes(StandardCharsets.UTF_8)),
          "identifier parser hash exactness must reject non-exact " + hashCase[2]);
    }

    for (final String[] timestampCase :
        new String[][] {
          {
            "\"executed_at_ms\":" + payload.execution().executedAtMs(),
            "\"executed_at_ms\":-1",
            "executed_at_ms"
          },
          {
            "\"executed_at_ms\":" + payload.execution().executedAtMs(),
            "\"executed_at_ms\":\"9223372036854775808\"",
            "executed_at_ms overflow"
          },
          {
            "\"expires_at_ms\":" + payload.execution().expiresAtMs(),
            "\"expires_at_ms\":-1",
            "execution expires_at_ms"
          },
          {
            "\"opened_at_ms\":" + payload.opening().payload().openedAtMs(),
            "\"opened_at_ms\":-1",
            "opened_at_ms"
          },
          {
            "\"opened_at_ms\":"
                + payload.opening().payload().openedAtMs()
                + ",\"expires_at_ms\":"
                + payload.opening().payload().expiresAtMs(),
            "\"opened_at_ms\":"
                + payload.opening().payload().openedAtMs()
                + ",\"expires_at_ms\":-1",
            "opening expires_at_ms"
          },
        }) {
      final String receiptJson = canonicalReceiptJson.replace(timestampCase[0], timestampCase[1]);
      expectRuntimeException(
          () -> IdentifierJsonParser.parseResolutionReceipt(receiptJson.getBytes(StandardCharsets.UTF_8)),
          "identifier parser timestamp u64 must reject " + timestampCase[2]);
    }

    for (final String proofBackend : new String[] {" halo2/ipa", "halo2/ipa "}) {
      final String attestationJson =
          "{\"kind\":\"proof\",\"proof_backend\":"
              + jsonString(proofBackend)
              + ",\"proof_b64\":\"AQID\"}";
      expectRuntimeException(
          () ->
              IdentifierJsonParser.parseResolutionReceipt(
                  identifierReceiptJson(
                          payload,
                          signed.signatureHex(),
                          payload.execution().backend(),
                          payload.execution().verificationMode(),
                          attestationJson)
                      .getBytes(StandardCharsets.UTF_8)),
          "identifier parser must reject non-exact proof backend tags");
    }
    for (final String proofB64 : new String[] {" AQID", "AQID "}) {
      final String attestationJson =
          "{\"kind\":\"proof\",\"proof_backend\":\"halo2/ipa\",\"proof_b64\":"
              + jsonString(proofB64)
              + "}";
      expectRuntimeException(
          () ->
              IdentifierJsonParser.parseResolutionReceipt(
                  identifierReceiptJson(
                          payload,
                          signed.signatureHex(),
                          payload.execution().backend(),
                          payload.execution().verificationMode(),
                          attestationJson)
                      .getBytes(StandardCharsets.UTF_8)),
          "identifier parser must reject non-exact proof_b64");
    }
    final String malformedProofAttestationJson =
        "{\"kind\":\"proof\",\"proof_backend\":\"halo2/ipa\",\"proof_b64\":\"@@@\"}";
    expectRuntimeException(
        () ->
            IdentifierJsonParser.parseResolutionReceipt(
                identifierReceiptJson(
                        payload,
                        signed.signatureHex(),
                        payload.execution().backend(),
                        payload.execution().verificationMode(),
                        malformedProofAttestationJson)
                    .getBytes(StandardCharsets.UTF_8)),
        "identifier parser must reject malformed proof_b64");
  }

  private static void identifierReceiptVerifierMatchesSharedReceiptVectors()
      throws Exception {
    final Map<String, Object> fixture = loadSharedReceiptFixture();
    assert "identifier-receipt-attestation-v1".equals(fixture.get("vector_set"))
        : "shared identifier receipt fixture set mismatch";
    final IdentifierPolicySummary policy = identifierPolicyFromReceiptFixture(object(fixture, "policy"));
    final IdentifierResolutionReceipt receipt = identifierReceiptFromFixture(object(fixture, "receipt"));

    assert string(fixture, "canonical_payload_sha256")
        .equals(sha256Hex(IdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload())))
        : "canonical receipt payload digest mismatch";
    assert receipt.verifyAttestation(policy)
        : "shared identifier receipt vector must verify";

    for (final String policyId :
        new String[] {" phone#retail", "phone#retail ", "phone #retail", "phone# retail"}) {
      final IdentifierResolutionReceipt mutatedReceipt =
          identifierReceiptFromFixture(object(fixture, "receipt"), null, null, null, policyId);
      expectIllegalArgument(
          () -> mutatedReceipt.verifyAttestation(policy),
          "policy_id exactness " + policyId);
    }

    for (final String programId :
        new String[] {" identifier_lookup_retail", "identifier_lookup_retail "}) {
      final IdentifierResolutionReceipt mutatedExecutionProgram =
          identifierReceiptFromFixture(
              object(fixture, "receipt"), null, null, null, null, programId, null);
      expectIllegalArgument(
          () -> mutatedExecutionProgram.verifyAttestation(policy),
          "execution program_id exactness " + programId);

      final IdentifierResolutionReceipt mutatedOpeningProgram =
          identifierReceiptFromFixture(
              object(fixture, "receipt"), null, null, null, null, null, programId);
      expectIllegalArgument(
          () -> mutatedOpeningProgram.verifyAttestation(policy),
          "opening program_id exactness " + programId);
    }

    final String accountId = string(object(object(fixture, "receipt"), "payload"), "account_id");
    for (final String paddedAccountId : new String[] {" " + accountId, accountId + " "}) {
      final IdentifierResolutionReceipt mutatedAccount =
          identifierReceiptFromFixture(
              object(fixture, "receipt"),
              null,
              null,
              null,
              null,
              null,
              null,
              paddedAccountId);
      expectIllegalArgument(
          () -> mutatedAccount.verifyAttestation(policy),
          "account_id exactness " + paddedAccountId);
    }

    for (final Map<String, Object> vector : objectList(fixture, "attestation_vectors")) {
      final String name = string(vector, "name");
      final IdentifierReceiptAttestation attestation =
          identifierAttestationFromFixture(object(vector, "attestation"), null);
      final byte[] encoded = IdentifierReceiptCanonicalEncoder.encodeAttestation(attestation);
      assert number(vector, "expected_attestation_bytes").intValue() == encoded.length
          : name + " attestation byte length mismatch";
      assert string(vector, "expected_attestation_sha256").equals(sha256Hex(encoded))
          : name + " attestation digest mismatch";

      final IdentifierReceiptAttestation decoded =
          IdentifierReceiptCanonicalEncoder.decodeAttestation(encoded);
      assert attestation.kind().equals(decoded.kind()) : name + " attestation kind mismatch";
      if ("signed".equals(attestation.kind())) {
        assert attestation.signature().equalsIgnoreCase(decoded.signature())
            : name + " signature roundtrip mismatch";
      } else if ("proof".equals(attestation.kind())) {
        assert Objects.equals(attestation.proofBackend(), decoded.proofBackend())
            : name + " proof backend roundtrip mismatch";
        assert Objects.equals(attestation.proofB64(), decoded.proofB64())
            : name + " proof payload roundtrip mismatch";
        expectIllegalArgument(
            () -> new IdentifierResolutionReceipt(receipt.payload(), attestation)
                .verifyAttestation(policy),
            name + " proof verifier gate");
      } else {
        throw new IllegalArgumentException("Unhandled attestation kind " + attestation.kind());
      }
    }

    for (final Map<String, Object> negative : objectList(fixture, "negative_cases")) {
      final String mutation = string(negative, "mutation");
      final java.util.function.Supplier<IdentifierPolicySummary> mutatedPolicy =
          () -> {
            if ("policy.resolver_public_key".equals(mutation)) {
              return identifierPolicyFromReceiptFixture(
                  object(fixture, "policy"), null, string(negative, "value"));
            }
            if ("policy.policy_id".equals(mutation)) {
              return identifierPolicyFromReceiptFixture(
                  object(fixture, "policy"), string(negative, "value"), null);
            }
            return policy;
          };
      final IdentifierResolutionReceipt mutatedReceipt;
      if ("receipt.payload.execution.output_ciphertext_hash".equals(mutation)) {
        mutatedReceipt =
            identifierReceiptFromFixture(
                object(fixture, "receipt"), string(negative, "value"), null, null);
      } else if ("receipt.attestation.signature".equals(mutation)) {
        mutatedReceipt =
            identifierReceiptFromFixture(
                object(fixture, "receipt"), null, string(negative, "value"), null);
      } else if ("receipt.attestation".equals(mutation)) {
        mutatedReceipt =
            identifierReceiptFromFixture(
                object(fixture, "receipt"), null, null, object(negative, "value"));
      } else {
        mutatedReceipt = receipt;
      }

      if (negative.get("expected_error_contains") instanceof String) {
        expectIllegalArgument(
            () -> mutatedReceipt.verifyAttestation(mutatedPolicy.get()),
            string(negative, "name"));
      } else {
        final boolean expected = Boolean.TRUE.equals(negative.get("expected_result"));
        assert mutatedReceipt.verifyAttestation(mutatedPolicy.get()) == expected
            : string(negative, "name") + " expected " + expected;
      }
    }
  }

  private static IdentifierResolutionPayload sampleIdentifierResolutionPayload(
      final String accountId, final String outputCiphertextByte) {
    return new IdentifierResolutionPayload(
        "phone#retail",
        new IdentifierResolutionExecutionPayload(
            "identifier_lookup_retail",
            "44".repeat(32),
            "bfv-programmed-sha3-256-v1",
            "signed",
            "55".repeat(32),
            outputCiphertextByte.repeat(32),
            "77".repeat(32),
            "88".repeat(32),
            "99".repeat(32),
            "aa".repeat(32),
            42L,
            142L),
        sampleOpening("identifier_lookup_retail"),
        "opaque:" + "11".repeat(32),
        "22".repeat(32),
        "uaid:" + "33".repeat(31) + "35",
        accountId);
  }

  private static IdentifierPolicySummary sampleIdentifierVerifierPolicy(
      final String accountId, final String resolverPublicKey, final String policyId) {
    return new IdentifierPolicySummary(
        policyId,
        accountId,
        true,
        IdentifierNormalization.PHONE_E164,
        resolverPublicKey,
        "bfv-programmed-sha3-256-v1",
        "bfv-v1",
        null,
        null,
        null);
  }

  private static void invalidateAndCancelDelegatesToExecutor() {
    final InvalidationTrackingExecutor executor = new InvalidationTrackingExecutor();
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(URI.create("https://localhost:8080")).build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    transport.invalidateAndCancel();
    assert executor.invalidated : "invalidateAndCancel should reach the executor";
  }

  private static void assertCanonicalSignature(
      final TransportRequest request,
      final java.security.PublicKey publicKey,
      final long timestampMs,
      final String nonce) throws Exception {
    final byte[] signature =
        Base64.getDecoder()
            .decode(request.headers().get(CanonicalRequestSigner.HEADER_SIGNATURE).get(0));
    final byte[] message =
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            request.method(), request.uri(), request.body(), timestampMs, nonce);
    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(publicKey);
    verifier.update(message);
    assert verifier.verify(signature) : "canonical request signature mismatch";
  }

  private static ToriiCanonicalRequestAuth canonicalAuth(
      final String accountId,
      final KeyPair keyPair,
      final Long timestampMs,
      final String nonce) {
    return new ToriiCanonicalRequestAuth(
        accountId,
        message -> signEd25519(keyPair, message),
        timestampMs,
        nonce);
  }

  private static byte[] signEd25519(final KeyPair keyPair, final byte[] message) {
    try {
      final Signature signer = Signature.getInstance("Ed25519");
      signer.initSign(keyPair.getPrivate());
      signer.update(message);
      return signer.sign();
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to sign canonical request fixture", ex);
    }
  }

  private static String vpnProfileJson() {
    return "{"
        + "\"available\":true,"
        + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
        + "\"supported_exit_classes\":[\"standard\",\"low-latency\",\"high-security\"],"
        + "\"default_exit_class\":\"standard\","
        + "\"lease_secs\":600,"
        + "\"dns_push_interval_secs\":60,"
        + "\"meter_family\":\"soranet.vpn.standard\","
        + "\"route_pushes\":[\"0.0.0.0/0\"],"
        + "\"excluded_routes\":[\"10.0.0.0/8\"],"
        + "\"dns_servers\":[\"1.1.1.1\"],"
        + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
        + "\"mtu_bytes\":1280,"
        + "\"display_billing_label\":\"standard XOR\","
        + "\"operator_account_id\":\"sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT\","
        + "\"lease_fee\":\"1000000.25\","
        + "\"settlement_grace_secs\":120,"
        + "\"flow_label_bits\":24,"
        + "\"padding_budget_ms\":15,"
        + "\"relay_id_hex\":\""
        + VALID_ED25519_PUBLIC_KEY_HEX
        + "\","
        + "\"descriptor_commit_hex\":\""
        + "cd".repeat(32)
        + "\","
        + "\"tls_server_name\":\"relay.example\","
        + "\"relay_tls_spki_sha256_hex\":\""
        + "ab".repeat(32)
        + "\","
        + "\"relay_certificate_sha256_hex\":\""
        + "ef".repeat(32)
        + "\","
        + "\"directory_snapshot_digest_hex\":\""
        + "42".repeat(32)
        + "\""
        + "}";
  }

  private static String vpnQuoteJson(final String quoteId, final String meteringKey) {
    return "{"
        + "\"quote_id\":\""
        + quoteId
        + "\",\"lease_id_hex\":\""
        + quoteId
        + "\",\"session_id_hex\":\""
        + "aa".repeat(16)
        + "\",\"payment_reference\":\""
        + quoteId
        + "\",\"account_id\":\"alice\","
        + "\"exit_class\":\"low-latency\","
        + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
        + "\"lease_secs\":600,"
        + "\"quote_expires_at_ms\":1700000600000,"
        + "\"fee_asset_id\":\"xor#universal.universal\","
        + "\"escrow_account_id\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\","
        + "\"operator_account_id\":\"sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT\","
        + "\"lease_fee\":\"1000000.25\","
        + "\"route_pushes\":[\"0.0.0.0/0\"],"
        + "\"excluded_routes\":[],"
        + "\"dns_servers\":[\"1.1.1.1\"],"
        + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
        + "\"mtu_bytes\":1280,"
        + "\"meter_family\":\"soranet.vpn.standard\","
        + "\"flow_label_bits\":24,"
        + "\"padding_budget_ms\":15,"
        + "\"relay_id_hex\":\""
        + meteringKey
        + "\","
        + "\"descriptor_commit_hex\":\""
        + "cd".repeat(32)
        + "\","
        + "\"tls_server_name\":\"relay.example\","
        + "\"relay_tls_spki_sha256_hex\":\""
        + "ab".repeat(32)
        + "\","
        + "\"relay_certificate_sha256_hex\":\""
        + "ef".repeat(32)
        + "\","
        + "\"directory_snapshot_digest_hex\":\""
        + "42".repeat(32)
        + "\",\"metering_public_key_hex\":\""
        + meteringKey
        + "\",\"open_lease_instruction\":{"
        + "\"wire_id\":\"iroha_data_model::isi::vpn::OpenVpnLeaseEscrow\","
        + "\"payload_hex\":\"cafe\"}"
        + "}";
  }

  private static String vpnSessionJson(final String sessionId, final String paymentTxHash) {
    return "{"
        + "\"session_id\":\""
        + sessionId
        + "\",\"account_id\":\"alice\","
        + "\"exit_class\":\"standard\","
        + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
        + "\"lease_secs\":600,"
        + "\"expires_at_ms\":1700000600000,"
        + "\"connected_at_ms\":1700000000000,"
        + "\"meter_family\":\"soranet.vpn.standard\","
        + "\"quote_id\":\""
        + sessionId
        + "\",\"payment_reference\":\""
        + sessionId
        + "\",\"payment_tx_hash\":\""
        + paymentTxHash
        + "\",\"fee_asset_id\":\"xor#universal.universal\","
        + "\"escrow_account_id\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\","
        + "\"operator_account_id\":\"sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT\","
        + "\"lease_fee\":\"1000000.25\","
        + "\"flow_label_bits\":24,"
        + "\"padding_budget_ms\":15,"
        + "\"relay_id_hex\":\""
        + VALID_ED25519_PUBLIC_KEY_HEX
        + "\","
        + "\"descriptor_commit_hex\":\""
        + "cd".repeat(32)
        + "\","
        + "\"tls_server_name\":\"relay.example\","
        + "\"relay_tls_spki_sha256_hex\":\""
        + "ab".repeat(32)
        + "\","
        + "\"relay_certificate_sha256_hex\":\""
        + "ef".repeat(32)
        + "\","
        + "\"directory_snapshot_digest_hex\":\""
        + "42".repeat(32)
        + "\",\"route_pushes\":[\"0.0.0.0/0\"],"
        + "\"excluded_routes\":[],"
        + "\"dns_servers\":[\"1.1.1.1\"],"
        + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
        + "\"mtu_bytes\":1280,"
        + "\"helper_ticket_hex\":\""
        + VPN_HELPER_TICKET_HEX
        + "\","
        + "\"bytes_in\":0,"
        + "\"bytes_out\":0,"
        + "\"status\":\"active\""
        + "}";
  }

  private static String vpnReceiptJson(
      final String sessionId, final String paymentTxHash, final boolean settled) {
    final String status = settled ? "settled" : "disconnected";
    final String source = settled ? "relay" : "torii";
    final String earned = settled ? "750000.125" : "0";
    final String refunded = settled ? "250000.125" : "1000000.25";
    final String settlement =
        settled
            ? ",\"settle_lease_instruction\":{"
                + "\"wire_id\":\"iroha_data_model::isi::vpn::SettleVpnLease\","
                + "\"payload_hex\":\"f00d\"}"
            : ",\"settle_lease_instruction\":null";
    return "{"
        + "\"session_id\":\""
        + sessionId
        + "\",\"account_id\":\"alice\","
        + "\"exit_class\":\"standard\","
        + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
        + "\"meter_family\":\"soranet.vpn.standard\","
        + "\"connected_at_ms\":1700000000000,"
        + "\"disconnected_at_ms\":1700000010000,"
        + "\"duration_ms\":10000,"
        + "\"bytes_in\":1024,"
        + "\"bytes_out\":2048,"
        + "\"status\":\""
        + status
        + "\",\"receipt_source\":\""
        + source
        + "\",\"quote_id\":\""
        + sessionId
        + "\",\"payment_tx_hash\":\""
        + paymentTxHash
        + "\",\"fee_asset_id\":\"xor#universal.universal\","
        + "\"escrow_account_id\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\","
        + "\"operator_account_id\":\"sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT\","
        + "\"lease_fee\":\"1000000.25\","
        + "\"earned_fee\":\""
        + earned
        + "\",\"refunded_fee\":\""
        + refunded
        + "\",\"lease_id_hex\":\""
        + sessionId
        + "\""
        + settlement
        + "}";
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02X", b & 0xFF));
    }
    return builder.toString();
  }

  private static byte[] hexToBytes(final String hex) {
    final byte[] bytes = new byte[hex.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      final int offset = index * 2;
      bytes[index] = (byte) Integer.parseInt(hex.substring(offset, offset + 2), 16);
    }
    return bytes;
  }

  private static String readBody(final TransportRequest request) {
    return new String(request.body(), StandardCharsets.UTF_8);
  }

  private record IdentifierReceiptFixture(
      String resolverPublicKey,
      String signatureHex) {}

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private TransportRequest lastRequest;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = Objects.requireNonNull(request, "request");
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", Map.of()));
    }
  }

  private static final class FailingExecutor implements HttpTransportExecutor {
    private final RuntimeException error;

    private FailingExecutor(final RuntimeException error) {
      this.error = error;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      future.completeExceptionally(error);
      return future;
    }
  }

  private static final class CountingFailingExecutor implements HttpTransportExecutor {
    private final RuntimeException error;
    private int callCount = 0;

    private CountingFailingExecutor(final RuntimeException error) {
      this.error = error;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      callCount++;
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      future.completeExceptionally(error);
      return future;
    }
  }

  private static final class SequencedExecutor implements HttpTransportExecutor {
    private int callCount = 0;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      callCount++;
      if (callCount == 1) {
        return CompletableFuture.completedFuture(
            new TransportResponse(503, "retry".getBytes(StandardCharsets.UTF_8), "", Map.of()));
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", Map.of()));
    }
  }

  private static final class ScriptedExecutor implements HttpTransportExecutor {
    private final TransportResponse[] responses;
    private int index = 0;

    private ScriptedExecutor(final TransportResponse... responses) {
      this.responses = Objects.requireNonNull(responses, "responses");
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      final int position = index < responses.length ? index : responses.length - 1;
      index++;
      return CompletableFuture.completedFuture(responses[position]);
    }
  }

  private static final class RecordingExecutor implements HttpTransportExecutor {
    private final List<byte[]> payloads = new ArrayList<>();

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      try {
        payloads.add(request.body());
      } catch (final Exception ex) {
        final CompletableFuture<TransportResponse> failed = new CompletableFuture<>();
        failed.completeExceptionally(ex);
        return failed;
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", Map.of()));
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<GaugeEvent> events = new ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {}

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      events.add(new GaugeEvent(signalId, fields));
    }

    GaugeEvent lastEvent(final String signalId) {
      for (int i = events.size() - 1; i >= 0; --i) {
        final GaugeEvent event = events.get(i);
        if (event.signalId().equals(signalId)) {
          return event;
        }
      }
      return null;
    }

    List<Map<String, Object>> eventsBySignal(final String signalId) {
      final List<Map<String, Object>> matches = new ArrayList<>();
      for (final GaugeEvent event : events) {
        if (event.signalId().equals(signalId)) {
          matches.add(event.fields());
        }
      }
      return matches;
    }

    private static final class GaugeEvent {
      private final String signalId;
      private final Map<String, Object> fields;

      private GaugeEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = fields;
      }

      String signalId() {
        return signalId;
      }

      Map<String, Object> fields() {
        return fields;
      }
    }
  }

  private static final class StubResponseExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;

    private StubResponseExecutor(final int statusCode, final byte[] body) {
      this(statusCode, body, "accepted");
    }

    private StubResponseExecutor(
        final int statusCode, final byte[] body, final String message) {
      this.response = new TransportResponse(statusCode, body, message, Map.of());
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      return CompletableFuture.completedFuture(response);
    }

    TransportRequest lastRequest() {
      return lastRequest;
    }
  }

  private static final class OneResponseExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;
    private int requestCount;

    private OneResponseExecutor(final TransportResponse response) {
      this.response = Objects.requireNonNull(response, "response");
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = Objects.requireNonNull(request, "request");
      requestCount++;
      return CompletableFuture.completedFuture(response);
    }
  }

  private record QueuedResponse(int statusCode, String body) {}

  private static final class QueueResponseExecutor implements HttpTransportExecutor {
    private final java.util.ArrayDeque<QueuedResponse> responses;
    private final List<TransportRequest> requests = new ArrayList<>();

    private QueueResponseExecutor(final List<QueuedResponse> responses) {
      this.responses = new java.util.ArrayDeque<>(responses);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      final QueuedResponse response = responses.removeFirst();
      return CompletableFuture.completedFuture(
          new TransportResponse(
              response.statusCode(),
              response.body().getBytes(StandardCharsets.UTF_8),
              "ok",
              Map.of()));
    }

    List<TransportRequest> requests() {
      return requests;
    }
  }

  private static final class InvalidationTrackingExecutor implements HttpTransportExecutor {
    private boolean invalidated = false;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      return CompletableFuture.completedFuture(
          new TransportResponse(200, new byte[0], "ok", Map.of()));
    }

    @Override
    public void invalidateAndCancel() {
      invalidated = true;
    }
  }

  private static final class RecordingObserver implements ClientObserver {
    private final AtomicInteger requestCount = new AtomicInteger();
    private final AtomicInteger responseCount = new AtomicInteger();
    private final AtomicInteger failureCount = new AtomicInteger();

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount.incrementAndGet();
    }

    @Override
    public void onResponse(final TransportRequest request, final ClientResponse response) {
      responseCount.incrementAndGet();
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failureCount.incrementAndGet();
    }
  }

  private static int aliasCounter = 0;

  private static SignedTransaction transactionWithPayload(final byte fillValue) {
    final byte[] signature = new byte[64];
    final byte[] publicKey = new byte[32];
    java.util.Arrays.fill(signature, (byte) (fillValue + 1));
    java.util.Arrays.fill(publicKey, (byte) (fillValue + 2));
    final TransactionPayload payload =
        TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList(), 1L))
            .setChainId(String.format("%08x", fillValue))
            .setAuthority(TestAccountIds.ed25519Authority(0x26))
            .setCreationTimeMs(1_700_000_000_000L + (fillValue & 0xFF))
            .setInstructionBytes(new byte[] {fillValue, (byte) (fillValue + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce(fillValue & 0xFF)
            .setMetadata(Map.of("note", "txn-" + fillValue))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final byte[] encodedPayload;
    try {
      encodedPayload = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }
    return new SignedTransaction(
        encodedPayload, signature, publicKey, codec.schemaName(), "alias-" + aliasCounter++);
  }

  private static byte[] statusPayload(final String hash, final String kind) {
    final boolean applied = "Applied".equals(kind);
    final String json =
        "{\"hash\":\""
            + hash
            + "\",\"status\":{\"kind\":\""
            + kind
            + "\""
            + (applied ? ",\"block_height\":7" : "")
            + "},\"summary\":\""
            + kind
            + "\",\"diagnostics\":[],\"scope\":\"global\",\"resolved_from\":\""
            + (applied ? "state" : "cache")
            + "\"}";
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static boolean payloadEquals(
      final SignedTransaction expected, final SignedTransaction actual) {
    return java.util.Arrays.equals(expected.encodedPayload(), actual.encodedPayload())
        && java.util.Arrays.equals(expected.signature(), actual.signature())
        && java.util.Arrays.equals(expected.publicKey(), actual.publicKey())
        && expected.schemaName().equals(actual.schemaName())
        && expected.keyAlias().equals(actual.keyAlias())
        && expected.exportedKeyBundle().equals(actual.exportedKeyBundle());
  }

}
