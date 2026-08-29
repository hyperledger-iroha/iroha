package org.hyperledger.iroha.android.client;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.bouncycastle.crypto.digests.SHA256Digest;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sccp.SccpLaneIdV1;
import org.hyperledger.iroha.android.sccp.SccpNetworkV1;
import org.hyperledger.iroha.android.sccp.SccpReplayV1;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Adversarial client tests for exact SCCP discovery, readback, and submission. */
public final class SccpClientExactTests {
  private static final String MESSAGE_ID = "11".repeat(32);
  private static final String TAIRA_CHAIN_ID =
      "fc56984b-2be7-431d-840e-21514d1883f0";
  private static final String AUTHORITY = canonicalAuthority(0x11);
  private static final String OTHER_AUTHORITY = canonicalAuthority(0x12);
  private static final FeePaymentIntent BRIDGE_FEE_PAYMENT =
      FeePaymentIntent.authority(Collections.emptyList());
  // These authenticate this fixture's semantic commitments and deployment code hashes.
  private static final String BSC_ROUTE_CONFIG_HASH =
      "FDCE93E148D8A9BD3BE2E7051AF681A757CA273F409073F9402F5534D32C399B";
  private static final String TRON_ROUTE_CONFIG_HASH =
      "60544E93C2B5E96761C90DC96E7E24EC1D6EDD5BEE2E2A946D7ED9632535177D";

  private SccpClientExactTests() {}

  public static void main(final String[] args) throws Exception {
    submitDtosExposeOnlyClosedArtifactFields();
    binaryProofRequestAcceptsOnlyTheTwoConcreteCurveTypes();
    signedSubmitPreservesExactTairaSponsorAcrossControllerOnlyWireIdentity();
    submitAuthorityRequiresExactTairaDiscriminant();
    submitPreflightRejectsRetiredOverridesAndSecrets();
    artifactValidationRejectsAliasesCorruptionAndZeroSchema();
    capabilitiesAreExactAndContainNoRetiredDiscoverySurface();
    registryValidatesSemanticPolicyAndExactFamilies();
    registryValidatesExactTonDeploymentAndRoleSeparation();
    registryRejectsMalformedAnchorHistories();
    registryRequiresExactRetiredRouteInboundFinalityCutoff();
    registryCountsOnlyNonRetiredRoutes();
    verifyingKeysAllowZeroCoordinatesButRejectInfinity();
    routeConfigurationIsNetworkExactAndPolicyBound();
    bundleAndProofRequestRejectRetiredAndAliasedRoles();
    tonProofRequestBindsExactBlsSignalsAndProfile();
    recentMessagesRequireExactLinksAndUniqueIds();
    detachedSigningResponseRejectsCrossFamilyLabels();
    System.out.println("[IrohaAndroid] exact SCCP client tests passed.");
  }

  private static void submitDtosExposeOnlyClosedArtifactFields() {
    assert SccpSubmitEncoding.MAX_GROTH16_ARTIFACT_BYTES
        == 16 * 1024 * 1024 + 64 * 1024;
    assert SccpSubmitEncoding.MAX_DESTINATION_ARTIFACT_BYTES
        == 16 * 1024 * 1024 + 128 * 1024;
    assert SccpSubmitEncoding.MAX_DESTINATION_ARTIFACT_BASE64_BYTES == 22_544_384;
    final String artifact = canonicalArtifact();
    final String nativeArtifact = canonicalNativeArtifact();
    final SccpDestinationProofSubmitRequest proof =
        destinationRequest(AUTHORITY, artifact);
    assert proof.toJsonMap().keySet().equals(
        Set.of("authority", "fee_payment", "destination_proof_b64"));
    HttpClientTransport.preflightSccpBridgeSubmitJson(
        proof.toJsonBytes(), "/v1/bridge/proofs/submit");

    final SccpNativeMessageSubmitRequest message =
        messageRequest(AUTHORITY, nativeArtifact);
    assert message.toJsonMap().keySet().equals(
        Set.of("authority", "fee_payment", "native_proof_b64", "replay_witness_b64"));
    HttpClientTransport.preflightSccpBridgeSubmitJson(
        message.toJsonBytes(), "/v1/bridge/messages");
    expectFailure(
        () ->
            new SccpNativeMessageSubmitRequest(
                AUTHORITY,
                nativeArtifact,
                Base64.getEncoder()
                    .encodeToString(
                        canonicalReplayWitnessBytes(
                            fill(32, 1), new byte[32], Collections.emptyList())),
                BRIDGE_FEE_PAYMENT));
    final byte[] defaultBitmap = new byte[32];
    defaultBitmap[31] = 1;
    expectFailure(
        () ->
            new SccpNativeMessageSubmitRequest(
                AUTHORITY,
                nativeArtifact,
                Base64.getEncoder()
                    .encodeToString(
                        canonicalReplayWitnessBytes(
                            new byte[32],
                            defaultBitmap,
                            List.of(SccpReplayV1.emptyHashes().get(0)))),
                BRIDGE_FEE_PAYMENT));

    final byte[] transactionBytes;
    try {
      transactionBytes =
          new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
              .encodeTransaction(
                  TransactionPayload.builder()
                      .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
                      .setNetworkId(
                          org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
                      .setAuthority(AUTHORITY)
                      .setCreationTimeMs(7)
                      .setInstructions(Collections.emptyList())
                      .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
                      .build());
    } catch (final Exception ex) {
      throw new IllegalStateException("encode exact SCCP transaction fixture", ex);
    }
    final String transaction = Base64.getEncoder().encodeToString(transactionBytes);
    final byte[] gasBoundTransaction;
    try {
      gasBoundTransaction =
          new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
              .encodeTransaction(
                  TransactionPayload.builder()
                      .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 9L))
                      .setNetworkId(
                          org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
                      .setAuthority(AUTHORITY)
                      .setCreationTimeMs(7)
                      .setInstructions(Collections.emptyList())
                      .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
                      .build());
    } catch (final Exception ex) {
      throw new IllegalStateException("encode fee-bound SCCP transaction fixture", ex);
    }
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY,
                artifact,
                Base64.getEncoder().encodeToString(fill(64, 1)),
                Base64.getEncoder().encodeToString(gasBoundTransaction),
                7L));
    final String signature = Base64.getEncoder().encodeToString(fill(64, 1));
    expectFailure(() -> destinationRequest(AUTHORITY, artifact, signature, transaction, 7L));
    expectFailure(() -> messageRequest(AUTHORITY, nativeArtifact, signature, transaction, 7L));
    final Map<String, Object> retiredSignedFields = new LinkedHashMap<>(proof.toJsonMap());
    retiredSignedFields.put("signature_b64", signature);
    retiredSignedFields.put("transaction_payload_b64", transaction);
    retiredSignedFields.put("creation_time_ms", 7);
    expectFailure(
        () ->
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                jsonBytes(retiredSignedFields), "/v1/bridge/proofs/submit"));
    final String ordinaryTransaction;
    try {
      final NoritoJavaCodecAdapter codec =
          new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
      ordinaryTransaction =
          Base64.getEncoder()
              .encodeToString(
                  codec.encodeTransaction(
                      codec
                          .decodeTransaction(transactionBytes)
                          .toBuilder()
                          .setAdmissionIntent(TransactionAdmissionIntent.ORDINARY)
                          .build()));
    } catch (final Exception ex) {
      throw new IllegalStateException("encode ordinary SCCP transaction fixture", ex);
    }
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY, artifact, signature, ordinaryTransaction, 7L));
    expectFailure(
        () ->
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                message.toJsonBytes(), "/v1/bridge/proofs/submit"));
    expectFailure(
        () ->
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                proof.toJsonBytes(), "/v1/bridge/messages"));

    final SccpSubmitExecutor submitExecutor =
        new SccpSubmitExecutor(List.of("application/json"));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            submitExecutor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build());
    transport.submitSccpDestinationProof(proof).join();
    transport.submitSccpNativeMessage(message).join();
    assert submitExecutor.requests.size() == 2;
    assert submitExecutor.requests.stream()
        .allMatch(request -> request.maximumResponseBytes() == 64L * 1024L * 1024L);

    final SccpSubmitExecutor missingContentType = new SccpSubmitExecutor(List.of());
    final HttpClientTransport strictTransport =
        HttpClientTransport.withExecutor(
            missingContentType,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build());
    try {
      strictTransport.submitSccpDestinationProof(proof).join();
      throw new AssertionError("SCCP submit must reject a missing Content-Type");
    } catch (final CompletionException expected) {
      // Expected.
    }
  }

  private static void binaryProofRequestAcceptsOnlyTheTwoConcreteCurveTypes() {
    for (final String schemaName : SccpSubmitEncoding.PROOF_REQUEST_SCHEMA_NAMES) {
      final byte[] frame = canonicalArtifactBytes(schemaName, 0);
      final SccpNoritoExecutor executor = new SccpNoritoExecutor(frame);
      final HttpClientTransport transport =
          HttpClientTransport.withExecutor(
              executor,
              ClientConfig.builder()
                  .setBaseUri(URI.create("https://torii.example"))
                  .build());
      assert Arrays.equals(frame, transport.getSccpProofRequestNorito(MESSAGE_ID).join());
      assert executor.request.maximumResponseBytes()
          == SccpSubmitEncoding.MAX_GROTH16_ARTIFACT_BYTES;
      assert executor.request.headers().get("Accept").equals(List.of("application/x-norito"));
    }

    final SccpNoritoExecutor unknown =
        new SccpNoritoExecutor(canonicalArtifactBytes("example::UnknownProofRequestV1", 0));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            unknown,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build());
    try {
      transport.getSccpProofRequestNorito(MESSAGE_ID).join();
      throw new AssertionError("accepted an unknown SCCP proof-request schema");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException;
    }
  }

  static void signedSubmitPreservesExactTairaSponsorAcrossControllerOnlyWireIdentity()
      throws Exception {
    final String selector =
        "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A/cbsi_web";
    final FeeSponsorProgramId program = FeeSponsorProgramId.parse(selector);
    final FeePaymentIntent expectedFeePayment =
        FeePaymentIntent.sponsor(program, 1L, Collections.emptyList(), 9L);
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final byte[] encoded =
        codec.encodeTransaction(
            TransactionPayload.builder()
                .setNetworkId(
                    org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
                .setAuthority(AUTHORITY)
                .setCreationTimeMs(7L)
                .setInstructions(Collections.emptyList())
                .setFeePayment(expectedFeePayment)
                .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
                .build());
    final TransactionPayload decoded = codec.decodeTransaction(encoded);
    final FeePaymentIntent.Sponsor decodedSponsor =
        (FeePaymentIntent.Sponsor) decoded.feePayment();
    assert AccountAddress.detectI105Discriminant(decodedSponsor.programId().sponsor())
        == SccpV1.TAIRA_I105_DISCRIMINANT_V1;
    assert Arrays.equals(codec.encodeTransaction(decoded), encoded);
    assert program.literal().equals(selector);

    expectFailure(() -> new FeeSponsorProgramId(program.sponsor(), "cbsi_e\u0301"));
  }

  private static void submitAuthorityRequiresExactTairaDiscriminant() throws Exception {
    final AccountAddress address =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x41), "ed25519");
    final String tairaAuthority =
        address.toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final String artifact = canonicalArtifact();
    final String nativeArtifact = canonicalNativeArtifact();
    assert SccpV1.TAIRA_I105_DISCRIMINANT_V1 == 369;
    assert AccountAddress.detectI105Discriminant(tairaAuthority) == 369;
    destinationRequest(tairaAuthority, artifact);
    messageRequest(tairaAuthority, nativeArtifact);

    final String checksumMutation =
        tairaAuthority.substring(0, tairaAuthority.length() - 1)
            + (tairaAuthority.endsWith("1") ? "2" : "1");
    final Map<String, String> invalidAuthorities = new LinkedHashMap<>();
    invalidAuthorities.put(
        "default discriminant 753",
        address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT));
    invalidAuthorities.put("generic canonical hex", address.canonicalHex());
    invalidAuthorities.put("development discriminant", address.toI105(0));
    invalidAuthorities.put("custom discriminant", address.toI105(42));
    invalidAuthorities.put("malformed account alias", "alice");
    invalidAuthorities.put("checksum mutation", checksumMutation);
    for (final Map.Entry<String, String> entry : invalidAuthorities.entrySet()) {
      final String invalidAuthority = entry.getValue();
      expectFailure(
          () -> destinationRequest(invalidAuthority, artifact));
      expectFailure(
          () -> messageRequest(invalidAuthority, nativeArtifact));
      final Map<String, Object> body = map();
      body.put("authority", invalidAuthority);
      body.put("destination_proof_b64", artifact);
      expectFailure(
          () -> HttpClientTransport.preflightSccpBridgeSubmitJson(
              jsonBytes(body), "/v1/bridge/proofs/submit"));
    }
  }

  private static void submitPreflightRejectsRetiredOverridesAndSecrets() {
    final String artifact = canonicalArtifact();
    for (final String field :
        List.of(
            "private_key",
            "public_key_hex",
            "message_bundle_b64",
            "proof_bytes_hex",
            "network_id_hex",
            "verifier_address_hex",
            "bridge_address_hex",
            "tron_verifier_address",
            "manifest",
            "job")) {
      final Map<String, Object> body = map();
      body.put("authority", AUTHORITY);
      body.put("destination_proof_b64", artifact);
      body.put(field, "retired");
      expectFailure(
          () ->
              HttpClientTransport.preflightSccpBridgeSubmitJson(
                  jsonBytes(body), "/v1/bridge/proofs/submit"));
    }
    for (final String field :
        List.of(
            "private_key",
            "public_key_hex",
            "message_bundle_b64",
            "destination_proof_b64",
            "settlement",
            "asset_id",
            "recipient")) {
      final Map<String, Object> body = map();
      body.put("authority", AUTHORITY);
      body.put("native_proof_b64", artifact);
      body.put(field, "retired");
      expectFailure(
          () ->
              HttpClientTransport.preflightSccpBridgeSubmitJson(
                  jsonBytes(body), "/v1/bridge/messages"));
    }
    for (final String body : List.of("[]", "null", "{", "42")) {
      expectFailure(
          () ->
              HttpClientTransport.preflightSccpBridgeSubmitJson(
                  body.getBytes(StandardCharsets.UTF_8), "/v1/bridge/proofs/submit"));
    }
    final String duplicate =
        "{\"authority\":\""
            + AUTHORITY
            + "\",\"authority\":\""
            + AUTHORITY
            + "\",\"destination_proof_b64\":\""
            + artifact
            + "\"}";
    expectFailure(
        () ->
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                duplicate.getBytes(StandardCharsets.UTF_8), "/v1/bridge/proofs/submit"));

    final byte[] transactionBytes;
    try {
      transactionBytes =
          new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
              .encodeTransaction(
                  TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                      .setNetworkId(
                          org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
                      .setAuthority(AUTHORITY)
                      .setCreationTimeMs(7)
                      .setInstructions(Collections.emptyList())
                      .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
                      .build());
    } catch (final Exception ex) {
      throw new IllegalStateException("encode exact SCCP transaction fixture", ex);
    }
    final String transaction = Base64.getEncoder().encodeToString(transactionBytes);
    final String signature = Base64.getEncoder().encodeToString(fill(64, 1));
    final List<Map<String, Object>> malformed = new ArrayList<>();
    final Map<String, Object> signatureOnly = map();
    signatureOnly.put("authority", AUTHORITY);
    signatureOnly.put("destination_proof_b64", artifact);
    signatureOnly.put("signature_b64", signature);
    signatureOnly.put("creation_time_ms", 7);
    malformed.add(signatureOnly);
    final Map<String, Object> payloadOnly = map();
    payloadOnly.put("authority", AUTHORITY);
    payloadOnly.put("destination_proof_b64", artifact);
    payloadOnly.put("transaction_payload_b64", transaction);
    payloadOnly.put("creation_time_ms", 7);
    malformed.add(payloadOnly);
    final Map<String, Object> noTime = map();
    noTime.put("authority", AUTHORITY);
    noTime.put("destination_proof_b64", artifact);
    noTime.put("signature_b64", signature);
    noTime.put("transaction_payload_b64", transaction);
    malformed.add(noTime);
    final Map<String, Object> explicitNull = map();
    explicitNull.put("authority", AUTHORITY);
    explicitNull.put("destination_proof_b64", artifact);
    explicitNull.put("signature_b64", null);
    explicitNull.put("transaction_payload_b64", null);
    malformed.add(explicitNull);
    final Map<String, Object> explicitNullCreationTime = map();
    explicitNullCreationTime.put("authority", AUTHORITY);
    explicitNullCreationTime.put("destination_proof_b64", artifact);
    explicitNullCreationTime.put("creation_time_ms", null);
    malformed.add(explicitNullCreationTime);
    for (final Map<String, Object> body : malformed) {
      expectFailure(
          () ->
              HttpClientTransport.preflightSccpBridgeSubmitJson(
                  jsonBytes(body), "/v1/bridge/proofs/submit"));
    }
  }

  private static void artifactValidationRejectsAliasesCorruptionAndZeroSchema() {
    assert "iroha_data_model::bridge::BridgeSccpDestinationProofV1"
        .equals(SccpSubmitEncoding.DESTINATION_ARTIFACT_SCHEMA_NAME);
    final byte[] canonical =
        canonicalArtifactBytes(SccpSubmitEncoding.DESTINATION_ARTIFACT_SCHEMA_NAME, 0);
    final String encoded = Base64.getEncoder().encodeToString(canonical);
    expectFailure(() -> destinationRequest("alice", encoded));
    expectFailure(
        () -> destinationRequest(AUTHORITY, encoded.replace("=", "")));
    expectFailure(() -> destinationRequest(AUTHORITY, " " + encoded));
    final String legacyBn254Artifact =
        Base64.getEncoder()
            .encodeToString(
                canonicalArtifactBytes(
                    "iroha_sccp::SccpGroth16Bn254ProofArtifactV1", 0));
    expectFailure(() -> destinationRequest(AUTHORITY, legacyBn254Artifact));
    final byte[] corrupt = canonical.clone();
    corrupt[corrupt.length - 1]++;
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY, Base64.getEncoder().encodeToString(corrupt)));
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY,
                Base64.getEncoder()
                    .encodeToString(Arrays.copyOf(canonical, canonical.length + 1))));
    final byte[] zeroSchema = canonical.clone();
    Arrays.fill(zeroSchema, 6, 22, (byte) 0);
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY, Base64.getEncoder().encodeToString(zeroSchema)));
    expectFailure(
        () -> destinationRequest(AUTHORITY, encoded, null, null, 0L));
    expectFailure(
        () -> destinationRequest(AUTHORITY, encoded, "AQ==", null, null));
    final String signature = Base64.getEncoder().encodeToString(fill(64, 1));
    final String transaction;
    try {
      transaction =
          Base64.getEncoder()
              .encodeToString(
                  new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
                      .encodeTransaction(
                          TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                              .setNetworkId(
                                  org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
                              .setAuthority(AUTHORITY)
                              .setCreationTimeMs(7)
                              .setInstructions(Collections.emptyList())
                              .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
                              .build()));
    } catch (final Exception ex) {
      throw new IllegalStateException("encode exact SCCP transaction fixture", ex);
    }
    expectFailure(
        () -> destinationRequest(
            AUTHORITY, encoded, signature, null, 7L));
    expectFailure(
        () -> destinationRequest(
            AUTHORITY, encoded, null, transaction, 7L));
    expectFailure(
        () -> destinationRequest(
            AUTHORITY, encoded, signature, transaction, null));
    expectFailure(
        () -> destinationRequest(
            AUTHORITY, encoded, signature, transaction, 8L));
    final String wrongAuthorityTransaction;
    try {
      wrongAuthorityTransaction =
          Base64.getEncoder()
              .encodeToString(
                  new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
                      .encodeTransaction(
                          TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                              .setNetworkId(
                                  org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
                              .setAuthority(OTHER_AUTHORITY)
                              .setCreationTimeMs(7)
                              .setInstructions(Collections.emptyList())
                              .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
                              .build()));
    } catch (final Exception ex) {
      throw new IllegalStateException("encode mismatched SCCP authority fixture", ex);
    }
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY, encoded, signature, wrongAuthorityTransaction, 7L));
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY,
                encoded,
                Base64.getEncoder().encodeToString(fill(64, 0)),
                transaction,
                7L));
    final byte[] nativeProof =
        canonicalArtifactBytes(SccpSubmitEncoding.NATIVE_INBOUND_PROOF_SCHEMA_NAME, 0);
    messageRequest(
        AUTHORITY, Base64.getEncoder().encodeToString(nativeProof));
    expectFailure(
        () -> messageRequest(AUTHORITY, encoded));
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY, Base64.getEncoder().encodeToString(nativeProof)));
    expectFailure(
        () ->
            messageRequest(
                AUTHORITY,
                Base64.getEncoder()
                    .encodeToString(
                        canonicalArtifactBytes(
                            SccpSubmitEncoding.NATIVE_INBOUND_PROOF_SCHEMA_NAME, 8))));
    expectFailure(
        () ->
            destinationRequest(
                AUTHORITY,
                Base64.getEncoder()
                    .encodeToString(
                        canonicalArtifactBytes(
                            SccpSubmitEncoding.DESTINATION_ARTIFACT_SCHEMA_NAME, 8))));
  }

  private static void capabilitiesAreExactAndContainNoRetiredDiscoverySurface() {
    final SccpModels.Capabilities parsed =
        SccpJsonParser.parseCapabilities(jsonBytes(capabilities()));
    assert parsed.registryPath.equals("/v1/sccp/registry");
    assert parsed.proofRequestPath.equals("/v1/sccp/proof-requests/{message_id}");
    assert parsed.registryLimits.maxRetainedRoutesPerLane == 64;
    assert parsed.registryLimits.maxRetainedNativeTrustAnchorsPerLane == 4_096;
    assert parsed.resourceLimits.maxOutboundMessagesPerBlock == 512;
    assert parsed.resourceLimits.maxOutboundMessagePayloadBytes.equals(
        BigInteger.valueOf(4_096));
    assert parsed.resourceLimits.maxPendingOutboundMessages.equals(
        BigInteger.valueOf(65_536));
    assert parsed.resourceLimits.maxPendingOutboundPayloadBytes.equals(
        BigInteger.valueOf(268_435_456));
    assert parsed.resourceLimits.maxBlsSignerContributionsPerTransaction == 131_713;
    assert parsed.proofSubmitPath == null;
    final Map<String, Object> enabled = capabilities();
    enabled.put("proof_submit_path", "/v1/bridge/proofs/submit");
    enabled.put("native_message_submit_path", "/v1/bridge/messages");
    final SccpModels.Capabilities enabledParsed =
        SccpJsonParser.parseCapabilities(jsonBytes(enabled));
    assert enabledParsed.proofSubmitPath.equals("/v1/bridge/proofs/submit");
    assert enabledParsed.nativeMessageSubmitPath.equals("/v1/bridge/messages");
    final Map<String, Object> proofOnly = capabilities();
    proofOnly.put("proof_submit_path", "/v1/bridge/proofs/submit");
    expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(proofOnly)));
    final Map<String, Object> messageOnly = capabilities();
    messageOnly.put("native_message_submit_path", "/v1/bridge/messages");
    expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(messageOnly)));
    for (final String field : List.of("outbound", "codecs", "inbound_lanes", "manifests")) {
      final Map<String, Object> hostile = capabilities();
      hostile.put(field, Map.of());
      expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(hostile)));
    }
    final Map<String, Object> retiredPath = capabilities();
    retiredPath.put("proof_request_path", "/v1/sccp/jobs/message/{message_id}");
    expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(retiredPath)));
    for (final String field :
        List.of(
            "max_outbound_messages_per_block",
            "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
            "max_proofs_per_transaction",
            "max_proofs_per_block",
            "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block",
            "max_native_headers_per_transaction",
            "max_native_headers_per_block",
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
            "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block",
            "max_secp256k1_recoveries_per_transaction",
            "max_secp256k1_recoveries_per_block",
            "max_bls_aggregate_checks_per_transaction",
            "max_bls_aggregate_checks_per_block",
            "max_bls_signer_contributions_per_transaction",
            "max_bls_signer_contributions_per_block",
            "max_ed25519_signature_checks_per_transaction",
            "max_ed25519_signature_checks_per_block",
            "max_ed25519_validator_key_checks_per_transaction",
            "max_ed25519_validator_key_checks_per_block",
            "max_bn254_pairing_checks_per_transaction",
            "max_bn254_pairing_checks_per_block",
            "max_bls12_381_pairing_checks_per_transaction",
            "max_bls12_381_pairing_checks_per_block")) {
      final Map<String, Object> hostile = capabilities();
      object(hostile.get("resource_limits")).put(field, 0);
      expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(hostile)));
    }
    for (final String field :
        List.of(
            "max_outbound_messages_per_block",
            "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes")) {
      final Map<String, Object> missing = capabilities();
      object(missing.get("resource_limits")).remove(field);
      expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(missing)));
    }
    final Map<String, Object> unknownResourceLimit = capabilities();
    object(unknownResourceLimit.get("resource_limits"))
        .put("max_outbound_messages_per_transaction", 1);
    expectFailure(
        () -> SccpJsonParser.parseCapabilities(jsonBytes(unknownResourceLimit)));
    final Map<String, List<Object>> hostileFixedLimits = new LinkedHashMap<>();
    hostileFixedLimits.put(
        "max_outbound_messages_per_block", List.of(511, 513, "512", Boolean.TRUE));
    hostileFixedLimits.put(
        "max_outbound_message_payload_bytes", List.of(4_095, 4_097, "4096", Boolean.TRUE));
    for (final Map.Entry<String, List<Object>> entry : hostileFixedLimits.entrySet()) {
      for (final Object replacement : entry.getValue()) {
        final Map<String, Object> hostile = capabilities();
        object(hostile.get("resource_limits")).put(entry.getKey(), replacement);
        expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(hostile)));
      }
      final Map<String, Object> nullValue = capabilities();
      object(nullValue.get("resource_limits")).put(entry.getKey(), null);
      expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(nullValue)));
    }
    final long jsSafeMaximum = 9_007_199_254_740_991L;
    final List<String> byteLimitFields =
        List.of(
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
            "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block",
            "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block");
    for (final String field :
        List.of(
            "max_outbound_messages_per_block",
            "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
            "max_proofs_per_transaction",
            "max_proofs_per_block",
            "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block",
            "max_native_headers_per_transaction",
            "max_native_headers_per_block",
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
            "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block",
            "max_secp256k1_recoveries_per_transaction",
            "max_secp256k1_recoveries_per_block",
            "max_bls_aggregate_checks_per_transaction",
            "max_bls_aggregate_checks_per_block",
            "max_bls_signer_contributions_per_transaction",
            "max_bls_signer_contributions_per_block",
            "max_ed25519_signature_checks_per_transaction",
            "max_ed25519_signature_checks_per_block",
            "max_ed25519_validator_key_checks_per_transaction",
            "max_ed25519_validator_key_checks_per_block",
            "max_bn254_pairing_checks_per_transaction",
            "max_bn254_pairing_checks_per_block",
            "max_bls12_381_pairing_checks_per_transaction",
            "max_bls12_381_pairing_checks_per_block")) {
      final long overflow =
          byteLimitFields.contains(field) ? jsSafeMaximum + 1L : 4_294_967_296L;
      for (final Object replacement : List.of(Boolean.TRUE, 1.5d, overflow)) {
        final Map<String, Object> hostile = capabilities();
        object(hostile.get("resource_limits")).put(field, replacement);
        expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(hostile)));
      }
    }
    final Map<String, Object> boundary = capabilities();
    final Map<String, Object> boundaryLimits = object(boundary.get("resource_limits"));
    for (final String field : byteLimitFields) {
      boundaryLimits.put(field, jsSafeMaximum);
    }
    final SccpModels.Capabilities boundaryParsed =
        SccpJsonParser.parseCapabilities(jsonBytes(boundary));
    assert boundaryParsed.resourceLimits.maxProofBytesPerBlock.equals(
        BigInteger.valueOf(jsSafeMaximum));
    for (final String field : byteLimitFields) {
      final Map<String, Object> overflow = capabilities();
      object(overflow.get("resource_limits")).put(field, jsSafeMaximum + 1L);
      expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(overflow)));
    }
    final String canonicalJson =
        new String(jsonBytes(capabilities()), StandardCharsets.UTF_8);
    final String integerToken = "\"max_proof_bytes_per_proof\":8388608";
    for (final String replacement : List.of("1.0", "1e0", "-1", "01", "١")) {
      final String hostile =
          canonicalJson.replace(
              integerToken, "\"max_proof_bytes_per_proof\":" + replacement);
      assert !hostile.equals(canonicalJson) : "integer fixture token must be present";
      try {
        SccpJsonParser.parseCapabilities(hostile.getBytes(StandardCharsets.UTF_8));
        throw new AssertionError("noncanonical integer token must be rejected: " + replacement);
      } catch (final RuntimeException expected) {
        // Expected.
      }
    }
    final List<List<String>> orderingRelations =
        List.of(
            List.of("max_proof_bytes_per_proof", "max_proof_bytes_per_transaction"),
            List.of("max_proofs_per_transaction", "max_proofs_per_block"),
            List.of("max_proof_bytes_per_transaction", "max_proof_bytes_per_block"),
            List.of("max_native_headers_per_transaction", "max_native_headers_per_block"),
            List.of(
                "max_ethereum_light_client_updates_per_transaction",
                "max_ethereum_light_client_updates_per_block"),
            List.of(
                "max_native_header_bytes_per_transaction",
                "max_native_header_bytes_per_block"),
            List.of(
                "max_secp256k1_recoveries_per_transaction",
                "max_secp256k1_recoveries_per_block"),
            List.of(
                "max_bls_aggregate_checks_per_transaction",
                "max_bls_aggregate_checks_per_block"),
            List.of(
                "max_bls_signer_contributions_per_transaction",
                "max_bls_signer_contributions_per_block"),
            List.of(
                "max_ed25519_signature_checks_per_transaction",
                "max_ed25519_signature_checks_per_block"),
            List.of(
                "max_ed25519_validator_key_checks_per_transaction",
                "max_ed25519_validator_key_checks_per_block"),
            List.of(
                "max_bn254_pairing_checks_per_transaction",
                "max_bn254_pairing_checks_per_block"),
            List.of(
                "max_bls12_381_pairing_checks_per_transaction",
                "max_bls12_381_pairing_checks_per_block"));
    for (final List<String> relation : orderingRelations) {
      final Map<String, Object> hostile = capabilities();
      final Map<String, Object> limits = object(hostile.get("resource_limits"));
      final String lowerField = relation.get(0);
      final String upperField = relation.get(1);
      limits.put(lowerField, ((Number) limits.get(upperField)).longValue() + 1L);
      expectFailure(() -> SccpJsonParser.parseCapabilities(jsonBytes(hostile)));
    }
    final Map<String, Object> driftedRegistryLimits = capabilities();
    object(driftedRegistryLimits.get("registry_limits"))
        .put("max_retained_routes_per_lane", 65);
    expectFailure(
        () -> SccpJsonParser.parseCapabilities(jsonBytes(driftedRegistryLimits)));
  }

  private static void registryValidatesSemanticPolicyAndExactFamilies() {
    final Map<String, Object> exactRegistry = registry();
    final String bscRouteConfigurationHash =
        (String) sourceIdentity(firstRoute(exactRegistry)).get("route_config_hash");
    assert bscRouteConfigurationHash.equals(BSC_ROUTE_CONFIG_HASH)
        : bscRouteConfigurationHash;
    final SccpModels.RegistryV1 parsed =
        SccpJsonParser.parseRegistry(jsonBytes(exactRegistry));
    assert parsed.version == 1 && parsed.lanes.size() == 1;

    final Map<String, Object> badKey = registry();
    deployment(badKey).put("verifier_key_hash", upper(0x2f, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(badKey)));

    final Map<String, Object> exactAnchors = registry();
    firstLane(exactAnchors)
        .put(
            "native_trust_anchors",
            new ArrayList<>(Collections.nCopies(4_096, null)));
    assert !expectFailureMessage(
            () -> SccpJsonParser.parseRegistry(jsonBytes(exactAnchors)))
        .contains("more than 4,096");
    final Map<String, Object> overAnchors = registry();
    firstLane(overAnchors)
        .put(
            "native_trust_anchors",
            new ArrayList<>(Collections.nCopies(4_097, null)));
    assert expectFailureMessage(
            () -> SccpJsonParser.parseRegistry(jsonBytes(overAnchors)))
        .contains("more than 4,096");

    final Map<String, Object> exactRoutes = registry();
    firstLane(exactRoutes)
        .put("routes", new ArrayList<>(Collections.nCopies(64, map())));
    assert !expectFailureMessage(
            () -> SccpJsonParser.parseRegistry(jsonBytes(exactRoutes)))
        .contains("more than 64 retained");
    final Map<String, Object> overRoutes = registry();
    firstLane(overRoutes)
        .put("routes", new ArrayList<>(Collections.nCopies(65, map())));
    assert expectFailureMessage(
            () -> SccpJsonParser.parseRegistry(jsonBytes(overRoutes)))
        .contains("more than 64 retained");

    final Map<String, Object> missingSignal = registry();
    final Map<String, Object> key = object(deployment(missingSignal).get("verifying_key"));
    object(key.get("ic")).remove("signal_10");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(missingSignal)));

    final Map<String, Object> retired = registry();
    object(list(retired, "lanes").get(0))
        .put(
            "lane_id",
            Map.of("source", network("solana-mainnet-beta"), "target", network("sora-taira")));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(retired)));

    final Map<String, Object> noncanonicalWireName = registry();
    object(object(firstLane(noncanonicalWireName).get("lane_id")).get("source"))
        .put("network", "bsc-mainnet");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(noncanonicalWireName)));

    final Map<String, Object> wrongSchema = registry();
    final Map<String, Object> policy = object(deployment(wrongSchema).get("outbound_proof_policy"));
    object(object(policy.get("semantic_profile")).get("commitments"))
        .put("public_signal_schema_hash", upper(0x2e, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(wrongSchema)));

    final Map<String, Object> replayAddressAlias = registry();
    deployment(replayAddressAlias)
        .put("replay_verifier_address", deployment(replayAddressAlias).get("route_address"));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(replayAddressAlias)));

    final Map<String, Object> replayAddressSubstitution = registry();
    deployment(replayAddressSubstitution).put("replay_verifier_address", upper(0x73, 20));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(replayAddressSubstitution)));

    final Map<String, Object> replayRuntimeSubstitution = registry();
    deployment(replayRuntimeSubstitution).put("replay_verifier_code_hash", upper(0x44, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(replayRuntimeSubstitution)));

    final Map<String, Object> breakerAddressSubstitution = registry();
    deployment(breakerAddressSubstitution).put("mint_breaker_address", upper(0x74, 20));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(breakerAddressSubstitution)));

    final Map<String, Object> breakerRuntimeSubstitution = registry();
    deployment(breakerRuntimeSubstitution).put("mint_breaker_code_hash", upper(0x45, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(breakerRuntimeSubstitution)));

    final Map<String, Object> swappedRoles = registry();
    final Map<String, Object> swappedDeployment = deployment(swappedRoles);
    final Object replayAddress = swappedDeployment.get("replay_verifier_address");
    final Object replayCodeHash = swappedDeployment.get("replay_verifier_code_hash");
    swappedDeployment.put(
        "replay_verifier_address", swappedDeployment.get("mint_breaker_address"));
    swappedDeployment.put(
        "replay_verifier_code_hash", swappedDeployment.get("mint_breaker_code_hash"));
    swappedDeployment.put("mint_breaker_address", replayAddress);
    swappedDeployment.put("mint_breaker_code_hash", replayCodeHash);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(swappedRoles)));

    final Map<String, Object> emptyRuntimeHash = registry();
    deployment(emptyRuntimeHash)
        .put(
            "mint_breaker_code_hash",
            "C5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(emptyRuntimeHash)));

    final Map<String, Object> zeroCap = registry();
    deployment(zeroCap).put("max_wrapped_supply", 0);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(zeroCap)));

    final Map<String, Object> missingExecutionPolicy = registry();
    firstRoute(missingExecutionPolicy).remove("sora_outbound_execution_policy");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(missingExecutionPolicy)));

    final Map<String, Object> wrongExecutionSemantics = registry();
    object(firstRoute(wrongExecutionSemantics).get("sora_outbound_execution_policy"))
        .put("semantics", "unproved_record_sccp_message_v1");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(wrongExecutionSemantics)));

    final Map<String, Object> wrongLiabilityCap = registry();
    object(firstRoute(wrongLiabilityCap).get("settlement"))
        .put("max_outstanding_liability", 8);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(wrongLiabilityCap)));
  }

  private static void registryRejectsMalformedAnchorHistories() {
    final Map<String, Object> canonical = registry();
    final Map<String, Object> canonicalLane = firstLane(canonical);
    final Map<String, Object> first = nativeTrustAnchor(0x91, 10, "bsc_parlia_v1");
    final Map<String, Object> second = nativeTrustAnchor(0x92, 20, "bsc_parlia_v1");
    canonicalLane.put(
        "native_trust_anchors", new ArrayList<>(List.of(first, second)));
    canonicalLane.put("current_native_trust_anchor_hash", upper(0x92, 32));
    SccpJsonParser.parseRegistry(jsonBytes(canonical));

    final Map<String, Object> singular = registry();
    final Map<String, Object> singularLane = firstLane(singular);
    singularLane.remove("native_trust_anchors");
    singularLane.remove("current_native_trust_anchor_hash");
    singularLane.put("native_trust_anchor", first);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(singular)));

    final Map<String, Object> nullAnchor = registry();
    final List<Object> anchorsWithNull = new ArrayList<>();
    anchorsWithNull.add(null);
    firstLane(nullAnchor).put("native_trust_anchors", anchorsWithNull);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(nullAnchor)));

    final Map<String, Object> duplicate = registry();
    firstLane(duplicate)
        .put(
            "native_trust_anchors",
            new ArrayList<>(
                List.of(
                    nativeTrustAnchor(0x91, 10, "bsc_parlia_v1"),
                    nativeTrustAnchor(0x91, 20, "bsc_parlia_v1"))));
    firstLane(duplicate).put("current_native_trust_anchor_hash", upper(0x91, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(duplicate)));

    final Map<String, Object> rollback = registry();
    firstLane(rollback)
        .put(
            "native_trust_anchors",
            new ArrayList<>(
                List.of(
                    nativeTrustAnchor(0x91, 20, "bsc_parlia_v1"),
                    nativeTrustAnchor(0x92, 20, "bsc_parlia_v1"))));
    firstLane(rollback).put("current_native_trust_anchor_hash", upper(0x92, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(rollback)));

    final Map<String, Object> wrongFamily = registry();
    firstLane(wrongFamily)
        .put(
            "native_trust_anchors",
            new ArrayList<>(List.of(nativeTrustAnchor(0x91, 10, "ethereum_beacon_v1"))));
    firstLane(wrongFamily).put("current_native_trust_anchor_hash", upper(0x91, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(wrongFamily)));

    final Map<String, Object> stalePointer = registry();
    firstLane(stalePointer)
        .put(
            "native_trust_anchors",
            new ArrayList<>(
                List.of(
                    nativeTrustAnchor(0x91, 10, "bsc_parlia_v1"),
                    nativeTrustAnchor(0x92, 20, "bsc_parlia_v1"))));
    firstLane(stalePointer).put("current_native_trust_anchor_hash", upper(0x91, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(stalePointer)));

    final Map<String, Object> pointerWithoutHistory = registry();
    firstLane(pointerWithoutHistory).put("current_native_trust_anchor_hash", upper(0x91, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(pointerWithoutHistory)));

    final Map<String, Object> historyWithoutPointer = registry();
    firstLane(historyWithoutPointer)
        .put(
            "native_trust_anchors",
            new ArrayList<>(List.of(nativeTrustAnchor(0x91, 10, "bsc_parlia_v1"))));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(historyWithoutPointer)));

    final Map<String, Object> inboundWithoutAnchor = registry();
    activation(firstRoute(inboundWithoutAnchor)).put("activation", "bidirectional");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(inboundWithoutAnchor)));

    final Map<String, Object> inboundWithAnchor = registry();
    activation(firstRoute(inboundWithAnchor)).put("activation", "bidirectional");
    firstLane(inboundWithAnchor)
        .put(
            "native_trust_anchors",
            new ArrayList<>(List.of(nativeTrustAnchor(0x91, 10, "bsc_parlia_v1"))));
    firstLane(inboundWithAnchor).put("current_native_trust_anchor_hash", upper(0x91, 32));
    SccpJsonParser.parseRegistry(jsonBytes(inboundWithAnchor));
  }

  private static void registryCountsOnlyNonRetiredRoutes() {
    SccpJsonParser.parseRegistry(jsonBytes(registryWithRouteHistory(10, 8)));
    expectFailure(
        () -> SccpJsonParser.parseRegistry(jsonBytes(registryWithRouteHistory(10, 9))));
  }

  private static void registryRequiresExactRetiredRouteInboundFinalityCutoff() {
    final Map<String, Object> missing = registry();
    firstRoute(missing).remove("inbound_finality_cutoff");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(missing)));

    final Map<String, Object> liveWithCutoff = registry();
    firstRoute(liveWithCutoff).put("inbound_finality_cutoff", inboundFinalityCutoff());
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(liveWithCutoff)));

    final Map<String, Object> retiredWithoutCutoff = registry();
    activation(firstRoute(retiredWithoutCutoff)).put("activation", "retired");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(retiredWithoutCutoff)));

    final Map<String, Object> valid = retiredRegistry();
    SccpJsonParser.parseRegistry(jsonBytes(valid));

    final Map<String, Object> unknownAnchor = retiredRegistry();
    object(firstRoute(unknownAnchor).get("inbound_finality_cutoff"))
        .put("trust_anchor_hash", upper(0x7f, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(unknownAnchor)));

    final Map<String, Object> openEnded = retiredRegistry();
    final Map<String, Object> openEndedCutoff =
        object(firstRoute(openEnded).get("inbound_finality_cutoff"));
    openEndedCutoff.put("trust_anchor_hash", upper(0x62, 32));
    openEndedCutoff.put("max_anchor_interval_height", 9);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(openEnded)));

    final Map<String, Object> partialInterval = retiredRegistry();
    object(firstRoute(partialInterval).get("inbound_finality_cutoff"))
        .put("max_anchor_interval_height", 7);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(partialInterval)));

    final Map<String, Object> legacyExtra = retiredRegistry();
    object(firstRoute(legacyExtra).get("inbound_finality_cutoff")).put("height", 8);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(legacyExtra)));
  }

  private static void verifyingKeysAllowZeroCoordinatesButRejectInfinity() {
    final Map<String, Object> individualZeros = registry();
    final Map<String, Object> key = object(deployment(individualZeros).get("verifying_key"));
    object(key.get("alpha1")).put("x", upper(0, 32));
    object(key.get("beta2")).put("x_c0", upper(0, 32));
    deployment(individualZeros).put("verifier_key_hash", keyHash(key));
    refreshRouteConfiguration(firstRoute(individualZeros));
    SccpJsonParser.parseRegistry(jsonBytes(individualZeros));

    final Map<String, Object> g1Infinity = registry();
    final Map<String, Object> g1 =
        object(object(deployment(g1Infinity).get("verifying_key")).get("alpha1"));
    g1.put("x", upper(0, 32));
    g1.put("y", upper(0, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(g1Infinity)));

    final Map<String, Object> g2Infinity = registry();
    final Map<String, Object> g2 =
        object(object(deployment(g2Infinity).get("verifying_key")).get("beta2"));
    for (final String field : List.of("x_c0", "x_c1", "y_c0", "y_c1")) {
      g2.put(field, upper(0, 32));
    }
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(g2Infinity)));

    final Map<String, Object> outsideField = registry();
    object(object(deployment(outsideField).get("verifying_key")).get("alpha1"))
        .put(
            "x",
            "30644E72E131A029B85045B68181585D97816A916871CA8D3C208C16D87CFD47");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(outsideField)));
  }

  private static void registryValidatesExactTonDeploymentAndRoleSeparation() {
    final Map<String, Object> canonical = tonRegistry();
    final SccpModels.RegistryV1 parsed = SccpJsonParser.parseRegistry(jsonBytes(canonical));
    assert parsed.lanes.size() == 1;
    final Map<String, Object> parsedLane = parsed.lanes.get(0);
    assert "ton_mainnet"
        .equals(object(object(parsedLane.get("lane_id")).get("source")).get("network"));
    assert list(parsedLane, "routes").size() == 1;

    final Map<String, Object> changedInitialData = tonRegistry();
    tonDeployment(changedInitialData).put("jetton_master_initial_data_hash", upper(0x38, 32));
    tonDeployment(changedInitialData).put("route_initial_data_hash", upper(0x39, 32));
    SccpJsonParser.parseRegistry(jsonBytes(changedInitialData));

    final Map<String, Object> missingInitialData = tonRegistry();
    tonDeployment(missingInitialData).remove("route_initial_data_hash");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(missingInitialData)));

    final Map<String, Object> initialDataAlias = tonRegistry();
    tonDeployment(initialDataAlias)
        .put(
            "route_initial_data_hash",
            tonDeployment(initialDataAlias).get("jetton_master_code_hash"));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(initialDataAlias)));

    final Map<String, Object> addressAlias = tonRegistry();
    tonSourceIdentity(addressAlias)
        .put("address", tonDeployment(addressAlias).get("jetton_master_address"));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(addressAlias)));

    final Map<String, Object> codeAlias = tonRegistry();
    tonSourceIdentity(codeAlias)
        .put("code_hash", tonDeployment(codeAlias).get("jetton_master_code_hash"));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(codeAlias)));

    final Map<String, Object> wrongProofProfile = tonRegistry();
    tonDeployment(wrongProofProfile).put("proof_profile_commitment", upper(0x7f, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(wrongProofProfile)));

    final Map<String, Object> uncompressedKey = tonRegistry();
    object(tonDeployment(uncompressedKey).get("verifying_key")).put("alpha1", upper(1, 48));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(uncompressedKey)));

    final Map<String, Object> unsortedGuardians = tonRegistry();
    final Map<String, Object> unsorted =
        object(tonDeployment(unsortedGuardians).get("mint_breaker_guardian_keys"));
    unsorted.put("guardian_1", unsorted.get("guardian_0"));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(unsortedGuardians)));

    final Map<String, Object> zeroGuardian = tonRegistry();
    object(tonDeployment(zeroGuardian).get("mint_breaker_guardian_keys"))
        .put("guardian_0", upper(0, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(zeroGuardian)));

    final Map<String, Object> zeroCap = tonRegistry();
    tonDeployment(zeroCap).put("max_wrapped_supply", 0);
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(zeroCap)));

    assert SccpModels.requireTonAmountWithinCapV1(BigInteger.valueOf(9), BigInteger.TEN)
        .equals(BigInteger.valueOf(9));
    expectFailure(
        () -> SccpModels.requireTonAmountWithinCapV1(BigInteger.valueOf(11), BigInteger.TEN));
  }

  private static void tonProofRequestBindsExactBlsSignalsAndProfile() {
    final Map<String, Object> canonical = tonProofRequest();
    final SccpModels.Groth16ProofRequestV1 parsed =
        SccpJsonParser.parseProofRequest(jsonBytes(canonical));
    assert "ton_groth16_bls12381_v1".equals(parsed.backend);
    assert parsed.targetNetwork == SccpNetworkV1.TON_MAINNET;
    assert parsed.publicSignals.size() == 11;
    assert parsed.verifierCircuitHash.equals(canonical.get("verifier_circuit_hash"));

    final Map<String, Object> alteredSignal = tonProofRequest();
    object(alteredSignal.get("public_signals")).put("route_configuration_hash", prefixed(0x7e));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(alteredSignal)));

    final Map<String, Object> missingProfile = tonProofRequest();
    missingProfile.remove("proof_profile_commitment");
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(missingProfile)));
  }

  private static void routeConfigurationIsNetworkExactAndPolicyBound() {
    final Map<String, Object> wrongRoute = registry();
    firstRoute(wrongRoute).put("route_id", "taira_eth_xor");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(wrongRoute)));

    final Map<String, Object> forgedConfiguration = registry();
    sourceIdentity(firstRoute(forgedConfiguration)).put("route_config_hash", upper(0x7a, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(forgedConfiguration)));

    final Map<String, Object> keyBound = registry();
    final Map<String, Object> key = object(deployment(keyBound).get("verifying_key"));
    object(key.get("alpha1")).put("x", upper(7, 32));
    deployment(keyBound).put("verifier_key_hash", keyHash(key));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(keyBound)));
    refreshRouteConfiguration(firstRoute(keyBound));
    SccpJsonParser.parseRegistry(jsonBytes(keyBound));

    final Map<String, Object> semanticBound = registry();
    final Map<String, Object> semanticPolicy =
        object(deployment(semanticBound).get("outbound_proof_policy"));
    object(object(semanticPolicy.get("semantic_profile")).get("commitments"))
        .put("circuit_commitment", upper(0x18, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(semanticBound)));
    refreshRouteConfiguration(firstRoute(semanticBound));
    SccpJsonParser.parseRegistry(jsonBytes(semanticBound));

    final Map<String, Object> anchorBound = registry();
    final Map<String, Object> anchorPolicy =
        object(deployment(anchorBound).get("outbound_proof_policy"));
    object(anchorPolicy.get("sora_finality_anchor"))
        .put("checkpoint_block_hash", upper(0x17, 32));
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(anchorBound)));
    refreshRouteConfiguration(firstRoute(anchorBound));
    SccpJsonParser.parseRegistry(jsonBytes(anchorBound));

    final Map<String, Object> tron = tronRegistry();
    final String tronRouteConfigurationHash =
        (String) sourceIdentity(firstRoute(tron)).get("route_config_hash");
    assert tronRouteConfigurationHash.equals(TRON_ROUTE_CONFIG_HASH)
        : tronRouteConfigurationHash;
    SccpJsonParser.parseRegistry(jsonBytes(tron));
    firstRoute(tron).put("route_id", "taira_bsc_xor");
    expectFailure(() -> SccpJsonParser.parseRegistry(jsonBytes(tron)));
  }

  private static void bundleAndProofRequestRejectRetiredAndAliasedRoles() {
    final SccpModels.MessageBundleV1 bundle =
        SccpJsonParser.parseMessageBundle(jsonBytes(messageBundle()));
    assert bundle.messageIdHex.equals(MESSAGE_ID);
    assert bundle.targetNetwork.profileKey().equals("bsc-mainnet");

    final Map<String, Object> retiredPayload = messageBundle();
    retiredPayload.put("payload", Map.of("TokenPause", Map.of()));
    expectFailure(() -> SccpJsonParser.parseMessageBundle(jsonBytes(retiredPayload)));

    final Map<String, Object> alias = messageBundle();
    final Map<String, Object> context =
        object(object(alias.get("commitment")).get("context"));
    context.put("route_configuration_hash", context.get("destination_binding_hash"));
    expectFailure(() -> SccpJsonParser.parseMessageBundle(jsonBytes(alias)));

    final SccpModels.Groth16ProofRequestV1 request =
        SccpJsonParser.parseProofRequest(jsonBytes(proofRequest()));
    assert request.messageIdHex.equals(MESSAGE_ID);
    assert request.backend.equals("evm_groth16_bn254_v1");
    assert request.semanticProofProfile.profile.equals(
        "sora_taira_finality_inclusion_groth16_bn254");
    assert request.semanticProofProfile.commitments.version == 1;
    assert request.semanticProofProfile.commitments.circuitCommitment.equals(upper(0x11, 32));
    assert request.semanticProofProfile.commitments.witnessGeneratorCommitment.equals(
        upper(0x12, 32));
    assert request.semanticProofProfile.commitments.publicSignalSchemaHash.equals(
        publicSignalSchemaHash());
    assert request.semanticProofProfile.profileHash.equals(
        "0x" + semanticProfileHash().toLowerCase());
    assert request.soraFinalityAnchor.version == 1;
    assert request.soraFinalityAnchor.sourceNetwork == SccpNetworkV1.SORA_TAIRA;
    assert request.soraFinalityAnchor.chainIdHash.equals(tairaChainIdHash());
    assert request.soraFinalityAnchor.checkpointHeight.equals(BigInteger.valueOf(7));
    assert request.soraFinalityAnchor.checkpointBlockHash.equals(upper(0xa1, 32));
    assert request.soraFinalityAnchor.protocolVersion == 4;
    assert request.soraFinalityAnchor.checkpointContextId.equals(upper(0xa2, 32));
    assert request.soraFinalityAnchor.checkpointFinalityArtifactHash.equals(upper(0xa3, 32));
    assert request.soraFinalityAnchor.anchorHash.equals(
        "0xcdbec097fed4ad21e44a354fe09a3c43ad489f4ac78cff8944ba8bb5cc2fd577");
    assert request.soraFinalityAnchor.anchorHash.equals(
        "0x" + finalityAnchorHash().toLowerCase());

    final Map<String, Object> retiredProtocol = proofRequest();
    final Map<String, Object> retiredProtocolAnchor =
        object(retiredProtocol.get("sora_finality_anchor"));
    retiredProtocolAnchor.put("protocol_version", 3);
    retiredProtocol.put(
        "sora_finality_anchor_hash",
        "0x" + finalityAnchorHash(retiredProtocolAnchor).toLowerCase());
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(retiredProtocol)));

    final Map<String, Object> wrongProtocol = proofRequest();
    object(wrongProtocol.get("sora_finality_anchor")).put("protocol_version", 1);
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(wrongProtocol)));
    final Map<String, Object> futureProtocol = proofRequest();
    object(futureProtocol.get("sora_finality_anchor")).put("protocol_version", 5);
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(futureProtocol)));
    final Map<String, Object> wrongProtocolType = proofRequest();
    object(wrongProtocolType.get("sora_finality_anchor")).put("protocol_version", "4");
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(wrongProtocolType)));
    final Map<String, Object> floatingProtocol = proofRequest();
    object(floatingProtocol.get("sora_finality_anchor")).put("protocol_version", 4.0d);
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(floatingProtocol)));
    final Map<String, Object> booleanProtocol = proofRequest();
    object(booleanProtocol.get("sora_finality_anchor")).put("protocol_version", true);
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(booleanProtocol)));
    final Map<String, Object> legacyAnchor = proofRequest();
    object(legacyAnchor.get("sora_finality_anchor")).put("validator_set_epoch", 3);
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(legacyAnchor)));
    final Map<String, Object> zeroContext = proofRequest();
    object(zeroContext.get("sora_finality_anchor")).put("checkpoint_context_id", upper(0, 32));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(zeroContext)));
    final Map<String, Object> aliasedContext = proofRequest();
    final Map<String, Object> contextAnchor = object(aliasedContext.get("sora_finality_anchor"));
    contextAnchor.put("checkpoint_context_id", contextAnchor.get("chain_id_hash"));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(aliasedContext)));
    final Map<String, Object> aliasedBlock = proofRequest();
    final Map<String, Object> blockAnchor = object(aliasedBlock.get("sora_finality_anchor"));
    blockAnchor.put("checkpoint_block_hash", blockAnchor.get("checkpoint_context_id"));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(aliasedBlock)));
    final Map<String, Object> zeroArtifact = proofRequest();
    object(zeroArtifact.get("sora_finality_anchor"))
        .put("checkpoint_finality_artifact_hash", upper(0, 32));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(zeroArtifact)));
    final Map<String, Object> aliasedArtifact = proofRequest();
    final Map<String, Object> aliasedAnchor = object(aliasedArtifact.get("sora_finality_anchor"));
    aliasedAnchor.put("checkpoint_finality_artifact_hash", aliasedAnchor.get("checkpoint_block_hash"));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(aliasedArtifact)));
    final Map<String, Object> missingArtifact = proofRequest();
    object(missingArtifact.get("sora_finality_anchor"))
        .remove("checkpoint_finality_artifact_hash");
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(missingArtifact)));

    final Map<String, Object> wrongBackend = proofRequest();
    final Map<String, Object> hostileBackend = map();
    hostileBackend.put("backend", "tron_groth16_bn254_v1");
    hostileBackend.put("family", null);
    wrongBackend.put("backend", hostileBackend);
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(wrongBackend)));
    final Map<String, Object> override = proofRequest();
    override.put("network_id_hex", prefixed(0x7f));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(override)));
    final Map<String, Object> semanticMismatch = proofRequest();
    semanticMismatch.put("semantic_proof_profile_hash", prefixed(0x7d));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(semanticMismatch)));
    final Map<String, Object> anchorMismatch = proofRequest();
    anchorMismatch.put("sora_finality_anchor_hash", prefixed(0x7e));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(anchorMismatch)));
    final Map<String, Object> archivedIdentity = proofRequest();
    object(archivedIdentity.get("sora_finality_anchor"))
        .put(
            "chain_id_hash",
            upperHex(keccak(hexBytes("809574f5fee75e69bfcf52451e42d50f"))));
    expectFailure(() -> SccpJsonParser.parseProofRequest(jsonBytes(archivedIdentity)));
    final Map<String, Object> oversizedAmount = messageBundle();
    object(object(oversizedAmount.get("payload")).get("Transfer"))
        .put("amount", BigInteger.ONE.shiftLeft(128).toString());
    expectFailure(() -> SccpJsonParser.parseMessageBundle(jsonBytes(oversizedAmount)));
  }

  private static void recentMessagesRequireExactLinksAndUniqueIds() {
    final Map<String, Object> body = map();
    body.put("items", List.of(recent(9, MESSAGE_ID), recent(8, hash(0x12))));
    body.put("next", Map.of("from", 8, "after_index", 0));
    final SccpModels.RecentMessages parsed =
        SccpJsonParser.parseRecentMessages(jsonBytes(body));
    assert parsed.items.size() == 2
        && parsed.items.get(0).height.equals(BigInteger.valueOf(9));
    assert parsed.items.get(0).commitmentIndex == 0;
    assert parsed.next != null
        && parsed.next.from.equals(BigInteger.valueOf(8))
        && parsed.next.afterIndex == 0;
    assert parsed.items.get(0).routeConfigurationHash.equals(prefixed(0x72));
    assert parsed.items.get(0).payloadProjection.containsKey("Transfer");
    try {
      object(parsed.items.get(0).payloadProjection.get("Transfer")).put("version", 2);
      throw new AssertionError("payload projection must be deeply immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected: public readback maps are immutable at every level.
    }

    final Map<String, Object> sameHeightBody = map();
    sameHeightBody.put(
        "items", List.of(recent(9, MESSAGE_ID, 0), recent(9, hash(0x12), 1)));
    sameHeightBody.put("next", null);
    final SccpModels.RecentMessages sameHeight =
        SccpJsonParser.parseRecentMessages(jsonBytes(sameHeightBody));
    assert sameHeight.items.get(0).commitmentIndex == 0;
    assert sameHeight.items.get(1).commitmentIndex == 1;
    assert sameHeight.next == null;
    assert SccpJsonParser.parseRecentMessages(jsonBytes(Map.of("items", List.of()))).next
        == null;
    final BigInteger maxU64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
    final Map<String, Object> maxHeightBody = map();
    maxHeightBody.put("items", List.of(recent(maxU64, MESSAGE_ID, 511)));
    maxHeightBody.put("next", Map.of("from", maxU64, "after_index", 511));
    final SccpModels.RecentMessages maxHeight =
        SccpJsonParser.parseRecentMessages(jsonBytes(maxHeightBody));
    assert maxHeight.items.get(0).height.equals(maxU64);
    assert maxHeight.next != null && maxHeight.next.from.equals(maxU64);
    assert maxHeight.next.afterIndex == 511;

    for (final Object replacement :
        List.of(-1, 512, "0", Double.valueOf(0.0d), Boolean.TRUE)) {
      final Map<String, Object> hostile = recent(9, MESSAGE_ID);
      hostile.put("commitment_index", replacement);
      expectFailure(
          () ->
              SccpJsonParser.parseRecentMessages(
                  jsonBytes(Map.of("items", List.of(hostile)))));
    }
    final Map<String, Object> nullCommitmentIndex = recent(9, MESSAGE_ID);
    nullCommitmentIndex.put("commitment_index", null);
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(nullCommitmentIndex)))));
    final Map<String, Object> missingCommitmentIndex = recent(9, MESSAGE_ID);
    missingCommitmentIndex.remove("commitment_index");
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(missingCommitmentIndex)))));
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(
                    Map.of(
                        "items",
                        List.of(recent(BigInteger.ONE.shiftLeft(64), MESSAGE_ID))))));
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(
                    Map.of(
                        "items",
                        List.of(recent(9, MESSAGE_ID, 1), recent(9, hash(0x12), 0))))));
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(
                    Map.of(
                        "items",
                        List.of(recent(9, MESSAGE_ID, 0), recent(9, hash(0x12), 0))))));

    final List<Map<String, Object>> cursorHostiles = new ArrayList<>();
    cursorHostiles.add(new LinkedHashMap<>(Map.of("from", 9)));
    cursorHostiles.add(new LinkedHashMap<>(Map.of("after_index", 0)));
    cursorHostiles.add(new LinkedHashMap<>(Map.of("from", 0, "after_index", 0)));
    cursorHostiles.add(
        new LinkedHashMap<>(
            Map.of("from", BigInteger.ONE.shiftLeft(64), "after_index", 0)));
    cursorHostiles.add(new LinkedHashMap<>(Map.of("from", 9, "after_index", 512)));
    cursorHostiles.add(
        new LinkedHashMap<>(Map.of("from", 9, "after_index", 0, "offset", 0)));
    cursorHostiles.add(new LinkedHashMap<>(Map.of("from", 8, "after_index", 0)));
    cursorHostiles.add(new LinkedHashMap<>(Map.of("from", 9, "after_index", 1)));
    for (final Map<String, Object> cursor : cursorHostiles) {
      final Map<String, Object> hostile = map();
      hostile.put("items", List.of(recent(9, MESSAGE_ID, 0)));
      hostile.put("next", cursor);
      expectFailure(() -> SccpJsonParser.parseRecentMessages(jsonBytes(hostile)));
    }
    final Map<String, Object> cursorOnEmptyPage = map();
    cursorOnEmptyPage.put("items", List.of());
    cursorOnEmptyPage.put("next", Map.of("from", 9, "after_index", 0));
    expectFailure(
        () -> SccpJsonParser.parseRecentMessages(jsonBytes(cursorOnEmptyPage)));

    final Map<String, Object> tronProjection = recent(9, MESSAGE_ID);
    tronProjection.put("target_profile", "tron-mainnet");
    tronProjection.put("target_domain", 3);
    tronProjection.put("route_id", "taira_tron_xor");
    final Map<String, Object> tronTransfer =
        object(object(tronProjection.get("payload_projection")).get("Transfer"));
    tronTransfer.put("dest_domain", 3);
    tronTransfer.put(
        "recipient",
        Map.of("TronAddress21", Map.of("bytes", "0x41" + "11".repeat(20))));
    tronTransfer.put("route_id", canonicalProjectionText("taira_tron_xor"));
    SccpJsonParser.parseRecentMessages(jsonBytes(Map.of("items", List.of(tronProjection))));

    final Map<String, Object> missingProjection = recent(9, MESSAGE_ID);
    missingProjection.remove("payload_projection");
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(missingProjection)))));
    final Map<String, Object> nullProjection = recent(9, MESSAGE_ID);
    nullProjection.put("payload_projection", null);
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(nullProjection)))));
    final Map<String, Object> wrongProjectionRoute = recent(9, MESSAGE_ID);
    object(
            object(
                    object(wrongProjectionRoute.get("payload_projection"))
                        .get("Transfer"))
                .get("route_id"))
        .put("CanonicalText", Map.of("value", "taira_eth_xor"));
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(wrongProjectionRoute)))));
    final Map<String, Object> zeroTronRecipient = recent(9, MESSAGE_ID);
    zeroTronRecipient.put("target_profile", "tron-mainnet");
    zeroTronRecipient.put("target_domain", 3);
    zeroTronRecipient.put("route_id", "taira_tron_xor");
    final Map<String, Object> zeroTronTransfer =
        object(object(zeroTronRecipient.get("payload_projection")).get("Transfer"));
    zeroTronTransfer.put("dest_domain", 3);
    zeroTronTransfer.put(
        "recipient",
        Map.of("TronAddress21", Map.of("bytes", "0x41" + "00".repeat(20))));
    zeroTronTransfer.put("route_id", canonicalProjectionText("taira_tron_xor"));
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(zeroTronRecipient)))));

    final Map<String, Object> retiredLink = recent(9, MESSAGE_ID);
    object(retiredLink.get("links")).put("artifact_path", "/retired");
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(retiredLink)))));
    final Map<String, Object> retiredKind = recent(9, MESSAGE_ID);
    retiredKind.put("kind", "token_pause");
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(Map.of("items", List.of(retiredKind)))));
    expectFailure(
        () ->
            SccpJsonParser.parseRecentMessages(
                jsonBytes(
                    Map.of("items", List.of(recent(9, MESSAGE_ID), recent(8, MESSAGE_ID))))));
  }

  private static void detachedSigningResponseRejectsCrossFamilyLabels() throws Exception {
    final byte[] transactionBytes =
        new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
            .encodeTransaction(
                TransactionPayload.builder()
                    .setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
                    .setNetworkId(
                        org.hyperledger.iroha.android.testing.TestNetworkIds.canonical())
                    .setAuthority(AUTHORITY)
                    .setCreationTimeMs(10)
                    .setInstructions(Collections.emptyList())
                    .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
                    .build());
    final Map<String, Object> response = map();
    response.put("submitted", false);
    response.put("payload_kind", "transfer");
    response.put("message_id_hex", MESSAGE_ID);
    response.put("backend", "evm-groth16-bn254-v1");
    response.put("counterparty_domain", 2);
    response.put("counterparty_chain", "bsc-mainnet");
    response.put("route_configuration_hash_hex", hash(0x41));
    response.put("range_start_height", 9);
    response.put("range_end_height", 9);
    response.put("creation_time_ms", 10);
    response.put("tx_hash_hex", null);
    response.put("transaction_payload_b64", Base64.getEncoder().encodeToString(transactionBytes));
    response.put(
        "signing_message_b64",
        Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionBytes)));
    final SccpBridgeSubmitResponse parsed =
        SccpBridgeSubmitResponse.parse(jsonBytes(response));
    assert !parsed.submitted && parsed.payloadKind == SccpModels.PayloadKindV1.TRANSFER;
    assert parsed.routeConfigurationHashHex.equals(hash(0x41));

    response.put("submitted", true);
    response.put("tx_hash_hex", "ab".repeat(32));
    response.put("transaction_payload_b64", null);
    response.put("signing_message_b64", null);
    assert SccpBridgeSubmitResponse.parse(jsonBytes(response)).submitted;
    response.put("tx_hash_hex", "aa".repeat(32));
    expectFailure(() -> SccpBridgeSubmitResponse.parse(jsonBytes(response)));
    response.put("submitted", false);
    response.put("tx_hash_hex", null);
    response.put("transaction_payload_b64", Base64.getEncoder().encodeToString(transactionBytes));
    response.put(
        "signing_message_b64",
        Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionBytes)));

    final byte[] ordinaryTransactionBytes =
        new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
            .encodeTransaction(
                new NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
                    .decodeTransaction(transactionBytes)
                    .toBuilder()
                    .setAdmissionIntent(TransactionAdmissionIntent.ORDINARY)
                    .build());
    response.put(
        "transaction_payload_b64",
        Base64.getEncoder().encodeToString(ordinaryTransactionBytes));
    response.put(
        "signing_message_b64",
        Base64.getEncoder().encodeToString(IrohaHash.prehash(ordinaryTransactionBytes)));
    expectFailure(() -> SccpBridgeSubmitResponse.parse(jsonBytes(response)));
    response.put(
        "transaction_payload_b64", Base64.getEncoder().encodeToString(transactionBytes));
    response.put(
        "signing_message_b64",
        Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionBytes)));

    response.put("backend", "tron-groth16-bn254-v1");
    expectFailure(() -> SccpBridgeSubmitResponse.parse(jsonBytes(response)));
    response.put("backend", "bridge/sccp/outbound-v1");
    expectFailure(() -> SccpBridgeSubmitResponse.parse(jsonBytes(response)));
    response.put("backend", "evm-groth16-bn254-v1");
    response.put("payload_kind", "route_activate");
    expectFailure(() -> SccpBridgeSubmitResponse.parse(jsonBytes(response)));
    response.put("payload_kind", "transfer");
    response.put("creation_time_ms", 11);
    expectFailure(() -> SccpBridgeSubmitResponse.parse(jsonBytes(response)));
    response.put("creation_time_ms", 10);
    response.put("manifest_hash_hex", response.get("route_configuration_hash_hex"));
    expectFailure(() -> SccpBridgeSubmitResponse.parse(jsonBytes(response)));
  }

  private static Map<String, Object> capabilities() {
    final Map<String, Object> value = map();
    value.put("version", 1);
    value.put("registry_revision", prefixed(0x10));
    value.put("registry_path", "/v1/sccp/registry");
    value.put("message_bundle_path", "/v1/sccp/proofs/message/{message_id}");
    value.put("proof_request_path", "/v1/sccp/proof-requests/{message_id}");
    value.put("recent_messages_path", "/v1/sccp/messages/recent");
    value.put(
        "sora_outbound_material_path",
        "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material");
    final Map<String, Object> registryLimits = map();
    registryLimits.put("max_governed_lanes", 16);
    registryLimits.put("max_live_governed_routes", 64);
    registryLimits.put("max_live_routes_per_lane", 8);
    registryLimits.put("max_retained_routes_per_lane", 64);
    registryLimits.put("max_retained_native_trust_anchors_per_lane", 4_096);
    value.put("registry_limits", registryLimits);
    final Map<String, Object> resourceLimits = map();
    resourceLimits.put("max_outbound_messages_per_block", 512);
    resourceLimits.put("max_outbound_message_payload_bytes", 4_096);
    resourceLimits.put("max_pending_outbound_messages", 65_536);
    resourceLimits.put("max_pending_outbound_payload_bytes", 268_435_456);
    resourceLimits.put("max_proofs_per_transaction", 1);
    resourceLimits.put("max_proofs_per_block", 4);
    resourceLimits.put("max_proof_bytes_per_proof", 8 * 1024 * 1024);
    resourceLimits.put("max_proof_bytes_per_transaction", 8 * 1024 * 1024);
    resourceLimits.put("max_proof_bytes_per_block", 32 * 1024 * 1024);
    resourceLimits.put("max_native_headers_per_transaction", 1_004);
    resourceLimits.put("max_native_headers_per_block", 4_016);
    resourceLimits.put("max_ethereum_light_client_updates_per_transaction", 128);
    resourceLimits.put("max_ethereum_light_client_updates_per_block", 512);
    resourceLimits.put("max_native_header_bytes_per_transaction", 8 * 1024 * 1024);
    resourceLimits.put("max_native_header_bytes_per_block", 32 * 1024 * 1024);
    resourceLimits.put("max_secp256k1_recoveries_per_transaction", 1_005);
    resourceLimits.put("max_secp256k1_recoveries_per_block", 4_020);
    resourceLimits.put("max_bls_aggregate_checks_per_transaction", 1_004);
    resourceLimits.put("max_bls_aggregate_checks_per_block", 4_016);
    resourceLimits.put("max_bls_signer_contributions_per_transaction", 131_713);
    resourceLimits.put("max_bls_signer_contributions_per_block", 526_852);
    resourceLimits.put("max_ed25519_signature_checks_per_transaction", 65_536);
    resourceLimits.put("max_ed25519_signature_checks_per_block", 262_144);
    resourceLimits.put("max_ed25519_validator_key_checks_per_transaction", 198_656);
    resourceLimits.put("max_ed25519_validator_key_checks_per_block", 794_624);
    resourceLimits.put("max_bn254_pairing_checks_per_transaction", 1);
    resourceLimits.put("max_bn254_pairing_checks_per_block", 4);
    resourceLimits.put("max_bls12_381_pairing_checks_per_transaction", 1);
    resourceLimits.put("max_bls12_381_pairing_checks_per_block", 4);
    value.put("resource_limits", resourceLimits);
    value.put("proof_submit_path", null);
    value.put("native_message_submit_path", null);
    return value;
  }

  private static Map<String, Object> network(final String profile) {
    final Map<String, Object> value = map();
    value.put("network", profile.replace('-', '_'));
    value.put("profile", null);
    return value;
  }

  private static Map<String, Object> lane() {
    return new LinkedHashMap<>(
        Map.of("source", network("bsc-mainnet"), "target", network("sora-taira")));
  }

  private static Map<String, Object> tronLane() {
    return new LinkedHashMap<>(
        Map.of("source", network("tron-mainnet"), "target", network("sora-taira")));
  }

  private static Map<String, Object> tonLane() {
    return new LinkedHashMap<>(
        Map.of("source", network("ton-mainnet"), "target", network("sora-taira")));
  }

  private static Map<String, Object> nativeTrustAnchor(
      final int hashByte, final long checkpointHeight, final String backendName) {
    final Map<String, Object> backend = map();
    backend.put("backend", backendName);
    backend.put("protocol", null);
    final Map<String, Object> anchor = map();
    anchor.put("backend", backend);
    anchor.put("anchor_hash", upper(hashByte, 32));
    anchor.put("checkpoint_height", checkpointHeight);
    return anchor;
  }

  private static Map<String, Object> g1() {
    return new LinkedHashMap<>(Map.of("x", upper(1, 32), "y", upper(2, 32)));
  }

  private static Map<String, Object> g2() {
    return new LinkedHashMap<>(
        Map.of(
            "x_c0",
            upper(3, 32),
            "x_c1",
            upper(4, 32),
            "y_c0",
            upper(5, 32),
            "y_c1",
            upper(6, 32)));
  }

  private static Map<String, Object> verifyingKey() {
    final Map<String, Object> ic = map();
    ic.put("constant", g1());
    for (int index = 0; index <= 10; index++) ic.put("signal_" + index, g1());
    final Map<String, Object> key = map();
    key.put("version", 1);
    key.put("alpha1", g1());
    key.put("beta2", g2());
    key.put("gamma2", g2());
    key.put("delta2", g2());
    key.put("ic", ic);
    return key;
  }

  private static String keyHash(final Map<String, Object> key) {
    final List<String> words = new ArrayList<>();
    final Map<String, Object> alpha = object(key.get("alpha1"));
    words.add((String) alpha.get("x"));
    words.add((String) alpha.get("y"));
    for (final String name : List.of("beta2", "gamma2", "delta2")) {
      final Map<String, Object> point = object(key.get(name));
      for (final String field : List.of("x_c0", "x_c1", "y_c0", "y_c1")) {
        words.add((String) point.get(field));
      }
    }
    final Map<String, Object> ic = object(key.get("ic"));
    final List<String> names = new ArrayList<>();
    names.add("constant");
    for (int index = 0; index <= 10; index++) names.add("signal_" + index);
    for (final String name : names) {
      final Map<String, Object> point = object(ic.get(name));
      words.add((String) point.get("x"));
      words.add((String) point.get("y"));
    }
    return upperHex(keccak(hexBytes(String.join("", words))));
  }

  private static Map<String, Object> semanticProfile() {
    final Map<String, Object> commitments = map();
    commitments.put("version", 1);
    commitments.put("circuit_commitment", upper(0x11, 32));
    commitments.put("witness_generator_commitment", upper(0x12, 32));
    commitments.put("public_signal_schema_hash", publicSignalSchemaHash());
    return new LinkedHashMap<>(
        Map.of(
            "profile", "sora_taira_finality_inclusion_groth16_bn254",
            "commitments", commitments));
  }

  private static Map<String, Object> finalityAnchor() {
    final Map<String, Object> anchor = map();
    anchor.put("version", 1);
    anchor.put("source_network", network("sora-taira"));
    anchor.put("protocol_version", 4);
    anchor.put("chain_id_hash", tairaChainIdHash());
    anchor.put("checkpoint_height", 7);
    anchor.put("checkpoint_block_hash", upper(0xa1, 32));
    anchor.put("checkpoint_context_id", upper(0xa2, 32));
    anchor.put("checkpoint_finality_artifact_hash", upper(0xa3, 32));
    return anchor;
  }

  private static Map<String, Object> outboundPolicy() {
    final Map<String, Object> policy = map();
    policy.put("version", 1);
    policy.put("semantic_profile", semanticProfile());
    policy.put("sora_finality_anchor", finalityAnchor());
    return policy;
  }

  private static Map<String, Object> soraOutboundExecutionPolicy() {
    final Map<String, Object> reference = map();
    reference.put("backend", "stark/fri/v1");
    reference.put("name", "ivm-execution-v1");
    reference.put("version", 1);
    reference.put("commitment", upper(0xb2, 32));
    final Map<String, Object> policy = map();
    policy.put("version", 1);
    policy.put("semantics", "ivm_proved_record_sccp_message_v1");
    policy.put("contract_artifact_sha256", upper(0xb1, 32));
    policy.put("vk_ref", reference);
    policy.put("gas_limit", 50_000_000);
    return policy;
  }

  private static Map<String, Object> bls12381VerifyingKey() {
    final String g1 = "80" + "0".repeat(94);
    final String g2 = "80" + "0".repeat(190);
    final Map<String, Object> ic = map();
    ic.put("constant", g1);
    for (int index = 0; index <= 10; index++) ic.put("signal_" + index, g1);
    final Map<String, Object> key = map();
    key.put("version", 1);
    key.put("alpha1", g1);
    key.put("beta2", g2);
    key.put("gamma2", g2);
    key.put("delta2", g2);
    key.put("ic", ic);
    return key;
  }

  private static byte[] bls12381VerifyingKeyBytes(final Map<String, Object> key) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeRaw(out, hexBytes((String) key.get("alpha1")));
    writeRaw(out, hexBytes((String) key.get("beta2")));
    writeRaw(out, hexBytes((String) key.get("gamma2")));
    writeRaw(out, hexBytes((String) key.get("delta2")));
    final Map<String, Object> ic = object(key.get("ic"));
    writeRaw(out, hexBytes((String) ic.get("constant")));
    for (int index = 0; index <= 10; index++) {
      writeRaw(out, hexBytes((String) ic.get("signal_" + index)));
    }
    return out.toByteArray();
  }

  private static List<String> bls12381SignalLabels() {
    return List.of(
        "sccp:groth16-bls12381:signal:message-id:v1",
        "sccp:groth16-bls12381:signal:payload-hash:v1",
        "sccp:groth16-bls12381:signal:target-domain:v1",
        "sccp:groth16-bls12381:signal:commitment-root:v1",
        "sccp:groth16-bls12381:signal:finality-height:v1",
        "sccp:groth16-bls12381:signal:finality-block-hash:v1",
        "sccp:groth16-bls12381:signal:source-domain:v1",
        "sccp:groth16-bls12381:signal:statement-hash:v1",
        "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
        "sccp:groth16-bls12381:signal:route-config-hash:v1",
        "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1");
  }

  private static byte[] bls12381PublicSignalSchemaHash() {
    final ByteArrayOutputStream canonical = new ByteArrayOutputStream();
    canonical.write(1);
    writeU32(canonical, bls12381SignalLabels().size());
    for (final String label : bls12381SignalLabels()) {
      writeVector(canonical, label.getBytes(StandardCharsets.UTF_8));
    }
    return sha256(
        concatenate(
            "sccp:groth16-bls12381:public-signal-schema:v1"
                .getBytes(StandardCharsets.UTF_8),
            canonical.toByteArray()));
  }

  private static Map<String, Object> tonOutboundPolicy() {
    final Map<String, Object> commitments = map();
    commitments.put("version", 1);
    commitments.put("circuit_commitment", upper(0xc1, 32));
    commitments.put("witness_generator_commitment", upper(0xc2, 32));
    commitments.put("public_signal_schema_hash", upperHex(bls12381PublicSignalSchemaHash()));
    final Map<String, Object> semantic = map();
    semantic.put("profile", "sora_taira_finality_inclusion_groth16_bls12381");
    semantic.put("commitments", commitments);
    final Map<String, Object> policy = map();
    policy.put("version", 1);
    policy.put("semantic_profile", semantic);
    policy.put("sora_finality_anchor", finalityAnchor());
    return policy;
  }

  private static byte[] tonProofProfileCommitment() {
    return sha256(
        concatenate(
            "sccp:ton:groth16-bls12381:proof-profile:v1"
                .getBytes(StandardCharsets.UTF_8),
            new byte[] {1},
            "ietf-bls12381-compressed-g1-48-g2-96".getBytes(StandardCharsets.US_ASCII),
            "groth16-a-g1-b-g2-c-g1".getBytes(StandardCharsets.US_ASCII),
            "sha256-sha256-label-value-mod-r".getBytes(StandardCharsets.US_ASCII),
            hexBytes("73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001"),
            bls12381PublicSignalSchemaHash()));
  }

  private static Map<String, Object> tonAddress(final int value) {
    final Map<String, Object> address = map();
    address.put("workchain", 0);
    address.put("account", upper(value, 32));
    return address;
  }

  private static Map<String, Object> tonRegistry() {
    final Map<String, Object> key = bls12381VerifyingKey();
    final byte[] keyHash = sha256(bls12381VerifyingKeyBytes(key));
    final Map<String, Object> policy = tonOutboundPolicy();
    final Map<String, Object> semantic = object(policy.get("semantic_profile"));
    final Map<String, Object> anchor = object(policy.get("sora_finality_anchor"));
    final byte[] semanticHash = hexBytes(semanticProfileHash(semantic));
    final byte[] anchorHash = hexBytes(finalityAnchorHash(anchor));
    final Map<String, Object> master = tonAddress(0x21);
    final Map<String, Object> routeAddress = tonAddress(0x23);
    final byte[] masterCode = fill(32, 0x31);
    final byte[] masterInitialData = fill(32, 0x36);
    final byte[] walletCode = fill(32, 0x32);
    final byte[] routeCode = fill(32, 0x34);
    final byte[] routeInitialData = fill(32, 0x37);
    final byte[] verifierCode = fill(32, 0x35);
    final byte[] circuit = hexBytes((String) object(semantic.get("commitments")).get("circuit_commitment"));
    final byte[] proofProfile = tonProofProfileCommitment();
    final List<byte[]> guardians =
        List.of(fill(32, 1), fill(32, 2), fill(32, 3), fill(32, 4), fill(32, 5));
    final BigInteger maxWrappedSupply = BigInteger.valueOf(9_000_000_000L);
    final byte[] binding =
        tonDestinationBindingHash(
            masterCode,
            walletCode,
            routeCode,
            verifierCode,
            circuit,
            keyHash,
            proofProfile,
            guardians,
            semanticHash,
            anchorHash);
    final byte[] configuration =
        tonRouteConfigurationHash(
            masterCode,
            walletCode,
            routeCode,
            verifierCode,
            circuit,
            keyHash,
            proofProfile,
            guardians,
            semanticHash,
            anchorHash,
            binding,
            1,
            maxWrappedSupply);

    final Map<String, Object> identity = map();
    identity.put("address", routeAddress);
    identity.put("code_hash", upperHex(routeCode));
    identity.put("route_config_hash", upperHex(configuration));
    final Map<String, Object> emitter = map();
    emitter.put("emitter", "ton");
    emitter.put("identity", identity);
    final Map<String, Object> source = map();
    source.put("lane", tonLane());
    source.put("emitter", emitter);

    final Map<String, Object> deployment = map();
    deployment.put("jetton_master_address", master);
    deployment.put("jetton_master_code_hash", upperHex(masterCode));
    deployment.put("jetton_master_initial_data_hash", upperHex(masterInitialData));
    deployment.put("jetton_wallet_code_hash", upperHex(walletCode));
    deployment.put("route_address", routeAddress);
    deployment.put("route_code_hash", upperHex(routeCode));
    deployment.put("route_initial_data_hash", upperHex(routeInitialData));
    deployment.put("embedded_verifier_code_hash", upperHex(verifierCode));
    deployment.put("verifier_circuit_hash", upperHex(circuit));
    deployment.put("verifying_key", key);
    deployment.put("verifier_key_hash", upperHex(keyHash));
    deployment.put("proof_profile_commitment", upperHex(proofProfile));
    final Map<String, Object> guardianObject = map();
    for (int index = 0; index < guardians.size(); index++) {
      guardianObject.put("guardian_" + index, upperHex(guardians.get(index)));
    }
    deployment.put("mint_breaker_guardian_keys", guardianObject);
    deployment.put("outbound_proof_policy", policy);
    deployment.put("taira_to_token_multiplier", 1);
    deployment.put("max_wrapped_supply", maxWrappedSupply);
    final Map<String, Object> destination = map();
    destination.put("family", "ton");
    destination.put("deployment", deployment);

    final Map<String, Object> settlement = map();
    settlement.put("asset_definition_id", "6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    settlement.put("payload_amount_scale", 9);
    settlement.put("max_outstanding_liability", maxWrappedSupply);
    final Map<String, Object> activation = map();
    activation.put("activation", "staged");
    activation.put("direction", null);
    final Map<String, Object> route = map();
    route.put("lane_id", tonLane());
    route.put("route_id", "taira_ton_xor");
    route.put("asset_key", "xor");
    route.put("revision", 1);
    route.put("activation", activation);
    route.put("inbound_finality_cutoff", null);
    route.put("source_identity", source);
    route.put("destination", destination);
    route.put("sora_outbound_execution_policy", soraOutboundExecutionPolicy());
    route.put("settlement", settlement);
    final Map<String, Object> laneRecord = map();
    laneRecord.put("lane_id", tonLane());
    laneRecord.put("native_trust_anchors", new ArrayList<>());
    laneRecord.put("current_native_trust_anchor_hash", null);
    laneRecord.put("routes", new ArrayList<>(List.of(route)));
    final Map<String, Object> root = map();
    root.put("version", 1);
    root.put("lanes", new ArrayList<>(List.of(laneRecord)));
    return root;
  }

  private static byte[] tonDestinationBindingHash(
      final byte[] masterCode,
      final byte[] walletCode,
      final byte[] routeCode,
      final byte[] verifierCode,
      final byte[] circuit,
      final byte[] keyHash,
      final byte[] proofProfile,
      final List<byte[]> guardians,
      final byte[] semanticHash,
      final byte[] anchorHash) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeRaw(out, "iroha:sccp:ton-destination-binding:v1".getBytes(StandardCharsets.UTF_8));
    out.write(1);
    writeVector(out, "ton-groth16-bls12381-v1".getBytes(StandardCharsets.US_ASCII));
    writeVector(out, SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_MAINNET));
    writeI32(out, -239);
    writeU32(out, 0);
    writeU32(out, 4);
    for (final byte[] role :
        List.of(
            masterCode,
            walletCode,
            routeCode,
            verifierCode,
            circuit,
            keyHash,
            proofProfile)) {
      writeRaw(out, role);
    }
    for (final byte[] guardian : guardians) writeRaw(out, guardian);
    writeRaw(out, semanticHash);
    writeRaw(out, anchorHash);
    return sha256(out.toByteArray());
  }

  private static byte[] tonRouteConfigurationHash(
      final byte[] masterCode,
      final byte[] walletCode,
      final byte[] routeCode,
      final byte[] verifierCode,
      final byte[] circuit,
      final byte[] keyHash,
      final byte[] proofProfile,
      final List<byte[]> guardians,
      final byte[] semanticHash,
      final byte[] anchorHash,
      final byte[] binding,
      final int revision,
      final BigInteger maxWrappedSupply) {
    final ByteArrayOutputStream deployment = new ByteArrayOutputStream();
    for (final byte[] role : List.of(masterCode, walletCode)) writeRaw(deployment, role);
    for (final byte[] role :
        List.of(
            routeCode,
            verifierCode,
            circuit,
            keyHash,
            proofProfile)) {
      writeRaw(deployment, role);
    }
    for (final byte[] guardian : guardians) writeRaw(deployment, guardian);
    writeRaw(deployment, semanticHash);
    writeRaw(deployment, anchorHash);
    writeRaw(deployment, binding);
    final ByteArrayOutputStream assetRoute = new ByteArrayOutputStream();
    writeVector(assetRoute, "xor".getBytes(StandardCharsets.US_ASCII));
    writeVector(assetRoute, "taira_ton_xor".getBytes(StandardCharsets.US_ASCII));
    writeU32(assetRoute, revision);
    writeU64(assetRoute, 1);
    writeU128(assetRoute, maxWrappedSupply);
    final SccpLaneIdV1 inbound =
        new SccpLaneIdV1(SccpNetworkV1.TON_MAINNET, SccpNetworkV1.SORA_TAIRA);
    final SccpLaneIdV1 outbound =
        new SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.TON_MAINNET);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeRaw(out, "sccp:concrete-route-config:v1".getBytes(StandardCharsets.UTF_8));
    out.write(1);
    writeU32(out, 4);
    writeVector(out, SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_MAINNET));
    writeI32(out, -239);
    writeRaw(out, SccpV1.laneHash(inbound));
    writeRaw(out, SccpV1.laneHash(outbound));
    writeRaw(out, sha256(deployment.toByteArray()));
    writeRaw(out, sha256(assetRoute.toByteArray()));
    return sha256(out.toByteArray());
  }

  private static Map<String, Object> tonDeployment(final Map<String, Object> registry) {
    return deployment(registry);
  }

  private static Map<String, Object> tonSourceIdentity(final Map<String, Object> registry) {
    return sourceIdentity(firstRoute(registry));
  }

  private static Map<String, Object> tonProofRequest() {
    final Map<String, Object> key = bls12381VerifyingKey();
    final byte[] keyHash = sha256(bls12381VerifyingKeyBytes(key));
    final Map<String, Object> policy = tonOutboundPolicy();
    final Map<String, Object> semantic = object(policy.get("semantic_profile"));
    final Map<String, Object> anchor = object(policy.get("sora_finality_anchor"));
    final String semanticHash = semanticProfileHash(semantic);
    final String anchorHash = finalityAnchorHash(anchor);
    final String messageId = prefixed(0x41);
    final String payloadHash = prefixed(0x42);
    final String commitmentRoot = prefixed(0x43);
    final String finalityBlockHash = prefixed(0x44);
    final String statementHash = prefixed(0x45);
    final String destinationBindingHash = prefixed(0x46);
    final String routeConfigurationHash = prefixed(0x47);
    final Map<String, Object> inputs = map();
    inputs.put("version", 1);
    inputs.put("message_id", messageId);
    inputs.put("payload_hash", payloadHash);
    inputs.put("target_domain", 4);
    inputs.put("commitment_root", commitmentRoot);
    inputs.put("finality_height", "9");
    inputs.put("finality_block_hash", finalityBlockHash);

    final List<byte[]> signalInputs =
        List.of(
            prefixedBytes(messageId),
            prefixedBytes(payloadHash),
            abiWord(4),
            prefixedBytes(commitmentRoot),
            abiWord(9),
            prefixedBytes(finalityBlockHash),
            abiWord(0),
            prefixedBytes(statementHash),
            prefixedBytes(destinationBindingHash),
            prefixedBytes(routeConfigurationHash),
            hexBytes(anchorHash));
    final Map<String, Object> signals = map();
    final List<String> signalFields =
        List.of(
            "message_id",
            "payload_hash",
            "target_domain",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "source_domain",
            "statement_hash",
            "destination_binding_hash",
            "route_configuration_hash",
            "sora_finality_anchor_hash");
    for (int index = 0; index < signalFields.size(); index++) {
      signals.put(
          signalFields.get(index),
          "0x"
              + upperHex(
                      bls12381SignalWord(
                          bls12381SignalLabels().get(index), signalInputs.get(index)))
                  .toLowerCase());
    }

    final Map<String, Object> backend = map();
    backend.put("backend", "ton_groth16_bls12381_v1");
    backend.put("family", null);
    final Map<String, Object> root = map();
    root.put("version", 1);
    root.put("backend", backend);
    root.put("source_network", network("sora-taira"));
    root.put("target_network", network("ton-mainnet"));
    root.put("public_inputs", inputs);
    root.put("public_signals", signals);
    root.put("verifying_key", key);
    root.put("verifier_key_hash", "0x" + upperHex(keyHash).toLowerCase());
    root.put("verifier_circuit_hash", "0x" + upper(0xc1, 32).toLowerCase());
    root.put(
        "proof_profile_commitment",
        "0x" + upperHex(tonProofProfileCommitment()).toLowerCase());
    root.put("semantic_proof_profile", semantic);
    root.put("semantic_proof_profile_hash", "0x" + semanticHash.toLowerCase());
    root.put("sora_finality_anchor", anchor);
    root.put("sora_finality_anchor_hash", "0x" + anchorHash.toLowerCase());
    root.put("bundle_bytes", "0x0102");
    root.put("statement_hash", statementHash);
    root.put("destination_binding_hash", destinationBindingHash);
    root.put("route_configuration_hash", routeConfigurationHash);
    root.put("request_hash", prefixed(0x48));
    return root;
  }

  private static byte[] bls12381SignalWord(final String label, final byte[] value) {
    final BigInteger modulus =
        new BigInteger(
            "73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001",
            16);
    final BigInteger scalar =
        new BigInteger(
                1,
                sha256(
                    concatenate(
                        sha256(label.getBytes(StandardCharsets.UTF_8)), value)))
            .mod(modulus);
    return fixedUnsigned(scalar, 32);
  }

  private static byte[] fixedUnsigned(final BigInteger value, final int size) {
    byte[] encoded = value.toByteArray();
    if (encoded.length == size + 1 && encoded[0] == 0) {
      encoded = Arrays.copyOfRange(encoded, 1, encoded.length);
    }
    if (encoded.length > size) throw new IllegalArgumentException("unsigned integer is too wide");
    final byte[] result = new byte[size];
    System.arraycopy(encoded, 0, result, size - encoded.length, encoded.length);
    return result;
  }

  private static byte[] prefixedBytes(final String value) {
    return hexBytes(value.substring(2));
  }

  private static Map<String, Object> registry() {
    final Map<String, Object> key = verifyingKey();
    final String routeAddress = upper(0x31, 20);
    final String routeCodeHash = upper(0x21, 32);
    final Map<String, Object> identity = map();
    identity.put("address", routeAddress);
    identity.put("runtime_code_hash", routeCodeHash);
    identity.put("route_config_hash", upper(0x22, 32));
    final Map<String, Object> emitter = map();
    emitter.put("emitter", "evm");
    emitter.put("identity", identity);
    final Map<String, Object> source = map();
    source.put("lane", lane());
    source.put("emitter", emitter);
    final Map<String, Object> deployment = map();
    deployment.put("token_address", upper(0x11, 20));
    deployment.put("token_code_hash", upper(0x23, 32));
    deployment.put("verifier_address", upper(0x12, 20));
    deployment.put("verifier_code_hash", upper(0x24, 32));
    deployment.put("verifying_key", key);
    deployment.put("verifier_key_hash", keyHash(key));
    deployment.put("outbound_proof_policy", outboundPolicy());
    deployment.put("route_address", routeAddress);
    deployment.put("route_code_hash", routeCodeHash);
    deployment.put("replay_verifier_address", upper(0x71, 20));
    deployment.put("replay_verifier_code_hash", upper(0x42, 32));
    deployment.put("mint_breaker_address", upper(0x72, 20));
    deployment.put("mint_breaker_code_hash", upper(0x43, 32));
    deployment.put("taira_to_token_multiplier", 1_000_000_000);
    deployment.put("max_wrapped_supply", 9_000_000_000L);
    final Map<String, Object> destination = map();
    destination.put("family", "evm");
    destination.put("deployment", deployment);
    final Map<String, Object> settlement = map();
    settlement.put("asset_definition_id", "6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    settlement.put("payload_amount_scale", 9);
    settlement.put("max_outstanding_liability", 9);
    final Map<String, Object> route = map();
    route.put("lane_id", lane());
    route.put("route_id", "taira_bsc_xor");
    route.put("asset_key", "xor");
    route.put("revision", 1);
    final Map<String, Object> activation = map();
    activation.put("activation", "staged");
    activation.put("direction", null);
    route.put("activation", activation);
    route.put("inbound_finality_cutoff", null);
    route.put("source_identity", source);
    route.put("destination", destination);
    route.put("sora_outbound_execution_policy", soraOutboundExecutionPolicy());
    route.put("settlement", settlement);
    refreshRouteConfiguration(route);
    final Map<String, Object> laneRecord = map();
    laneRecord.put("lane_id", lane());
    laneRecord.put("native_trust_anchors", new ArrayList<>());
    laneRecord.put("current_native_trust_anchor_hash", null);
    laneRecord.put("routes", new ArrayList<>(List.of(route)));
    final Map<String, Object> root = map();
    root.put("version", 1);
    root.put("lanes", new ArrayList<>(List.of(laneRecord)));
    return root;
  }

  private static Map<String, Object> registryWithRouteHistory(
      final int retiredCount, final int liveCount) {
    final Map<String, Object> result = registry();
    firstLane(result)
        .put(
            "native_trust_anchors",
            new ArrayList<>(
                List.of(
                    nativeTrustAnchor(0x61, 7, "bsc_parlia_v1"),
                    nativeTrustAnchor(0x62, 8, "bsc_parlia_v1"))));
    firstLane(result).put("current_native_trust_anchor_hash", upper(0x62, 32));
    final List<Object> routes = list(firstLane(result), "routes");
    routes.clear();
    final int total = retiredCount + liveCount;
    for (int revision = 1; revision <= total; revision++) {
      final Map<String, Object> route = firstRoute(registry());
      route.put("revision", revision);
      activation(route).put("activation", revision <= retiredCount ? "retired" : "staged");
      route.put(
          "inbound_finality_cutoff",
          revision <= retiredCount ? inboundFinalityCutoff() : null);
      final String routeAddress = upper(0x31 + revision, 20);
      routeDeployment(route).put("route_address", routeAddress);
      sourceIdentity(route).put("address", routeAddress);
      refreshRouteConfiguration(route);
      routes.add(route);
    }
    return result;
  }

  private static Map<String, Object> retiredRegistry() {
    final Map<String, Object> result = registry();
    firstLane(result)
        .put(
            "native_trust_anchors",
            new ArrayList<>(
                List.of(
                    nativeTrustAnchor(0x61, 7, "bsc_parlia_v1"),
                    nativeTrustAnchor(0x62, 8, "bsc_parlia_v1"))));
    firstLane(result).put("current_native_trust_anchor_hash", upper(0x62, 32));
    activation(firstRoute(result)).put("activation", "retired");
    firstRoute(result).put("inbound_finality_cutoff", inboundFinalityCutoff());
    return result;
  }

  private static Map<String, Object> inboundFinalityCutoff() {
    final Map<String, Object> cutoff = map();
    cutoff.put("trust_anchor_hash", upper(0x61, 32));
    cutoff.put("max_anchor_interval_height", 8);
    return cutoff;
  }

  private static Map<String, Object> tronRegistry() {
    final Map<String, Object> result = registry();
    final Map<String, Object> route = firstRoute(result);
    firstLane(result).put("lane_id", tronLane());
    route.put("lane_id", tronLane());
    route.put("route_id", "taira_tron_xor");
    final Map<String, Object> source = object(route.get("source_identity"));
    source.put("lane", tronLane());
    object(source.get("emitter")).put("emitter", "tron");
    object(route.get("destination")).put("family", "tron");
    refreshRouteConfiguration(route);
    return result;
  }

  private static Map<String, Object> deployment(final Map<String, Object> registry) {
    final Map<String, Object> lane = object(list(registry, "lanes").get(0));
    final Map<String, Object> route = object(list(lane, "routes").get(0));
    return object(object(route.get("destination")).get("deployment"));
  }

  private static Map<String, Object> routeDeployment(final Map<String, Object> route) {
    return object(object(route.get("destination")).get("deployment"));
  }

  private static Map<String, Object> activation(final Map<String, Object> route) {
    return object(route.get("activation"));
  }

  private static Map<String, Object> firstLane(final Map<String, Object> registry) {
    return object(list(registry, "lanes").get(0));
  }

  private static Map<String, Object> firstRoute(final Map<String, Object> registry) {
    return object(list(firstLane(registry), "routes").get(0));
  }

  private static Map<String, Object> sourceIdentity(final Map<String, Object> route) {
    return object(object(object(route.get("source_identity")).get("emitter")).get("identity"));
  }

  private static void refreshRouteConfiguration(final Map<String, Object> route) {
    sourceIdentity(route).put("route_config_hash", routeConfigurationHash(route));
  }

  private static String routeConfigurationHash(final Map<String, Object> route) {
    final Map<String, Object> laneValue = object(route.get("lane_id"));
    final SccpNetworkV1 sourceNetwork =
        parsedNetwork(object(laneValue.get("source")));
    final SccpNetworkV1 targetNetwork =
        parsedNetwork(object(laneValue.get("target")));
    final SccpLaneIdV1 inboundLane = new SccpLaneIdV1(sourceNetwork, targetNetwork);
    final Map<String, Object> destination = object(route.get("destination"));
    final String family = (String) destination.get("family");
    final boolean tron = "tron".equals(family);
    final Map<String, Object> deployment = object(destination.get("deployment"));
    final Map<String, Object> policy = object(deployment.get("outbound_proof_policy"));
    final byte[] semanticHash =
        hexBytes(semanticProfileHash(object(policy.get("semantic_profile"))));
    final byte[] anchorHash =
        hexBytes(finalityAnchorHash(object(policy.get("sora_finality_anchor"))));
    final long chainOrNetworkId = chainOrNetworkId(sourceNetwork);
    final byte[] verifierAddress = hexBytes((String) deployment.get("verifier_address"));
    final byte[] routeAddress = hexBytes((String) deployment.get("route_address"));
    final byte[] verifierCodeHash = hexBytes((String) deployment.get("verifier_code_hash"));
    final byte[] verifierKeyHash = hexBytes((String) deployment.get("verifier_key_hash"));
    final byte[] replayVerifierAddress =
        hexBytes((String) deployment.get("replay_verifier_address"));
    final byte[] replayVerifierCodeHash =
        hexBytes((String) deployment.get("replay_verifier_code_hash"));
    final byte[] mintBreakerAddress =
        hexBytes((String) deployment.get("mint_breaker_address"));
    final byte[] mintBreakerCodeHash =
        hexBytes((String) deployment.get("mint_breaker_code_hash"));
    final byte[] destinationBinding =
        keccak(
            concatenate(
                keccak(
                    (tron
                            ? "iroha:sccp:tron-destination-binding:v1"
                            : "iroha:sccp:evm-destination-binding:v1")
                        .getBytes(StandardCharsets.UTF_8)),
                keccak(
                    (tron ? "tron-groth16-bn254-v1" : "evm-groth16-bn254-v1")
                        .getBytes(StandardCharsets.UTF_8)),
                abiWord(chainOrNetworkId),
                abiWord(0),
                abiWord(sourceNetwork.domainId()),
                abiAddress(verifierAddress, tron),
                abiAddress(routeAddress, tron),
                verifierCodeHash,
                verifierKeyHash,
                semanticHash,
                anchorHash,
                abiAddress(replayVerifierAddress, tron),
                replayVerifierCodeHash,
                abiAddress(mintBreakerAddress, tron),
                mintBreakerCodeHash));

    final byte[] sourceLaneHash = SccpV1.laneHash(inboundLane);
    final byte[] destinationLaneHash =
        SccpV1.laneHash(new SccpLaneIdV1(targetNetwork, sourceNetwork));
    final List<byte[]> deploymentWords = new ArrayList<>();
    deploymentWords.add(abiAddress(hexBytes((String) deployment.get("token_address")), false));
    deploymentWords.add(hexBytes((String) deployment.get("token_code_hash")));
    deploymentWords.add(abiAddress(verifierAddress, false));
    deploymentWords.add(verifierCodeHash);
    deploymentWords.add(verifierKeyHash);
    deploymentWords.add(semanticHash);
    deploymentWords.add(anchorHash);
    if (tron) deploymentWords.add(destinationBinding);
    deploymentWords.add(abiAddress(replayVerifierAddress, false));
    deploymentWords.add(replayVerifierCodeHash);
    deploymentWords.add(abiAddress(mintBreakerAddress, false));
    deploymentWords.add(mintBreakerCodeHash);
    final byte[] deploymentHash =
        keccak(concatenate(deploymentWords.toArray(new byte[0][])));
    final byte[] assetRouteHash =
        keccak(
            concatenate(
                keccak("xor".getBytes(StandardCharsets.UTF_8)),
                keccak(((String) route.get("route_id")).getBytes(StandardCharsets.UTF_8)),
                abiWord(((Number) route.get("revision")).longValue()),
                abiWord(((Number) deployment.get("taira_to_token_multiplier")).longValue()),
                abiWord(((Number) deployment.get("max_wrapped_supply")).longValue())));
    return upperHex(
        keccak(
            concatenate(
                keccak("sccp:concrete-route-config:v1".getBytes(StandardCharsets.UTF_8)),
                abiWord(sourceNetwork.domainId()),
                abiWord(sourceNetwork.tag()),
                abiWord(chainOrNetworkId),
                sourceLaneHash,
                destinationLaneHash,
                deploymentHash,
                assetRouteHash)));
  }

  private static SccpNetworkV1 parsedNetwork(final Map<String, Object> value) {
    return SccpNetworkV1.fromProfileKey(
        ((String) value.get("network")).replace('_', '-'));
  }

  private static long chainOrNetworkId(final SccpNetworkV1 network) {
    return switch (network) {
      case ETHEREUM_MAINNET -> 1;
      case BSC_MAINNET -> 56;
      case TRON_MAINNET -> 0x2b66_53dcL;
      case TON_MAINNET, SORA_TAIRA ->
          throw new IllegalArgumentException("expected EVM or TRON network");
    };
  }

  private static byte[] abiWord(final long value) {
    if (value < 0) throw new IllegalArgumentException("expected unsigned ABI integer");
    final byte[] result = new byte[32];
    for (int index = 0; index < 8; index++) {
      result[result.length - 1 - index] = (byte) ((value >>> (index * 8)) & 0xff);
    }
    return result;
  }

  private static byte[] abiAddress(final byte[] address, final boolean tron) {
    final byte[] result = new byte[32];
    if (tron) result[11] = 0x41;
    System.arraycopy(address, 0, result, 12, address.length);
    return result;
  }

  private static byte[] concatenate(final byte[]... values) {
    int length = 0;
    for (final byte[] value : values) length += value.length;
    final byte[] result = new byte[length];
    int offset = 0;
    for (final byte[] value : values) {
      System.arraycopy(value, 0, result, offset, value.length);
      offset += value.length;
    }
    return result;
  }

  private static Map<String, Object> transferPayload() {
    final Map<String, Object> transfer = map();
    transfer.put("version", 1);
    transfer.put("source_domain", 0);
    transfer.put("dest_domain", 2);
    transfer.put("nonce", "7");
    transfer.put("route_revision", 1);
    transfer.put("asset_home_domain", 0);
    transfer.put("asset_id_codec", 0);
    transfer.put("asset_id", "0x786f72");
    transfer.put("amount", "1000");
    transfer.put("sender_codec", 0);
    transfer.put("sender", "0x616c696365407461697261");
    transfer.put("recipient_codec", 1);
    transfer.put("recipient", "0x" + hash(0x11).substring(0, 40));
    transfer.put("route_id_codec", 0);
    transfer.put("route_id", "0x74616972615f6273635f786f72");
    return transfer;
  }

  private static Map<String, Object> messageBundle() {
    final Map<String, Object> context = map();
    context.put(
        "lane",
        Map.of("source", network("sora-taira"), "target", network("bsc-mainnet")));
    context.put("destination_binding_hash", prefixed(0x52));
    context.put("route_configuration_hash", prefixed(0x53));
    final Map<String, Object> commitment = map();
    commitment.put("version", 1);
    commitment.put("kind", "Transfer");
    commitment.put("context", context);
    commitment.put("message_id", "0x" + MESSAGE_ID);
    commitment.put("payload_hash", prefixed(0x54));
    final Map<String, Object> root = map();
    root.put("version", 1);
    root.put("commitment_root", prefixed(0x51));
    root.put("commitment", commitment);
    root.put("merkle_proof", Map.of("steps", List.of()));
    root.put("payload", Map.of("Transfer", transferPayload()));
    root.put("finality_proof", "0x01");
    return root;
  }

  private static Map<String, Object> proofRequest() {
    final Map<String, Object> key = verifyingKey();
    final Map<String, Object> inputs = map();
    inputs.put("version", 1);
    inputs.put("message_id", "0x" + MESSAGE_ID);
    inputs.put("payload_hash", prefixed(0x51));
    inputs.put("target_domain", 2);
    inputs.put("commitment_root", prefixed(0x52));
    inputs.put("finality_height", "9");
    inputs.put("finality_block_hash", prefixed(0x53));
    final Map<String, Object> root = map();
    final Map<String, Object> backend = map();
    backend.put("backend", "evm_groth16_bn254_v1");
    backend.put("family", null);
    root.put("version", 1);
    root.put("backend", backend);
    root.put("source_network", network("sora-taira"));
    root.put("target_network", network("bsc-mainnet"));
    root.put("public_inputs", inputs);
    root.put("verifying_key", key);
    root.put("verifier_key_hash", "0x" + keyHash(key).toLowerCase());
    root.put("semantic_proof_profile", semanticProfile());
    root.put("semantic_proof_profile_hash", "0x" + semanticProfileHash().toLowerCase());
    root.put("sora_finality_anchor", finalityAnchor());
    root.put("sora_finality_anchor_hash", "0x" + finalityAnchorHash().toLowerCase());
    root.put("bundle_bytes", "0x0102");
    root.put("statement_hash", prefixed(0x63));
    root.put("destination_binding_hash", prefixed(0x64));
    root.put("route_configuration_hash", prefixed(0x65));
    root.put("request_hash", prefixed(0x66));
    return root;
  }

  private static Map<String, Object> recent(final Number height, final String id) {
    return recent(height, id, 0);
  }

  private static Map<String, Object> recent(
      final Number height, final String id, final int commitmentIndex) {
    final Map<String, Object> links = map();
    links.put("bundle_path", "/v1/sccp/proofs/message/" + id);
    links.put("proof_request_path", "/v1/sccp/proof-requests/" + id);
    final Map<String, Object> value = map();
    value.put("height", height);
    value.put("commitment_index", commitmentIndex);
    value.put("message_id_hex", id);
    value.put("kind", "transfer");
    value.put("source_profile", "sora-taira");
    value.put("target_profile", "bsc-mainnet");
    value.put("destination_binding_hash", prefixed(0x71));
    value.put("route_configuration_hash", prefixed(0x72));
    value.put("target_domain", 2);
    value.put("asset_id", "xor");
    value.put("route_id", "taira_bsc_xor");
    value.put("recipient", null);
    value.put("amount", "1000");
    value.put("payload_projection", payloadProjection());
    value.put("links", links);
    return value;
  }

  private static Map<String, Object> payloadProjection() {
    final Map<String, Object> transfer = map();
    transfer.put("version", 1);
    transfer.put("source_domain", 0);
    transfer.put("dest_domain", 2);
    transfer.put("nonce", 9);
    transfer.put("route_revision", 1);
    transfer.put("asset_home_domain", 0);
    transfer.put("asset_id", canonicalProjectionText("xor"));
    transfer.put("amount", 1000);
    transfer.put("sender", canonicalProjectionText("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"));
    transfer.put("recipient", Map.of("EvmAddress20", Map.of("bytes", "0x" + "11".repeat(20))));
    transfer.put("route_id", canonicalProjectionText("taira_bsc_xor"));
    return new LinkedHashMap<>(Map.of("Transfer", transfer));
  }

  private static Map<String, Object> canonicalProjectionText(final String value) {
    return new LinkedHashMap<>(
        Map.of("CanonicalText", new LinkedHashMap<>(Map.of("value", value))));
  }

  private static String canonicalArtifact() {
    return Base64.getEncoder()
        .encodeToString(
            canonicalArtifactBytes(SccpSubmitEncoding.DESTINATION_ARTIFACT_SCHEMA_NAME, 0));
  }

  private static String canonicalNativeArtifact() {
    return Base64.getEncoder()
        .encodeToString(
            canonicalArtifactBytes(SccpSubmitEncoding.NATIVE_INBOUND_PROOF_SCHEMA_NAME, 0));
  }

  private static String canonicalReplayWitnessArtifact() {
    return Base64.getEncoder()
        .encodeToString(canonicalReplayWitnessBytes(new byte[32], new byte[32], List.of()));
  }

  private static byte[] canonicalReplayWitnessBytes(
      final byte[] priorRecordDigest,
      final byte[] siblingBitmap,
      final List<byte[]> siblings) {
    final ByteArrayOutputStream siblingSequence = new ByteArrayOutputStream();
    writeU64(siblingSequence, siblings.size());
    for (final byte[] sibling : siblings) writeCompactField(siblingSequence, sibling);
    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    writeCompactField(payload, SccpReplayV1.emptyHashes().get(SccpReplayV1.DEPTH));
    writeCompactField(payload, priorRecordDigest);
    writeCompactField(payload, siblingBitmap);
    writeCompactField(payload, siblingSequence.toByteArray());
    final byte[] body = payload.toByteArray();
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(SccpSubmitEncoding.REPLAY_WITNESS_SCHEMA_NAME),
            body.length,
            CRC64.compute(body),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] result = Arrays.copyOf(header.encode(), NoritoHeader.HEADER_LENGTH + body.length);
    System.arraycopy(body, 0, result, NoritoHeader.HEADER_LENGTH, body.length);
    return result;
  }

  private static byte[] canonicalArtifactBytes(
      final String schemaName, final int padding) {
    final byte[] schema = SchemaHash.hash16(schemaName);
    final byte[] payload = {1, 2, 3};
    final NoritoHeader header =
        new NoritoHeader(
            schema,
            payload.length,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] encoded = new byte[NoritoHeader.HEADER_LENGTH + padding + payload.length];
    System.arraycopy(header.encode(), 0, encoded, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(payload, 0, encoded, NoritoHeader.HEADER_LENGTH + padding, payload.length);
    return encoded;
  }

  private static String publicSignalSchemaHash() {
    final List<String> labels =
        List.of(
            "sccp:groth16-bn254:signal:message-id:v1",
            "sccp:groth16-bn254:signal:payload-hash:v1",
            "sccp:groth16-bn254:signal:target-domain:v1",
            "sccp:groth16-bn254:signal:commitment-root:v1",
            "sccp:groth16-bn254:signal:finality-height:v1",
            "sccp:groth16-bn254:signal:finality-block-hash:v1",
            "sccp:groth16-bn254:signal:source-domain:v1",
            "sccp:groth16-bn254:signal:statement-hash:v1",
            "sccp:groth16-bn254:signal:destination-binding-hash:v1",
            "sccp:groth16-bn254:signal:route-configuration-hash:v1",
            "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1");
    final ByteArrayOutputStream body = new ByteArrayOutputStream();
    body.write(1);
    writeU32(body, labels.size());
    for (final String label : labels) {
      final byte[] bytes = label.getBytes(StandardCharsets.UTF_8);
      writeU32(body, bytes.length);
      body.write(bytes, 0, bytes.length);
    }
    final byte[] prefix =
        "sccp:groth16-bn254:public-signal-schema:v1".getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = Arrays.copyOf(prefix, prefix.length + body.size());
    System.arraycopy(body.toByteArray(), 0, preimage, prefix.length, body.size());
    return upperHex(keccak(preimage));
  }

  private static String tairaChainIdHash() {
    return upperHex(keccak(hexBytes("fc56984b2be7431d840e21514d1883f0")));
  }

  private static String semanticProfileHash() {
    return semanticProfileHash(semanticProfile());
  }

  private static String semanticProfileHash(final Map<String, Object> profile) {
    final ByteArrayOutputStream body = new ByteArrayOutputStream();
    body.write(1);
    body.write(
        "sora_taira_finality_inclusion_groth16_bls12381".equals(profile.get("profile"))
            ? 1
            : 0);
    body.write(1);
    final Map<String, Object> commitments = object(profile.get("commitments"));
    for (final String word :
        List.of(
            (String) commitments.get("circuit_commitment"),
            (String) commitments.get("witness_generator_commitment"),
            (String) commitments.get("public_signal_schema_hash"))) {
      final byte[] bytes = hexBytes(word);
      body.write(bytes, 0, bytes.length);
    }
    return upperHex(prefixedKeccak("sccp:semantic-proof-profile:v1", body.toByteArray()));
  }

  private static String finalityAnchorHash() {
    return finalityAnchorHash(finalityAnchor());
  }

  private static String finalityAnchorHash(final Map<String, Object> anchor) {
    final ByteArrayOutputStream body = new ByteArrayOutputStream();
    body.write(1);
    body.write(parsedNetwork(object(anchor.get("source_network"))).tag());
    writeU16(body, ((Number) anchor.get("protocol_version")).intValue());
    byte[] bytes = hexBytes((String) anchor.get("chain_id_hash"));
    body.write(bytes, 0, bytes.length);
    writeU64(body, ((Number) anchor.get("checkpoint_height")).longValue());
    bytes = hexBytes((String) anchor.get("checkpoint_block_hash"));
    body.write(bytes, 0, bytes.length);
    bytes = hexBytes((String) anchor.get("checkpoint_context_id"));
    body.write(bytes, 0, bytes.length);
    bytes = hexBytes((String) anchor.get("checkpoint_finality_artifact_hash"));
    body.write(bytes, 0, bytes.length);
    assert body.size() == 140;
    return upperHex(prefixedKeccak("sccp:sora-finality-anchor:v1", body.toByteArray()));
  }

  private static void writeU32(final ByteArrayOutputStream out, final int value) {
    for (int shift = 0; shift < 4; shift++) out.write((value >>> (shift * 8)) & 0xff);
  }

  private static void writeI32(final ByteArrayOutputStream out, final int value) {
    writeU32(out, value);
  }

  private static void writeVector(final ByteArrayOutputStream out, final byte[] value) {
    writeU32(out, value.length);
    writeRaw(out, value);
  }

  private static void writeRaw(final ByteArrayOutputStream out, final byte[] value) {
    out.write(value, 0, value.length);
  }

  private static byte[] keccak(final byte[] value) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(value, 0, value.length);
    final byte[] result = new byte[32];
    digest.doFinal(result, 0);
    return result;
  }

  private static byte[] sha256(final byte[] value) {
    final SHA256Digest digest = new SHA256Digest();
    digest.update(value, 0, value.length);
    final byte[] result = new byte[32];
    digest.doFinal(result, 0);
    return result;
  }

  private static byte[] prefixedKeccak(final String prefix, final byte[] value) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = Arrays.copyOf(prefixBytes, prefixBytes.length + value.length);
    System.arraycopy(value, 0, preimage, prefixBytes.length, value.length);
    return keccak(preimage);
  }

  private static void writeU64(final ByteArrayOutputStream out, final long value) {
    for (int shift = 0; shift < 8; shift++) out.write((int) ((value >>> (shift * 8)) & 0xff));
  }

  private static void writeCompactField(
      final ByteArrayOutputStream out, final byte[] value) {
    int remaining = value.length;
    do {
      int next = remaining & 0x7f;
      remaining >>>= 7;
      if (remaining != 0) next |= 0x80;
      out.write(next);
    } while (remaining != 0);
    out.write(value, 0, value.length);
  }

  private static void writeU128(final ByteArrayOutputStream out, final BigInteger value) {
    for (int shift = 0; shift < 16; shift++) {
      out.write(value.shiftRight(shift * 8).and(BigInteger.valueOf(0xff)).intValue());
    }
  }

  private static void writeU16(final ByteArrayOutputStream out, final int value) {
    for (int shift = 0; shift < 2; shift++) out.write((value >>> (shift * 8)) & 0xff);
  }

  private static byte[] hexBytes(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }

  private static String upperHex(final byte[] value) {
    final StringBuilder result = new StringBuilder(value.length * 2);
    for (final byte item : value) result.append(String.format("%02X", item & 0xff));
    return result.toString();
  }

  private static String hash(final int value) {
    return String.format("%02x", value).repeat(32);
  }

  private static String prefixed(final int value) {
    return "0x" + hash(value);
  }

  private static String upper(final int value, final int bytes) {
    return String.format("%02X", value).repeat(bytes);
  }

  private static byte[] fill(final int length, final int value) {
    final byte[] result = new byte[length];
    Arrays.fill(result, (byte) value);
    return result;
  }

  private static String canonicalAuthority(final int keyByte) {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(keyByte), "ed25519")
          .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new ExceptionInInitializerError(ex);
    }
  }

  private static byte[] jsonBytes(final Object value) {
    return JsonEncoder.encode(value).getBytes(StandardCharsets.UTF_8);
  }

  private static SccpDestinationProofSubmitRequest destinationRequest(
      final String authority, final String destinationProofB64) {
    return destinationRequest(authority, destinationProofB64, null, null, null);
  }

  private static SccpDestinationProofSubmitRequest destinationRequest(
      final String authority,
      final String destinationProofB64,
      final String signatureB64,
      final String transactionPayloadB64,
      final Long creationTimeMs) {
    if (signatureB64 != null || transactionPayloadB64 != null || creationTimeMs != null) {
      throw new IllegalArgumentException(
          "signed SCCP request fields are not part of the first-release SDK surface");
    }
    return new SccpDestinationProofSubmitRequest(
        authority,
        destinationProofB64,
        BRIDGE_FEE_PAYMENT);
  }

  private static SccpNativeMessageSubmitRequest messageRequest(
      final String authority, final String nativeProofB64) {
    return messageRequest(authority, nativeProofB64, null, null, null);
  }

  private static SccpNativeMessageSubmitRequest messageRequest(
      final String authority,
      final String nativeProofB64,
      final String signatureB64,
      final String transactionPayloadB64,
      final Long creationTimeMs) {
    if (signatureB64 != null || transactionPayloadB64 != null || creationTimeMs != null) {
      throw new IllegalArgumentException(
          "signed SCCP request fields are not part of the first-release SDK surface");
    }
    return new SccpNativeMessageSubmitRequest(
        authority,
        nativeProofB64,
        canonicalReplayWitnessArtifact(),
        BRIDGE_FEE_PAYMENT);
  }

  private static final class SccpSubmitExecutor implements HttpTransportExecutor {
    private final List<String> contentTypes;
    private final List<TransportRequest> requests = new ArrayList<>();

    private SccpSubmitExecutor(final List<String> contentTypes) {
      this.contentTypes = contentTypes;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      final TransportResponse.Builder builder =
          TransportResponse.builder()
              .setStatusCode(200)
              .setBody("{}".getBytes(StandardCharsets.UTF_8));
      for (final String contentType : contentTypes) {
        builder.addHeader("Content-Type", contentType);
      }
      return CompletableFuture.completedFuture(builder.build());
    }
  }

  private static final class SccpNoritoExecutor implements HttpTransportExecutor {
    private final byte[] body;
    private TransportRequest request;

    private SccpNoritoExecutor(final byte[] body) {
      this.body = body.clone();
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.request = request;
      return CompletableFuture.completedFuture(
          TransportResponse.builder()
              .setStatusCode(200)
              .setBody(body)
              .addHeader("Content-Type", "application/x-norito")
              .build());
    }
  }

  private static Map<String, Object> map() {
    return new LinkedHashMap<>();
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value) {
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Map<String, Object> value, final String key) {
    return (List<Object>) value.get(key);
  }

  private static void expectFailure(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected rejection");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static String expectFailureMessage(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected rejection");
    } catch (final IllegalArgumentException expected) {
      return expected.getMessage() == null ? "" : expected.getMessage();
    }
  }
}
