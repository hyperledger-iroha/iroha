package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Collections;
import java.util.function.Consumer;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class TronSccpProverTests {
  private static final String TEST_SOURCE_CHAIN_PROOF_ENVELOPE_SCHEMA =
      "iroha_sccp::SccpSourceChainProofEnvelopeV1";

  private TronSccpProverTests() {}

  public static void main(final String[] args) {
    derivesTronRouteCanaryEvidenceHash();
    derivesGroth16PublicSignalWords();
    proofRequestBindsPublicSignalsAndRelayContext();
    proverRequiresLinkedProofEngine();
    proverWrapsExternalProofBytes();
    proverResolvesWitnessProviderBeforeBuildingRequest();
    rejectsMalformedGroth16ProofTuple();
    buildsContractCallSubmission();
    System.out.println("[IrohaAndroid] TRON SCCP prover tests passed.");
  }

  private static void derivesTronRouteCanaryEvidenceHash() {
    final TronSccpProver.RouteCanaryEvidenceInput evidence = sampleTronRouteCanaryEvidence();

    assert TronSccpProver.canonicalRouteCanaryEvidenceBytes(evidence).length == 551
        : "TRON route canary transcript length must match Rust";
    assert TronSccpProver.routeCanaryEvidenceHash(evidence)
        .equals("0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56")
        : "TRON route canary hash must match Rust/script vector";

    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(
                    "0x" + repeat("78", 32), null, null, true, true, null, true, null)),
        "routeAllowlistHash");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(
                    null, "0x" + repeat("78", 32), null, true, true, null, true, null)),
        "destinationBindingHash");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(null, null, "0", true, true, null, true, null)),
        "blockNumber");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(null, null, null, false, true, null, true, null)),
        "usedMessageProof");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(null, null, null, true, false, null, true, null)),
        "rawDataOwnerMatchesTransaction");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(null, null, null, true, true, null, false, null)),
        "signatureRecoversToOwner");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(
                    null, null, null, true, true, "0x41" + repeat("12", 20), true, null)),
        "signatureRecoveredAddress");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidence(
                    null, null, null, true, true, null, true, "0x" + repeat("78", 32))),
        "routeCanaryEvidenceHash");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidenceWithGovernedHashes(
                    evidence.destinationBindingHash(), evidence.destinationBindingHash(), null, null)),
        "TRON route canary governed hashes");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidenceWithGovernedHashes(
                    evidence.sourceVerifierMaterialHash(), null, null, null)),
        "TRON route canary governed hashes");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidenceWithGovernedHashes(
                    evidence.sourceAdapterEngineDeploymentHash(), null, null, null)),
        "TRON route canary governed hashes");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidenceWithGovernedHashes(
                    null, evidence.destinationBindingHash(), evidence.destinationBindingHash(), null)),
        "TRON route canary governed hashes");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidenceWithGovernedHashes(
                    null, evidence.destinationBindingHash(), null, evidence.destinationBindingHash())),
        "TRON route canary governed hashes");
    assertThrows(
        () ->
            TronSccpProver.routeCanaryEvidenceHash(
                sampleTronRouteCanaryEvidenceWithGovernedHashes(
                    null, null, null, evidence.sourceVerifierMaterialHash())),
        "TRON route canary governed hashes");
  }

  private static void derivesGroth16PublicSignalWords() {
    final List<String> signals =
        TronSccpProver.groth16Bn254PublicSignalWords(
            samplePublicInputs(),
            TronSccpProver.DOMAIN_SORA,
            repeat("55", 32),
            repeat("66", 32));

    assert signals.equals(
            Arrays.asList(
                "0x065b440c575edfc60df3235cfcf9e94d9b109e34d2ec6474934ab183d1306607",
                "0x23bc72d067e4874a3c42a8e8efa48c64e2612f164e6332c5b2a65e1e23950939",
                "0x21aac4195d8db839756f61c0780675823e15456c92acf135c36e02367c8fd11f",
                "0x00d836b75e1646c15194e6e7db40b834b101fe4dad47ab46607d91c9a026ec70",
                "0x0ca6bbc36d23183d027c8df09f06c39e64abbb0bb4d6a4c37369d2c36f41a888",
                "0x2b153d0fe1bc6e2a6d44e851523edb1511dac55443ca80c22cbe9cb7423886dc",
                "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
                "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
                "0x1157cd422e2089145c9cf93794dd6a0a1c3b1a611c22a5fe999d0542f62535d8"))
        : "TRON Groth16 public signal words must match the verifier transcript";

    final List<String> changed =
        TronSccpProver.groth16Bn254PublicSignalWords(
            samplePublicInputs(),
            TronSccpProver.DOMAIN_SORA,
            repeat("55", 32),
            repeat("67", 32));
    assert signals.subList(0, 8).equals(changed.subList(0, 8))
        : "only the destination-binding signal should change";
    assert !signals.get(8).equals(changed.get(8))
        : "destination binding must affect the ninth signal";
  }

  private static void proofRequestBindsPublicSignalsAndRelayContext() {
    final TronSccpProver.ProofRequest request =
        TronSccpProver.buildProofRequest(sampleProofRequestInput(new byte[0], repeat("56", 32)));
    assert TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())
        : "backend must be TRON Groth16";
    assert request.sourceDomain() == TronSccpProver.DOMAIN_SORA : "source domain must be SORA";
    assert request.targetDomain() == TronSccpProver.DOMAIN_TRON : "target domain must be TRON";
    assert request.publicSignalWords().size() == 9 : "request must expose nine public signals";
    assert ("0x" + repeat("56", 32)).equals(request.statementHash())
        : "statement hash must be normalized";
    assert ("0x" + repeat("78", 32)).equals(request.destinationBindingHash())
        : "destination binding hash must be normalized";
    assert request.requestHash().matches("0x[0-9a-f]{64}") : "request hash must be hex";
    final TronSccpProver.ProofRequest callbackSnapshot =
        TronSccpProver.callbackRequestSnapshot(request);
    assert callbackSnapshot != request : "TRON proof engine must receive a request snapshot";
    assert callbackSnapshot.version() == request.version() : "snapshot version must match";
    assert callbackSnapshot.backend().equals(request.backend()) : "snapshot backend must match";
    assert callbackSnapshot.sourceDomain() == request.sourceDomain() : "snapshot source domain must match";
    assert callbackSnapshot.targetDomain() == request.targetDomain() : "snapshot target domain must match";
    assert callbackSnapshot.publicSignalWords().equals(request.publicSignalWords())
        : "snapshot public signals must match";
    assert Arrays.equals(callbackSnapshot.publicInputsBytes(), request.publicInputsBytes())
        : "snapshot public inputs must match";
    assert Arrays.equals(callbackSnapshot.bundleBytes(), request.bundleBytes())
        : "snapshot bundle bytes must match";
    assert Arrays.equals(callbackSnapshot.sourceProofBytes(), request.sourceProofBytes())
        : "snapshot source proof bytes must match";
    final byte[] snapshotBundle = callbackSnapshot.bundleBytes();
    final byte[] snapshotSourceProof = callbackSnapshot.sourceProofBytes();
    snapshotBundle[0] = 77;
    assert Arrays.equals(sampleBundleBytes(), callbackSnapshot.bundleBytes())
        : "snapshot bundle bytes must be defensive copies";
    assert Arrays.equals(new byte[0], snapshotSourceProof)
        : "snapshot source proof view must be empty for SORA bundles";
    assert Arrays.equals(new byte[0], callbackSnapshot.sourceProofBytes())
        : "snapshot source proof bytes must be defensive copies";

    final SourceSccpProofs.TronDestinationBinding destinationBinding =
        sampleDestinationBinding(samplePublicInputs());
    final TronSccpProver.ProofRequest boundRequest =
        TronSccpProver.buildProofRequest(
            new TronSccpProver.ProofRequestInput(
                samplePublicInputs(),
                sampleBundleBytes(),
                new byte[0],
                repeat("56", 32),
                destinationBinding));
    assert destinationBinding.hash.equals(boundRequest.destinationBindingHash())
        : "destination binding object constructor must thread the derived hash";
    assert destinationBinding == boundRequest.destinationBinding()
        : "bound request must carry destination binding deployment material";
    assert !request.requestHash().equals(boundRequest.requestHash())
        : "request hash must bind the derived destination binding";

    boolean threw = false;
    try {
      TronSccpProver.buildProofRequest(sampleProofRequestInput(new byte[] {9, 11}, repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must be empty for SORA source bundle");
    }
    assert threw : "TRON proof requests must reject source proof bytes for SORA bundles";
    assert !request
        .requestHash()
        .equals(
            TronSccpProver.buildProofRequest(
                    new TronSccpProver.ProofRequestInput(
                        sampleBundleFixture(328L).publicInputs,
                        sampleBundleFixture(328L).bundleBytes,
                        new byte[0],
                        repeat("56", 32),
                        repeat("78", 32),
                        TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
                        TronSccpProver.DOMAIN_SORA))
                .requestHash())
        : "request hash must bind the bundle/source-proof split";

    final SampleBundleFixture nonSoraBundle =
        sampleBundleFixture(SourceSccpProofs.DOMAIN_ETH, 327L);
    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          new TronSccpProver.ProofRequestInput(
              nonSoraBundle.publicInputs,
              nonSoraBundle.bundleBytes,
              new byte[] {9, 10},
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must match bundleBytes finality proof");
    }
    assert threw : "TRON proof requests must bind non-SORA source proofs to bundle finality proofs";

    threw = false;
    final SampleBundleFixture replaySourceFixture =
        sampleBundleFixture(SourceSccpProofs.DOMAIN_ETH, 328L);
    final SampleBundleFixture replayBoundSourceFixture =
        sampleBundleFixture(
            SourceSccpProofs.DOMAIN_ETH,
            327L,
            testCanonicalSccpSourceProofBytes(SourceSccpProofs.DOMAIN_ETH, nonSoraBundle));
    try {
      TronSccpProver.buildProofRequest(
          new TronSccpProver.ProofRequestInput(
              replayBoundSourceFixture.publicInputs,
              replayBoundSourceFixture.bundleBytes,
              testCanonicalSccpSourceProofBytes(SourceSccpProofs.DOMAIN_ETH, replaySourceFixture),
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must match bundleBytes finality proof");
    }
    assert threw : "TRON proof requests must reject replayed canonical source proof bytes";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          new TronSccpProver.ProofRequestInput(
              nonSoraBundle.publicInputs,
              nonSoraBundle.bundleBytes,
              new byte[] {0x01, 0x02, 0x03},
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must decode as SccpSourceChainProofEnvelopeV1");
    }
    assert threw : "TRON proof requests must reject undecodable non-SORA source proof bytes";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(sampleProofRequestInput(new byte[0], ""));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("statementHash");
    }
    assert threw : "missing statement hash must be rejected";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(repeat("00", 32)),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("payloadHash");
    }
    assert threw : "zero payload hash must be rejected";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(" " + repeat("22", 32)),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("payloadHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded TRON payload hash must be rejected";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(new byte[0], repeat("56", 32) + " "));
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("statementHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded TRON statement hash must be rejected";

    for (final String finalityHeight : new String[] {"019", "0x13", "+19", " 19", "19 "}) {
      threw = false;
      try {
        TronSccpProver.buildProofRequest(
            sampleProofRequestInput(
                samplePublicInputs(repeat("22", 32), TronSccpProver.DOMAIN_TRON, finalityHeight),
                new byte[0],
                repeat("56", 32),
                repeat("78", 32),
                TronSccpProver.DOMAIN_SORA));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("finalityHeight");
      }
      assert threw : "noncanonical TRON finality height must be rejected";
    }

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(repeat("22", 32), TronSccpProver.DOMAIN_TRON, "0"),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalityHeight");
    }
    assert threw : "zero TRON finality height must be rejected";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          new TronSccpProver.ProofRequestInput(
              samplePublicInputs(),
              new byte[0],
              new byte[] {9, 10},
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes");
    }
    assert threw : "empty bundle bytes must be rejected";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(new byte[] {0, 0}, repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must not be all zero");
    }
    assert threw : "all-zero TRON source proof bytes must be rejected";

    final byte[] oversizedSourceProof =
        new byte[TronSccpProver.SOURCE_STATE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedSourceProof, (byte) 1);
    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(oversizedSourceProof, repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must be at most");
    }
    assert threw : "oversized TRON source proof bytes must be rejected";
    assert TronSccpProver.buildProofRequest(sampleProofRequestInput(new byte[0], repeat("56", 32)))
        .sourceProofBytes()
        .length == 0 : "empty optional TRON source proof bytes must remain valid";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(), new byte[0], repeat("56", 32), repeat("78", 32), SourceSccpProofs.DOMAIN_ETH));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "non-SORA source domain must be rejected";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(repeat("22", 32), TonSccpProver.DOMAIN_TON),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("targetDomain must be TRON");
    }
    assert threw : "non-TRON target domain must be rejected";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(), new byte[0], repeat("56", 32), repeat("00", 32), TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "zero destination binding hash must be rejected";

    threw = false;
    try {
      new TronSccpProver.ProofRequestInput(
          samplePublicInputs(repeat("22", 32), TonSccpProver.DOMAIN_TON),
          sampleBundleBytes(),
          repeat("56", 32),
          destinationBinding);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBinding.targetDomain");
    }
    assert threw : "destination binding target must match TRON public inputs";

    threw = false;
    try {
      TronSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              "debug-tron-backend",
              TronSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("tron-groth16-bn254-v1");
    }
    assert threw : "wrong TRON proof backend must be rejected";
  }

  private static void proverRequiresLinkedProofEngine() {
    boolean threw = false;
    try {
      new TronSccpProver().prove(sampleProofRequestInput(new byte[0], repeat("56", 32)));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("not linked");
    }
    assert threw : "expected missing local prover to throw";
  }

  private static void proverWrapsExternalProofBytes() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final TronSccpProver.ProofRequest[] seenRequests = new TronSccpProver.ProofRequest[2];
    final int[] seenRequestCount = new int[] {0};
    final TronSccpProver prover =
        new TronSccpProver(
            null,
            request -> {
              seenRequests[seenRequestCount[0]++] = request;
              assert TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())
                  : "backend must be TRON Groth16";
              assert request.publicSignalWords().size() == 9 : "request must carry public signals";
              return proofBytes;
            });

    final TronSccpProver.ProofResult result =
        prover.prove(sampleProductionProofRequestInput(new byte[0], repeat("56", 32)));
    final TronSccpProver.ProofResult omittedSourceResult =
        prover.prove(sampleProductionProofRequestInput(new byte[0], repeat("56", 32)));
    assert Arrays.equals(proofBytes, result.proofBytes())
        : "proof bytes must be preserved";
    assert Arrays.equals(new byte[0], result.sourceProofBytes())
        : "TRON production proofs must keep SORA source proof bytes empty";
    assert Arrays.equals(new byte[0], omittedSourceResult.sourceProofBytes())
        : "TRON production proofs may omit source proof bytes";
    assert !result.proofBase64().isEmpty() : "proof base64 must be exposed";
    assert ("0x" + repeat("56", 32)).equals(result.statementHash())
        : "result must expose statement hash";
    final TronSccpProver.ProofRequest request =
        TronSccpProver.buildProofRequest(sampleProductionProofRequestInput(new byte[0], repeat("56", 32)));
    final TronSccpProver.ProofRequest omittedSourceRequest =
        TronSccpProver.buildProofRequest(sampleProductionProofRequestInput(new byte[0], repeat("56", 32)));
    assert request.destinationBindingHash().equals(result.destinationBindingHash())
        : "result must expose destination binding hash";
    assert request.destinationBinding().hash.equals(result.destinationBinding().hash)
        : "result must carry destination binding deployment material";
    assert request.requestHash().equals(result.requestHash()) : "request hash must match";
    assert result.envelopeHash().matches("0x[0-9a-f]{64}") : "envelope hash must be hex";

    assert seenRequestCount[0] == 2 : "proof engine must receive both TRON callback requests";
    assert seenRequests[0] != request : "TRON proof engine must receive a request snapshot";
    assert seenRequests[0].requestHash().equals(request.requestHash())
        : "TRON callback snapshot must match the canonical request hash";
    assert seenRequests[0].publicSignalWords().equals(request.publicSignalWords())
        : "TRON callback snapshot must match public signals";
    assert Arrays.equals(seenRequests[0].bundleBytes(), request.bundleBytes())
        : "TRON callback snapshot must copy bundle bytes";
    assert Arrays.equals(seenRequests[0].sourceProofBytes(), request.sourceProofBytes())
        : "TRON callback snapshot must copy source proof bytes";
    assert seenRequests[1] != omittedSourceRequest
        : "TRON proof engine must receive an omitted-source request snapshot";
    assert seenRequests[1].requestHash().equals(omittedSourceRequest.requestHash())
        : "TRON omitted-source callback snapshot must match canonical request";
    boolean threw = false;
    try {
      TronSccpProver.wrapProofResult(new byte[] {0, 0}, request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "TRON proof result wrapper must reject all-zero proof bytes";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(new byte[] {1, 2, 3, 4}, request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("384 bytes");
    }
    assert threw : "TRON proof result wrapper must reject non-canonical proof lengths";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(
          new byte[] {1}, tronRequestWithBackend(request, "debug-tron-backend"));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("tron-groth16-bn254-v1");
    }
    assert threw : "TRON proof result wrapper must reject wrong backends";

    boolean canonicalThrew = false;
    try {
      TronSccpProver.wrapProofResult(
          proofBytes, tronRequestWithRequestHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      canonicalThrew = ex.getMessage().contains("canonical");
    }
    assert canonicalThrew : "TRON proof result wrapper must reject non-canonical requests";

    boolean missingBindingThrew = false;
    try {
      TronSccpProver.wrapProofResult(
          proofBytes,
          TronSccpProver.buildProofRequest(sampleProofRequestInput(new byte[0], repeat("56", 32))));
    } catch (final IllegalArgumentException ex) {
      missingBindingThrew = ex.getMessage().contains("destinationBinding");
    }
    assert missingBindingThrew : "TRON proof result wrapper must reject hash-only destination bindings";

    final byte[] exposedProof = result.proofBytes();
    exposedProof[0] = 9;
    assert Arrays.equals(proofBytes, result.proofBytes())
        : "TRON proof result bytes must be defensive copies";

    final ArrayList<String> mutableSignals = new ArrayList<>(result.publicSignalWords());
    final TronSccpProver.ProofResult manualResult =
        new TronSccpProver.ProofResult(
            result.version(),
            result.backend(),
            result.proofBytes(),
            result.proofBase64(),
            result.publicInputs(),
            mutableSignals,
            result.bundleBytes(),
            result.sourceProofBytes(),
            result.proofContext(),
            result.statementHash(),
            result.destinationBindingHash(),
            result.requestHash(),
            result.envelopeHash());
    mutableSignals.set(0, "0x" + repeat("99", 32));
    assert manualResult.publicSignalWords().equals(result.publicSignalWords())
        : "TRON proof result public signals must be construction snapshots";
    boolean immutableSignals = false;
    try {
      manualResult.publicSignalWords().set(0, "0x" + repeat("88", 32));
    } catch (final UnsupportedOperationException ex) {
      immutableSignals = true;
    }
    assert immutableSignals : "TRON proof result public signals must be immutable";
  }

  private static void proverResolvesWitnessProviderBeforeBuildingRequest() {
    final boolean[] resolved = new boolean[] {false};
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final byte[] bundleBytes = sampleBundleBytes();
    final TronSccpProver.ProofRequestInput userInput =
        sampleProductionProofRequestInput(samplePublicInputs(), bundleBytes, new byte[0], repeat("56", 32));
    final TronSccpProver prover =
        new TronSccpProver(
            input -> {
              assert Arrays.equals(new byte[0], input.sourceProofBytes())
                  : "UI witness provider should receive unresolved request input";
              assert input.bundleBytes() != bundleBytes
                  : "UI witness provider must receive a byte snapshot";
              input.bundleBytes()[0] = 0x7f;
              resolved[0] = true;
              return sampleProductionProofRequestInput(
                  input.publicInputs(),
                  new byte[0],
                  input.statementHash());
            },
            request -> {
              assert resolved[0] : "witness provider must run before proof engine";
              assert Arrays.equals(new byte[0], request.sourceProofBytes())
                  : "proof engine must receive empty SORA source proof bytes";
              return proofBytes;
            });

    final TronSccpProver.ProofResult result = prover.prove(userInput);

    assert Arrays.equals(new byte[0], result.sourceProofBytes())
        : "wrapped result must preserve empty SORA source proof bytes";
    assert Arrays.equals(sampleBundleBytes(), userInput.bundleBytes())
        : "UI-owned TRON bundle bytes must not be mutated by witness provider";
    assert Arrays.equals(sampleBundleBytes(), bundleBytes)
        : "UI-owned TRON bundle array must not be mutated by witness provider";
  }

  private static void rejectsMalformedGroth16ProofTuple() {
    final TronSccpProver.ProofRequest request =
        TronSccpProver.buildProofRequest(sampleProductionProofRequestInput(new byte[0], repeat("56", 32)));

    boolean threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytes(0, abiWord(2)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.version");
    }
    assert threw : "TRON proof wrapper must reject non-v1 Groth16 ABI tuples";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytes(4, repeatedWord(0xff)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("BN254 base-field");
    }
    assert threw : "TRON proof wrapper must reject out-of-range BN254 coordinates";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytesWithZeroA(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.a");
    }
    assert threw : "TRON proof wrapper must reject zero Groth16 A points";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytesWithZeroB(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.b");
    }
    assert threw : "TRON proof wrapper must reject zero Groth16 B points";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytesWithZeroC(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.c");
    }
    assert threw : "TRON proof wrapper must reject zero Groth16 C points";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytes(6, abiWord(4)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.b");
    }
    assert threw : "TRON proof wrapper must reject off-curve Groth16 B points";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytesWithNonSubgroupB(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.b");
    }
    assert threw : "TRON proof wrapper must reject non-subgroup Groth16 B points";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytes(1, repeatedWord(0x22)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("messageId must match");
    }
    assert threw : "TRON proof wrapper must reject message-id mismatches";

    threw = false;
    try {
      TronSccpProver.wrapProofResult(sampleGroth16ProofBytes(2, abiWord(999)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must match");
    }
    assert threw : "TRON proof wrapper must reject source-domain mismatches";

    threw = false;
    try {
      TronSccpProver.submitMessageProofCallData(
          sampleGroth16ProofBytes(2, abiWord(SourceSccpProofs.DOMAIN_ETH)),
          samplePublicInputs(),
          repeat("56", 32));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must match");
    }
    assert threw : "TRON direct calldata helper must reject source-domain mismatches";

    threw = false;
    try {
      TronSccpProver.submitMessageProofCallData(
          sampleGroth16ProofBytes(2, abiWord(SourceSccpProofs.DOMAIN_ETH)),
          samplePublicInputs(),
          repeat("56", 32),
          SourceSccpProofs.DOMAIN_ETH);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "TRON direct calldata helper must reject non-SORA source domains";

    threw = false;
    try {
      TronSccpProver.buildSubmission(
          new TronSccpProver.SubmissionInput(
              samplePublicInputs(repeat("22", 32), TronSccpProver.DOMAIN_TRON),
              sampleGroth16ProofBytes(3, repeatedWord(0x44)),
              repeat("56", 32),
              repeat("78", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("commitmentRoot must match");
    }
    assert threw : "TRON submission builder must reject commitment-root mismatches";
  }

  private static void buildsContractCallSubmission() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final TronSccpProver.ProofRequest request =
        TronSccpProver.buildProofRequest(sampleProductionProofRequestInput(new byte[0], repeat("56", 32)));
    final TronSccpProver.ProofResult proofResult =
        TronSccpProver.wrapProofResult(proofBytes, request);
    final TronSccpProver.Submission submission =
        TronSccpProver.buildSubmission(new TronSccpProver.SubmissionInput(proofResult));

    assert "contract_call".equals(submission.submissionKind())
        : "TRON submission kind must be contract_call";
    assert "tron_contract_call".equals(submission.platformPayload())
        : "TRON submission platform payload must identify contract calls";
    assert TronSccpProver.CONTRACT_CALL_ABI_TUPLE_V1.equals(submission.envelopeEncoding())
        : "TRON envelope encoding must be ABI tuple v1";
    assert TronSccpProver.SUBMIT_MESSAGE_PROOF_SELECTOR_V1.equals(submission.functionSelector())
        : "TRON function selector must be exposed";
    assert submission.callDataHex().startsWith(TronSccpProver.SUBMIT_MESSAGE_PROOF_SELECTOR_V1)
        : "call data must start with the selector";
    assert submission.callData().length == 676 : "TRON call data length must be ABI canonical";
    assert ("0x" + repeat("00", 30) + "0100")
        .equals("0x" + hexLower(Arrays.copyOfRange(submission.callData(), 4, 36)))
        : "dynamic proof offset must be 0x100";
    assert ("0x" + repeat("00", 30) + "0180")
        .equals("0x" + hexLower(Arrays.copyOfRange(submission.callData(), 260, 292)))
        : "proof length must be 384 bytes";
    assert TronSccpProver.messageTransparentPublicInputAbiWords(samplePublicInputs())
        .equals(submission.publicInputWords()) : "public input ABI words must be exposed";
    assert proofResult.publicSignalWords().equals(submission.publicSignalWords())
        : "public signal words must be carried";
    assert Arrays.equals(sampleBundleBytes(), proofResult.bundleBytes())
        : "proof results must retain request bundle bytes";
    assert Arrays.equals(new byte[0], proofResult.sourceProofBytes())
        : "proof results must keep SORA source proof bytes empty";
    assert Arrays.equals(proofBytes, submission.proofBytes()) : "proof bytes must be preserved";
    assert Arrays.equals(submission.callData(), submission.envelopeBytes())
        : "TRON envelope bytes must equal call data";
    assert Arrays.equals(
            submission.callData(),
            TronSccpProver.submitMessageProofCallData(
                proofBytes, proofResult.publicInputs(), proofResult.statementHash()))
        : "direct calldata helper must match submission call data";
    final SourceSccpProofs.TronDestinationBinding destinationBinding =
        sampleDestinationBinding(proofResult.publicInputs());
    final TronSccpProver.Submission bindingSubmission =
        TronSccpProver.buildSubmission(
            new TronSccpProver.SubmissionInput(
                proofResult.publicInputs(),
                proofBytes,
                proofResult.statementHash(),
                destinationBinding));
    assert destinationBinding.hash.equals(bindingSubmission.destinationBindingHash())
        : "TRON submission input must accept a derived destination binding";

    final TronSccpProver.ProofResult omittedSourceProofResult =
        TronSccpProver.wrapProofResult(
            proofBytes,
            TronSccpProver.buildProofRequest(
                sampleProductionProofRequestInput(new byte[0], repeat("56", 32))));
    final TronSccpProver.Submission omittedSourceSubmission =
        TronSccpProver.buildSubmission(
            new TronSccpProver.SubmissionInput(omittedSourceProofResult));
    assert Arrays.equals(new byte[0], omittedSourceProofResult.sourceProofBytes())
        : "TRON submit-ready proof results may omit source proof bytes";
    assert Arrays.equals(proofBytes, omittedSourceSubmission.proofBytes())
        : "TRON omitted-source submission must preserve proof bytes";

    final byte[] exposedCallData = submission.callData();
    exposedCallData[0] = 0;
    assert submission.callData()[0] != 0 : "submission call data must be a defensive copy";

    final byte[] proofMismatch = Arrays.copyOf(proofBytes, proofBytes.length);
    proofMismatch[4 * 32 + 31] = 9;
    boolean threw = false;
    try {
      TronSccpProver.buildSubmission(
          new TronSccpProver.SubmissionInput(
              proofResult.publicInputs(),
              proofMismatch,
              proofResult.statementHash(),
              proofResult.destinationBindingHash(),
              TronSccpProver.DOMAIN_SORA,
              proofResult,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes");
    }
    assert threw : "submission must reject proof bytes that differ from the wrapped result";

    threw = false;
    try {
      new TronSccpProver.SubmissionInput(
          samplePublicInputs(repeat("22", 32), TonSccpProver.DOMAIN_TON),
          proofBytes,
          proofResult.statementHash(),
          destinationBinding);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBinding.targetDomain");
    }
    assert threw : "submission destination binding target must match TRON public inputs";

    threw = false;
    try {
      TronSccpProver.buildSubmission(
          new TronSccpProver.SubmissionInput(
              tronResultWithEnvelopeHash(proofResult, "0x" + repeat("aa", 32))));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("wrapped proof bytes");
    }
    assert threw : "submission must reject tampered wrapped proof-result envelope hashes";

    threw = false;
    try {
      TronSccpProver.buildSubmission(
          new TronSccpProver.SubmissionInput(tronResultWithProofBase64(proofResult, "AAAA")));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBase64");
    }
    assert threw : "submission must reject tampered wrapped proof-result proofBase64";

    threw = false;
    try {
      TronSccpProver.buildSubmission(
          new TronSccpProver.SubmissionInput(
              tronResultWithSourceProofBytes(proofResult, new byte[] {9, 11})));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requestHash");
    }
    assert threw : "submission must reject stale wrapped proof-result request context";

    final ArrayList<String> mismatchedSignals = new ArrayList<>(proofResult.publicSignalWords());
    mismatchedSignals.set(0, "0x" + repeat("99", 32));
    threw = false;
    try {
      TronSccpProver.buildSubmission(
          new TronSccpProver.SubmissionInput(
              proofResult.publicInputs(),
              proofBytes,
              proofResult.statementHash(),
              proofResult.destinationBindingHash(),
              TronSccpProver.DOMAIN_SORA,
              null,
              mismatchedSignals));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("publicSignalWords");
    }
    assert threw : "submission must reject public-signal mismatches";

    threw = false;
    try {
      TronSccpProver.buildSubmission(
          new TronSccpProver.SubmissionInput(
              samplePublicInputs(repeat("22", 32), TonSccpProver.DOMAIN_TON),
              proofBytes,
              proofResult.statementHash(),
              proofResult.destinationBindingHash()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("TRON");
    }
    assert threw : "submission must reject non-TRON target domains";
  }

  private static byte[] sampleGroth16ProofBytes() {
    return flattenGroth16ProofWords(
        sampleGroth16ProofWords(samplePublicInputs(), TronSccpProver.DOMAIN_SORA));
  }

  private static byte[] sampleGroth16ProofBytes(final int wordIndex, final byte[] word) {
    final byte[][] words =
        sampleGroth16ProofWords(samplePublicInputs(), TronSccpProver.DOMAIN_SORA);
    words[wordIndex] = Arrays.copyOf(word, word.length);
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithZeroA() {
    final byte[][] words =
        sampleGroth16ProofWords(samplePublicInputs(), TronSccpProver.DOMAIN_SORA);
    words[4] = new byte[32];
    words[5] = new byte[32];
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithZeroB() {
    final byte[][] words =
        sampleGroth16ProofWords(samplePublicInputs(), TronSccpProver.DOMAIN_SORA);
    words[6] = new byte[32];
    words[7] = new byte[32];
    words[8] = new byte[32];
    words[9] = new byte[32];
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithZeroC() {
    final byte[][] words =
        sampleGroth16ProofWords(samplePublicInputs(), TronSccpProver.DOMAIN_SORA);
    words[10] = new byte[32];
    words[11] = new byte[32];
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithNonSubgroupB() {
    final byte[][] words =
        sampleGroth16ProofWords(samplePublicInputs(), TronSccpProver.DOMAIN_SORA);
    words[6] = abiWord(0);
    words[7] = abiWord(1);
    words[8] = hexWord("0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8");
    words[9] = hexWord("07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2");
    return flattenGroth16ProofWords(words);
  }

  private static byte[][] sampleGroth16ProofWords(
      final TronSccpProver.PublicInputsInput publicInputs, final int sourceDomain) {
    return new byte[][] {
      abiWord(1),
      hexWord(stripHex(publicInputs.messageId())),
      abiWord(sourceDomain),
      hexWord(stripHex(publicInputs.commitmentRoot())),
      abiWord(1),
      abiWord(2),
      hexWord("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
      hexWord("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
      hexWord("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
      hexWord("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
      abiWord(1),
      abiWord(2)
    };
  }

  private static byte[] flattenGroth16ProofWords(final byte[][] words) {
    final byte[] out = new byte[TronSccpProver.GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1];
    for (int index = 0; index < words.length; index++) {
      System.arraycopy(words[index], 0, out, index * 32, 32);
    }
    return out;
  }

  private static byte[] abiWord(final long value) {
    final byte[] out = new byte[32];
    long working = value;
    for (int index = 31; index >= 0; index--) {
      out[index] = (byte) (working & 0xffL);
      working >>>= 8;
      if (working == 0L) {
        break;
      }
    }
    return out;
  }

  private static byte[] repeatedWord(final int value) {
    final byte[] out = new byte[32];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static byte[] hexWord(final String hex) {
    if (hex.length() != 64) {
      throw new IllegalArgumentException("hex word must be 32 bytes");
    }
    final byte[] out = new byte[32];
    for (int index = 0; index < out.length; index++) {
      out[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
    }
    return out;
  }

  private static byte[] filledBytes(final int length, final int value) {
    final byte[] out = new byte[length];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static String stripHex(final String value) {
    return value.startsWith("0x") ? value.substring(2) : value;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  private static TronSccpProver.ProofRequest tronRequestWithBackend(
      final TronSccpProver.ProofRequest request, final String backend) {
    return new TronSccpProver.ProofRequest(
        request.version(),
        backend,
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.publicSignalWords(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash(),
        request.destinationBinding());
  }

  private static TronSccpProver.ProofResult tronResultWithEnvelopeHash(
      final TronSccpProver.ProofResult result, final String envelopeHash) {
    return new TronSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.publicSignalWords(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        envelopeHash,
        result.destinationBinding());
  }

  private static TronSccpProver.ProofResult tronResultWithBundleBytes(
      final TronSccpProver.ProofResult result, final byte[] bundleBytes) {
    return new TronSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.publicSignalWords(),
        bundleBytes,
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        result.envelopeHash(),
        result.destinationBinding());
  }

  private static TronSccpProver.ProofResult tronResultWithSourceProofBytes(
      final TronSccpProver.ProofResult result, final byte[] sourceProofBytes) {
    return new TronSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.publicSignalWords(),
        result.bundleBytes(),
        sourceProofBytes,
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        result.envelopeHash(),
        result.destinationBinding());
  }

  private static TronSccpProver.ProofResult tronResultWithProofBase64(
      final TronSccpProver.ProofResult result, final String proofBase64) {
    return new TronSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        proofBase64,
        result.publicInputs(),
        result.publicSignalWords(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        result.envelopeHash(),
        result.destinationBinding());
  }

  private static TronSccpProver.ProofRequest tronRequestWithRequestHash(
      final TronSccpProver.ProofRequest request, final String requestHash) {
    return new TronSccpProver.ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.publicSignalWords(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        requestHash,
        request.destinationBinding());
  }

  private static TronSccpProver.ProofRequestInput sampleProofRequestInput(
      final byte[] sourceProofBytes, final String statementHash) {
    return sampleProofRequestInput(
        samplePublicInputs(),
        sourceProofBytes,
        statementHash,
        repeat("78", 32),
        TronSccpProver.DOMAIN_SORA);
  }

  private static TronSccpProver.ProofRequestInput sampleProofRequestInput(
      final TronSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final int sourceDomain) {
    return sampleProofRequestInput(
        publicInputs,
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain);
  }

  private static TronSccpProver.ProofRequestInput sampleProofRequestInput(
      final TronSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final String backend,
      final int sourceDomain) {
    return new TronSccpProver.ProofRequestInput(
        publicInputs,
        sampleBundleBytes(),
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        backend,
        sourceDomain);
  }

  private static TronSccpProver.ProofRequestInput sampleProductionProofRequestInput(
      final byte[] sourceProofBytes, final String statementHash) {
    return sampleProductionProofRequestInput(samplePublicInputs(), sourceProofBytes, statementHash);
  }

  private static TronSccpProver.ProofRequestInput sampleProductionProofRequestInput(
      final TronSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return sampleProductionProofRequestInput(
        publicInputs, sampleBundleBytes(), sourceProofBytes, statementHash);
  }

  private static TronSccpProver.ProofRequestInput sampleProductionProofRequestInput(
      final TronSccpProver.PublicInputsInput publicInputs,
      final byte[] bundleBytes,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return new TronSccpProver.ProofRequestInput(
        publicInputs,
        bundleBytes,
        sourceProofBytes,
        statementHash,
        sampleDestinationBinding(publicInputs),
        TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        TronSccpProver.DOMAIN_SORA);
  }

  private static final class SampleBundleFixture {
    final TronSccpProver.PublicInputsInput publicInputs;
    final byte[] bundleBytes;

    SampleBundleFixture(
        final TronSccpProver.PublicInputsInput publicInputs, final byte[] bundleBytes) {
      this.publicInputs = publicInputs;
      this.bundleBytes = bundleBytes;
    }
  }

  private static byte[] sampleBundleBytes() {
    return sampleBundleFixture().bundleBytes;
  }

  private static SampleBundleFixture sampleBundleFixture() {
    return sampleBundleFixture(327L);
  }

  private static SampleBundleFixture sampleBundleFixture(final long nonce) {
    return sampleBundleFixture(TronSccpProver.DOMAIN_SORA, nonce);
  }

  private static SampleBundleFixture sampleBundleFixture(final int sourceDomain, final long nonce) {
    return sampleBundleFixture(sourceDomain, nonce, new byte[] {0x01, 0x02, 0x03});
  }

  private static SampleBundleFixture sampleBundleFixture(
      final int sourceDomain, final long nonce, final byte[] finalityProofBytes) {
    final int senderCodec = sourceDomain == TronSccpProver.DOMAIN_SORA ? 1 : 2;
    final String sender =
        sourceDomain == TronSccpProver.DOMAIN_SORA
            ? "alice@sora"
            : "0x52908400098527886E0F7030069857D2E4169EE7";
    final ByteArrayOutputStream payloadBody = new ByteArrayOutputStream();
    payloadBody.write(1);
    writeTestU32Le(payloadBody, sourceDomain);
    writeTestU32Le(payloadBody, TronSccpProver.DOMAIN_TRON);
    writeTestU64Le(payloadBody, BigInteger.valueOf(nonce));
    writeTestU32Le(payloadBody, TronSccpProver.DOMAIN_SORA);
    payloadBody.write(1);
    writeTestBytes(payloadBody, "xor#sccp".getBytes(StandardCharsets.UTF_8));
    writeTestU128Le(payloadBody, BigInteger.valueOf(42L));
    payloadBody.write(senderCodec);
    writeTestBytes(payloadBody, sender.getBytes(StandardCharsets.UTF_8));
    payloadBody.write(5);
    writeTestBytes(payloadBody, "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".getBytes(StandardCharsets.UTF_8));
    payloadBody.write(1);
    writeTestBytes(payloadBody, "sora-tron-xor".getBytes(StandardCharsets.UTF_8));

    final byte[] payloadBodyBytes = payloadBody.toByteArray();
    final byte[] payloadBytes = concatTestBytes(new byte[] {0x02}, payloadBodyBytes);
    final String messageId =
        "0x" + hexLower(prefixedKeccakBytes("sccp:transfer:v1", payloadBodyBytes));
    final String payloadHash =
        "0x"
            + hexLower(
                Blake2b.digest256(
                    concatTestBytes("sccp:payload:v1".getBytes(StandardCharsets.UTF_8), payloadBytes)));

    final ByteArrayOutputStream commitment = new ByteArrayOutputStream();
    commitment.write(1);
    commitment.write(6);
    writeTestU32Le(commitment, TronSccpProver.DOMAIN_TRON);
    writeTestRawBytes(commitment, hexBytes(messageId.substring(2)));
    writeTestRawBytes(commitment, hexBytes(payloadHash.substring(2)));
    final byte[] commitmentBytes = commitment.toByteArray();
    final byte[] currentRoot =
        Blake2b.digest256(
            concatTestBytes("sccp:hub:leaf:v1".getBytes(StandardCharsets.UTF_8), commitmentBytes));
    final String commitmentRoot = "0x" + hexLower(currentRoot);

    final ByteArrayOutputStream merkleProof = new ByteArrayOutputStream();
    writeTestU32Le(merkleProof, 0);

    final ByteArrayOutputStream bundle = new ByteArrayOutputStream();
    bundle.write(1);
    writeTestRawBytes(bundle, currentRoot);
    writeTestBytes(bundle, commitmentBytes);
    writeTestBytes(bundle, merkleProof.toByteArray());
    writeTestBytes(bundle, payloadBytes);
    writeTestBytes(bundle, finalityProofBytes);

    return new SampleBundleFixture(
        new TronSccpProver.PublicInputsInput(
            1,
            messageId,
            payloadHash,
            TronSccpProver.DOMAIN_TRON,
            commitmentRoot,
            "19",
            repeat("44", 32)),
        bundle.toByteArray());
  }

  private static final class TestSccpSourceChainProofEnvelope {
    final int sourceDomain;
    final int targetDomain;
    final String sourceChain;
    final int sourceProofPlan;
    final int finalityModel;
    final String messageId;
    final String payloadHash;
    final String sourceEventDigest;
    final String commitmentRoot;
    final long finalityHeight;
    final String finalityBlockHash;
    final String finalizedHeaderHash;
    final String receiptOrMessageRoot;
    final byte[] consensusProofBytes;
    final byte[] messageInclusionProofBytes;
    final List<byte[]> inclusionBranch;

    TestSccpSourceChainProofEnvelope(
        final int sourceDomain,
        final int targetDomain,
        final String sourceChain,
        final int sourceProofPlan,
        final int finalityModel,
        final String messageId,
        final String payloadHash,
        final String sourceEventDigest,
        final String commitmentRoot,
        final long finalityHeight,
        final String finalityBlockHash,
        final String finalizedHeaderHash,
        final String receiptOrMessageRoot,
        final byte[] consensusProofBytes,
        final byte[] messageInclusionProofBytes,
        final List<byte[]> inclusionBranch) {
      this.sourceDomain = sourceDomain;
      this.targetDomain = targetDomain;
      this.sourceChain = sourceChain;
      this.sourceProofPlan = sourceProofPlan;
      this.finalityModel = finalityModel;
      this.messageId = messageId;
      this.payloadHash = payloadHash;
      this.sourceEventDigest = sourceEventDigest;
      this.commitmentRoot = commitmentRoot;
      this.finalityHeight = finalityHeight;
      this.finalityBlockHash = finalityBlockHash;
      this.finalizedHeaderHash = finalizedHeaderHash;
      this.receiptOrMessageRoot = receiptOrMessageRoot;
      this.consensusProofBytes = consensusProofBytes;
      this.messageInclusionProofBytes = messageInclusionProofBytes;
      this.inclusionBranch = inclusionBranch;
    }
  }

  private static final TypeAdapter<TestSccpSourceChainProofEnvelope>
      TEST_SOURCE_CHAIN_PROOF_ADAPTER =
          new TypeAdapter<TestSccpSourceChainProofEnvelope>() {
            @Override
            public void encode(
                final NoritoEncoder encoder, final TestSccpSourceChainProofEnvelope value) {
              writeTestNoritoField(encoder, child -> child.writeUInt(1, 8));
              writeTestNoritoField(encoder, child -> child.writeUInt(value.sourceDomain, 32));
              writeTestNoritoField(encoder, child -> child.writeUInt(value.targetDomain, 32));
              writeTestNoritoField(encoder, child -> writeTestNoritoString(child, value.sourceChain));
              writeTestNoritoField(encoder, child -> child.writeUInt(value.sourceProofPlan, 32));
              writeTestNoritoField(encoder, child -> child.writeUInt(value.finalityModel, 32));
              writeTestNoritoField(encoder, child -> child.writeBytes(hexBytes(stripHex(value.messageId))));
              writeTestNoritoField(encoder, child -> child.writeBytes(hexBytes(stripHex(value.payloadHash))));
              writeTestNoritoField(
                  encoder, child -> child.writeBytes(hexBytes(stripHex(value.sourceEventDigest))));
              writeTestNoritoField(encoder, child -> child.writeBytes(hexBytes(stripHex(value.commitmentRoot))));
              writeTestNoritoField(encoder, child -> child.writeUInt(value.finalityHeight, 64));
              writeTestNoritoField(
                  encoder, child -> child.writeBytes(hexBytes(stripHex(value.finalityBlockHash))));
              writeTestNoritoField(
                  encoder, child -> child.writeBytes(hexBytes(stripHex(value.finalizedHeaderHash))));
              writeTestNoritoField(
                  encoder, child -> child.writeBytes(hexBytes(stripHex(value.receiptOrMessageRoot))));
              writeTestNoritoField(
                  encoder, child -> writeTestNoritoRawByteVec(child, value.consensusProofBytes));
              writeTestNoritoField(
                  encoder,
                  child -> writeTestNoritoRawByteVec(child, value.messageInclusionProofBytes));
              writeTestNoritoField(
                  encoder, child -> writeTestNoritoRawByteVecSequence(child, value.inclusionBranch));
            }

            @Override
            public TestSccpSourceChainProofEnvelope decode(final NoritoDecoder decoder) {
              throw new UnsupportedOperationException("test source proof decoding is not used");
            }
          };

  private static byte[] testCanonicalSccpSourceProofBytes(
      final int sourceDomain, final SampleBundleFixture bundle) {
    final TronSccpProver.PublicInputsInput publicInputs = bundle.publicInputs;
    return NoritoCodec.encode(
        new TestSccpSourceChainProofEnvelope(
            sourceDomain,
            publicInputs.targetDomain(),
            testSourceChainKeyForDomain(sourceDomain),
            testSourceProofPlanForDomain(sourceDomain),
            testFinalityModelForDomain(sourceDomain),
            publicInputs.messageId(),
            publicInputs.payloadHash(),
            testSccpSourceEventDigest(
                sourceDomain,
                publicInputs.targetDomain(),
                publicInputs.messageId(),
                publicInputs.payloadHash()),
            publicInputs.commitmentRoot(),
            Long.parseLong(publicInputs.finalityHeight()),
            publicInputs.finalityBlockHash(),
            "0x" + repeat("55", 32),
            "0x" + repeat("66", 32),
            new byte[] {0x71, 0x72},
            new byte[] {0x73, 0x74},
            Collections.singletonList(filledBytes(32, 0x75))),
        TEST_SOURCE_CHAIN_PROOF_ENVELOPE_SCHEMA,
        TEST_SOURCE_CHAIN_PROOF_ADAPTER);
  }

  private static String testSccpSourceEventDigest(
      final int sourceDomain, final int targetDomain, final String messageId, final String payloadHash) {
    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    payload.write(1);
    writeTestU32Le(payload, sourceDomain);
    writeTestU32Le(payload, targetDomain);
    writeTestRawBytes(payload, hexBytes(stripHex(messageId)));
    writeTestRawBytes(payload, hexBytes(stripHex(payloadHash)));
    return "0x"
        + hexLower(
            Blake2b.digest256(
                concatTestBytes(
                    "sccp:source:event:v1".getBytes(StandardCharsets.UTF_8),
                    payload.toByteArray())));
  }

  private static String testSourceChainKeyForDomain(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return "eth";
    }
    if (domain == SourceSccpProofs.DOMAIN_BSC) {
      return "bsc";
    }
    if (domain == SolanaSccpProver.DOMAIN_SOLANA) {
      return "sol";
    }
    if (domain == TonSccpProver.DOMAIN_TON) {
      return "ton";
    }
    if (domain == TronSccpProver.DOMAIN_TRON) {
      return "tron";
    }
    throw new IllegalArgumentException("unsupported source domain");
  }

  private static int testSourceProofPlanForDomain(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return 1;
    }
    if (domain == SourceSccpProofs.DOMAIN_BSC) {
      return 2;
    }
    if (domain == SolanaSccpProver.DOMAIN_SOLANA) {
      return 3;
    }
    if (domain == TonSccpProver.DOMAIN_TON) {
      return 4;
    }
    if (domain == TronSccpProver.DOMAIN_TRON) {
      return 5;
    }
    throw new IllegalArgumentException("unsupported source domain");
  }

  private static int testFinalityModelForDomain(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return 0;
    }
    if (domain == SourceSccpProofs.DOMAIN_BSC) {
      return 1;
    }
    if (domain == SolanaSccpProver.DOMAIN_SOLANA) {
      return 2;
    }
    if (domain == TonSccpProver.DOMAIN_TON) {
      return 3;
    }
    if (domain == TronSccpProver.DOMAIN_TRON) {
      return 4;
    }
    throw new IllegalArgumentException("unsupported source domain");
  }

  private static void writeTestNoritoField(
      final NoritoEncoder encoder, final Consumer<NoritoEncoder> writePayload) {
    final NoritoEncoder child = encoder.childEncoder();
    writePayload.accept(child);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, true);
    encoder.writeBytes(payload);
  }

  private static void writeTestNoritoString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, true);
    encoder.writeBytes(bytes);
  }

  private static void writeTestNoritoRawByteVec(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeLength(value.length, false);
    encoder.writeBytes(value);
  }

  private static void writeTestNoritoRawByteVecSequence(
      final NoritoEncoder encoder, final List<byte[]> values) {
    encoder.writeLength(values.size(), false);
    for (final byte[] value : values) {
      final NoritoEncoder child = encoder.childEncoder();
      writeTestNoritoRawByteVec(child, value);
      final byte[] payload = child.toByteArray();
      encoder.writeLength(payload.length, true);
      encoder.writeBytes(payload);
    }
  }

  private static void writeTestBytes(final ByteArrayOutputStream out, final byte[] value) {
    writeTestU32Le(out, value.length);
    writeTestRawBytes(out, value);
  }

  private static void writeTestRawBytes(final ByteArrayOutputStream out, final byte[] value) {
    out.write(value, 0, value.length);
  }

  private static void writeTestU32Le(final ByteArrayOutputStream out, final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u32 test value must be non-negative");
    }
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeTestU64Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int index = 0; index < 8; index++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static void writeTestU128Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int index = 0; index < 16; index++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static byte[] concatTestBytes(final byte[] left, final byte[] right) {
    final byte[] out = Arrays.copyOf(left, left.length + right.length);
    System.arraycopy(right, 0, out, left.length, right.length);
    return out;
  }

  private static byte[] hexBytes(final String hex) {
    if ((hex.length() & 1) != 0) {
      throw new IllegalArgumentException("hex test value must have even length");
    }
    final byte[] out = new byte[hex.length() / 2];
    for (int index = 0; index < out.length; index++) {
      out[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
    }
    return out;
  }

  private static byte[] prefixedKeccakBytes(final String prefix, final byte[] payload) {
    final KeccakDigest digest = new KeccakDigest(256);
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    digest.update(prefixBytes, 0, prefixBytes.length);
    digest.update(payload, 0, payload.length);
    final byte[] out = new byte[32];
    digest.doFinal(out, 0);
    return out;
  }

  private static SourceSccpProofs.TronDestinationBinding sampleDestinationBinding(
      final TronSccpProver.PublicInputsInput publicInputs) {
    return SourceSccpProofs.tronDestinationBinding(
        TronSccpProver.DOMAIN_SORA,
        publicInputs.targetDomain(),
        "0x" + repeat("33", 32),
        "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        "0x" + repeat("bb", 32),
        "0x" + repeat("cc", 32));
  }

  private static TronSccpProver.RouteCanaryEvidenceInput sampleTronRouteCanaryEvidence() {
    return sampleTronRouteCanaryEvidence(null, null, null, true, true, null, true, null);
  }

  private static TronSccpProver.RouteCanaryEvidenceInput sampleTronRouteCanaryEvidence(
      final String routeAllowlistHash,
      final String destinationBindingHash,
      final String blockNumber,
      final boolean usedMessageProof,
      final boolean rawDataOwnerMatchesTransaction,
      final String signatureRecoveredAddress,
      final boolean signatureRecoversToOwner,
      final String routeCanaryEvidenceHash) {
    final SourceSccpProofs.TronDestinationBinding binding =
        sampleDestinationBinding(samplePublicInputs());
    return new TronSccpProver.RouteCanaryEvidenceInput(
        routeAllowlistHash == null
            ? "0xfea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a"
            : routeAllowlistHash,
        destinationBindingHash == null ? binding.hash : destinationBindingHash,
        null,
        "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
        "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
        binding.networkId,
        binding.verifierAddress,
        binding.verifierCodeHash,
        binding.verifierKeyHash,
        TronSccpProver.DOMAIN_SORA,
        TronSccpProver.DOMAIN_TRON,
        "0x" + repeat("fa", 32),
        "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
        blockNumber == null ? "234" : blockNumber,
        "567000",
        0,
        "0x" + repeat("dd", 32),
        "0xf96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951",
        "0x" + repeat("ab", 32),
        "0x" + repeat("ee", 32),
        "0x" + repeat("00", 31) + "7b",
        "0x" + repeat("cd", 32),
        "0x" + repeat("f1", 32),
        1,
        TronSccpProver.DOMAIN_SORA,
        usedMessageProof,
        rawDataOwnerMatchesTransaction,
        "0x" + repeat("c4", 32),
        signatureRecoveredAddress == null
            ? "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf"
            : signatureRecoveredAddress,
        signatureRecoversToOwner,
        routeCanaryEvidenceHash == null
            ? "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56"
            : routeCanaryEvidenceHash);
  }

  private static TronSccpProver.RouteCanaryEvidenceInput sampleTronRouteCanaryEvidenceWithGovernedHashes(
      final String routeAllowlistHash,
      final String destinationBindingHash,
      final String sourceVerifierMaterialHash,
      final String sourceAdapterEngineDeploymentHash) {
    final SourceSccpProofs.TronDestinationBinding binding =
        sampleDestinationBinding(samplePublicInputs());
    return new TronSccpProver.RouteCanaryEvidenceInput(
        routeAllowlistHash == null
            ? "0xfea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a"
            : routeAllowlistHash,
        destinationBindingHash == null ? binding.hash : destinationBindingHash,
        null,
        sourceVerifierMaterialHash == null
            ? "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8"
            : sourceVerifierMaterialHash,
        sourceAdapterEngineDeploymentHash == null
            ? "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8"
            : sourceAdapterEngineDeploymentHash,
        binding.networkId,
        binding.verifierAddress,
        binding.verifierCodeHash,
        binding.verifierKeyHash,
        TronSccpProver.DOMAIN_SORA,
        TronSccpProver.DOMAIN_TRON,
        "0x" + repeat("fa", 32),
        "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
        "234",
        "567000",
        0,
        "0x" + repeat("dd", 32),
        "0xf96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951",
        "0x" + repeat("ab", 32),
        "0x" + repeat("ee", 32),
        "0x" + repeat("00", 31) + "7b",
        "0x" + repeat("cd", 32),
        "0x" + repeat("f1", 32),
        1,
        TronSccpProver.DOMAIN_SORA,
        true,
        true,
        "0x" + repeat("c4", 32),
        "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
        true,
        "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56");
  }

  private static TronSccpProver.PublicInputsInput samplePublicInputs() {
    return samplePublicInputs(null, TronSccpProver.DOMAIN_TRON);
  }

  private static TronSccpProver.PublicInputsInput samplePublicInputs(final String payloadHash) {
    return samplePublicInputs(payloadHash, TronSccpProver.DOMAIN_TRON);
  }

  private static TronSccpProver.PublicInputsInput samplePublicInputs(
      final String payloadHash, final int targetDomain) {
    return samplePublicInputs(payloadHash, targetDomain, "19");
  }

  private static TronSccpProver.PublicInputsInput samplePublicInputs(
      final String payloadHash, final int targetDomain, final String finalityHeight) {
    final TronSccpProver.PublicInputsInput fixture = sampleBundleFixture().publicInputs;
    return new TronSccpProver.PublicInputsInput(
        1,
        fixture.messageId(),
        payloadHash == null ? fixture.payloadHash() : payloadHash,
        targetDomain,
        fixture.commitmentRoot(),
        finalityHeight,
        fixture.finalityBlockHash());
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }

  private static void assertThrows(final Runnable operation, final String messageFragment) {
    boolean threw = false;
    try {
      operation.run();
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains(messageFragment);
    }
    assert threw : "expected exception containing " + messageFragment;
  }
}
