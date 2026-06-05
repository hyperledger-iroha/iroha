package org.hyperledger.iroha.android.client;

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
import java.security.Signature;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference;
import org.hyperledger.iroha.android.client.queue.FilePendingTransactionQueue;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
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
import org.hyperledger.iroha.android.sccp.EvmSccpProver;
import org.hyperledger.iroha.android.sccp.SourceSccpProofs;
import org.hyperledger.iroha.android.sccp.TronSccpProver;
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
  private static final String VPN_HELPER_TICKET_HEX = "5356504e48543100" + "00".repeat(248);
  private static final String SCCP_TEST_MESSAGE_ID = "11".repeat(32);
  private static final String SCCP_TEST_COMMITMENT_ROOT = "33".repeat(32);
  private static final String SCCP_TEST_GROTH16_PROOF_HEX =
      sampleSccpProofHex(SCCP_TEST_MESSAGE_ID, 0, SCCP_TEST_COMMITMENT_ROOT);
  private static final String SCCP_TEST_TRON_NETWORK_ID = "71".repeat(32);
  private static final String SCCP_TEST_TRON_VERIFIER_ADDRESS =
      "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8";
  private static final String SCCP_TEST_TRON_VERIFIER_CODE_HASH = "72".repeat(32);
  private static final String SCCP_TEST_TRON_VERIFIER_KEY_HASH = "73".repeat(32);
  private static final String SCCP_TEST_EVM_NETWORK_ID = "aa".repeat(32);
  private static final String SCCP_TEST_EVM_VERIFIER_ADDRESS = "bb".repeat(20);
  private static final String SCCP_TEST_EVM_BRIDGE_ADDRESS = "cc".repeat(20);
  private static final String SCCP_TEST_EVM_VERIFIER_CODE_HASH = "dd".repeat(32);
  private static final String SCCP_TEST_EVM_VERIFIER_KEY_HASH = "ee".repeat(32);

  private HttpClientTransportTests() {}

  private static String abiWordHex(final int value) {
    final String hex = Integer.toHexString(value);
    final StringBuilder out = new StringBuilder(64);
    for (int i = hex.length(); i < 64; i++) {
      out.append('0');
    }
    out.append(hex);
    return out.toString();
  }

  private static String sampleSccpProofHex(
      final String messageId, final int sourceDomain, final String commitmentRoot) {
    final StringBuilder out = new StringBuilder(384 * 2);
    out.append(abiWordHex(1));
    out.append(messageId);
    out.append(abiWordHex(sourceDomain));
    out.append(commitmentRoot);
    out.append(abiWordHex(1));
    out.append(abiWordHex(2));
    out.append("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed");
    out.append("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2");
    out.append("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa");
    out.append("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b");
    out.append(abiWordHex(1));
    out.append(abiWordHex(2));
    return out.toString();
  }

  private static String sccpMessageBundleJson() {
    return "{\"version\":1,\"commitment_root\":\""
        + SCCP_TEST_COMMITMENT_ROOT
        + "\",\"commitment\":{\"version\":1,\"kind\":\"Transfer\",\"target_domain\":5,"
        + "\"message_id\":\""
        + SCCP_TEST_MESSAGE_ID
        + "\",\"payload_hash\":\""
        + "22".repeat(32)
        + "\"}}";
  }

  private static byte[] sccpBridgeMessageBody(final String proofHex) {
    return ("{\"authority\":\"alice\",\"message_bundle\":"
            + sccpMessageBundleJson()
            + ",\"proof_bytes_hex\":\"0x"
            + proofHex
            + "\"}")
        .getBytes(StandardCharsets.UTF_8);
  }

  private static String sampleSccpTronDestinationBindingHash() {
    final String hash =
        SourceSccpProofs.tronDestinationBindingHash(
            SourceSccpProofs.DOMAIN_SORA,
            SourceSccpProofs.DOMAIN_TRON,
            "0x" + SCCP_TEST_TRON_NETWORK_ID,
            SCCP_TEST_TRON_VERIFIER_ADDRESS,
            "0x" + SCCP_TEST_TRON_VERIFIER_CODE_HASH,
            "0x" + SCCP_TEST_TRON_VERIFIER_KEY_HASH);
    return hash.startsWith("0x") ? hash.substring(2) : hash;
  }

  public static void main(final String[] args) throws Exception {
    submitBuildsToriiRequest();
    submitTransactionJsonBuildsJsonIngressRequest();
    bridgeSubmitJsonHelpersPostRawProofAndMessagePayloads();
    bridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions();
    bridgeSubmitJsonHelpersRejectPlaceholderProofBytesBeforeRequest();
    submitPropagatesExecutorFailure();
    submitSkipsRetryWhenNetworkRetriesDisabled();
    submitRetriesOnServerError();
    retryPolicyRecognizesRetryableStatus();
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
    uaidRequestsRespectBasePath();
    uaidBindingsRequestParsesResponse();
    uaidManifestsRequestSupportsQuery();
    identifierPoliciesRequestParsesResponse();
    ramLfeProgramPoliciesRequestParsesResponse();
    identifierResolveRequestParsesResponse();
    identifierResolveRequestParsesProgrammedReceiptResponse();
    identifierResolveRequestAllowsNotFound();
    identifierHiddenFunctionRequestsRejectMalformedCiphertextEnvelopeFields();
    identifierClaimLookupAllowsNotFound();
    identifierClaimReceiptUsesAccountPath();
    ramLfeExecuteRequestParsesResponse();
    ramLfeExecuteRequestAllowsNotFound();
    ramLfeReceiptVerifyUsesRawReceipt();
    vpnProfileRequestParsesNativeLeaseFields();
    vpnQuoteRequestSignsCanonicalBodyAndParsesOpenLeaseInstruction();
    vpnSessionAndReceiptRequestsUseNativeLeaseDtos();
    deployContractRequestParsesResponse();
    callContractRequestParsesResponse();
    proposeMultisigRequestParsesResponse();
    proposeMultisigRejectsAdversarialRequestShapes();
    multisigResponseParserRejectsMalformedFields();
    callContractRejectsAmbiguousTarget();
    governanceContractRequestParsesResponse();
    resolveAccountAliasRequestParsesResponse();
    resolveAccountAliasRequestParsesResponseWithoutIndex();
    resolveAccountAliasAllowsNotFound();
    resolveAccountAliasRejectsNonIntegerIndex();
    resolveAccountAliasFailsOnMalformedJson();
    identifierNormalizationCanonicalizesInputs();
    identifierBfvEnvelopeBuilderMatchesSharedSoracloudVectors();
    identifierBfvEnvelopeBuilderMatchesSharedSoracloudOperationInputVectors();
    sharedSoracloudBfvKeyBundleComponentVectorsAreComplete();
    identifierBfvEnvelopeBuilderProducesDeterministicCiphertext();
    identifierBfvEnvelopeBuilderRejectsAdversarialPublicParameters();
    identifierReceiptVerifierAcceptsEd25519Receipt();
    identifierReceiptVerifierRejectsAdversarialReceipts();
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

    assert observer.requestCount.get() == 1 : "Observer must see request";
    assert observer.responseCount.get() == 1 : "Observer must see response";
    assert observer.failureCount.get() == 0 : "Observer must not see failure";
  }

  private static void submitTransactionJsonBuildsJsonIngressRequest() {
    final CapturingExecutor executor = new CapturingExecutor();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://127.0.0.1:8080"))
            .setWireFormatPreference(WireFormatPreference.JSON_PREFERRED)
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    final byte[] body = "{\"version\":1,\"content\":{}}".getBytes(StandardCharsets.UTF_8);

    final ClientResponse response = transport.submitTransactionJson(body).join();

    assert response.statusCode() == 202 : "Expected JSON submit to be accepted";
    final TransportRequest request = executor.lastRequest;
    assert "POST".equals(request.method()) : "JSON submit must use POST";
    assert request.uri().toString().equals("https://127.0.0.1:8080/v1/pipeline/transactions")
        : "JSON submit endpoint must target Torii pipeline route";
    assert request.headers().get("Content-Type").contains("application/json")
        : "JSON submit Content-Type must be application/json";
    assert request.headers().get("Accept").contains(WireFormatPreference.JSON_PREFERRED.acceptHeader())
        : "JSON submit Accept header must use configured wire preference";
    assert java.util.Arrays.equals(body, request.body()) : "JSON submit body must be preserved";
  }

  private static void bridgeSubmitJsonHelpersPostRawProofAndMessagePayloads() {
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, "{\"ok\":true}".getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://127.0.0.1:8080/base"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    final String bindingHash = sampleSccpTronDestinationBindingHash();
    final byte[] proofBody =
        ("{\"authority\":\"alice\",\"message_bundle\":"
                + sccpMessageBundleJson()
                + ",\"network_id_hex\":\"0x"
                + SCCP_TEST_TRON_NETWORK_ID
                + "\",\"verifier_code_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_CODE_HASH
                + "\",\"verifier_key_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_KEY_HASH
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + bindingHash
                + "\",\"tron_verifier_address\":\""
                + SCCP_TEST_TRON_VERIFIER_ADDRESS
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] messageBody =
        "{\"authority\":\"alice\",\"message_bundle\":{}}".getBytes(StandardCharsets.UTF_8);
    final Map<String, Object> commitment = new LinkedHashMap<>();
    commitment.put("message_id", SCCP_TEST_MESSAGE_ID);
    final Map<String, Object> typedMessageBundle = new LinkedHashMap<>();
    typedMessageBundle.put("commitment_root", SCCP_TEST_COMMITMENT_ROOT);
    typedMessageBundle.put("commitment", commitment);

    final ClientResponse proofResponse = transport.postBridgeProofSubmitJson(proofBody).join();
    final TransportRequest proofRequest = executor.lastRequest();
    assert proofResponse.statusCode() == 200 : "Bridge proof submit must expect HTTP 200";
    assert "POST".equals(proofRequest.method()) : "Bridge proof submit must use POST";
    assert proofRequest
        .uri()
        .toString()
        .equals("https://127.0.0.1:8080/base/v1/bridge/proofs/submit")
        : "Bridge proof submit endpoint must target Torii bridge route";
    assert proofRequest.headers().get("Content-Type").contains("application/json")
        : "Bridge proof submit Content-Type must be application/json";
    assert proofRequest.headers().get("Accept").contains("application/json")
        : "Bridge proof submit Accept header must be application/json";
    assert java.util.Arrays.equals(proofBody, proofRequest.body())
        : "Bridge proof submit body must be preserved";

    final ClientResponse messageResponse = transport.postBridgeMessageSubmitJson(messageBody).join();
    final TransportRequest messageRequest = executor.lastRequest();
    assert messageResponse.statusCode() == 200 : "Bridge message submit must expect HTTP 200";
    assert "POST".equals(messageRequest.method()) : "Bridge message submit must use POST";
    assert messageRequest
        .uri()
        .toString()
        .equals("https://127.0.0.1:8080/base/v1/bridge/messages")
        : "Bridge message submit endpoint must target Torii bridge route";
    assert messageRequest.headers().get("Content-Type").contains("application/json")
        : "Bridge message submit Content-Type must be application/json";
    assert messageRequest.headers().get("Accept").contains("application/json")
        : "Bridge message submit Accept header must be application/json";
    assert java.util.Arrays.equals(messageBody, messageRequest.body())
        : "Bridge message submit body must be preserved";

    final ClientResponse typedResponse =
        transport
            .submitBridgeProof(
                BridgeProofSubmitRequest.builder()
                    .authority("alice")
                    .messageBundle(typedMessageBundle)
                    .networkIdHex("0x" + SCCP_TEST_TRON_NETWORK_ID)
                    .verifierCodeHashHex("0x" + SCCP_TEST_TRON_VERIFIER_CODE_HASH)
                    .verifierKeyHashHex("0x" + SCCP_TEST_TRON_VERIFIER_KEY_HASH)
                    .expectedDestinationBindingHashHex("0x" + bindingHash)
                    .tronVerifierAddress(SCCP_TEST_TRON_VERIFIER_ADDRESS)
                    .proofBytesHex("0x" + SCCP_TEST_GROTH16_PROOF_HEX)
                    .build())
            .join();
    final TransportRequest typedRequest = executor.lastRequest();
    final String typedBody =
        "{\"authority\":\"alice\",\"expected_destination_binding_hash_hex\":\"0x"
            + bindingHash
            + "\",\"message_bundle\":{\"commitment\":{\"message_id\":\""
            + SCCP_TEST_MESSAGE_ID
            + "\"},\"commitment_root\":\""
            + SCCP_TEST_COMMITMENT_ROOT
            + "\"},\"network_id_hex\":\"0x"
            + SCCP_TEST_TRON_NETWORK_ID
            + "\",\"proof_bytes_hex\":\"0x"
            + SCCP_TEST_GROTH16_PROOF_HEX
            + "\",\"tron_verifier_address\":\""
            + SCCP_TEST_TRON_VERIFIER_ADDRESS
            + "\",\"verifier_code_hash_hex\":\"0x"
            + SCCP_TEST_TRON_VERIFIER_CODE_HASH
            + "\",\"verifier_key_hash_hex\":\"0x"
            + SCCP_TEST_TRON_VERIFIER_KEY_HASH
            + "\"}";
    assert typedResponse.statusCode() == 200 : "Typed bridge proof submit must expect HTTP 200";
    assert typedRequest
        .uri()
        .toString()
        .equals("https://127.0.0.1:8080/base/v1/bridge/proofs/submit")
        : "Typed bridge proof submit endpoint mismatch";
    assert typedBody.equals(new String(typedRequest.body(), StandardCharsets.UTF_8))
        : "Typed bridge proof submit body mismatch";
  }

  @SuppressWarnings("unchecked")
  private static void bridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions() {
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, "{\"ok\":true}".getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://127.0.0.1:8080/base"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    final Map<String, Object> commitment = new LinkedHashMap<>();
    commitment.put("message_id", SCCP_TEST_MESSAGE_ID);
    final Map<String, Object> messageBundle = new LinkedHashMap<>();
    messageBundle.put("commitment_root", SCCP_TEST_COMMITMENT_ROOT);
    messageBundle.put("commitment", commitment);
    final byte[] proofBytes = hexToBytes(SCCP_TEST_GROTH16_PROOF_HEX);

    final SourceSccpProofs.EvmDestinationBinding evmBinding =
        SourceSccpProofs.evmDestinationBinding(
            SourceSccpProofs.DOMAIN_SORA,
            SourceSccpProofs.DOMAIN_ETH,
            "0x" + "71".repeat(32),
            "0x" + "44".repeat(20),
            "0x" + "45".repeat(20),
            "0x" + "72".repeat(32),
            "0x" + "73".repeat(32));
    final EvmSccpProver.Submission evmSubmission =
        EvmSccpProver.buildSubmission(
            new EvmSccpProver.SubmissionInput(
                new EvmSccpProver.PublicInputsInput(
                    1,
                    SCCP_TEST_MESSAGE_ID,
                    "22".repeat(32),
                    SourceSccpProofs.DOMAIN_ETH,
                    SCCP_TEST_COMMITMENT_ROOT,
                    "7",
                    "44".repeat(32)),
                proofBytes,
                "0x" + "55".repeat(32),
                evmBinding));

    transport
        .submitBridgeProof(
            BridgeProofSubmitRequest.fromEvmSccpSubmission(
                "alice", messageBundle, evmSubmission, evmBinding))
        .join();

    final Map<String, Object> evmBody =
        (Map<String, Object>) JsonParser.parse(readBody(executor.lastRequest()));
    assert evmBinding.networkId.equals(evmBody.get("network_id_hex"))
        : "EVM submit request must carry governed network id";
    assert evmBinding.verifierAddress.equals(evmBody.get("verifier_address_hex"))
        : "EVM submit request must carry governed verifier";
    assert evmBinding.bridgeAddress.equals(evmBody.get("bridge_address_hex"))
        : "EVM submit request must carry governed bridge";
    assert evmBinding.verifierCodeHash.equals(evmBody.get("verifier_code_hash_hex"))
        : "EVM submit request must carry governed code hash";
    assert evmBinding.verifierKeyHash.equals(evmBody.get("verifier_key_hash_hex"))
        : "EVM submit request must carry governed verifier key hash";
    assert evmBinding.hash.equals(evmBody.get("expected_destination_binding_hash_hex"))
        : "EVM submit request must carry governed binding hash";
    assert ("0x" + SCCP_TEST_GROTH16_PROOF_HEX).equals(evmBody.get("proof_bytes_hex"))
        : "EVM submit request must carry generated proof bytes";
    try {
      BridgeProofSubmitRequest.fromEvmSccpSubmission(
          "alice", java.util.Collections.emptyMap(), evmSubmission, evmBinding);
      throw new AssertionError("EVM SCCP submit helper must reject missing message bundle proof context");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("message_bundle.commitment.message_id")
          : "EVM SCCP submit helper rejection must name message bundle context";
    }
    final byte[] invalidProofBytes = proofBytes.clone();
    java.util.Arrays.fill(invalidProofBytes, 4 * 32, 6 * 32, (byte) 0);
    final EvmSccpProver.Submission invalidEvmSubmission =
        new EvmSccpProver.Submission(
            evmSubmission.version(),
            evmSubmission.proofFamily(),
            evmSubmission.verifierBackend(),
            evmSubmission.platformPayload(),
            evmSubmission.envelopeEncoding(),
            evmSubmission.submissionKind(),
            evmSubmission.verifierEntrypoint(),
            evmSubmission.contractMethod(),
            evmSubmission.functionSelector(),
            evmSubmission.sourceDomain(),
            evmSubmission.targetDomain(),
            evmSubmission.publicInputs(),
            evmSubmission.publicInputWords(),
            evmSubmission.publicSignalWords(),
            evmSubmission.statementHash(),
            evmSubmission.destinationBindingHash(),
            evmSubmission.arguments(),
            evmSubmission.callDataHex(),
            evmSubmission.envelopeHex(),
            invalidProofBytes,
            evmSubmission.publicInputWordsBytes(),
            evmSubmission.callData());
    try {
      BridgeProofSubmitRequest.fromEvmSccpSubmission(
          "alice", messageBundle, invalidEvmSubmission, evmBinding);
      throw new AssertionError("EVM SCCP submit helper must reject invalid Groth16 proof tuple");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex.a")
          : "EVM SCCP submit helper rejection must name the invalid proof point";
    }

    final SourceSccpProofs.TronDestinationBinding tronBinding =
        SourceSccpProofs.tronDestinationBinding(
            SourceSccpProofs.DOMAIN_SORA,
            SourceSccpProofs.DOMAIN_TRON,
            "0x" + "81".repeat(32),
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            "0x" + "82".repeat(32),
            "0x" + "83".repeat(32));
    final TronSccpProver.Submission tronSubmission =
        TronSccpProver.buildSubmission(
            new TronSccpProver.SubmissionInput(
                new TronSccpProver.PublicInputsInput(
                    1,
                    SCCP_TEST_MESSAGE_ID,
                    "22".repeat(32),
                    SourceSccpProofs.DOMAIN_TRON,
                    SCCP_TEST_COMMITMENT_ROOT,
                    "7",
                    "44".repeat(32)),
                proofBytes,
                "0x" + "66".repeat(32),
                tronBinding));

    transport
        .submitBridgeProof(
            BridgeProofSubmitRequest.fromTronSccpSubmission(
                "alice", messageBundle, tronSubmission, tronBinding))
        .join();

    final Map<String, Object> tronBody =
        (Map<String, Object>) JsonParser.parse(readBody(executor.lastRequest()));
    assert tronBinding.networkId.equals(tronBody.get("network_id_hex"))
        : "TRON submit request must carry governed network id";
    assert tronBinding.verifierAddress.equals(tronBody.get("tron_verifier_address"))
        : "TRON submit request must carry governed verifier";
    assert tronBinding.verifierCodeHash.equals(tronBody.get("verifier_code_hash_hex"))
        : "TRON submit request must carry governed code hash";
    assert tronBinding.verifierKeyHash.equals(tronBody.get("verifier_key_hash_hex"))
        : "TRON submit request must carry governed verifier key hash";
    assert tronBinding.hash.equals(tronBody.get("expected_destination_binding_hash_hex"))
        : "TRON submit request must carry governed binding hash";
    assert !tronBody.containsKey("verifier_address_hex")
        : "TRON submit request must not include EVM verifier field";
    assert !tronBody.containsKey("bridge_address_hex")
        : "TRON submit request must not include EVM bridge field";
    assert ("0x" + SCCP_TEST_GROTH16_PROOF_HEX).equals(tronBody.get("proof_bytes_hex"))
        : "TRON submit request must carry generated proof bytes";
  }

  private static void bridgeSubmitJsonHelpersRejectPlaceholderProofBytesBeforeRequest() {
    final StubResponseExecutor executor =
        new StubResponseExecutor(200, "{\"ok\":true}".getBytes(StandardCharsets.UTF_8));
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://127.0.0.1:8080/base"))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    final byte[] body = "{\"proof_bytes_hex\":\"0x0000\"}".getBytes(StandardCharsets.UTF_8);
    final byte[] shortBody = "{\"proof_bytes_hex\":\"0x0102ab\"}".getBytes(StandardCharsets.UTF_8);
    final String bindingHash = sampleSccpTronDestinationBindingHash();
    final byte[] proofOnlyBody =
        ("{\"proof_bytes_hex\":\"0x" + SCCP_TEST_GROTH16_PROOF_HEX + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] destinationOnlyBody =
        ("{\"network_id_hex\":\"0x"
                + "71".repeat(32)
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] invalidNetworkIdBody =
        ("{\"network_id_hex\":\"0x1234\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] paddedNetworkIdBody =
        ("{\"network_id_hex\":\" 0x"
                + "71".repeat(32)
                + "\",\"verifier_code_hash_hex\":\"0x"
                + "72".repeat(32)
                + "\",\"verifier_key_hash_hex\":\"0x"
                + "73".repeat(32)
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\",\"tron_verifier_address\":\"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] paddedProofBytesBody =
        ("{\"network_id_hex\":\"0x"
                + "71".repeat(32)
                + "\",\"verifier_code_hash_hex\":\"0x"
                + "72".repeat(32)
                + "\",\"verifier_key_hash_hex\":\"0x"
                + "73".repeat(32)
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\",\"tron_verifier_address\":\"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + " \"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] zeroEvmVerifierBody =
        ("{\"verifier_address_hex\":\"0x"
                + "00".repeat(20)
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] blankTronVerifierBody =
        ("{\"tron_verifier_address\":\"   \",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] paddedTronVerifierBody =
        ("{\"tron_verifier_address\":\" TJRabPrwbZy45sbavfcjinPJC18kjpRTv8\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] zeroTronVerifierBody =
        ("{\"tron_verifier_address\":\"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] invalidTronVerifierBody =
        ("{\"tron_verifier_address\":\"TJRabPrwbZy45sbavfcjinPJC18kjpRTv9\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] partialDestinationBody =
        ("{\"network_id_hex\":\"0x"
                + "71".repeat(32)
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] mixedDestinationBody =
        ("{\"network_id_hex\":\"0x"
                + "71".repeat(32)
                + "\",\"verifier_address_hex\":\"0x"
                + "22".repeat(20)
                + "\",\"bridge_address_hex\":\"0x"
                + "33".repeat(20)
                + "\",\"verifier_code_hash_hex\":\"0x"
                + "72".repeat(32)
                + "\",\"verifier_key_hash_hex\":\"0x"
                + "73".repeat(32)
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\",\"tron_verifier_address\":\"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] fullDestinationNoBundleBody =
        ("{\"authority\":\"alice\",\"network_id_hex\":\"0x"
                + SCCP_TEST_TRON_NETWORK_ID
                + "\",\"verifier_code_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_CODE_HASH
                + "\",\"verifier_key_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_KEY_HASH
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + bindingHash
                + "\",\"tron_verifier_address\":\""
                + SCCP_TEST_TRON_VERIFIER_ADDRESS
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] burnDestinationBody =
        ("{\"authority\":\"alice\",\"burn_bundle\":{},\"network_id_hex\":\"0x"
                + SCCP_TEST_TRON_NETWORK_ID
                + "\",\"verifier_code_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_CODE_HASH
                + "\",\"verifier_key_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_KEY_HASH
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + bindingHash
                + "\",\"tron_verifier_address\":\""
                + SCCP_TEST_TRON_VERIFIER_ADDRESS
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] mismatchedDestinationBindingBody =
        ("{\"authority\":\"alice\",\"message_bundle\":"
                + sccpMessageBundleJson()
                + ",\"network_id_hex\":\"0x"
                + SCCP_TEST_TRON_NETWORK_ID
                + "\",\"verifier_code_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_CODE_HASH
                + "\",\"verifier_key_hash_hex\":\"0x"
                + SCCP_TEST_TRON_VERIFIER_KEY_HASH
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\",\"tron_verifier_address\":\""
                + SCCP_TEST_TRON_VERIFIER_ADDRESS
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] mismatchedEvmDestinationBindingBody =
        ("{\"authority\":\"alice\",\"message_bundle\":"
                + sccpMessageBundleJson()
                + ",\"network_id_hex\":\"0x"
                + SCCP_TEST_EVM_NETWORK_ID
                + "\",\"verifier_address_hex\":\"0x"
                + SCCP_TEST_EVM_VERIFIER_ADDRESS
                + "\",\"bridge_address_hex\":\"0x"
                + SCCP_TEST_EVM_BRIDGE_ADDRESS
                + "\",\"verifier_code_hash_hex\":\"0x"
                + SCCP_TEST_EVM_VERIFIER_CODE_HASH
                + "\",\"verifier_key_hash_hex\":\"0x"
                + SCCP_TEST_EVM_VERIFIER_KEY_HASH
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\",\"proof_bytes_hex\":\"0x"
                + SCCP_TEST_GROTH16_PROOF_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] wrongSourceDomainDestinationBody =
        ("{\"authority\":\"alice\",\"network_id_hex\":\"0x"
                + "71".repeat(32)
                + "\",\"verifier_code_hash_hex\":\"0x"
                + "72".repeat(32)
                + "\",\"verifier_key_hash_hex\":\"0x"
                + "73".repeat(32)
                + "\",\"expected_destination_binding_hash_hex\":\"0x"
                + "74".repeat(32)
                + "\",\"tron_verifier_address\":\"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
                + "\",\"proof_bytes_hex\":\"0x"
                + sampleSccpProofHex(SCCP_TEST_MESSAGE_ID, 5, SCCP_TEST_COMMITMENT_ROOT)
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);

    try {
      transport.postBridgeProofSubmitJson(body);
      throw new AssertionError("Bridge proof submit must reject all-zero proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex")
          : "Bridge proof rejection must name proof_bytes_hex";
      assert ex.getMessage().contains("all zero")
          : "Bridge proof rejection must explain all-zero proof bytes";
    }

    try {
      transport.postBridgeMessageSubmitJson(body);
      throw new AssertionError("Bridge message submit must reject all-zero proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex")
          : "Bridge message rejection must name proof_bytes_hex";
      assert ex.getMessage().contains("all zero")
          : "Bridge message rejection must explain all-zero proof bytes";
    }

    try {
      transport.postBridgeProofSubmitJson(shortBody);
      throw new AssertionError("Bridge proof submit must reject short proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex")
          : "Bridge proof rejection must name proof_bytes_hex";
      assert ex.getMessage().contains("384-byte")
          : "Bridge proof rejection must explain canonical proof byte length";
    }

    try {
      transport.postBridgeMessageSubmitJson(shortBody);
      throw new AssertionError("Bridge message submit must reject short proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex")
          : "Bridge message rejection must name proof_bytes_hex";
      assert ex.getMessage().contains("384-byte")
          : "Bridge message rejection must explain canonical proof byte length";
    }

    final StringBuilder offCurveC = new StringBuilder(SCCP_TEST_GROTH16_PROOF_HEX);
    offCurveC.setCharAt(11 * 64 + 63, '3');
    try {
      transport.postBridgeProofSubmitJson(
          ("{\"network_id_hex\":\"0x"
                  + "71".repeat(32)
                  + "\",\"expected_destination_binding_hash_hex\":\"0x"
                  + "74".repeat(32)
                  + "\",\"proof_bytes_hex\":\"0x"
                  + offCurveC
                  + "\"}")
              .getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("Bridge proof submit must reject off-curve G1 proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex.c")
          : "Bridge proof rejection must name off-curve G1 proof point";
    }

    final StringBuilder offCurveB = new StringBuilder(SCCP_TEST_GROTH16_PROOF_HEX);
    final int offCurveBIndex = 6 * 64 + 63;
    offCurveB.setCharAt(offCurveBIndex, offCurveB.charAt(offCurveBIndex) == '0' ? '1' : '0');
    try {
      transport.postBridgeMessageSubmitJson(
          ("{\"verifier_address_hex\":\"0x"
                  + "22".repeat(20)
                  + "\",\"proof_bytes_hex\":\"0x"
                  + offCurveB
                  + "\"}")
              .getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("Bridge message submit must reject off-curve G2 proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex.b")
          : "Bridge message rejection must name off-curve G2 proof point";
    }

    try {
      transport.postBridgeProofSubmitJson(
          sccpBridgeMessageBody(
              sampleSccpProofHex("44".repeat(32), 0, SCCP_TEST_COMMITMENT_ROOT)));
      throw new AssertionError("Bridge proof submit must reject message-id drift");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex.message_id")
          : "Bridge proof rejection must name proof tuple message id";
    }

    try {
      transport.postBridgeMessageSubmitJson(
          sccpBridgeMessageBody(sampleSccpProofHex(SCCP_TEST_MESSAGE_ID, 5, SCCP_TEST_COMMITMENT_ROOT)));
      throw new AssertionError("Bridge message submit must reject source-domain drift");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex.source_domain")
          : "Bridge message rejection must name proof tuple source domain";
    }

    try {
      transport.postBridgeMessageSubmitJson(
          sccpBridgeMessageBody(sampleSccpProofHex(SCCP_TEST_MESSAGE_ID, 0, "55".repeat(32))));
      throw new AssertionError("Bridge message submit must reject commitment-root drift");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex.commitment_root")
          : "Bridge message rejection must name proof tuple commitment root";
    }

    try {
      transport.postBridgeProofSubmitJson(
          ("{\"authority\":\"alice\",\"message_bundle\":{},\"network_id_hex\":\"0x"
                  + "71".repeat(32)
                  + "\",\"verifier_code_hash_hex\":\"0x"
                  + "72".repeat(32)
                  + "\",\"verifier_key_hash_hex\":\"0x"
                  + "73".repeat(32)
                  + "\",\"expected_destination_binding_hash_hex\":\"0x"
                  + "74".repeat(32)
                  + "\",\"tron_verifier_address\":\"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
                  + "\",\"proof_bytes_hex\":\"0x"
                  + SCCP_TEST_GROTH16_PROOF_HEX
                  + "\"}")
              .getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("Bridge proof submit must reject missing message bundle proof context");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("message_bundle.commitment.message_id")
          : "Bridge proof rejection must name message bundle context";
    }

    try {
      transport.postBridgeProofSubmitJson(wrongSourceDomainDestinationBody);
      throw new AssertionError("Bridge proof submit must reject wrong source-domain destination proof");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex.source_domain")
          : "Bridge proof rejection must name proof tuple source domain";
    }

    try {
      transport.postBridgeProofSubmitJson(proofOnlyBody);
      throw new AssertionError("Bridge proof submit must reject proof bytes without destination material");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("deployment destination fields")
          : "Bridge proof rejection must require destination material";
    }

    try {
      transport.postBridgeProofSubmitJson(partialDestinationBody);
      throw new AssertionError("Bridge proof submit must reject partial destination tuple");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("complete EVM or TRON")
          : "Bridge proof rejection must require a complete destination tuple";
    }

    try {
      transport.postBridgeMessageSubmitJson(mixedDestinationBody);
      throw new AssertionError("Bridge message submit must reject mixed EVM/TRON tuple");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("cannot be mixed")
          : "Bridge message rejection must reject mixed EVM/TRON material";
    }

    try {
      transport.postBridgeMessageSubmitJson(mismatchedDestinationBindingBody);
      throw new AssertionError("Bridge message submit must reject mismatched TRON binding hash");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("canonical TRON destination binding")
          : "Bridge message rejection must reject mismatched TRON binding hashes";
    }

    try {
      transport.postBridgeMessageSubmitJson(mismatchedEvmDestinationBindingBody);
      throw new AssertionError("Bridge message submit must reject mismatched EVM binding hash");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("canonical EVM destination binding")
          : "Bridge message rejection must reject mismatched EVM binding hashes";
    }

    try {
      transport.postBridgeProofSubmitJson(fullDestinationNoBundleBody);
      throw new AssertionError("Bridge proof submit must reject missing bundle selection");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("exactly one of burn_bundle or message_bundle")
          : "Bridge proof rejection must require one bundle";
    }

    try {
      transport.postBridgeProofSubmitJson(burnDestinationBody);
      throw new AssertionError("Bridge proof submit must reject destination tuple on burn bundle");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("message_bundle submissions")
          : "Bridge proof rejection must reject destination material on burn bundles";
    }

    try {
      transport.postBridgeMessageSubmitJson(destinationOnlyBody);
      throw new AssertionError("Bridge message submit must reject destination material without proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex is required")
          : "Bridge message rejection must require proof bytes";
    }

    try {
      transport.postBridgeProofSubmitJson(invalidNetworkIdBody);
      throw new AssertionError("Bridge proof submit must reject invalid destination network id");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("network_id_hex")
          : "Bridge proof rejection must name the invalid destination field";
      assert ex.getMessage().contains("32-byte")
          : "Bridge proof rejection must explain destination network id length";
    }

    try {
      transport.postBridgeProofSubmitJson(paddedNetworkIdBody);
      throw new AssertionError("Bridge proof submit must reject padded destination network id");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("network_id_hex")
          : "Bridge proof rejection must name the padded destination field";
      assert ex.getMessage().contains("canonical hex")
          : "Bridge proof rejection must require exact destination hex";
    }

    try {
      transport.postBridgeMessageSubmitJson(paddedProofBytesBody);
      throw new AssertionError("Bridge message submit must reject padded proof bytes");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("proof_bytes_hex")
          : "Bridge message rejection must name padded proof bytes";
      assert ex.getMessage().contains("canonical hex")
          : "Bridge message rejection must require exact proof hex";
    }

    try {
      transport.postBridgeProofSubmitJson(zeroEvmVerifierBody);
      throw new AssertionError("Bridge proof submit must reject all-zero EVM verifier address");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("verifier_address_hex")
          : "Bridge proof rejection must name the zero EVM verifier field";
      assert ex.getMessage().contains("all zero")
          : "Bridge proof rejection must explain all-zero EVM verifier material";
    }

    try {
      transport.postBridgeMessageSubmitJson(blankTronVerifierBody);
      throw new AssertionError("Bridge message submit must reject blank TRON verifier address");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("tron_verifier_address")
          : "Bridge message rejection must name the invalid TRON verifier field";
    }

    try {
      transport.postBridgeMessageSubmitJson(paddedTronVerifierBody);
      throw new AssertionError("Bridge message submit must reject padded TRON verifier address");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("tron_verifier_address")
          : "Bridge message rejection must name the padded TRON verifier field";
    }

    try {
      transport.postBridgeMessageSubmitJson(zeroTronVerifierBody);
      throw new AssertionError("Bridge message submit must reject zero TRON verifier address");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("tron_verifier_address")
          : "Bridge message rejection must name the invalid TRON verifier field";
    }

    try {
      transport.postBridgeMessageSubmitJson(invalidTronVerifierBody);
      throw new AssertionError("Bridge message submit must reject invalid TRON verifier address");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("tron_verifier_address")
          : "Bridge message rejection must name the invalid TRON verifier field";
    }

    assert executor.lastRequest() == null : "Invalid bridge submit payload must not be sent";
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
      final Throwable cause = ex.getCause();
      assert ex == transportError || cause == transportError
          : "Original runtime exception must propagate";
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
    assert observer.requestCount.get() == 2 : "Observer should see both attempts";
    assert observer.responseCount.get() == 1 : "Only the successful attempt reports a response";
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
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new ScriptedExecutor(
                new TransportResponse(202, statusPayload("Pending"), "", Map.of()),
                new TransportResponse(200, statusPayload("Committed"), "", Map.of())),
            ClientConfig.builder()
                .setBaseUri(URI.create("https://status-telemetry.test:8080"))
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    final String hashHex = "deadbeefcafefeed";
    final Map<String, Object> payload =
        transport
            .waitForTransactionStatus(
                hashHex, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();
    assert "Committed".equals(PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        : "Expected committed status";

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
    assert "Pending".equals(pending.get("status_kind"))
        : "Pending signal must record status kind";
    assert "pending".equals(pending.get("outcome"))
        : "Pending signal must use pending outcome";
    assert ((Number) pending.get("attempts")).intValue() == 1
        : "Pending signal must record first attempt";

    assert expectedAuthorityHash.equals(success.get("authority_hash"))
        : "Success signal must carry hashed authority";
    assert hashHex.equals(success.get("tx_hash"))
        : "Success signal must carry transaction hash";
    assert "Committed".equals(success.get("status_kind"))
        : "Success signal must record committed status";
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
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            new ScriptedExecutor(
                new TransportResponse(200, statusPayload("Committed"), "", Map.of())),
            ClientConfig.builder()
                .setBaseUri(URI.create("http:/")) // No authority -> redaction failure path.
                .setTelemetryOptions(telemetryOptions)
                .setTelemetrySink(telemetrySink)
                .build());

    transport
        .waitForTransactionStatus(
            "beadfeed", PipelineStatusOptions.builder().intervalMillis(0L).build())
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
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), keyManager);
    final TransactionPayload payload = TransactionPayload.builder().build();
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
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), keyManager);
    final TransactionPayload payload = TransactionPayload.builder().build();
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
            .setSorafsGatewayUri(URI.create("https://gateway.example:8443/gateway"))
            .setRequestTimeout(Duration.ofSeconds(12))
            .putDefaultHeader("X-Trace", "android-client")
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("primary")
            .setProviderIdHex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
            .setBaseUrl("https://storage.example/direct")
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
            "https://gateway.example:8443/gateway/v1/sorafs/gateway/fetch")
        : "Gateway URI must combine base path with fetch endpoint";
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
            + "\"account_id\":\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\","
            + "\"label\":\"Primary\","
            + "\"assets\":[{"
            + "\"asset_id\":\""
            + assetDefinitionId
            + "#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
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
        transport.getUaidPortfolio("  UAID:" + hex.toUpperCase() + " ").join();
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
    assert "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".equals(account.accountId())
        : "Account ID mismatch";
    assert "Primary".equals(account.label()) : "Account label mismatch";
    assert account.assets().size() == 1 : "Expected single asset entry";
    final UaidPortfolioResponse.UaidPortfolioAsset asset = account.assets().get(0);
    assert (assetDefinitionId + "#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB").equals(asset.assetId())
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
            + "\"accounts\":[\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\",\"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D\"]"
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
            + "\"accounts\":[\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\"],"
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
    assert record.accounts().contains("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB") : "Accounts must surface";
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
    final String json =
        "{"
            + "\"total\":1,"
            + "\"items\":[{"
            + "\"policy_id\":\"phone#retail\","
            + "\"owner\":\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\","
            + "\"active\":true,"
            + "\"normalization\":\"phone_e164\","
            + "\"resolver_public_key\":\"ed25519:resolver-key\","
            + "\"backend\":\"bfv-affine-sha3-256-v1\","
            + "\"input_encryption\":\"bfv-v1\","
            + "\"input_encryption_public_parameters\":\"ABCD\","
            + "\"input_encryption_public_parameters_decoded\":{"
            + "\"parameters\":{\"polynomial_degree\":64,\"plaintext_modulus\":257,\"ciphertext_modulus\":1099511627776,\"decomposition_base_log\":12},"
            + "\"public_key\":{\"b\":[1,2,3],\"a\":[4,5,6]},"
            + "\"max_input_bytes\":32"
            + "},"
            + "\"note\":\"retail phone policy\""
            + "}]"
            + "}";
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
    assert "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".equals(item.owner()) : "Owner mismatch";
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

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Identifier policy request must be captured";
    assert "GET".equals(request.method()) : "Identifier policy list must use GET";
    assert request.uri().toString().equals("https://torii.example/v1/identifier-policies")
        : "Identifier policy URI mismatch";
    assert request.headers().getOrDefault("Accept", List.of()).contains("application/json")
        : "Identifier policy request must accept JSON";
  }

  private static void ramLfeProgramPoliciesRequestParsesResponse() {
    final String json =
        "{"
            + "\"total\":1,"
            + "\"items\":[{"
            + "\"program_id\":\"identifier_lookup_retail\","
            + "\"owner\":\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\","
            + "\"active\":true,"
            + "\"resolver_public_key\":\"ed25519:resolver-key\","
            + "\"backend\":\"bfv-programmed-sha3-256-v1\","
            + "\"verification_mode\":\"signed\","
            + "\"input_encryption\":\"bfv-v1\","
            + "\"input_encryption_public_parameters\":\"ABCD\","
            + "\"input_encryption_public_parameters_decoded\":{"
            + "\"parameters\":{\"polynomial_degree\":64,\"plaintext_modulus\":257,\"ciphertext_modulus\":1099511627776,\"decomposition_base_log\":12},"
            + "\"public_key\":{\"b\":[1,2,3],\"a\":[4,5,6]},"
            + "\"max_input_bytes\":32"
            + "},"
            + "\"note\":\"retail programmed policy\""
            + "}]"
            + "}";
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
    assert "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".equals(item.owner()) : "Owner mismatch";
    assert item.active() : "Program policy should be active";
    assert "signed".equals(item.verificationMode()) : "Verification mode mismatch";
    assert "bfv-v1".equals(item.inputEncryption()) : "Input encryption mismatch";
    assert item.inputEncryptionPublicParametersDecoded() != null
        : "Decoded BFV parameters should be present";
    assert item.inputEncryptionPublicParametersDecoded().parameters().polynomialDegree() == 64L
        : "Decoded BFV polynomial degree mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "RAM-LFE policy request must be captured";
    assert "GET".equals(request.method()) : "RAM-LFE policy list must use GET";
    assert request.uri().toString().equals("https://torii.example/v1/ram-lfe/program-policies")
        : "RAM-LFE policy URI mismatch";
    assert request.headers().getOrDefault("Accept", List.of()).contains("application/json")
        : "RAM-LFE policy request must accept JSON";
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
    return "{"
        + "\"payload\":"
        + identifierPayloadJson(payload)
        + ",\"attestation\":{\"kind\":\"signed\",\"signature\":"
        + jsonString(signatureHex)
        + "}"
        + "}";
  }

  private static String identifierPayloadJson(final IdentifierResolutionPayload payload) {
    return "{"
        + "\"policy_id\":"
        + jsonString(payload.policyId())
        + ",\"execution\":"
        + identifierExecutionJson(payload.execution())
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
    final String expires =
        execution.expiresAtMs() == null ? "" : ",\"expires_at_ms\":" + execution.expiresAtMs();
    return "{"
        + "\"program_id\":"
        + jsonString(execution.programId())
        + ",\"program_digest\":"
        + jsonString(execution.programDigest())
        + ",\"backend\":"
        + jsonString(execution.backend())
        + ",\"verification_mode\":"
        + jsonString(execution.verificationMode())
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
    final String accountId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
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
        "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
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

  private static void identifierClaimReceiptUsesAccountPath() {
    final String accountId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
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
    final String json =
        "{"
            + "\"program_id\":\"identifier_lookup_retail\","
            + "\"opaque_hash\":\"opaque-hash-literal\","
            + "\"receipt_hash\":\"receipt-hash-literal\","
            + "\"output_ciphertext\":\"abcd\","
            + "\"output_hash\":\"output-hash-literal\","
            + "\"associated_data_hash\":\"associated-data-hash-literal\","
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
    assert "output-hash-literal".equals(execute.outputHash()) : "Output hash mismatch";
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
    final String json =
        "{"
            + "\"valid\":true,"
            + "\"program_id\":\"identifier_lookup_retail\","
            + "\"backend\":\"bfv-programmed-sha3-256-v1\","
            + "\"verification_mode\":\"signed\","
            + "\"output_hash\":\"output-hash-literal\","
            + "\"associated_data_hash\":\"associated-data-hash-literal\","
            + "\"output_hash_matches\":true"
            + "}";
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

  private static void vpnProfileRequestParsesNativeLeaseFields() {
    final String json =
        "{"
            + "\"available\":true,"
            + "\"relay_endpoint\":\"/dns/relay.example/udp/9443/quic\","
            + "\"supported_exit_classes\":[\"standard\",\"low-latency\"],"
            + "\"default_exit_class\":\"standard\","
            + "\"lease_secs\":600,"
            + "\"dns_push_interval_secs\":60,"
            + "\"meter_family\":\"soranet.vpn.standard\","
            + "\"route_pushes\":[\"0.0.0.0/0\"],"
            + "\"excluded_routes\":[\"10.0.0.0/8\"],"
            + "\"dns_servers\":[\"1.1.1.1\"],"
            + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
            + "\"mtu_bytes\":1024,"
            + "\"display_billing_label\":\"standard XOR\","
            + "\"fee_asset_id\":\"xor#universal.universal\","
            + "\"escrow_account_id\":\"sorauEscrow\","
            + "\"operator_account_id\":\"sorauOperator\","
            + "\"lease_fee_nanos\":1000000,"
            + "\"settlement_grace_secs\":120,"
            + "\"flow_label_bits\":24,"
            + "\"padding_budget_ms\":15,"
            + "\"relay_tls_spki_sha256_hex\":\""
            + "ab".repeat(32)
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
    assert "xor#universal.universal".equals(profile.feeAssetId()) : "VPN fee asset mismatch";
    assert "sorauEscrow".equals(profile.escrowAccountId()) : "VPN escrow account mismatch";
    assert "sorauOperator".equals(profile.operatorAccountId()) : "VPN operator account mismatch";
    assert profile.leaseFeeNanos() == 1_000_000L : "VPN lease fee mismatch";
    assert profile.settlementGraceSecs() == 120L : "VPN settlement grace mismatch";
    assert "ab".repeat(32).equals(profile.relayTlsSpkiSha256Hex()) : "VPN TLS pin mismatch";
    assert "GET".equals(executor.lastRequest().method()) : "VPN profile must use GET";
    assert executor.lastRequest().uri().toString().equals("https://torii.example/v1/vpn/profile")
        : "VPN profile URI mismatch";
  }

  private static void vpnQuoteRequestSignsCanonicalBodyAndParsesOpenLeaseInstruction()
      throws Exception {
    final String quoteId = "11".repeat(32);
    final String meteringKey = "22".repeat(32);
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
    assert quote.txInstructions().size() == 1 : "VPN quote should have one native instruction";

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

  private static void vpnSessionAndReceiptRequestsUseNativeLeaseDtos() throws Exception {
    final String sessionId = "33".repeat(32);
    final String paymentTxHash = "44".repeat(32);
    final String meteringKey = "55".repeat(32);
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
    assert fetched.isPresent() : "VPN session lookup should be present";
    assert deleted.isPresent() : "VPN delete receipt should be present";
    assert "disconnected".equals(deleted.get().status()) : "VPN delete status mismatch";
    assert "settled".equals(submitted.status()) : "VPN settled receipt status mismatch";
    assert submitted.earnedFeeNanos() == 750_000L : "VPN earned fee mismatch";
    assert submitted.refundedFeeNanos() == 250_000L : "VPN refund mismatch";
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

  private static void deployContractRequestParsesResponse() {
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            200,
            ("{"
                    + "\"ok\":true,"
                    + "\"bundle_name\":\"single-contract-deploy\","
                    + "\"bundle_digest\":\"mock-bundle-digest\","
                    + "\"chain_fingerprint\":\"mock-chain@height-0\","
                    + "\"dry_run\":false,"
                    + "\"completed_stages\":[\"plan\",\"deploy\"],"
                    + "\"failure_point\":null,"
                    + "\"contracts\":[{"
                    + "\"name\":\"router::universal\","
                    + "\"contract_alias\":\"router::universal\","
                    + "\"contract_address\":\"tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7\","
                    + "\"previous_contract_address\":null,"
                    + "\"upgraded\":false,"
                    + "\"dataspace\":\"router\","
                    + "\"deploy_nonce\":9,"
                    + "\"tx_hash_hex\":\""
                    + "11".repeat(32)
                    + "\","
                    + "\"code_hash_hex\":\""
                    + "22".repeat(32)
                    + "\","
                    + "\"abi_hash_hex\":\""
                    + "33".repeat(32)
                    + "\","
                    + "\"status\":\"submitted\""
                    + "}],"
                    + "\"init_calls\":[],"
                    + "\"assertions\":[]"
                    + "}")
                .getBytes(StandardCharsets.UTF_8),
            "ok");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build());

    final Optional<ContractDeployResponse> response =
        transport.deployContract("alice", "privkey", "AQID", "router::universal").join();

    assert response.isPresent() : "Deploy response should be present";
    final ContractDeployResponse parsed = response.get();
    assert parsed.ok() : "Deploy response should be successful";
    assert "mock-bundle-digest".equals(parsed.bundleDigest()) : "Bundle digest mismatch";
    assert "router::universal".equals(parsed.contracts().get(0).contractAlias())
        : "Contract alias mismatch";
    assert "router".equals(parsed.contracts().get(0).dataspace()) : "Dataspace mismatch";
    assert Long.valueOf(9L).equals(parsed.contracts().get(0).deployNonce())
        : "Deploy nonce mismatch";
    assert "11".repeat(32).equals(parsed.contracts().get(0).txHashHex())
        : "tx_hash_hex mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Deploy request must be captured";
    assert request.uri().toString().equals("https://torii.example/api/v1/contracts/deploy")
        : "Deploy URI mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> payload =
        (Map<String, Object>) JsonParser.parse(readBody(request));
    assert "alice".equals(payload.get("authority")) : "Deploy authority mismatch";
    assert "privkey".equals(payload.get("private_key")) : "Deploy private key mismatch";
    assert "AQID".equals(payload.get("code_b64")) : "Deploy code_b64 mismatch";
    assert "router::universal".equals(payload.get("contract_alias"))
        : "Deploy contract_alias mismatch";
    assert !payload.containsKey("lease_expiry_ms") : "lease_expiry_ms should be omitted by default";
  }

  private static void callContractRequestParsesResponse() {
    final String contractAddress =
        "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7";
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            200,
            ("{"
                    + "\"ok\":true,"
                    + "\"submitted\":true,"
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
                    + "\"tx_hash_hex\":\""
                    + "66".repeat(32)
                    + "\","
                    + "\"entrypoint\":\"contribute\","
                    + "\"transaction_scaffold_b64\":\"AQID\","
                    + "\"signed_transaction_b64\":\"BAUG\","
                    + "\"signing_message_b64\":\"BwgJ\"}")
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
            .callContract(
                "alice",
                "privkey",
                5000L,
                null,
                "router::universal",
                "contribute",
                contractPayload,
                "xor#sora")
            .join();

    assert response.ok() : "Call response should be successful";
    assert response.submitted() : "Call should be marked submitted";
    assert "router".equals(response.dataspace()) : "Call dataspace mismatch";
    assert "contribute".equals(response.entrypoint()) : "Entrypoint mismatch";
    assert "AQID".equals(response.transactionScaffoldB64())
        : "transaction_scaffold_b64 mismatch";
    assert "BAUG".equals(response.signedTransactionB64()) : "signed_transaction_b64 mismatch";
    assert "BwgJ".equals(response.signingMessageB64()) : "signing_message_b64 mismatch";

    final TransportRequest request = executor.lastRequest();
    assert request != null : "Contract call request must be captured";
    assert request.uri().toString().equals("https://torii.example/api/v1/contracts/call")
        : "Call URI mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> payload =
        (Map<String, Object>) JsonParser.parse(readBody(request));
    assert "alice".equals(payload.get("authority")) : "Call authority mismatch";
    assert "privkey".equals(payload.get("private_key")) : "Call private key mismatch";
    assert Long.valueOf(5000L).equals(((Number) payload.get("gas_limit")).longValue())
        : "gas_limit mismatch";
    assert "router::universal".equals(payload.get("contract_alias"))
        : "contract_alias mismatch";
    assert !payload.containsKey("contract_address") : "contract_address should be absent";
    assert "contribute".equals(payload.get("entrypoint")) : "Call entrypoint mismatch";
    assert "xor#sora".equals(payload.get("gas_asset_id")) : "gas_asset_id mismatch";
    @SuppressWarnings("unchecked")
    final Map<String, Object> requestPayload = (Map<String, Object>) payload.get("payload");
    assert "alice".equals(requestPayload.get("buyer")) : "Nested buyer mismatch";
    assert Long.valueOf(1L).equals(((Number) requestPayload.get("payment_amount")).longValue())
        : "Nested payment_amount mismatch";
  }

  private static void proposeMultisigRequestParsesResponse() {
    final byte[] instructionBytes = new byte[] {1, 2, 3, 4};
    final String proposalId = "aa".repeat(32);
    final StubResponseExecutor executor =
        new StubResponseExecutor(
            200,
            ("{"
                    + "\"ok\":true,"
                    + "\"resolved_multisig_account_id\":\"multisig\","
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
                MultisigProposeRequest.builder()
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instructionBytes)
                    .setCreationTimeMs(123L)
                    .setFeeSponsor("fee-sponsor")
                    .build())
            .join();

    assert response.ok() : "Multisig response should be successful";
    assert "multisig".equals(response.resolvedMultisigAccountId())
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
    assert "fee-sponsor".equals(payload.get("fee_sponsor")) : "fee_sponsor mismatch";
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
          MultisigProposeRequest.builder()
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
                MultisigProposeRequest.builder()
                    .setMultisigAccountId("aid:multisig")
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .build()),
        "ambiguous multisig selector must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder()
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .build()),
        "missing multisig selector must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder()
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setSignatureB64("not base64")
                    .build()),
        "malformed detached signature must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder()
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setPublicKeyHex("aa")
                    .build()),
        "short detached public key must be rejected");
    expectIllegalArgument(
        () ->
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest.builder()
                    .setMultisigAccountAlias("cbdc@banka")
                    .setSignerAccountId("alice")
                    .addInstructionBytes(instruction)
                    .setCreationTimeMs(-1L)
                    .build()),
        "negative creation time must be rejected");
  }

  private static void multisigResponseParserRejectsMalformedFields() {
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":false,"
                        + "\"resolved_multisig_account_id\":\"multisig\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "false ok response must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\"multisig\","
                        + "\"submitted\":\"false\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "string submitted flag must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\"multisig\","
                        + "\"instructions_hash\":\"aa\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "short instructions hash must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\"multisig\","
                        + "\"signing_message_b64\":\"not base64\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "malformed signing message must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\"multisig\","
                        + "\"signing_message_b64\":\"\"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "empty signing message must be rejected");
    expectRuntimeException(
        () ->
            ContractJsonParser.parseMultisigResponse(
                ("{"
                        + "\"ok\":true,"
                        + "\"resolved_multisig_account_id\":\"multisig\","
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
      transport.callContract(
          "alice",
          "privkey",
          5000L,
          "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
          "router::universal",
          null,
          null,
          null);
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("Exactly one");
    }
    assert failed : "expected ambiguous contract target rejection";
  }

  private static void governanceContractRequestParsesResponse() {
    final String contractAddress =
        "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7";
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

    final GovernanceContractResponse response = transport.getGovernanceContract(contractAddress).join();

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
    final String accountId =
        "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
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
    assert Long.valueOf(7L).equals(resolution.index()) : "Index mismatch";
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
    final String json =
        "{"
            + "\"alias\":\"banking@centralbank.universal\","
            + "\"account_id\":\"aid:banking-123\","
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
    assert "aid:banking-123".equals(resolution.accountId()) : "Account id mismatch";
    assert resolution.index() == null : "Index should be absent when the payload omits it";
    assert "rekey_record".equals(resolution.source()) : "Source mismatch";
  }

  private static void resolveAccountAliasRejectsNonIntegerIndex() {
    final String json =
        "{"
            + "\"alias\":\"alice@universal\","
            + "\"account_id\":\"aid:alice-123\","
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
  }

  private static void identifierBfvEnvelopeBuilderProducesDeterministicCiphertext() {
    final IdentifierPolicySummary policy =
        new IdentifierPolicySummary(
            "string#retail",
            "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            true,
            IdentifierNormalization.EXACT,
            "ed25519:ed0120" + "11".repeat(32),
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
            "ed25519:ed0120" + "11".repeat(32),
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
        "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
        true,
        IdentifierNormalization.EXACT,
        "ed25519:ed0120" + "11".repeat(32),
        "bfv-affine-sha3-256-v1",
        "bfv-v1",
        null,
        parameters,
        null);
  }

  private static IdentifierPolicySummary samplePlaintextOnlyIdentifierPolicy() {
    return new IdentifierPolicySummary(
        "string#retail",
        "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
        true,
        IdentifierNormalization.EXACT,
        "ed25519:ed0120" + "11".repeat(32),
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
    return new IdentifierResolutionReceipt(
        identifierPayloadFromFixture(object(receipt, "payload"), outputCiphertextHashOverride),
        identifierAttestationFromFixture(
            attestationOverride != null ? attestationOverride : object(receipt, "attestation"),
            signatureOverride));
  }

  private static IdentifierResolutionPayload identifierPayloadFromFixture(
      final Map<String, Object> payload, final String outputCiphertextHashOverride) {
    return new IdentifierResolutionPayload(
        string(payload, "policy_id"),
        identifierExecutionFromFixture(object(payload, "execution"), outputCiphertextHashOverride),
        outputOpeningFromFixture(object(payload, "opening")),
        string(payload, "opaque_id"),
        string(payload, "receipt_hash"),
        string(payload, "uaid"),
        string(payload, "account_id"));
  }

  private static IdentifierResolutionExecutionPayload identifierExecutionFromFixture(
      final Map<String, Object> execution, final String outputCiphertextHashOverride) {
    return new IdentifierResolutionExecutionPayload(
        string(execution, "program_id"),
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
    final Map<String, Object> payload = object(opening, "payload");
    return new RamLfeOutputOpening(
        new RamLfeOutputOpeningPayload(
            string(payload, "program_id"),
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
    final String accountId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
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
    final String accountId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
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
      final IdentifierPolicySummary mutatedPolicy;
      if ("policy.resolver_public_key".equals(mutation)) {
        mutatedPolicy =
            identifierPolicyFromReceiptFixture(
                object(fixture, "policy"), null, string(negative, "value"));
      } else if ("policy.policy_id".equals(mutation)) {
        mutatedPolicy =
            identifierPolicyFromReceiptFixture(
                object(fixture, "policy"), string(negative, "value"), null);
      } else {
        mutatedPolicy = policy;
      }
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
            () -> mutatedReceipt.verifyAttestation(mutatedPolicy),
            string(negative, "name"));
      } else {
        final boolean expected = Boolean.TRUE.equals(negative.get("expected_result"));
        assert mutatedReceipt.verifyAttestation(mutatedPolicy) == expected
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
        + "\"escrow_account_id\":\"sorauEscrow\","
        + "\"operator_account_id\":\"sorauOperator\","
        + "\"lease_fee_nanos\":1000000,"
        + "\"route_pushes\":[\"0.0.0.0/0\"],"
        + "\"excluded_routes\":[],"
        + "\"dns_servers\":[\"1.1.1.1\"],"
        + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
        + "\"mtu_bytes\":1024,"
        + "\"meter_family\":\"soranet.vpn.standard\","
        + "\"flow_label_bits\":24,"
        + "\"padding_budget_ms\":15,"
        + "\"relay_tls_spki_sha256_hex\":\""
        + "ab".repeat(32)
        + "\",\"metering_public_key_hex\":\""
        + meteringKey
        + "\",\"open_lease_instruction\":{"
        + "\"wire_id\":\"iroha_data_model::isi::vpn::OpenVpnLeaseEscrow\","
        + "\"payload_hex\":\"cafe\"},"
        + "\"tx_instructions\":[{"
        + "\"wire_id\":\"iroha_data_model::isi::vpn::OpenVpnLeaseEscrow\","
        + "\"payload_hex\":\"cafe\"}]"
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
        + "\"escrow_account_id\":\"sorauEscrow\","
        + "\"operator_account_id\":\"sorauOperator\","
        + "\"lease_fee_nanos\":1000000,"
        + "\"flow_label_bits\":24,"
        + "\"padding_budget_ms\":15,"
        + "\"relay_tls_spki_sha256_hex\":\""
        + "ab".repeat(32)
        + "\",\"route_pushes\":[\"0.0.0.0/0\"],"
        + "\"excluded_routes\":[],"
        + "\"dns_servers\":[\"1.1.1.1\"],"
        + "\"tunnel_addresses\":[\"10.208.0.2/32\"],"
        + "\"mtu_bytes\":1024,"
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
    final long earned = settled ? 750_000L : 0L;
    final long refunded = settled ? 250_000L : 1_000_000L;
    final String settlement =
        settled
            ? ",\"settle_lease_instruction\":{"
                + "\"wire_id\":\"iroha_data_model::isi::vpn::SettleVpnLease\","
                + "\"payload_hex\":\"f00d\"},"
                + "\"tx_instructions\":[{"
                + "\"wire_id\":\"iroha_data_model::isi::vpn::SettleVpnLease\","
                + "\"payload_hex\":\"f00d\"}]"
            : ",\"settle_lease_instruction\":null,\"tx_instructions\":[]";
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
        + "\"escrow_account_id\":\"sorauEscrow\","
        + "\"operator_account_id\":\"sorauOperator\","
        + "\"lease_fee_nanos\":1000000,"
        + "\"earned_fee_nanos\":"
        + earned
        + ",\"refunded_fee_nanos\":"
        + refunded
        + ",\"lease_id_hex\":\""
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
      final int position = index < responses.length ? index : responses.length - 1;
      index++;
      return CompletableFuture.completedFuture(responses[position]);
    }
  }

  private static final class RecordingExecutor implements HttpTransportExecutor {
    private final List<byte[]> payloads = new ArrayList<>();

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
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
      return CompletableFuture.completedFuture(response);
    }

    TransportRequest lastRequest() {
      return lastRequest;
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
        TransactionPayload.builder()
            .setChainId(String.format("%08x", fillValue))
            .setAuthority(TestAccountIds.ed25519Authority(0x26))
            .setCreationTimeMs(1_700_000_000_000L + (fillValue & 0xFF))
            .setInstructionBytes(new byte[] {fillValue, (byte) (fillValue + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce(fillValue & 0xFF)
            .setMetadata(Map.of("note", "txn-" + fillValue))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final byte[] encodedPayload;
    try {
      encodedPayload = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }
    return new SignedTransaction(
        encodedPayload, signature, publicKey, codec.schemaName(), "alias-" + aliasCounter++);
  }

  private static byte[] statusPayload(final String kind) {
    final String json =
        "{\"kind\":\"Transaction\",\"content\":{\"status\":{\"kind\":\"" + kind + "\"}}}";
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
