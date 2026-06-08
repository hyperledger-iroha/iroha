package org.hyperledger.iroha.android.sccp;

import com.sun.net.httpserver.HttpServer;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class EvmSccpProverTests {
  private static final String ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS =
      "0x" + repeat("ff", 42) + "3f" + repeat("00", 21);
  private static final String ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION = "342";
  private static final List<String> ETHEREUM_FINALITY_BRANCH = ethereumFinalityBranch();
  private static final String BEACON_HEADER_ROOT_SLOT_64 =
      "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c";

  private EvmSccpProverTests() {}

  public static void main(final String[] args) throws Exception {
    proofRequestBindsPublicSignalsAndRelayContext();
    proverRequiresLinkedProofEngine();
    proverWrapsExternalProofBytes();
    proverResolvesWitnessProviderBeforeBuildingRequest();
    rejectsMalformedGroth16ProofTuple();
    buildsContractCallSubmission();
    bscMainnetFacadeRequiresChainId56AndBscTarget();
    bscMainnetFacadeBuildsLocalAdmissionSubmission();
    ethereumMainnetFacadeRequiresChainId1AndEthTarget();
    ethereumMainnetInboundProverReceivesCallbackEvidenceSnapshot();
    ethereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary();
    bscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary();
    ethereumReceiptTrieProofBuilderUsesRlpTransactionIndexKeys();
    ethereumInboundCollectionBuildsReceiptProofFromBlockReceipts();
    ethereumMainnetFacadeBuildsLocalAdmissionSubmission();
    ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence();
    ethereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp();
    ethereumMainnetBeaconRestHttpTransportRejectsOversizedBodies();
    ethereumMainnetBeaconRestConsensusProviderRejectsUnsafeFinality();
    bscMainnetInboundFacadeUsesMainnetRpcAndRejectsDrift();
    mainnetFacadesSnapshotWitnessProviderInputs();
    System.out.println("[IrohaAndroid] EVM-family SCCP prover tests passed.");
  }

  private static void proofRequestBindsPublicSignalsAndRelayContext() {
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    assert EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())
        : "backend must be EVM-family Groth16";
    assert request.sourceDomain() == SolanaSccpProver.DOMAIN_SORA : "source domain must be SORA";
    assert request.targetDomain() == EvmSccpProver.DOMAIN_ETH : "target domain must be Ethereum";
    assert request.publicSignalWords().size() == 9 : "request must expose nine public signals";
    assert "0x2eb6b5dbab56255a979f433862429637ba1e8251106271606f0a279f593d7a39"
        .equals(request.publicSignalWords().get(2)) : "target-domain signal must bind Ethereum";
    assert ("0x" + repeat("56", 32)).equals(request.statementHash())
        : "statement hash must be normalized";
    assert ("0x" + repeat("78", 32)).equals(request.destinationBindingHash())
        : "destination binding hash must be normalized";
    assert "0xfb990c2ffdf826c9beb0e74105b060af467570720a1382b48abc42d32850f5ea"
        .equals(request.requestHash()) : "request hash must bind EVM proof material";
    final EvmSccpProver.ProofRequest callbackSnapshot =
        EvmSccpProver.callbackRequestSnapshot(request);
    assert callbackSnapshot != request : "EVM proof engine must receive a request snapshot";
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
    snapshotSourceProof[0] = 77;
    assert Arrays.equals(new byte[] {5, 6, 7}, callbackSnapshot.bundleBytes())
        : "snapshot bundle bytes must be defensive copies";
    assert Arrays.equals(new byte[] {9, 10}, callbackSnapshot.sourceProofBytes())
        : "snapshot source proof bytes must be defensive copies";

    final SourceSccpProofs.EvmDestinationBinding destinationBinding =
        sampleDestinationBinding(samplePublicInputs(EvmSccpProver.DOMAIN_ETH));
    final EvmSccpProver.ProofRequest boundRequest =
        EvmSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[] {5, 6, 7},
                new byte[] {9, 10},
                repeat("56", 32),
                destinationBinding));
    assert destinationBinding.hash.equals(boundRequest.destinationBindingHash())
        : "destination binding object constructor must thread the derived hash";
    assert destinationBinding == boundRequest.destinationBinding()
        : "bound request must carry destination binding deployment material";
    assert !request.requestHash().equals(boundRequest.requestHash())
        : "request hash must bind the derived destination binding";

    final EvmSccpProver.ProofRequest bscRequest =
        EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_BSC), new byte[] {9, 10}, repeat("56", 32)));
    assert bscRequest.targetDomain() == EvmSccpProver.DOMAIN_BSC : "target domain must support BSC";
    assert !request.publicSignalWords().get(2).equals(bscRequest.publicSignalWords().get(2))
        : "target-domain signal must distinguish ETH and BSC";
    assert !request.requestHash().equals(bscRequest.requestHash())
        : "request hash must distinguish ETH and BSC targets";
    final EvmSccpProver.ProofRequest shiftedSplitRequest =
        EvmSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[] {5, 6, 7, 9},
                new byte[] {10},
                repeat("56", 32),
                repeat("78", 32),
                EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
                SolanaSccpProver.DOMAIN_SORA));
    assert !request.requestHash().equals(shiftedSplitRequest.requestHash())
        : "request hash must distinguish shifted EVM bundle/proof splits";

    boolean threw = false;
    final EvmSccpProver.ProofRequest artifactRequest =
        EvmSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[] {5, 6, 7},
                new byte[] {9, 10},
                repeat("56", 32),
                repeat("78", 32),
                EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
                SolanaSccpProver.DOMAIN_SORA,
                null,
                repeat("91", 32),
                repeat("92", 32)));
    assert ("0x" + repeat("91", 32)).equals(artifactRequest.proofArtifactHash())
        : "proof artifact hash must be normalized";
    assert ("0x" + repeat("92", 32)).equals(artifactRequest.provingKeyHash())
        : "proving key hash must be normalized";
    assert !request.requestHash().equals(artifactRequest.requestHash())
        : "request hash must bind proof artifact metadata";
    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[] {5, 6, 7},
              new byte[] {9, 10},
              repeat("56", 32),
              repeat("78", 32),
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              SolanaSccpProver.DOMAIN_SORA,
              null,
              repeat("91", 32),
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofArtifactHash and provingKeyHash");
    }
    assert threw : "partial proof artifact metadata must be rejected";
    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[] {5, 6, 7},
              new byte[] {9, 10},
              repeat("56", 32),
              repeat("78", 32),
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              SolanaSccpProver.DOMAIN_SORA,
              null,
              repeat("00", 32),
              repeat("92", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofArtifactHash");
    }
    assert threw : "zero proof artifact hash must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], ""));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("statementHash");
    }
    assert threw : "missing statement hash must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH, "0"),
              new byte[0],
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalityHeight");
    }
    assert threw : "zero finality height must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              new EvmSccpProver.PublicInputsInput(
                  1,
                  repeat("11", 32),
                  " " + repeat("22", 32),
                  EvmSccpProver.DOMAIN_ETH,
                  repeat("33", 32),
                  "19",
                  repeat("44", 32)),
              new byte[0],
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("payloadHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded EVM payload hash must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32) + " "));
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("statementHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded EVM statement hash must be rejected";

    for (final String finalityHeight : new String[] {"019", "0x13", "+19", " 19", "19 "}) {
      threw = false;
      try {
        EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH, finalityHeight),
                new byte[0],
                repeat("56", 32)));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("finalityHeight");
      }
      assert threw : "noncanonical EVM finality height must be rejected";
    }

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              EvmSccpProver.DOMAIN_ETH));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "non-SORA source domains must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "non-SORA source domains must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(TonSccpProver.DOMAIN_TON),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("publicInputs.targetDomain must be ETH or BSC");
    }
    assert threw : "non-EVM-family target domains must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("00", 32),
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "zero destination binding hash must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes");
    }
    assert threw : "empty EVM bundle bytes must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[] {0, 0},
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must not be all zero");
    }
    assert threw : "all-zero EVM source proof bytes must be rejected";

    final byte[] oversizedSourceProof =
        new byte[EvmSccpProver.SOURCE_STATE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedSourceProof, (byte) 1);
    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              oversizedSourceProof,
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must be at most");
    }
    assert threw : "oversized EVM source proof bytes must be rejected";
    assert EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[0],
                repeat("56", 32)))
        .sourceProofBytes()
        .length == 0 : "empty optional EVM source proof bytes must remain valid";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              "debug-evm-backend",
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("evm-groth16-bn254-v1");
    }
    assert threw : "wrong EVM proof backend must be rejected";

    final SourceSccpProofs.EvmDestinationBinding bscDestinationBinding =
        SourceSccpProofs.evmDestinationBinding(
            EvmSccpProver.DOMAIN_SORA,
            EvmSccpProver.DOMAIN_BSC,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EvmSccpProver.ProofRequestInput(
          samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
          new byte[] {5, 6, 7},
          repeat("56", 32),
          bscDestinationBinding);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBinding.targetDomain");
    }
    assert threw : "destination binding target must match EVM public inputs";
  }

  private static void proverRequiresLinkedProofEngine() {
    boolean threw = false;
    try {
      new EvmSccpProver()
          .prove(sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("not linked");
    }
    assert threw : "expected missing local prover to throw";
  }

  private static void proverWrapsExternalProofBytes() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final EvmSccpProver.ProofRequest[] seenRequests = new EvmSccpProver.ProofRequest[2];
    final int[] seenRequestCount = new int[] {0};
    final EvmSccpProver prover =
        new EvmSccpProver(
            null,
            request -> {
              seenRequests[seenRequestCount[0]++] = request;
              assert EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())
                  : "backend must be EVM-family Groth16";
              assert request.targetDomain() == EvmSccpProver.DOMAIN_ETH
                  : "target domain must be Ethereum";
              assert request.publicSignalWords().size() == 9 : "request must carry public signals";
              return proofBytes;
            });

    final EvmSccpProver.ProofResult result =
        prover.prove(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    final EvmSccpProver.ProofResult omittedSourceResult =
        prover.prove(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    assert Arrays.equals(proofBytes, result.proofBytes())
        : "proof bytes must be preserved";
    assert Arrays.equals(new byte[0], omittedSourceResult.sourceProofBytes())
        : "EVM production proofs may omit source proof bytes";
    assert !result.proofBase64().isEmpty() : "proof base64 must be exposed";
    assert ("0x" + repeat("56", 32)).equals(result.statementHash())
        : "result must expose statement hash";
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    final EvmSccpProver.ProofRequest omittedSourceRequest =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    assert request.destinationBindingHash().equals(result.destinationBindingHash())
        : "result must expose destination binding hash";
    assert request.destinationBinding().hash.equals(result.destinationBinding().hash)
        : "result must carry destination binding deployment material";
    assert request.requestHash().equals(result.requestHash()) : "result must expose the request hash";
    assert result.envelopeHash().matches("0x[0-9a-f]{64}")
        : "result must bind proof bytes to the EVM request";
    final EvmSccpProver.ProofRequest artifactRequest =
        EvmSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[] {5, 6, 7},
                new byte[] {9, 10},
                repeat("56", 32),
                sampleDestinationBinding(samplePublicInputs(EvmSccpProver.DOMAIN_ETH)),
                EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
                SolanaSccpProver.DOMAIN_SORA,
                repeat("91", 32),
                repeat("92", 32)));
    final EvmSccpProver.ProofResult artifactResult =
        EvmSccpProver.wrapProofResult(proofBytes, artifactRequest);
    assert artifactRequest.proofArtifactHash().equals(artifactResult.proofArtifactHash())
        : "proof result must carry proof artifact hash";
    assert artifactRequest.provingKeyHash().equals(artifactResult.provingKeyHash())
        : "proof result must carry proving key hash";
    assert !artifactRequest.requestHash().equals(request.requestHash())
        : "artifact-bound request hash must differ";

    assert seenRequestCount[0] == 2 : "proof engine must receive both EVM callback requests";
    assert seenRequests[0] != request : "EVM proof engine must receive a request snapshot";
    assert seenRequests[0].requestHash().equals(request.requestHash())
        : "EVM callback snapshot must match the canonical request hash";
    assert Arrays.equals(seenRequests[0].bundleBytes(), request.bundleBytes())
        : "EVM callback snapshot must copy bundle bytes";
    assert Arrays.equals(seenRequests[0].sourceProofBytes(), request.sourceProofBytes())
        : "EVM callback snapshot must copy source proof bytes";
    assert seenRequests[1] != omittedSourceRequest
        : "EVM proof engine must receive an omitted-source request snapshot";
    assert seenRequests[1].requestHash().equals(omittedSourceRequest.requestHash())
        : "EVM omitted-source callback snapshot must match canonical request";
    boolean threw = false;
    try {
      EvmSccpProver.wrapProofResult(new byte[] {0, 0}, request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "EVM proof result wrapper must reject all-zero proof bytes";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(new byte[] {1, 2, 3, 4}, request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("384 bytes");
    }
    assert threw : "EVM proof result wrapper must reject non-canonical proof lengths";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(
          new byte[] {1}, evmRequestWithBackend(request, "debug-evm-backend"));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("evm-groth16-bn254-v1");
    }
    assert threw : "EVM proof result wrapper must reject wrong backends";

    boolean canonicalThrew = false;
    try {
      EvmSccpProver.wrapProofResult(
          proofBytes, evmRequestWithRequestHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      canonicalThrew = ex.getMessage().contains("canonical");
    }
    assert canonicalThrew : "EVM proof result wrapper must reject non-canonical requests";

    boolean missingBindingThrew = false;
    try {
      EvmSccpProver.wrapProofResult(
          proofBytes,
          EvmSccpProver.buildProofRequest(
              sampleProofRequestInput(
                  samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32))));
    } catch (final IllegalArgumentException ex) {
      missingBindingThrew = ex.getMessage().contains("destinationBinding");
    }
    assert missingBindingThrew : "EVM proof result wrapper must reject hash-only destination bindings";

    final byte[] exposedProof = result.proofBytes();
    exposedProof[0] = 9;
    assert Arrays.equals(proofBytes, result.proofBytes())
        : "EVM proof result bytes must be defensive copies";

    final ArrayList<String> mutableSignals = new ArrayList<>(result.publicSignalWords());
    final EvmSccpProver.ProofResult manualResult =
        new EvmSccpProver.ProofResult(
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
        : "EVM proof result public signals must be construction snapshots";
    boolean immutableSignals = false;
    try {
      manualResult.publicSignalWords().set(0, "0x" + repeat("88", 32));
    } catch (final UnsupportedOperationException ex) {
      immutableSignals = true;
    }
    assert immutableSignals : "EVM proof result public signals must be immutable";
  }

  private static void proverResolvesWitnessProviderBeforeBuildingRequest() {
    final boolean[] resolved = new boolean[] {false};
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final byte[] bundleBytes = new byte[] {5, 6, 7};
    final EvmSccpProver.ProofRequestInput userInput =
        sampleProductionProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH), bundleBytes, new byte[0], repeat("56", 32));
    final EvmSccpProver prover =
        new EvmSccpProver(
            input -> {
              assert Arrays.equals(new byte[0], input.sourceProofBytes())
                  : "UI witness provider should receive unresolved request input";
              assert input.bundleBytes() != bundleBytes
                  : "UI witness provider must receive a byte snapshot";
              input.bundleBytes()[0] = 0x7f;
              resolved[0] = true;
              return sampleProductionProofRequestInput(
                  input.publicInputs(),
                  new byte[] {9, 10},
                  input.statementHash());
            },
            request -> {
              assert resolved[0] : "witness provider must run before proof engine";
              assert Arrays.equals(new byte[] {9, 10}, request.sourceProofBytes())
                  : "proof engine must receive provider-resolved source proof bytes";
              return proofBytes;
            });

    final EvmSccpProver.ProofResult result = prover.prove(userInput);

    assert Arrays.equals(new byte[] {9, 10}, result.sourceProofBytes())
        : "wrapped result must preserve provider-resolved source proof bytes";
    assert Arrays.equals(new byte[] {5, 6, 7}, userInput.bundleBytes())
        : "UI-owned EVM bundle bytes must not be mutated by witness provider";
    assert Arrays.equals(new byte[] {5, 6, 7}, bundleBytes)
        : "UI-owned EVM bundle array must not be mutated by witness provider";
  }

  private static void rejectsMalformedGroth16ProofTuple() {
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));

    boolean threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(0, abiWord(2)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.version");
    }
    assert threw : "EVM proof wrapper must reject non-v1 Groth16 ABI tuples";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(4, repeatedWord(0xff)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("BN254 base-field");
    }
    assert threw : "EVM proof wrapper must reject out-of-range BN254 coordinates";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytesWithZeroB(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.b");
    }
    assert threw : "EVM proof wrapper must reject zero Groth16 B points";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(11, abiWord(3)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.c");
    }
    assert threw : "EVM proof wrapper must reject off-curve Groth16 C points";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytesWithNonSubgroupB(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.b");
    }
    assert threw : "EVM proof wrapper must reject non-subgroup Groth16 B points";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(1, repeatedWord(0x22)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("messageId must match");
    }
    assert threw : "EVM proof wrapper must reject message-id mismatches";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(2, abiWord(999)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must match");
    }
    assert threw : "EVM proof wrapper must reject source-domain mismatches";

    threw = false;
    try {
      EvmSccpProver.submitMessageProofCallData(
          sampleGroth16ProofBytes(2, abiWord(EvmSccpProver.DOMAIN_ETH)),
          samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
          repeat("56", 32));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must match");
    }
    assert threw : "EVM direct calldata helper must reject source-domain mismatches";

    threw = false;
    try {
      EvmSccpProver.submitMessageProofCallData(
          sampleGroth16ProofBytes(2, abiWord(EvmSccpProver.DOMAIN_ETH)),
          samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
          repeat("56", 32),
          EvmSccpProver.DOMAIN_ETH);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "EVM direct calldata helper must reject non-SORA source domains";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              sampleGroth16ProofBytes(3, repeatedWord(0x44)),
              repeat("56", 32),
              repeat("78", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("commitmentRoot must match");
    }
    assert threw : "EVM submission builder must reject commitment-root mismatches";
  }

  private static void buildsContractCallSubmission() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    final EvmSccpProver.ProofResult proofResult =
        EvmSccpProver.wrapProofResult(proofBytes, request);
    final EvmSccpProver.Submission submission =
        EvmSccpProver.buildSubmission(new EvmSccpProver.SubmissionInput(proofResult));

    assert "contract_call".equals(submission.submissionKind())
        : "EVM submission kind must be contract_call";
    assert "evm_groth16_contract_call".equals(submission.platformPayload())
        : "EVM submission platform payload must identify contract calls";
    assert EvmSccpProver.CONTRACT_CALL_ABI_TUPLE_V1.equals(submission.envelopeEncoding())
        : "EVM envelope encoding must be ABI tuple v1";
    assert EvmSccpProver.SUBMIT_MESSAGE_PROOF_SELECTOR_V1.equals(submission.functionSelector())
        : "EVM function selector must be exposed";
    assert submission.callDataHex().startsWith(EvmSccpProver.SUBMIT_MESSAGE_PROOF_SELECTOR_V1)
        : "call data must start with the selector";
    assert submission.callData().length == 676 : "EVM call data length must be ABI canonical";
    assert ("0x" + repeat("00", 30) + "0100")
        .equals("0x" + hexLower(Arrays.copyOfRange(submission.callData(), 4, 36)))
        : "dynamic proof offset must be 0x100";
    assert ("0x" + repeat("00", 30) + "0180")
        .equals("0x" + hexLower(Arrays.copyOfRange(submission.callData(), 260, 292)))
        : "proof length must be 384 bytes";
    assert EvmSccpProver.messageTransparentPublicInputAbiWords(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH))
        .equals(submission.publicInputWords()) : "public input ABI words must be exposed";
    assert proofResult.publicSignalWords().equals(submission.publicSignalWords())
        : "public signal words must be carried";
    assert Arrays.equals(new byte[] {5, 6, 7}, proofResult.bundleBytes())
        : "proof results must retain request bundle bytes";
    assert Arrays.equals(new byte[] {9, 10}, proofResult.sourceProofBytes())
        : "proof results must retain source proof bytes";
    assert Arrays.equals(proofBytes, submission.proofBytes()) : "proof bytes must be preserved";
    assert Arrays.equals(submission.callData(), submission.envelopeBytes())
        : "EVM envelope bytes must equal call data";
    assert Arrays.equals(
            submission.callData(),
            EvmSccpProver.submitMessageProofCallData(
                proofBytes, proofResult.publicInputs(), proofResult.statementHash()))
        : "direct calldata helper must match submission call data";
    final SourceSccpProofs.EvmDestinationBinding destinationBinding =
        sampleDestinationBinding(proofResult.publicInputs());
    final EvmSccpProver.Submission bindingSubmission =
        EvmSccpProver.buildSubmission(
            new EvmSccpProver.SubmissionInput(
                proofResult.publicInputs(),
                proofBytes,
                proofResult.statementHash(),
                destinationBinding));
    assert destinationBinding.hash.equals(bindingSubmission.destinationBindingHash())
        : "EVM submission input must accept a derived destination binding";

    final EvmSccpProver.ProofResult omittedSourceProofResult =
        EvmSccpProver.wrapProofResult(
            proofBytes,
            EvmSccpProver.buildProofRequest(
                sampleProductionProofRequestInput(
                    samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32))));
    final EvmSccpProver.Submission omittedSourceSubmission =
        EvmSccpProver.buildSubmission(
            new EvmSccpProver.SubmissionInput(omittedSourceProofResult));
    assert Arrays.equals(new byte[0], omittedSourceProofResult.sourceProofBytes())
        : "EVM submit-ready proof results may omit source proof bytes";
    assert Arrays.equals(proofBytes, omittedSourceSubmission.proofBytes())
        : "EVM omitted-source submission must preserve proof bytes";

    final byte[] exposedCallData = submission.callData();
    exposedCallData[0] = 0;
    assert submission.callData()[0] != 0 : "submission call data must be a defensive copy";

    final byte[] proofMismatch = Arrays.copyOf(proofBytes, proofBytes.length);
    proofMismatch[4 * 32 + 31] = 9;
    boolean threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              proofResult.publicInputs(),
              proofMismatch,
              proofResult.statementHash(),
              proofResult.destinationBindingHash(),
              SolanaSccpProver.DOMAIN_SORA,
              proofResult,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes");
    }
    assert threw : "submission must reject proof bytes that differ from the wrapped result";

    final SourceSccpProofs.EvmDestinationBinding bscDestinationBinding =
        SourceSccpProofs.evmDestinationBinding(
            EvmSccpProver.DOMAIN_SORA,
            EvmSccpProver.DOMAIN_BSC,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EvmSccpProver.SubmissionInput(
          proofResult.publicInputs(),
          proofBytes,
          proofResult.statementHash(),
          bscDestinationBinding);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBinding.targetDomain");
    }
    assert threw : "submission destination binding target must match EVM public inputs";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              evmResultWithEnvelopeHash(proofResult, "0x" + repeat("aa", 32))));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("wrapped proof bytes");
    }
    assert threw : "submission must reject tampered wrapped proof-result envelope hashes";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(evmResultWithProofBase64(proofResult, "AAAA")));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBase64");
    }
    assert threw : "submission must reject tampered wrapped proof-result proofBase64";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              evmResultWithBundleBytes(proofResult, new byte[] {5, 6, 8})));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requestHash");
    }
    assert threw : "submission must reject stale wrapped proof-result request context";

    final ArrayList<String> mismatchedSignals = new ArrayList<>(proofResult.publicSignalWords());
    mismatchedSignals.set(0, "0x" + repeat("99", 32));
    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              proofResult.publicInputs(),
              proofBytes,
              proofResult.statementHash(),
              proofResult.destinationBindingHash(),
              SolanaSccpProver.DOMAIN_SORA,
              null,
              mismatchedSignals));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("publicSignalWords");
    }
    assert threw : "submission must reject public-signal mismatches";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              samplePublicInputs(TonSccpProver.DOMAIN_TON),
              proofBytes,
              proofResult.statementHash(),
              proofResult.destinationBindingHash()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("ETH or BSC");
    }
    assert threw : "submission must reject non-EVM-family target domains";
  }

  private static void bscMainnetFacadeRequiresChainId56AndBscTarget() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final SourceSccpProofs.EvmDestinationBinding binding =
        BscSccpProver.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    assert SourceSccpProofs.BSC_MAINNET_NETWORK_ID.equals(binding.networkId)
        : "BSC binding must default to chain id 56";
    assert binding.targetDomain == EvmSccpProver.DOMAIN_BSC
        : "BSC binding must target BSC";
    assert binding.hash.equals(
            BscSccpProver.destinationBindingHash(
                "0x" + repeat("11", 20),
                "0x" + repeat("22", 20),
                "0x" + repeat("bb", 32),
                "0x" + repeat("cc", 32)))
        : "BSC binding hash helper must match binding";

    final EvmSccpProver.ProofRequest request =
        BscSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
                new byte[] {5, 6, 7},
                new byte[] {9, 10},
                repeat("56", 32),
                binding));
    assert request.targetDomain() == EvmSccpProver.DOMAIN_BSC
        : "BSC request must target BSC";
    assert binding.hash.equals(request.destinationBindingHash())
        : "BSC request must bind the BSC destination binding";

    final EvmSccpProver.ProofResult result =
        BscSccpProver.wrapProofResult(proofBytes, request);
    boolean threw = false;
    try {
      BscSccpProver.wrapProofResult(
          proofBytes, evmRequestWithDestinationBindingHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "BSC wrapProofResult must reject forged destinationBindingHash";
    final EvmSccpProver.Submission submission =
        BscSccpProver.buildSubmission(new EvmSccpProver.SubmissionInput(result));
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_BSC
        : "BSC submission must target BSC";
    assert Arrays.equals(proofBytes, submission.proofBytes())
        : "BSC submission must preserve proof bytes";
    final Object submitted =
        new BscMainnetSccp(
                null,
                null,
                null,
                null,
                null,
                null,
                outboundSubmission -> {
                  assert outboundSubmission.targetDomain() == EvmSccpProver.DOMAIN_BSC
                      : "BSC outbound submitter must receive BSC calldata";
                  assert Arrays.equals(proofBytes, outboundSubmission.proofBytes())
                      : "BSC outbound submitter must receive proof bytes";
                  assert binding.hash.equals(outboundSubmission.destinationBindingHash())
                      : "BSC outbound submitter must receive the bound destination hash";
                  return "bsc-submitted";
                })
            .submitOutboundToBsc(new EvmSccpProver.SubmissionInput(result));
    assert "bsc-submitted".equals(submitted)
        : "BSC outbound submitter must return app-owned submission result";
    threw = false;
    try {
      new BscMainnetSccp().submitOutboundToBsc(new EvmSccpProver.SubmissionInput(result));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("outbound submitter");
    }
    assert threw : "BSC outbound submission requires an app-owned submitter";

    threw = false;
    try {
      BscSccpProver.destinationBinding(
          "0x" + repeat("11", 20),
          "0x" + repeat("22", 20),
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32),
          "0x" + repeat("33", 32));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("chain id 56");
    }
    assert threw : "BSC destination helper must reject non-mainnet network ids";

    final SourceSccpProofs.EvmDestinationBinding wrongNetworkBinding =
        SourceSccpProofs.evmDestinationBinding(
            EvmSccpProver.DOMAIN_SORA,
            EvmSccpProver.DOMAIN_BSC,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    threw = false;
    try {
      BscSccpProver.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
              new byte[] {5, 6, 7},
              new byte[0],
              repeat("56", 32),
              wrongNetworkBinding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("chain id 56");
    }
    assert threw : "BSC request helper must reject non-mainnet destination bindings";

    threw = false;
    try {
      BscSccpProver.buildProofRequest(
          sampleProductionProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("target BSC");
    }
    assert threw : "BSC request helper must reject ETH public inputs";
  }

  private static void bscMainnetFacadeBuildsLocalAdmissionSubmission() {
    final BscMainnetSccp.LocalAdmissionSubmissionInput input =
        new BscMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32));
    final BscMainnetSccp.LocalAdmissionSubmission submission =
        BscMainnetSccp.buildLocalAdmissionSubmission(input);
    final BscMainnetSccp.LocalAdmissionSubmission facadeSubmission =
        new BscMainnetSccp().buildLocalAdmission(input);

    assert BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1.equals(submission.platformPayload())
        : "BSC local admission platform payload must be local_admission";
    assert BscMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1.equals(submission.envelopeEncoding())
        : "BSC local admission must use the Norito envelope";
    assert BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1.equals(submission.verifierEntrypoint())
        : "BSC local admission must target SubmitBridgeProof";
    assert submission.sourceDomain() == EvmSccpProver.DOMAIN_BSC
        : "BSC local admission source must be BSC";
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_SORA
        : "BSC local admission target must be SORA";
    assert submission.arguments().isEmpty() : "BSC local admission must not add call arguments";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "BSC local admission must copy proof bytes";
    assert Arrays.equals(new byte[] {4, 5, 6}, submission.publicInputsBytes())
        : "BSC local admission must copy public input bytes";
    assert Arrays.equals(new byte[] {7, 8, 9}, submission.bundleBytes())
        : "BSC local admission must copy bundle bytes";
    assert Arrays.equals(new byte[] {10, 11, 12}, submission.envelopeBytes())
        : "BSC local admission must copy envelope bytes";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.localAdmission().proofBytes())
        : "BSC local admission payload must carry proof bytes";
    assert submission.envelopeHex().equals(facadeSubmission.envelopeHex())
        : "facade local admission helper must match static helper";

    input.proofBytes()[0] = 99;
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "BSC local admission must not expose mutable proof storage";

    boolean threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              BscMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("BSC -> SORA");
    }
    assert threw : "BSC local admission must reject wrong source domains";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {0, 0},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be all zero");
    }
    assert threw : "BSC local admission must reject all-zero proof bytes";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[0],
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeBytes must not be empty");
    }
    assert threw : "BSC local admission must reject empty envelope bytes";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              BscMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              "abi_tuple_v1",
              BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeEncoding");
    }
    assert threw : "BSC local admission must reject stale metadata";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              "debug-proof-family",
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofFamily");
    }
    assert threw : "BSC local admission must reject stale proof families";
  }

  private static void ethereumMainnetFacadeRequiresChainId1AndEthTarget() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    EthereumMainnetSccp.requireMainnetChainId(1L);
    assert "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727"
            .equals(EthereumMainnetSccp.sourceEventTopic())
        : "Ethereum source-event topic must bind SccpSourceEvent(bytes32)";
    boolean threw = false;
    try {
      EthereumMainnetSccp.requireMainnetChainId(56L);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 1");
    }
    assert threw : "Ethereum mainnet chain guard must reject BSC chain id";

    final SourceSccpProofs.EvmDestinationBinding binding =
        EthereumMainnetSccp.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    assert SourceSccpProofs.ETH_MAINNET_NETWORK_ID.equals(binding.networkId)
        : "Ethereum binding must default to chain id 1";
    assert binding.sourceDomain == EvmSccpProver.DOMAIN_SORA
        : "Ethereum binding must start from SORA";
    assert binding.targetDomain == EvmSccpProver.DOMAIN_ETH
        : "Ethereum binding must target ETH";
    assert binding.hash.equals(
            EthereumMainnetSccp.destinationBindingHash(
                "0x" + repeat("11", 20),
                "0x" + repeat("22", 20),
                "0x" + repeat("bb", 32),
                "0x" + repeat("cc", 32)))
        : "Ethereum binding hash helper must match binding";

    final EvmSccpProver.ProofRequestInput input =
        new EvmSccpProver.ProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
            new byte[] {5, 6, 7},
            new byte[] {9, 10},
            repeat("56", 32),
            binding);
    final EvmSccpProver.ProofRequest request = EthereumMainnetSccp.buildProofRequest(input);
    assert request.targetDomain() == EvmSccpProver.DOMAIN_ETH
        : "Ethereum request must target ETH";
    assert binding.hash.equals(request.destinationBindingHash())
        : "Ethereum request must bind the Ethereum destination binding";
    final EvmSccpProver.ProofRequest instanceRequest =
        new EthereumMainnetSccp().buildOutboundProofRequest(input);
    assert request.requestHash().equals(instanceRequest.requestHash())
        : "Ethereum facade request builder must use the static helper request hash";
    assert Arrays.equals(request.bundleBytes(), instanceRequest.bundleBytes())
        : "Ethereum facade request builder must preserve bundle bytes";
    assert Arrays.equals(request.sourceProofBytes(), instanceRequest.sourceProofBytes())
        : "Ethereum facade request builder must preserve source proof bytes";
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle =
        sampleEthereumNativeEvmProverBundle(binding.hash, true, false);
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle parsedNativeProverBundle =
        EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
            sampleEthereumNativeEvmProverBundleJson(binding.hash), binding.hash);
    assert nativeProverBundle.proofArtifactHash().equals(parsedNativeProverBundle.proofArtifactHash())
        : "Ethereum native prover bundle parser must preserve proofArtifactHash";
    assert "artifacts/eth-mainnet/proof-artifact.bin".equals(parsedNativeProverBundle.proofArtifact())
        : "Ethereum native prover bundle parser must preserve proofArtifact";
    assert nativeProverBundle.provingKeyHash().equals(parsedNativeProverBundle.provingKeyHash())
        : "Ethereum native prover bundle parser must preserve provingKeyHash";
    assert "artifacts/eth-mainnet/proving-key.bin".equals(parsedNativeProverBundle.provingKey())
        : "Ethereum native prover bundle parser must preserve provingKey";
    assert "artifacts/eth-mainnet/verifier-key.bin".equals(parsedNativeProverBundle.verifierKey())
        : "Ethereum native prover bundle parser must preserve verifierKey";
    assert nativeProverBundle
        .destinationBindingHash()
        .equals(parsedNativeProverBundle.destinationBindingHash())
        : "Ethereum native prover bundle parser must preserve destinationBindingHash";
    assert parsedNativeProverBundle.nativeSdkArtifacts().stream()
        .anyMatch(row -> "java-android".equals(row.sdk())
            && "artifacts/eth-mainnet/java-android-implementation.bin"
                .equals(row.implementationArtifact()))
        : "Ethereum native prover bundle parser must preserve implementationArtifact";
    final EvmSccpProver.EthereumMainnetNativeEvmProverParityFixture parityFixture =
        EvmSccpProver.EthereumMainnetNativeEvmProverParityFixture.fromJson(
            sampleEthereumNativeEvmProverParityFixtureJson(nativeProverBundle),
            nativeProverBundle);
    assert EvmSccpProver.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1.equals(
            parityFixture.schema())
        : "Ethereum native prover parity fixture parser must preserve schema";
    assert binding.hash.equals(parityFixture.destinationBindingHash())
        : "Ethereum native prover parity fixture must bind destinationBindingHash";
    assert parityFixture.publicSignalWords().size() == 9
        : "Ethereum native prover parity fixture must preserve public signal words";
    assert parityFixture
        .toriiSubmitPayloadHash()
        .equals(parityFixture.sdkResults().get("java-android").toriiSubmitPayloadHash())
        : "Ethereum native prover parity fixture must bind Java Android output";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverParityFixture.fromJson(
          sampleEthereumNativeEvmProverParityFixtureJson(
              nativeProverBundle, "0x" + repeat("96", 32)),
          nativeProverBundle);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sdkResults.java-android.calldataHash");
    }
    assert threw : "Ethereum native prover parity fixture must reject SDK drift";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverParityFixture.fromJson(
          sampleEthereumNativeEvmProverParityFixtureJson(nativeProverBundle)
              .replace(
                  "\"schema\":\""
                      + EvmSccpProver.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1
                      + "\"",
                  "\"schema\":\"forged\","
                      + "\"schema\":\""
                      + EvmSccpProver.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1
                      + "\""),
          nativeProverBundle);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("Duplicate JSON object key: schema");
    }
    assert threw : "Ethereum native prover parity fixture parser must reject duplicate keys";
    final EvmSccpProver.EthereumMainnetNativeEvmProverSelfTestFixture selfTestFixture =
        EvmSccpProver.EthereumMainnetNativeEvmProverSelfTestFixture.fromJson(
            sampleEthereumNativeEvmProverSelfTestFixtureJson(nativeProverBundle),
            nativeProverBundle);
    assert EvmSccpProver.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1.equals(
            selfTestFixture.schema())
        : "Ethereum native prover self-test fixture parser must preserve schema";
    assert binding.hash.equals(selfTestFixture.destinationBindingHash())
        : "Ethereum native prover self-test fixture must bind destinationBindingHash";
    assert selfTestFixture.publicSignalWords().size() == 9
        : "Ethereum native prover self-test fixture must preserve public signal words";
    assert selfTestFixture
        .proofHash()
        .equals(selfTestFixture.sdkResults().get("java-android").proofHash())
        : "Ethereum native prover self-test fixture must bind Java Android output";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverSelfTestFixture.fromJson(
          sampleEthereumNativeEvmProverSelfTestFixtureJson(
              nativeProverBundle, "0x" + repeat("96", 32)),
          nativeProverBundle);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sdkResults.java-android.proofHash");
    }
    assert threw : "Ethereum native prover self-test fixture must reject SDK drift";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverSelfTestFixture.fromJson(
          sampleEthereumNativeEvmProverSelfTestFixtureJson(nativeProverBundle)
              .replace(
                  "\"schema\":\""
                      + EvmSccpProver.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1
                      + "\"",
                  "\"schema\":\"forged\","
                      + "\"schema\":\""
                      + EvmSccpProver.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1
                      + "\""),
          nativeProverBundle);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("Duplicate JSON object key: schema");
    }
    assert threw : "Ethereum native prover self-test fixture parser must reject duplicate keys";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash, false, false), binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("noWasm");
    }
    assert threw : "Ethereum native prover bundle parser must reject WASM manifests";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson("0x" + repeat("95", 32), true, false),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "Ethereum native prover bundle parser must match destinationBindingHash";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash)
              .replace("\"domain\":1", "\"domain\":\"01\""),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("domain")
              && ex.getMessage().contains("canonical decimal integer");
    }
    assert threw : "Ethereum native prover bundle parser must reject noncanonical domain text";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash)
              .replace(
                  "\"bundle_id\":\"sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1\"",
                  "\"bundle_id\":\"forged\","
                      + "\"bundle_id\":\"sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1\""),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("Duplicate JSON object key: bundle_id");
    }
    assert threw : "Ethereum native prover bundle parser must reject duplicate JSON keys";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(
              binding.hash, true, false, "../proof-artifact.bin"),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofArtifact");
    }
    assert threw : "Ethereum native prover bundle parser must reject escaping artifact paths";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(
              binding.hash, true, false, "ipfs:proof-artifact.bin"),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofArtifact") && ex.getMessage().contains("URI schemes");
    }
    assert threw : "Ethereum native prover bundle parser must reject URI-style artifact paths";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(
              binding.hash, true, false, "artifacts/eth-mainnet/proof.wasm"),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("proofArtifact")
              && ex.getMessage().contains("forbidden prover dependency marker: wasm");
    }
    assert threw : "Ethereum native prover bundle parser must reject WASM artifact paths";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash)
              .replace("\"audit_hashes\":", "\"experimental_manifest_note\":true,\"audit_hashes\":"),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("nativeProverBundle")
              && ex.getMessage().contains("experimental_manifest_note")
              && ex.getMessage().contains("unknown field");
    }
    assert threw : "Ethereum native prover bundle parser must reject unknown manifest fields";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash)
              .replace(
                  "\"proof_artifact_hash\":\"0x" + repeat("91", 32) + "\"",
                  "\"proofArtifactHash\":\"0x"
                      + repeat("91", 32)
                      + "\",\"proof_artifact_hash\":\"0x"
                      + repeat("91", 32)
                      + "\""),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("proofArtifactHash")
              && ex.getMessage().contains("multiple aliases");
    }
    assert threw : "Ethereum native prover bundle parser must reject duplicate manifest aliases";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash)
              .replace(
                  "\"implementation_hash\":",
                  "\"experimental_manifest_note\":true,\"implementation_hash\":"),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("nativeSdkArtifacts[0]")
              && ex.getMessage().contains("experimental_manifest_note")
              && ex.getMessage().contains("unknown field");
    }
    assert threw : "Ethereum native prover bundle parser must reject unknown artifact fields";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash)
              .replace("\"0x" + repeat("a1", 32) + "\"", "\"0x" + repeat("A1", 32) + "\""),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("auditHashes.circuit_security_audit")
              && ex.getMessage().contains("canonical lowercase");
    }
    assert threw : "Ethereum native prover bundle parser must reject noncanonical audit hashes";
    threw = false;
    try {
      EvmSccpProver.EthereumMainnetNativeEvmProverBundle.fromJson(
          sampleEthereumNativeEvmProverBundleJson(binding.hash)
              .replace("\"0x" + repeat("a1", 32) + "\"", "\"0x" + repeat("91", 32) + "\""),
          binding.hash);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("auditHashes.circuit_security_audit")
              && ex.getMessage().contains("proofArtifactHash")
              && ex.getMessage().contains("role-separated");
    }
    assert threw : "Ethereum native prover bundle parser must reject replayed audit hash roles";
    final EvmSccpProver.ProofRequest bundledRequest =
        EthereumMainnetSccp.buildProofRequest(input, nativeProverBundle);
    assert ("0x" + repeat("91", 32)).equals(bundledRequest.proofArtifactHash())
        : "Ethereum native prover bundle must bind proofArtifactHash";
    assert ("0x" + repeat("92", 32)).equals(bundledRequest.provingKeyHash())
        : "Ethereum native prover bundle must bind provingKeyHash";
    assert !request.requestHash().equals(bundledRequest.requestHash())
        : "Ethereum native prover bundle hashes must enter the request hash";
    assert bundledRequest
        .requestHash()
        .equals(new EthereumMainnetSccp(nativeProverBundle).buildOutboundProofRequest(input).requestHash())
        : "Ethereum facade must apply configured native prover bundle";
    threw = false;
    try {
      new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
              EvmSccpProver.NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
              EvmSccpProver.ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
              EvmSccpProver.DOMAIN_ETH,
              "eth",
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              "artifacts/eth-mainnet/proof-artifact.bin",
              "0x" + repeat("91", 32),
              "artifacts/eth-mainnet/proving-key.bin",
              "0x" + repeat("92", 32),
              "artifacts/eth-mainnet/verifier-key.bin",
              "0x" + repeat("dd", 32),
              binding.hash,
              true,
              false,
              "pure-typescript",
              nativeProverBundle.nativeSdkArtifacts(),
              nativeProverBundle.auditHashes())
          .applyTo(input);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("nativeProverBundle.verifierKeyHash");
    }
    assert threw : "Ethereum native prover bundle must match destination verifierKeyHash";
    final byte[] proofArtifactBytes = nativeEvmProverArtifactBytes("java android proof artifact v1");
    final byte[] provingKeyBytes = nativeEvmProverArtifactBytes("java android proving key v1");
    final byte[] verifierKeyBytes = nativeEvmProverArtifactBytes("java android verifier key v1");
    final byte[] implementationBytes =
        nativeEvmProverArtifactBytes("java android implementation artifact v1");
    final String proofArtifactHash = sha256Hex(proofArtifactBytes);
    final String provingKeyHash = sha256Hex(provingKeyBytes);
    final String verifierKeyHash = sha256Hex(verifierKeyBytes);
    final String implementationHash = sha256Hex(implementationBytes);
    final SourceSccpProofs.EvmDestinationBinding artifactBinding =
        EthereumMainnetSccp.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            verifierKeyHash);
    final EvmSccpProver.ProofRequestInput artifactInput =
        new EvmSccpProver.ProofRequestInput(
            input.publicInputs(),
            input.bundleBytes(),
            input.sourceProofBytes(),
            input.statementHash(),
            artifactBinding);
    final ArrayList<EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact>
        verifiedSdkArtifacts = new ArrayList<>();
    int artifactIndex = 0;
    for (final Map.Entry<String, String> entry :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.entrySet()) {
      artifactIndex++;
      verifiedSdkArtifacts.add(
          new EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact(
              entry.getKey(),
              entry.getValue(),
              proofArtifactHash,
              provingKeyHash,
              "artifacts/eth-mainnet/" + entry.getKey() + "-implementation.bin",
              "java-android".equals(entry.getKey())
                  ? implementationHash
                  : "0x" + repeat(String.format("%02x", artifactIndex), 32)));
    }
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle draftVerifiedBundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            EvmSccpProver.NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
            EvmSccpProver.ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
            EvmSccpProver.DOMAIN_ETH,
            "eth",
            EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
            "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash,
            "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash,
            "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash,
            artifactBinding.hash,
            true,
            false,
            "pure-typescript",
            "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            "artifacts/eth-mainnet/native-prover-self-test.json",
            verifiedSdkArtifacts,
            sampleEthereumNativeAuditHashes());
    final byte[] parityFixtureBytes =
        sampleEthereumNativeEvmProverParityFixtureJson(draftVerifiedBundle)
            .getBytes(StandardCharsets.UTF_8);
    final String parityFixtureHash = sha256Hex(parityFixtureBytes);
    final byte[] selfTestFixtureBytes =
        sampleEthereumNativeEvmProverSelfTestFixtureJson(draftVerifiedBundle)
            .getBytes(StandardCharsets.UTF_8);
    final String selfTestFixtureHash = sha256Hex(selfTestFixtureBytes);
    final Map<String, String> verifiedAuditHashes = sampleEthereumNativeAuditHashes();
    verifiedAuditHashes.put("cross_sdk_fixture_parity", parityFixtureHash);
    verifiedAuditHashes.put("native_prover_self_test", selfTestFixtureHash);
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle verifiedBundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            EvmSccpProver.NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
            EvmSccpProver.ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
            EvmSccpProver.DOMAIN_ETH,
            "eth",
            EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
            "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash,
            "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash,
            "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash,
            artifactBinding.hash,
            true,
            false,
            "pure-typescript",
            "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            "artifacts/eth-mainnet/native-prover-self-test.json",
            verifiedSdkArtifacts,
            verifiedAuditHashes);
    final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts verifiedArtifacts =
        verifiedBundle.verifiedArtifacts(
            proofArtifactBytes,
            provingKeyBytes,
            verifierKeyBytes,
            "java-android",
            implementationBytes,
            parityFixtureBytes,
            selfTestFixtureBytes);
    assert EvmSccpProver.NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1.equals(
            verifiedArtifacts.hashAlgorithm())
        : "Android native prover artifact verifier must report sha256";
    assert proofArtifactHash.equals(verifiedArtifacts.proofArtifactHash())
        : "Android native prover artifact verifier must bind proof artifact bytes";
    assert provingKeyHash.equals(verifiedArtifacts.provingKeyHash())
        : "Android native prover artifact verifier must bind proving key bytes";
    assert verifierKeyHash.equals(verifiedArtifacts.verifierKeyHash())
        : "Android native prover artifact verifier must bind verifier key bytes";
    assert parityFixtureHash.equals(verifiedArtifacts.crossSdkFixtureParityHash())
        : "Android native prover artifact verifier must bind parity fixture bytes";
    assert ("0x" + repeat("d3", 32)).equals(verifiedArtifacts.crossSdkFixtureParity().calldataHash())
        : "Android native prover artifact verifier must parse parity fixture bytes";
    assert selfTestFixtureHash.equals(verifiedArtifacts.nativeProverSelfTestHash())
        : "Android native prover artifact verifier must bind self-test fixture bytes";
    assert ("0x" + repeat("e4", 32)).equals(verifiedArtifacts.nativeProverSelfTest().proofHash())
        : "Android native prover artifact verifier must parse self-test fixture bytes";
    assert "native-java".equals(verifiedArtifacts.implementation())
        : "Android native prover artifact verifier must select java implementation";
    assert implementationHash.equals(verifiedArtifacts.implementationHash())
        : "Android native prover artifact verifier must bind implementation bytes";
    final Map<String, byte[]> artifactBytesByPath = new LinkedHashMap<>();
    artifactBytesByPath.put(verifiedBundle.proofArtifact(), proofArtifactBytes);
    artifactBytesByPath.put(verifiedBundle.provingKey(), provingKeyBytes);
    artifactBytesByPath.put(verifiedBundle.verifierKey(), verifierKeyBytes);
    artifactBytesByPath.put(verifiedBundle.crossSdkFixtureParityArtifact(), parityFixtureBytes);
    artifactBytesByPath.put(verifiedBundle.nativeProverSelfTestArtifact(), selfTestFixtureBytes);
    String javaImplementationArtifact = null;
    for (final EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact row :
        verifiedBundle.nativeSdkArtifacts()) {
      if ("java-android".equals(row.sdk())) {
        javaImplementationArtifact = row.implementationArtifact();
        break;
      }
    }
    if (javaImplementationArtifact == null) {
      throw new AssertionError("missing java-android implementation artifact");
    }
    artifactBytesByPath.put(javaImplementationArtifact, implementationBytes);
    final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts resolverVerifiedArtifacts =
        verifiedBundle.verifiedArtifacts(
            "java-android",
            path -> {
              final byte[] bytes = artifactBytesByPath.get(path);
              if (bytes == null) {
                throw new IllegalArgumentException(path);
              }
              return bytes;
            });
    assert implementationHash.equals(resolverVerifiedArtifacts.implementationHash())
        : "Android native prover artifact resolver must bind implementation bytes";
    assert parityFixtureHash.equals(resolverVerifiedArtifacts.crossSdkFixtureParityHash())
        : "Android native prover artifact resolver must bind parity fixture bytes";
    assert selfTestFixtureHash.equals(resolverVerifiedArtifacts.nativeProverSelfTestHash())
        : "Android native prover artifact resolver must bind self-test fixture bytes";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          "java-android",
          path -> {
            if (verifiedBundle.crossSdkFixtureParityArtifact().equals(path)) {
              throw new IllegalArgumentException("crossSdkFixtureParityArtifact");
            }
            return artifactBytesByPath.get(path);
          });
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("crossSdkFixtureParityArtifact");
    }
    assert threw : "Android native prover artifact resolver must fail closed on missing parity bytes";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          "java-android",
          path -> {
            if (verifiedBundle.nativeProverSelfTestArtifact().equals(path)) {
              throw new IllegalArgumentException("nativeProverSelfTestArtifact");
            }
            return artifactBytesByPath.get(path);
          });
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("nativeProverSelfTestArtifact");
    }
    assert threw : "Android native prover artifact resolver must fail closed on missing self-test bytes";
    final boolean[] missingArtifactsProverCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              proofRequest -> {
                missingArtifactsProverCalled[0] = true;
                return proofBytes;
              })
          .proveOutboundToEthereum(input);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("verified native EVM prover artifacts");
    }
    assert threw : "Ethereum outbound prove facade must require verified native artifacts";
    assert !missingArtifactsProverCalled[0]
        : "Ethereum outbound prover callback must not run without verified artifacts";
    final EvmSccpProver.ProofRequest[] artifactBoundRequest =
        new EvmSccpProver.ProofRequest[] {null};
    final boolean[] artifactBoundSelfTestCalled = new boolean[] {false};
    final EthereumMainnetSccp artifactBoundFacade =
        new EthereumMainnetSccp(
            null,
            proofRequest -> {
              artifactBoundRequest[0] = proofRequest;
              assert proofArtifactHash.equals(proofRequest.proofArtifactHash())
                  : "verified artifacts must bind proofArtifactHash before callback";
              assert provingKeyHash.equals(proofRequest.provingKeyHash())
                  : "verified artifacts must bind provingKeyHash before callback";
              return proofBytes;
            },
            null,
            null,
            null,
            null,
            null,
            null,
            verifiedArtifacts,
            (fixture, expected, artifacts) -> {
              artifactBoundSelfTestCalled[0] = true;
              assert ("0x" + repeat("e4", 32)).equals(fixture.proofHash())
                  : "native prover self-test fixture must reach callback";
              assert selfTestFixtureHash.equals(artifacts.nativeProverSelfTestHash())
                  : "native prover self-test callback must see verified artifacts";
              return expected;
            },
            null);
    final EvmSccpProver.EthereumMainnetNativeEvmProverSelfTestSdkResult
        preflightSelfTestResult = artifactBoundFacade.runNativeProverSelfTest();
    assert artifactBoundSelfTestCalled[0]
        : "Ethereum native prover self-test preflight must run the app-linked self-test";
    assert ("0x" + repeat("e4", 32)).equals(preflightSelfTestResult.proofHash())
        : "Ethereum native prover self-test preflight must return the fixture proof hash";
    artifactBoundSelfTestCalled[0] = false;
    final EvmSccpProver.ProofResult artifactBoundResult =
        artifactBoundFacade.proveOutboundToEthereum(artifactInput);
    assert artifactBoundSelfTestCalled[0]
        : "Ethereum outbound proof must run native self-test before proof callback";
    assert proofArtifactHash.equals(artifactBoundRequest[0].proofArtifactHash())
        : "Ethereum artifact-bound prover callback must see proofArtifactHash";
    assert proofArtifactHash.equals(artifactBoundResult.proofArtifactHash())
        : "Ethereum artifact-bound proof result must carry proofArtifactHash";
    assert provingKeyHash.equals(artifactBoundResult.provingKeyHash())
        : "Ethereum artifact-bound proof result must carry provingKeyHash";
    final boolean[] missingSelfTestHookProverCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              proofRequest -> {
                missingSelfTestHookProverCalled[0] = true;
                return proofBytes;
              },
              null,
              null,
              null,
              null,
              null,
              null,
              verifiedArtifacts,
              null)
          .proveOutboundToEthereum(artifactInput);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("nativeProverSelfTest runner");
    }
    assert threw : "Ethereum outbound proof must require native self-test runner";
    assert !missingSelfTestHookProverCalled[0]
        : "Ethereum outbound prover callback must not run without native self-test runner";
    final boolean[] driftingSelfTestHookProverCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              proofRequest -> {
                driftingSelfTestHookProverCalled[0] = true;
                return proofBytes;
              },
              null,
              null,
              null,
              null,
              null,
              null,
              verifiedArtifacts,
              (fixture, expected, artifacts) ->
                  new EvmSccpProver.EthereumMainnetNativeEvmProverSelfTestSdkResult(
                      expected.requestHash(),
                      expected.witnessHash(),
                      expected.sourceProofHash(),
                      "0x" + repeat("97", 32),
                      expected.publicSignalWords(),
                      expected.calldataHash(),
                      expected.toriiSubmitPayloadHash()),
              null)
          .proveOutboundToEthereum(artifactInput);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("nativeProverSelfTest result");
    }
    assert threw : "Ethereum outbound proof must reject drifting native self-test output";
    assert !driftingSelfTestHookProverCalled[0]
        : "Ethereum outbound prover callback must not run after self-test drift";
    final EvmSccpProver.ProofRequest[] factoryBoundRequest =
        new EvmSccpProver.ProofRequest[] {null};
    final EvmSccpProver.ProofResult factoryBoundResult =
        EthereumMainnetSccp.fromNativeProverBundle(
                null,
                proofRequest -> {
                  factoryBoundRequest[0] = proofRequest;
                  return proofBytes;
                },
                null,
                null,
                null,
                null,
                null,
                (fixture, expected, artifacts) -> expected,
                verifiedBundle,
                "java-android",
                path -> {
                  final byte[] bytes = artifactBytesByPath.get(path);
                  if (bytes == null) {
                    throw new IllegalArgumentException(path);
                  }
                  return bytes;
                },
                null)
            .proveOutboundToEthereum(artifactInput);
    assert proofArtifactHash.equals(factoryBoundRequest[0].proofArtifactHash())
        : "Android bundle factory must bind proofArtifactHash before callback";
    assert provingKeyHash.equals(factoryBoundRequest[0].provingKeyHash())
        : "Android bundle factory must bind provingKeyHash before callback";
    assert proofArtifactHash.equals(factoryBoundResult.proofArtifactHash())
        : "Android bundle factory proof result must carry proofArtifactHash";
    assert provingKeyHash.equals(factoryBoundResult.provingKeyHash())
        : "Android bundle factory proof result must carry provingKeyHash";
    threw = false;
    try {
      EthereumMainnetSccp.fromNativeProverBundle(
          verifiedBundle,
          "java-android",
          path -> {
            if (verifiedBundle.crossSdkFixtureParityArtifact().equals(path)) {
              throw new IllegalArgumentException("crossSdkFixtureParityArtifact");
            }
            return artifactBytesByPath.get(path);
          });
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("crossSdkFixtureParityArtifact");
    }
    assert threw : "Android bundle factory must fail closed on missing parity bytes";
    threw = false;
    try {
      EthereumMainnetSccp.fromNativeProverBundle(
          verifiedBundle,
          "java-android",
          path -> {
            if (verifiedBundle.nativeProverSelfTestArtifact().equals(path)) {
              throw new IllegalArgumentException("nativeProverSelfTestArtifact");
            }
            return artifactBytesByPath.get(path);
          });
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("nativeProverSelfTestArtifact");
    }
    assert threw : "Android bundle factory must fail closed on missing self-test bytes";
    final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts implementationUnboundArtifacts =
        new EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts(
            EvmSccpProver.NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
            verifiedBundle,
            proofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            parityFixtureHash,
            verifiedArtifacts.crossSdkFixtureParity(),
            selfTestFixtureHash,
            verifiedArtifacts.nativeProverSelfTest(),
            "java-android",
            "native-java",
            null);
    final boolean[] implementationUnboundProverCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              proofRequest -> {
                implementationUnboundProverCalled[0] = true;
                return proofBytes;
              },
              null,
              null,
              null,
              null,
              null,
              null,
              implementationUnboundArtifacts,
              null)
            .proveOutboundToEthereum(artifactInput);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage()
              .contains("nativeProverArtifacts must bind sdk implementation and implementationHash");
    }
    assert threw : "Android native prover artifacts must bind implementation hash";
    assert !implementationUnboundProverCalled[0]
        : "Android prover callback must not run with unbound implementation artifacts";
    threw = false;
    final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts verifierKeyUnboundArtifacts =
        new EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts(
            EvmSccpProver.NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
            verifiedBundle,
            proofArtifactHash,
            provingKeyHash,
            "0x" + repeat("ef", 32),
            parityFixtureHash,
            verifiedArtifacts.crossSdkFixtureParity(),
            selfTestFixtureHash,
            verifiedArtifacts.nativeProverSelfTest(),
            "java-android",
            "native-java",
            implementationHash);
    final boolean[] verifierKeyUnboundProverCalled = new boolean[] {false};
    try {
      new EthereumMainnetSccp(
              null,
              proofRequest -> {
                verifierKeyUnboundProverCalled[0] = true;
                return proofBytes;
              },
              null,
              null,
              null,
              null,
              null,
              null,
              verifierKeyUnboundArtifacts,
              null)
            .proveOutboundToEthereum(artifactInput);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage()
              .contains("nativeProverArtifacts verifierKeyHash must match nativeProverBundle");
    }
    assert threw : "Android native prover artifacts must bind verifier key hash";
    assert !verifierKeyUnboundProverCalled[0]
        : "Android prover callback must not run with verifier-key-unbound artifacts";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(new byte[] {0}, provingKeyBytes, verifierKeyBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofArtifactBytes sha256");
    }
    assert threw : "Android native prover artifact verifier must reject tampered artifacts";
    final byte[] tinyProofArtifactBytes = new byte[] {1, 2, 3, 4, 5, 6, 7};
    final String tinyProofArtifactHash = sha256Hex(tinyProofArtifactBytes);
    final ArrayList<EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact>
        tinySdkArtifacts = new ArrayList<>();
    artifactIndex = 0;
    for (final Map.Entry<String, String> entry :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.entrySet()) {
      artifactIndex++;
      tinySdkArtifacts.add(
          new EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact(
              entry.getKey(),
              entry.getValue(),
              tinyProofArtifactHash,
              provingKeyHash,
              "java-android".equals(entry.getKey())
                  ? implementationHash
                  : "0x" + repeat(String.format("%02x", artifactIndex), 32)));
    }
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle draftTinyBundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            tinyProofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.hash,
            tinySdkArtifacts,
            sampleEthereumNativeAuditHashes());
    final byte[] tinyParityFixtureBytes =
        sampleEthereumNativeEvmProverParityFixtureJson(draftTinyBundle)
            .getBytes(StandardCharsets.UTF_8);
    final byte[] tinySelfTestFixtureBytes =
        sampleEthereumNativeEvmProverSelfTestFixtureJson(draftTinyBundle)
            .getBytes(StandardCharsets.UTF_8);
    final Map<String, String> tinyAuditHashes = sampleEthereumNativeAuditHashes();
    tinyAuditHashes.put("cross_sdk_fixture_parity", sha256Hex(tinyParityFixtureBytes));
    tinyAuditHashes.put("native_prover_self_test", sha256Hex(tinySelfTestFixtureBytes));
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle tinyBundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            tinyProofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.hash,
            tinySdkArtifacts,
            tinyAuditHashes);
    threw = false;
    try {
      tinyBundle.verifiedArtifacts(
          tinyProofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          "java-android",
          implementationBytes,
          tinyParityFixtureBytes,
          tinySelfTestFixtureBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofArtifactBytes must be at least 256 bytes");
    }
    assert threw : "Android native prover artifact verifier must reject tiny hash-consistent artifacts";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          null,
          implementationBytes,
          parityFixtureBytes,
          selfTestFixtureBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sdk must be a non-empty string");
    }
    assert threw : "Android native prover artifact verifier must require sdk for implementation bytes";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          "java-android",
          null,
          parityFixtureBytes,
          selfTestFixtureBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("implementationBytes are required");
    }
    assert threw : "Android native prover artifact verifier must require implementation bytes";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          "java-android",
          "tampered".getBytes(StandardCharsets.UTF_8),
          parityFixtureBytes,
          selfTestFixtureBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("implementationBytes sha256");
    }
    assert threw : "Android native prover artifact verifier must reject tampered implementations";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          "java-android",
          implementationBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("crossSdkFixtureParityBytes");
    }
    assert threw : "Android native prover artifact verifier must require parity fixture bytes";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          "java-android",
          implementationBytes,
          parityFixtureBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("nativeProverSelfTestBytes");
    }
    assert threw : "Android native prover artifact verifier must require self-test fixture bytes";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          "java-android",
          implementationBytes,
          "{}".getBytes(StandardCharsets.UTF_8));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("crossSdkFixtureParityBytes sha256");
    }
    assert threw : "Android native prover artifact verifier must reject tampered parity fixture bytes";
    threw = false;
    try {
      verifiedBundle.verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          "java-android",
          implementationBytes,
          parityFixtureBytes,
          "{}".getBytes(StandardCharsets.UTF_8));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("nativeProverSelfTestBytes sha256");
    }
    assert threw : "Android native prover artifact verifier must reject tampered self-test fixture bytes";
    final byte[] flaggedArtifactBytes = nativeEvmProverArtifactBytes("proof.wasm java android marker");
    final String flaggedArtifactHash = sha256Hex(flaggedArtifactBytes);
    final ArrayList<EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact>
        flaggedSdkArtifacts = new ArrayList<>();
    artifactIndex = 0;
    for (final Map.Entry<String, String> entry :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.entrySet()) {
      artifactIndex++;
      flaggedSdkArtifacts.add(
          new EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact(
              entry.getKey(),
              entry.getValue(),
              flaggedArtifactHash,
              provingKeyHash,
              "java-android".equals(entry.getKey())
                  ? implementationHash
                  : "0x" + repeat(String.format("%02x", artifactIndex), 32)));
    }
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle draftFlaggedBundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            flaggedArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.hash,
            flaggedSdkArtifacts,
            sampleEthereumNativeAuditHashes());
    final byte[] flaggedParityFixtureBytes =
        sampleEthereumNativeEvmProverParityFixtureJson(draftFlaggedBundle)
            .getBytes(StandardCharsets.UTF_8);
    final byte[] flaggedSelfTestFixtureBytes =
        sampleEthereumNativeEvmProverSelfTestFixtureJson(draftFlaggedBundle)
            .getBytes(StandardCharsets.UTF_8);
    final Map<String, String> flaggedAuditHashes = sampleEthereumNativeAuditHashes();
    flaggedAuditHashes.put("cross_sdk_fixture_parity", sha256Hex(flaggedParityFixtureBytes));
    flaggedAuditHashes.put("native_prover_self_test", sha256Hex(flaggedSelfTestFixtureBytes));
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle flaggedBundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            flaggedArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.hash,
            flaggedSdkArtifacts,
            flaggedAuditHashes);
    threw = false;
    try {
      flaggedBundle.verifiedArtifacts(
          flaggedArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          null,
          null,
          flaggedParityFixtureBytes,
          flaggedSelfTestFixtureBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofArtifactBytes contains forbidden");
    }
    assert threw
        : "Android native prover artifact verifier must reject forbidden dependency markers";
    threw = false;
    try {
      sampleEthereumNativeEvmProverBundle(binding.hash, false, false);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("noWasm");
    }
    assert threw : "Ethereum native prover bundle must reject WASM manifests";
    threw = false;
    try {
      sampleEthereumNativeEvmProverBundle("0x" + repeat("95", 32), true, false)
          .applyTo(input);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "Ethereum native prover bundle must match destinationBindingHash";

    final EvmSccpProver.ProofResult result =
        EthereumMainnetSccp.wrapProofResult(proofBytes, request);
    threw = false;
    try {
      EthereumMainnetSccp.wrapProofResult(
          proofBytes, evmRequestWithDestinationBindingHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "Ethereum wrapProofResult must reject forged destinationBindingHash";
    threw = false;
    try {
      new EthereumMainnetSccp().buildEthereumCalldata(new EvmSccpProver.SubmissionInput(result));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("verified native EVM prover artifacts");
    }
    assert threw : "Ethereum calldata helper must require verified native prover artifacts";
    final EvmSccpProver.Submission submission =
        new EthereumMainnetSccp(verifiedArtifacts)
            .buildEthereumCalldata(new EvmSccpProver.SubmissionInput(artifactBoundResult));
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_ETH
        : "Ethereum submission must target ETH";
    assert Arrays.equals(proofBytes, submission.proofBytes())
        : "Ethereum submission must preserve proof bytes";
    final Object submitted =
        new EthereumMainnetSccp(
                null,
                null,
                null,
                null,
                null,
                null,
                outboundSubmission -> {
                  assert outboundSubmission.targetDomain() == EvmSccpProver.DOMAIN_ETH
                      : "Ethereum outbound submitter must receive ETH calldata";
                  assert Arrays.equals(proofBytes, outboundSubmission.proofBytes())
                      : "Ethereum outbound submitter must receive proof bytes";
                  return "eth-submitted";
                },
                null,
                verifiedArtifacts,
                null)
            .submitOutboundToEthereum(new EvmSccpProver.SubmissionInput(artifactBoundResult));
    assert "eth-submitted".equals(submitted)
        : "Ethereum outbound submitter must return app-owned submission result";
    final boolean[] guardedSubmitterCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              (method, params) -> {
                assert "eth_chainId".equals(method)
                    : "Ethereum outbound submit must validate the configured execution provider";
                return "0x38";
              },
              null,
              null,
              null,
              outboundSubmission -> {
                guardedSubmitterCalled[0] = true;
                return "wrong-chain";
              },
              null,
              verifiedArtifacts,
              null)
          .submitOutboundToEthereum(new EvmSccpProver.SubmissionInput(artifactBoundResult));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 1");
    }
    assert threw : "Ethereum outbound submitter must reject configured non-mainnet execution RPC";
    assert !guardedSubmitterCalled[0] : "Ethereum outbound submitter must not run after chain-id failure";
    threw = false;
    try {
      new EthereumMainnetSccp(verifiedArtifacts)
          .submitOutboundToEthereum(new EvmSccpProver.SubmissionInput(artifactBoundResult));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("outbound submitter");
    }
    assert threw : "Ethereum outbound submission requires an app-owned submitter";

    threw = false;
    try {
      new EthereumMainnetSccp()
          .buildEthereumCalldata(
              new EvmSccpProver.SubmissionInput(
                  samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                  proofBytes,
                  repeat("56", 32),
                  binding.hash));
    } catch (final NullPointerException ex) {
      threw = ex.getMessage().contains("wrapped proofResult");
    }
    assert threw : "Ethereum mainnet calldata helper must require a wrapped proof result";

    threw = false;
    try {
      SourceSccpProofs.ethereumMainnetDestinationBinding(
          "0x" + repeat("11", 20),
          "0x" + repeat("22", 20),
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32),
          "0x" + repeat("33", 32));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("chain id 1");
    }
    assert threw : "Ethereum destination helper must reject non-mainnet network ids";

    threw = false;
    final SourceSccpProofs.EvmDestinationBinding bscBinding =
        BscSccpProver.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    try {
      EthereumMainnetSccp.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
              new byte[] {5, 6, 7},
              new byte[0],
              repeat("56", 32),
              bscBinding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("target ETH");
    }
    assert threw : "Ethereum request helper must reject BSC public inputs";
    final boolean[] outboundProverCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              proofRequest -> {
                outboundProverCalled[0] = true;
                return proofBytes;
              })
          .proveOutboundToEthereum(
              new EvmSccpProver.ProofRequestInput(
                  samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
                  new byte[] {5, 6, 7},
                  new byte[0],
                  repeat("56", 32),
                  bscBinding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("target ETH");
    }
    assert threw : "Ethereum outbound prove facade must reject BSC requests";
    assert !outboundProverCalled[0]
        : "Ethereum outbound prover callback must not see BSC requests";

    threw = false;
    try {
      EthereumMainnetSccp.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[] {5, 6, 7},
              new byte[0],
              repeat("56", 32),
              binding.hash,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              EvmSccpProver.DOMAIN_BSC,
              binding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("SORA -> ETH");
    }
    assert threw : "Ethereum request helper must reject non-SORA outbound source domains";

    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("12", 20);
    final Map<String, Object> unrelatedLog = linkedMap(
        "address", "0x" + repeat("00", 20),
        "topics", Arrays.asList("0x" + repeat("00", 32)),
        "data", "0x1234");
    final Map<String, Object> sourceEventLog = linkedMap(
        "address", sourceBridgeEmitterAddress,
        "transactionHash", txHash,
        "blockHash", blockHash,
        "blockNumber", "0x1234",
        "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
        "data", "0x");
    final Map<String, Object> receipt = linkedMap(
        "transactionHash", txHash,
        "blockHash", blockHash,
        "blockNumber", "0x1234",
        "status", "0x1");
    final Map<String, Object> receiptWithSourceEvent = new LinkedHashMap<>(receipt);
    receiptWithSourceEvent.put("logs", Arrays.asList(unrelatedLog, sourceEventLog));
    final Map<String, Object> block = linkedMap(
        "hash", blockHash,
        "number", "0x1234",
        "receiptsRoot", "0x" + repeat("cc", 32));
    final EthereumMainnetSccp.BeaconFinalityEvidence beaconFinalityEvidence =
        new EthereumMainnetSccp.BeaconFinalityEvidence(
            "0x1234",
            blockHash,
            "0x" + repeat("cc", 32),
            "0x20",
            ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
            "0x" + repeat("34", 96),
            ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION,
            "65",
            linkedMap(
                "finalityBranch", ETHEREUM_FINALITY_BRANCH,
                "finalizedHeaderRoot", "0x" + repeat("dd", 32),
                "syncCommitteeRoot", "0x" + repeat("aa", 32)));
    final Map<String, Object> beaconFinality = beaconFinalityEvidence.toMap();
    final EthereumMainnetSccp.ReceiptProof receiptProof =
        new EthereumMainnetSccp.ReceiptProof(
            sourceEventDigest,
            "32",
            "4660",
            blockHash,
            "0x" + repeat("cc", 32),
            "0x" + repeat("dd", 32),
            "0x" + repeat("aa", 32),
            "3",
            Arrays.asList(new byte[] {1}, new byte[] {2, 3}),
            Arrays.asList(hexWord(repeat("11", 32))));
    final String receiptProofHash =
        SourceSccpProofs.evmReceiptProofHash(
            receiptProof.sourceEventDigest(),
            receiptProof.beaconSlot(),
            receiptProof.executionBlockNumber(),
            receiptProof.executionBlockHash(),
            receiptProof.executionReceiptsRoot(),
            receiptProof.beaconFinalizedRoot(),
            receiptProof.syncCommitteeRoot(),
            receiptProof.receiptRootIndex(),
            receiptProof.receiptTrieProofNodes(),
            receiptProof.inclusionBranch());
    assert "0x39f014e3f5f8d38b44d59f1afdf72ceb71d10d6d937f268c404b046f092b38f0"
            .equals(receiptProofHash)
        : "Ethereum receipt-proof vector must match the shared native hash";
    threw = false;
    try {
      SourceSccpProofs.canonicalEvmReceiptProofBytes(
          receiptProof.sourceEventDigest(),
          receiptProof.beaconSlot(),
          receiptProof.executionBlockNumber(),
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          new ArrayList<byte[]>(),
          receiptProof.inclusionBranch());
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptTrieProofNodes");
    }
    assert threw : "Ethereum receipt-proof transcript must reject empty receiptTrieProofNodes";
    threw = false;
    try {
      SourceSccpProofs.evmReceiptProofHash(
          receiptProof.sourceEventDigest(),
          receiptProof.beaconSlot(),
          receiptProof.executionBlockNumber(),
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          receiptProof.receiptTrieProofNodes(),
          new ArrayList<byte[]>());
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("inclusionBranch must not be empty");
    }
    assert threw : "Ethereum receipt-proof transcript must reject empty inclusionBranch";
    threw = false;
    try {
      SourceSccpProofs.evmReceiptProofHash(
          receiptProof.sourceEventDigest(),
          receiptProof.beaconSlot(),
          receiptProof.executionBlockNumber(),
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          receiptProof.receiptTrieProofNodes(),
          receiptProof.inclusionBranch(),
          EvmSccpProver.DOMAIN_BSC);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be ETH");
    }
    assert threw : "Ethereum receipt-proof transcript must reject BSC sourceDomain";
    threw = false;
    try {
      SourceSccpProofs.canonicalBscReceiptProofBytes(
          receiptProof.sourceEventDigest(),
          "21",
          "22",
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          receiptProof.receiptTrieProofNodes(),
          new ArrayList<byte[]>());
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("inclusionBranch must not be empty");
    }
    assert threw : "BSC receipt-proof transcript must reject empty inclusionBranch";
    threw = false;
    try {
      SourceSccpProofs.canonicalBscReceiptProofBytes(
          receiptProof.sourceEventDigest(),
          "21",
          "22",
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          receiptProof.receiptTrieProofNodes(),
          receiptProof.inclusionBranch(),
          EvmSccpProver.DOMAIN_ETH);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be BSC");
    }
    assert threw : "BSC receipt-proof transcript must reject ETH sourceDomain";
    final ArrayList<String> calls = new ArrayList<>();
    final int[] consensusCalls = new int[1];
    final EthereumMainnetSccp sdk =
        new EthereumMainnetSccp(
            null,
            null,
            (method, params) -> {
              calls.add(method);
              if ("eth_chainId".equals(method)) {
                return "0x1";
              }
              if ("eth_getTransactionReceipt".equals(method)) {
                assert params.size() == 1 && txHash.equals(params.get(0))
                    : "receipt request must use the requested tx hash";
                return receipt;
              }
              if ("eth_getBlockByHash".equals(method)) {
                assert params.size() == 2 && blockHash.equals(params.get(0))
                    : "block request must use receipt block hash";
                return block;
              }
              throw new IllegalArgumentException("unexpected method " + method);
            },
            (collectedReceipt, collectedBlock, collectedTransactionHash) -> {
              consensusCalls[0]++;
              assert receipt.equals(collectedReceipt)
                  : "consensus provider must receive collected receipt";
              assert block.equals(collectedBlock)
                  : "consensus provider must receive collected block";
              assert txHash.equals(collectedTransactionHash)
                  : "consensus provider must receive collected tx hash";
              return beaconFinality;
            },
            evidence -> {
              assert evidence.sourceDomain() == EvmSccpProver.DOMAIN_ETH
                  : "inbound evidence must be ETH sourced";
              assert evidence.targetDomain() == EvmSccpProver.DOMAIN_SORA
                  : "inbound evidence must target SORA";
              assert txHash.equals(evidence.transactionHash())
                  : "inbound evidence must carry normalized tx hash";
              assert "4660".equals(evidence.beaconFinality().get("executionBlockNumber"))
                  : "inbound evidence must carry normalized finality block number";
              assert blockHash.equals(evidence.beaconFinality().get("executionBlockHash"))
                  : "inbound evidence must carry finality block hash";
              assert ("0x" + repeat("dd", 32)).equals(
                      evidence.beaconFinality().get("finalizedHeaderRoot"))
                  : "inbound evidence must carry finalized beacon root";
              assert ("0x" + repeat("aa", 32)).equals(
                      evidence.beaconFinality().get("syncCommitteeRoot"))
                  : "inbound evidence must carry sync committee root";
              assert "32".equals(evidence.beaconFinality().get("beaconSlot"))
                  : "inbound evidence must carry normalized beacon slot";
              assert receiptProofHash.equals(evidence.receiptProofHash())
                  : "inbound evidence must carry receipt proof hash";
              assert receiptProof.sourceEventDigest().equals(evidence.receiptProof().sourceEventDigest())
                  : "inbound evidence must carry receipt proof material";
              assert sourceEventDigest.equals(evidence.sourceEventDigest())
                  : "inbound evidence must carry validated source event digest";
              return new byte[] {1, 2, 3};
            },
            proof -> {
              assert Arrays.equals(new byte[] {1, 2, 3}, proof)
                  : "inbound submitter must receive proof bytes";
              return "submitted";
            });
    final EthereumMainnetSccp.InboundEvidence evidence =
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                txHash,
                null,
                null,
                null,
                null));
    assert txHash.equals(evidence.transactionHash()) : "inbound evidence must retain tx hash";
    assert receipt.equals(evidence.receipt()) : "inbound evidence must carry receipt";
    assert block.equals(evidence.block()) : "inbound evidence must carry block";
    assert "4660".equals(evidence.beaconFinality().get("executionBlockNumber"))
        : "inbound evidence must normalize beacon finality block number";
    assert ("0x" + repeat("cc", 32)).equals(evidence.beaconFinality().get("executionReceiptsRoot"))
        : "inbound evidence must carry beacon finality receipt root";
    assert ("0x" + repeat("dd", 32)).equals(evidence.beaconFinality().get("finalizedHeaderRoot"))
        : "inbound evidence must carry finalized beacon root";
    assert ("0x" + repeat("aa", 32)).equals(evidence.beaconFinality().get("syncCommitteeRoot"))
        : "inbound evidence must carry sync committee root";
    assert "32".equals(evidence.beaconFinality().get("beaconSlot"))
        : "inbound evidence must carry normalized beacon slot";
    assert consensusCalls[0] == 1 : "consensus provider must be called once";
    assert calls.equals(Arrays.asList("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"))
        : "inbound collection must validate mainnet and fetch receipt/block";
    assert "submitted".equals(sdk.submitInboundToIroha(new byte[] {1, 2, 3}))
        : "inbound submitter must return caller result";

    final EthereumMainnetSccp.InboundEvidence sourceEventEvidence =
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receiptWithSourceEvent,
                block,
                beaconFinality,
                null,
                null,
                sourceBridgeEmitterAddress));
    assert sourceEventDigest.equals(sourceEventEvidence.sourceEventDigest())
        : "source-event validation must derive the receipt event digest";
    assert sourceBridgeEmitterAddress.equals(sourceEventEvidence.sourceBridgeEmitterAddress())
        : "source-event validation must retain the normalized bridge emitter";
    final EthereumMainnetSccp.InboundEvidence explicitSourceEventEvidence =
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receiptWithSourceEvent,
                block,
                beaconFinality,
                null,
                sourceEventDigest,
                sourceBridgeEmitterAddress));
    assert sourceEventDigest.equals(explicitSourceEventEvidence.sourceEventDigest())
        : "source-event validation must accept the expected digest";
    final EthereumMainnetSccp configuredSourceBridgeSdk =
        new EthereumMainnetSccp(
            null, null, null, null, null, null, null, sourceBridgeEmitterAddress);
    final EthereumMainnetSccp.InboundEvidence configuredSourceEventEvidence =
        configuredSourceBridgeSdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receiptWithSourceEvent,
                block,
                beaconFinality,
                null,
                null,
                null));
    assert sourceEventDigest.equals(configuredSourceEventEvidence.sourceEventDigest())
        : "configured Ethereum source bridge emitter must derive the receipt event digest";
    assert sourceBridgeEmitterAddress.equals(configuredSourceEventEvidence.sourceBridgeEmitterAddress())
        : "configured Ethereum source bridge emitter must be retained on evidence";
    threw = false;
    try {
      configuredSourceBridgeSdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receiptWithSourceEvent,
              block,
              beaconFinality,
              null,
              null,
              "0x" + repeat("13", 20)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceBridgeEmitterAddress");
    }
    assert threw : "Ethereum source-event validation must reject configured/input bridge drift";

    final EthereumMainnetSccp.InboundEvidence receiptProofEvidence =
        new EthereumMainnetSccp()
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    null,
                    null,
                    null,
                    receiptProof,
                    receiptProofHash,
                    null,
                    null));
    assert receiptProofHash.equals(receiptProofEvidence.receiptProofHash())
        : "Ethereum inbound collection must derive receiptProofHash from receiptProof";
    assert receiptProofEvidence.receiptProof() != null
        : "Ethereum inbound collection must retain app-collected receiptProof";
    assert receiptProof != receiptProofEvidence.receiptProof()
        : "Ethereum inbound collection must detach app-collected receiptProof";
    assert receiptProof.sourceEventDigest().equals(receiptProofEvidence.receiptProof().sourceEventDigest())
        : "Ethereum inbound collection must preserve receiptProof source event digest";
    assert receiptProof.receiptRootIndex().equals(receiptProofEvidence.receiptProof().receiptRootIndex())
        : "Ethereum inbound collection must preserve receiptProof index";
    assert receiptProof.receiptTrieProofNodes().size()
            == receiptProofEvidence.receiptProof().receiptTrieProofNodes().size()
        : "Ethereum inbound collection must preserve receiptProof trie nodes";
    final EthereumMainnetSccp.InboundEvidence receiptProofHashOnlyEvidence =
        new EthereumMainnetSccp()
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    null,
                    null,
                    null,
                    receiptProofHash));
    assert receiptProofHash.equals(receiptProofHashOnlyEvidence.receiptProofHash())
        : "Ethereum inbound collection must accept hash-only receiptProofHash evidence";
    assert receiptProofHashOnlyEvidence.receiptProof() == null
        : "hash-only Ethereum evidence must not synthesize a receiptProof";
    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  "0x" + repeat("00", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProofHash must not be zero");
    }
    assert threw : "Ethereum inbound collection must reject zero hash-only receiptProofHash";
    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  receiptProofHash + " "));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProofHash must be canonical");
    }
    assert threw : "Ethereum inbound collection must reject noncanonical hash-only receiptProofHash";
    final EthereumMainnetSccp.InboundEvidence unanchoredProofEvidence =
        new EthereumMainnetSccp.InboundEvidence(
            EvmSccpProver.DOMAIN_ETH,
            EvmSccpProver.DOMAIN_SORA,
            evidence.transactionHash(),
            evidence.receipt(),
            evidence.block(),
            evidence.beaconFinality(),
            receiptProof,
            receiptProofHash,
            null,
            null);
    threw = false;
    try {
      sdk.proveInboundToSora(unanchoredProofEvidence);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt source event validation");
    }
    assert threw : "Ethereum inbound proving must reject receipt proofs without source event validation";
    final int[] prebuiltProofOnlyProverCalls = new int[] {0};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              proofOnlyEvidence -> {
                prebuiltProofOnlyProverCalls[0] += 1;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  sourceEventEvidence.beaconFinality(),
                  receiptProof,
                  receiptProofHash,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt source event validation");
    }
    assert threw
        : "Ethereum inbound proving must reject proof-only evidence without source event validation";
    assert prebuiltProofOnlyProverCalls[0] == 0
        : "prebuilt proof-only evidence must fail before the Ethereum inbound prover callback";
    final EthereumMainnetSccp.InboundEvidence proofReadyEvidence =
        new EthereumMainnetSccp.InboundEvidence(
            EvmSccpProver.DOMAIN_ETH,
            EvmSccpProver.DOMAIN_SORA,
            sourceEventEvidence.transactionHash(),
            sourceEventEvidence.receipt(),
            sourceEventEvidence.block(),
            sourceEventEvidence.beaconFinality(),
            receiptProof,
            receiptProofHash,
            sourceEventEvidence.sourceEventDigest(),
            sourceEventEvidence.sourceBridgeEmitterAddress());
    final Map<String, Object> missingFinalityBranchFinality =
        new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
    missingFinalityBranchFinality.remove("finalityBranch");
    threw = false;
    try {
      sdk.proveInboundToSora(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              sourceEventEvidence.transactionHash(),
              sourceEventEvidence.receipt(),
              sourceEventEvidence.block(),
              missingFinalityBranchFinality,
              receiptProof,
              receiptProofHash,
              sourceEventEvidence.sourceEventDigest(),
              sourceEventEvidence.sourceBridgeEmitterAddress()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.finalityBranch");
    }
    assert threw : "Ethereum inbound proving must reject missing finality branch";
    final Map<String, Object> missingSyncBitsFinality =
        new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
    missingSyncBitsFinality.remove("syncCommitteeBits");
    threw = false;
    try {
      sdk.proveInboundToSora(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              sourceEventEvidence.transactionHash(),
              sourceEventEvidence.receipt(),
              sourceEventEvidence.block(),
              missingSyncBitsFinality,
              receiptProof,
              receiptProofHash,
              sourceEventEvidence.sourceEventDigest(),
              sourceEventEvidence.sourceBridgeEmitterAddress()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncCommitteeBits");
    }
    assert threw : "Ethereum inbound proving must reject missing sync-committee bits";
    final Map<String, Object> conflictingSyncBitsFinality =
        new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
    conflictingSyncBitsFinality.put("sync_committee_bits", "0x02" + repeat("00", 63));
    threw = false;
    try {
      sdk.proveInboundToSora(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              sourceEventEvidence.transactionHash(),
              sourceEventEvidence.receipt(),
              sourceEventEvidence.block(),
              conflictingSyncBitsFinality,
              receiptProof,
              receiptProofHash,
              sourceEventEvidence.sourceEventDigest(),
              sourceEventEvidence.sourceBridgeEmitterAddress()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncCommitteeBits");
    }
    assert threw : "Ethereum inbound proving must reject sync-committee bit aliases";
    final Map<String, Object> mismatchedSyncParticipationFinality =
        new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
    mismatchedSyncParticipationFinality.put("syncCommitteeParticipation", "341");
    threw = false;
    try {
      sdk.proveInboundToSora(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              sourceEventEvidence.transactionHash(),
              sourceEventEvidence.receipt(),
              sourceEventEvidence.block(),
              mismatchedSyncParticipationFinality,
              receiptProof,
              receiptProofHash,
              sourceEventEvidence.sourceEventDigest(),
              sourceEventEvidence.sourceBridgeEmitterAddress()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncCommitteeParticipation");
    }
    assert threw : "Ethereum inbound proving must reject sync-committee participation drift";
    final Map<String, Object> underQuorumSyncBitsFinality =
        new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
    underQuorumSyncBitsFinality.put("syncCommitteeBits", "0x01" + repeat("00", 63));
    underQuorumSyncBitsFinality.put("syncCommitteeParticipation", "1");
    threw = false;
    try {
      sdk.proveInboundToSora(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              sourceEventEvidence.transactionHash(),
              sourceEventEvidence.receipt(),
              sourceEventEvidence.block(),
              underQuorumSyncBitsFinality,
              receiptProof,
              receiptProofHash,
              sourceEventEvidence.sourceEventDigest(),
              sourceEventEvidence.sourceBridgeEmitterAddress()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncCommitteeBits");
    }
    assert threw : "Ethereum inbound proving must reject under-quorum sync-committee bits";
    final Map<String, Object> staleSyncSignatureSlotFinality =
        new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
    staleSyncSignatureSlotFinality.put("syncSignatureSlot", "31");
    threw = false;
    try {
      sdk.proveInboundToSora(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              sourceEventEvidence.transactionHash(),
              sourceEventEvidence.receipt(),
              sourceEventEvidence.block(),
              staleSyncSignatureSlotFinality,
              receiptProof,
              receiptProofHash,
              sourceEventEvidence.sourceEventDigest(),
              sourceEventEvidence.sourceBridgeEmitterAddress()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncSignatureSlot");
    }
    assert threw : "Ethereum inbound proving must reject stale sync-signature slot";
    final Map<String, Object> zeroSyncCommitteeSignatureFinality =
        new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
    zeroSyncCommitteeSignatureFinality.put("syncCommitteeSignature", "0x" + repeat("00", 96));
    threw = false;
    try {
      sdk.proveInboundToSora(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              sourceEventEvidence.transactionHash(),
              sourceEventEvidence.receipt(),
              sourceEventEvidence.block(),
              zeroSyncCommitteeSignatureFinality,
              receiptProof,
              receiptProofHash,
              sourceEventEvidence.sourceEventDigest(),
              sourceEventEvidence.sourceBridgeEmitterAddress()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncCommitteeSignature");
    }
    assert threw : "Ethereum inbound proving must reject zero sync-committee signatures";
    final Map<String, Object> aliasOnlyFinality = new LinkedHashMap<>();
    aliasOnlyFinality.put("execution_block_number", "0x1234");
    aliasOnlyFinality.put("finality_block_hash", blockHash);
    aliasOnlyFinality.put("receipts_root", "0x" + repeat("cc", 32));
    aliasOnlyFinality.put("finalized_header_root", "0x" + repeat("dd", 32));
    aliasOnlyFinality.put("sync_committee_root", "0x" + repeat("aa", 32));
    aliasOnlyFinality.put("beacon_slot", "0x20");
    aliasOnlyFinality.put("finality_branch", ETHEREUM_FINALITY_BRANCH);
    aliasOnlyFinality.put("sync_committee_bits", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS);
    aliasOnlyFinality.put("sync_committee_signature", "0x" + repeat("34", 96));
    aliasOnlyFinality.put(
        "sync_committee_participation", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION);
    aliasOnlyFinality.put("signature_slot", "65");
    aliasOnlyFinality.put("extensionWitness", "kept");
    final byte[] aliasOnlyProof =
        new EthereumMainnetSccp(
                null,
                null,
                null,
                null,
                aliasEvidence -> {
                  final Map<String, Object> finality = aliasEvidence.beaconFinality();
                  assert "4660".equals(finality.get("executionBlockNumber"))
                      : "alias-only finality must normalize block number";
                  assert blockHash.equals(finality.get("executionBlockHash"))
                      : "alias-only finality must normalize block hash";
                  assert ("0x" + repeat("cc", 32)).equals(finality.get("executionReceiptsRoot"))
                      : "alias-only finality must normalize receipts root";
                  assert ("0x" + repeat("dd", 32)).equals(finality.get("finalizedHeaderRoot"))
                      : "alias-only finality must normalize finalized root";
                  assert ("0x" + repeat("aa", 32)).equals(finality.get("syncCommitteeRoot"))
                      : "alias-only finality must normalize sync root";
                  assert "32".equals(finality.get("beaconSlot"))
                      : "alias-only finality must normalize beacon slot";
                  assert ETHEREUM_FINALITY_BRANCH.equals(finality.get("finalityBranch"))
                      : "alias-only finality must normalize finality branch";
                  assert ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS.equals(
                          finality.get("syncCommitteeBits"))
                      : "alias-only finality must normalize sync bits";
                  assert ("0x" + repeat("34", 96)).equals(finality.get("syncCommitteeSignature"))
                      : "alias-only finality must normalize sync signature";
                  assert ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION.equals(
                          finality.get("syncCommitteeParticipation"))
                      : "alias-only finality must normalize sync participation";
                  assert "65".equals(finality.get("syncSignatureSlot"))
                      : "alias-only finality must normalize signature slot";
                  assert "kept".equals(finality.get("extensionWitness"))
                      : "unknown finality extension fields must be preserved";
                  for (final String alias :
                      Arrays.asList(
                          "execution_block_number",
                          "finalityHeight",
                          "finality_block_hash",
                          "receipts_root",
                          "finalized_header_root",
                          "sync_committee_root",
                          "beacon_slot",
                          "finality_branch",
                          "sync_committee_bits",
                          "sync_committee_signature",
                          "sync_committee_participation",
                          "signature_slot")) {
                    assert !finality.containsKey(alias)
                        : "callback finality must not retain alias " + alias;
                  }
                  return new byte[] {4, 5, 6};
                },
                null)
            .proveInboundToSora(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    sourceEventEvidence.transactionHash(),
                    sourceEventEvidence.receipt(),
                    sourceEventEvidence.block(),
                    aliasOnlyFinality,
                    receiptProof,
                    receiptProofHash,
                    sourceEventEvidence.sourceEventDigest(),
                    sourceEventEvidence.sourceBridgeEmitterAddress()));
    assert Arrays.equals(new byte[] {4, 5, 6}, aliasOnlyProof)
        : "alias-only finality must reach the prover with canonical keys";
    final Object[][] conflictingFinalityAliases =
        new Object[][] {
          {"finalized_header_root", "0x" + repeat("13", 32), "beaconFinality.finalizedHeaderRoot"},
          {"sync_committee_root", "0x" + repeat("14", 32), "beaconFinality.syncCommitteeRoot"},
          {"beacon_slot", "33", "beaconFinality.beaconSlot"}
        };
    for (final Object[] conflict : conflictingFinalityAliases) {
      final Map<String, Object> conflictingFinality =
          new LinkedHashMap<>(sourceEventEvidence.beaconFinality());
      conflictingFinality.put((String) conflict[0], conflict[1]);
      threw = false;
      try {
        sdk.proveInboundToSora(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                sourceEventEvidence.transactionHash(),
                sourceEventEvidence.receipt(),
                sourceEventEvidence.block(),
                conflictingFinality,
                receiptProof,
                receiptProofHash,
                sourceEventEvidence.sourceEventDigest(),
                sourceEventEvidence.sourceBridgeEmitterAddress()));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains((String) conflict[2]);
      }
      assert threw : "Ethereum inbound proving must reject conflicting finality aliases";
    }
    assert Arrays.equals(
            new byte[] {1, 2, 3}, sdk.proveInboundToSora(proofReadyEvidence))
        : "inbound prover must receive receipt-proof-backed validated evidence";
    threw = false;
    try {
      new EthereumMainnetSccp(
              null, null, null, null, emptyProofEvidence -> new byte[0], null)
          .proveInboundToSora(proofReadyEvidence);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be empty");
    }
    assert threw : "Ethereum inbound prover output must reject empty proof bytes";
    threw = false;
    try {
      new EthereumMainnetSccp(
              null, null, null, null, zeroProofEvidence -> new byte[] {0, 0}, null)
          .proveInboundToSora(proofReadyEvidence);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be all zero");
    }
    assert threw : "Ethereum inbound prover output must reject all-zero proof bytes";
    final byte[] oversizedInboundProof =
        new byte[EthereumMainnetSccp.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedInboundProof, (byte) 1);
    threw = false;
    try {
      new EthereumMainnetSccp(
              null, null, null, null, oversizedProofEvidence -> oversizedInboundProof, null)
          .proveInboundToSora(proofReadyEvidence);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must be at most");
    }
    assert threw : "Ethereum inbound prover output must reject oversized proof bytes";
    threw = false;
    try {
      sdk.submitInboundToIroha(oversizedInboundProof);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must be at most");
    }
    assert threw : "Ethereum inbound submitter must reject oversized proof bytes";
    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  receiptProof,
                  "0x" + repeat("99", 32),
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProofHash must match receiptProof");
    }
    assert threw : "Ethereum inbound collection must reject conflicting receiptProofHash";
    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  new EthereumMainnetSccp.ReceiptProof(
                      EvmSccpProver.DOMAIN_BSC,
                      receiptProof.sourceEventDigest(),
                      receiptProof.beaconSlot(),
                      receiptProof.executionBlockNumber(),
                      receiptProof.executionBlockHash(),
                      receiptProof.executionReceiptsRoot(),
                      receiptProof.beaconFinalizedRoot(),
                      receiptProof.syncCommitteeRoot(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.sourceDomain");
    }
    assert threw : "Ethereum inbound collection must reject cross-lane receiptProof transcripts";

    assert Arrays.equals(
            new byte[] {7, 8, 9},
            new EthereumMainnetSccp(
                    null,
                    null,
                    null,
                    null,
                    typedEvidence -> {
                      assert txHash.equals(typedEvidence.transactionHash())
                          : "typed beacon finality evidence must preserve receipt tx hash";
                      assert blockHash.equals(
                              typedEvidence.beaconFinality().get("executionBlockHash"))
                          : "typed beacon finality evidence must preserve block hash";
                      return new byte[] {7, 8, 9};
                    },
                    null)
                .proveInboundToSora(
                    new EthereumMainnetSccp.InboundEvidence(
                        EvmSccpProver.DOMAIN_ETH,
                        EvmSccpProver.DOMAIN_SORA,
                        null,
                        receiptWithSourceEvent,
                        block,
                        beaconFinalityEvidence.toMap(),
                        receiptProof,
                        receiptProofHash,
                        null,
                        sourceBridgeEmitterAddress)))
        : "typed beacon finality evidence must feed inbound proof collection";

    final ArrayList<String> perCallProviderCalls = new ArrayList<>();
    final int[] perCallConsensusCalls = new int[1];
    final EthereumMainnetSccp perCallSdk =
        new EthereumMainnetSccp(
            null,
            null,
            null,
            null,
            perCallEvidence -> {
              assert txHash.equals(perCallEvidence.transactionHash())
                  : "per-call provider must collect receipt evidence";
              assert blockHash.equals(perCallEvidence.beaconFinality().get("executionBlockHash"))
                  : "per-call consensus provider must attach finality";
              return new byte[] {4, 5, 6};
            },
            null);
    assert Arrays.equals(
            new byte[] {4, 5, 6},
            perCallSdk.proveInboundToSora(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    txHash,
                    null,
                    null,
                    null,
                    receiptProof,
                    receiptProofHash,
                    null,
                    sourceBridgeEmitterAddress),
                (method, params) -> {
                  perCallProviderCalls.add(method);
                  if ("eth_chainId".equals(method)) {
                    return "0x1";
                  }
                  if ("eth_getTransactionReceipt".equals(method)) {
                    return receiptWithSourceEvent;
                  }
                  if ("eth_getBlockByHash".equals(method)) {
                    return block;
                  }
                  throw new IllegalArgumentException("unexpected method " + method);
                },
                (collectedReceipt, collectedBlock, collectedTransactionHash) -> {
                  perCallConsensusCalls[0]++;
                  return beaconFinality;
                }))
        : "per-call providers must be usable for inbound proving";
    assert perCallProviderCalls.equals(
            Arrays.asList("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"))
        : "per-call execution provider must collect mainnet receipt and block";
    assert perCallConsensusCalls[0] == 1
        : "per-call consensus provider must be called once";

    final int[] missingFinalityProverCalls = new int[1];
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceWithoutFinality -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality");
    }
    assert threw : "Ethereum inbound proving must reject missing beacon finality";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without beacon finality";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceWithoutReceiptProof -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  beaconFinality,
                  receiptProofHash));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof");
    }
    assert threw : "Ethereum inbound proving must reject hash-only receipt proof evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without receipt proof material";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedReceiptProofEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  beaconFinality,
                  new EthereumMainnetSccp.ReceiptProof(
                      receiptProof.sourceEventDigest(),
                      receiptProof.beaconSlot(),
                      receiptProof.executionBlockNumber(),
                      receiptProof.executionBlockHash(),
                      "0x" + repeat("99", 32),
                      receiptProof.beaconFinalizedRoot(),
                      receiptProof.syncCommitteeRoot(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.executionReceiptsRoot");
    }
    assert threw : "Ethereum inbound proving must reject drifted receipt proof transcripts";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted receipt proof material";

    final Map<String, Object> missingFinalizedRootFinality = linkedMap(
        "syncCommitteeRoot", "0x" + repeat("aa", 32),
        "beaconSlot", "0x20",
        "executionBlockNumber", "0x1234",
        "executionBlockHash", blockHash,
        "executionReceiptsRoot", "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              missingFinalizedRootEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receiptWithSourceEvent,
                  block,
                  missingFinalizedRootFinality,
                  receiptProof,
                  null,
                  null,
                  sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.finalizedHeaderRoot");
    }
    assert threw : "Ethereum inbound proving must require finalized beacon root evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without finalized beacon root evidence";

    final Map<String, Object> missingSyncRootFinality = linkedMap(
        "finalizedHeaderRoot", "0x" + repeat("dd", 32),
        "beaconSlot", "0x20",
        "executionBlockNumber", "0x1234",
        "executionBlockHash", blockHash,
        "executionReceiptsRoot", "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              missingSyncRootEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receiptWithSourceEvent,
                  block,
                  missingSyncRootFinality,
                  receiptProof,
                  null,
                  null,
                  sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncCommitteeRoot");
    }
    assert threw : "Ethereum inbound proving must require sync committee root evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without sync committee root evidence";

    final Map<String, Object> missingBeaconSlotFinality = linkedMap(
        "finalizedHeaderRoot", "0x" + repeat("dd", 32),
        "syncCommitteeRoot", "0x" + repeat("aa", 32),
        "executionBlockNumber", "0x1234",
        "executionBlockHash", blockHash,
        "executionReceiptsRoot", "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              missingBeaconSlotEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receiptWithSourceEvent,
                  block,
                  missingBeaconSlotFinality,
                  receiptProof,
                  null,
                  null,
                  sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.beaconSlot");
    }
    assert threw : "Ethereum inbound proving must require finalized beacon slot evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without finalized beacon slot evidence";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedFinalizedRootEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  beaconFinality,
                  new EthereumMainnetSccp.ReceiptProof(
                      receiptProof.sourceEventDigest(),
                      receiptProof.beaconSlot(),
                      receiptProof.executionBlockNumber(),
                      receiptProof.executionBlockHash(),
                      receiptProof.executionReceiptsRoot(),
                      "0x" + repeat("99", 32),
                      receiptProof.syncCommitteeRoot(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.beaconFinalizedRoot");
    }
    assert threw : "Ethereum inbound proving must reject drifted finalized beacon roots";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted finalized beacon root";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedSyncRootEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  beaconFinality,
                  new EthereumMainnetSccp.ReceiptProof(
                      receiptProof.sourceEventDigest(),
                      receiptProof.beaconSlot(),
                      receiptProof.executionBlockNumber(),
                      receiptProof.executionBlockHash(),
                      receiptProof.executionReceiptsRoot(),
                      receiptProof.beaconFinalizedRoot(),
                      "0x" + repeat("99", 32),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.syncCommitteeRoot");
    }
    assert threw : "Ethereum inbound proving must reject drifted sync committee roots";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted sync committee root";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedBeaconSlotEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  beaconFinality,
                  new EthereumMainnetSccp.ReceiptProof(
                      receiptProof.sourceEventDigest(),
                      "33",
                      receiptProof.executionBlockNumber(),
                      receiptProof.executionBlockHash(),
                      receiptProof.executionReceiptsRoot(),
                      receiptProof.beaconFinalizedRoot(),
                      receiptProof.syncCommitteeRoot(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.beaconSlot");
    }
    assert threw : "Ethereum inbound proving must reject drifted finalized beacon slots";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted finalized beacon slot";

    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  txHash,
                  null,
                  null,
                  null,
                  null));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("execution provider");
    }
    assert threw : "Ethereum inbound collection by transaction hash must require a provider";

    threw = false;
    try {
      new EthereumMainnetSccp(null, null, (method, params) -> "0x38", null, null)
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 1");
    }
    assert threw : "Ethereum inbound collection must reject non-mainnet RPC";

    threw = false;
    try {
      new EthereumMainnetSccp(null, null, (method, params) -> "1", null, null)
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "Ethereum inbound collection must reject decimal eth_chainId RPC";

    threw = false;
    try {
      new EthereumMainnetSccp(null, null, (method, params) -> "0x01", null, null)
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "Ethereum inbound collection must reject leading-zero eth_chainId RPC";

    threw = false;
    try {
      new EthereumMainnetSccp(null, null, (method, params) -> Long.valueOf(1L), null, null)
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "Ethereum inbound collection must reject numeric eth_chainId RPC";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x0"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("status must be 0x1");
    }
    assert threw : "Ethereum inbound collection must reject failed receipts";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "Ethereum inbound collection must reject receipts without block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x0",
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "Ethereum inbound collection must reject zero receipt block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              txHash,
              linkedMap(
                  "transactionHash", "0x" + repeat("ab", 32),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash must match");
    }
    assert threw : "Ethereum inbound collection must reject receipt tx drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", "0x" + repeat("bc", 32),
                  "number", "0x1234",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.hash must match");
    }
    assert threw : "Ethereum inbound collection must reject block hash drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "Ethereum inbound collection must reject blocks without numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "number", "0x0",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "Ethereum inbound collection must reject zero block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash.toUpperCase(),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical lowercase");
    }
    assert threw : "Ethereum inbound collection must reject uppercase RPC hashes";

    final Map<String, Object> driftedFinalityHash = new LinkedHashMap<>(beaconFinality);
    driftedFinalityHash.put("executionBlockHash", "0x" + repeat("bc", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityHash,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.executionBlockHash");
    }
    assert threw : "Ethereum inbound collection must reject finality block-hash drift";

    final Map<String, Object> driftedFinalityNumber = new LinkedHashMap<>(beaconFinality);
    driftedFinalityNumber.put("executionBlockNumber", "0x1235");
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityNumber,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.executionBlockNumber");
    }
    assert threw : "Ethereum inbound collection must reject finality block-number drift";

    final Map<String, Object> driftedFinalityReceiptsRoot = new LinkedHashMap<>(beaconFinality);
    driftedFinalityReceiptsRoot.put("executionReceiptsRoot", "0x" + repeat("cd", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityReceiptsRoot,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.executionReceiptsRoot");
    }
    assert threw : "Ethereum inbound collection must reject finality receipts-root drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receiptWithSourceEvent,
              block,
              beaconFinality,
              null,
              sourceEventDigest,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceBridgeEmitterAddress");
    }
    assert threw : "Ethereum source-event validation must require the source bridge emitter";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logs");
    }
    assert threw : "Ethereum source-event validation must require receipt logs";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receiptWithSourceEvent,
              block,
              beaconFinality,
              null,
              null,
              "0x" + repeat("13", 20)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("expected SCCP source event");
    }
    assert threw : "Ethereum source-event validation must reject a wrong bridge emitter";

    final Map<String, Object> wrongTopicLog = new LinkedHashMap<>(sourceEventLog);
    wrongTopicLog.put("topics", Arrays.asList("0x" + repeat("ab", 32), sourceEventDigest));
    final Map<String, Object> wrongTopicReceipt = new LinkedHashMap<>(receipt);
    wrongTopicReceipt.put("logs", Arrays.asList(wrongTopicLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              wrongTopicReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("expected SCCP source event");
    }
    assert threw : "Ethereum source-event validation must reject a wrong event topic";

    final Map<String, Object> extraTopicLog = new LinkedHashMap<>(sourceEventLog);
    extraTopicLog.put(
        "topics",
        Arrays.asList(
            EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest, "0x" + repeat("66", 32)));
    final Map<String, Object> extraTopicReceipt = new LinkedHashMap<>(receipt);
    extraTopicReceipt.put("logs", Arrays.asList(extraTopicLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              extraTopicReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("exactly 2 topics");
    }
    assert threw : "Ethereum source-event validation must reject extra source-event topics";

    final Map<String, Object> nonEmptyDataLog = new LinkedHashMap<>(sourceEventLog);
    nonEmptyDataLog.put("data", "0x01");
    final Map<String, Object> nonEmptyDataReceipt = new LinkedHashMap<>(receipt);
    nonEmptyDataReceipt.put("logs", Arrays.asList(nonEmptyDataLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              nonEmptyDataReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("data must be 0x");
    }
    assert threw : "Ethereum source-event validation must reject non-empty source-event data";

    final Map<String, Object> zeroDigestLog = new LinkedHashMap<>(sourceEventLog);
    zeroDigestLog.put(
        "topics",
        Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), "0x" + repeat("00", 32)));
    final Map<String, Object> zeroDigestReceipt = new LinkedHashMap<>(receipt);
    zeroDigestReceipt.put("logs", Arrays.asList(zeroDigestLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              zeroDigestReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("digest must not be zero");
    }
    assert threw : "Ethereum source-event validation must reject zero source-event digest";

    final Map<String, Object> duplicateReceipt = new LinkedHashMap<>(receipt);
    duplicateReceipt.put("logs", Arrays.asList(sourceEventLog, sourceEventLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              duplicateReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("exactly one");
    }
    assert threw : "Ethereum source-event validation must reject duplicate matching events";

    final Map<String, Object> removedLog = new LinkedHashMap<>(sourceEventLog);
    removedLog.put("removed", Boolean.TRUE);
    final Map<String, Object> removedReceipt = new LinkedHashMap<>(receipt);
    removedReceipt.put("logs", Arrays.asList(removedLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              removedReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("removed logs");
    }
    assert threw : "Ethereum source-event validation must reject removed logs";

    final Map<String, Object> nonObjectLogReceipt = new LinkedHashMap<>(receipt);
    nonObjectLogReceipt.put("logs", Arrays.asList("not-a-log"));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              nonObjectLogReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logs[0] must be an object");
    }
    assert threw : "Ethereum source-event validation must reject non-object logs";

    final Map<String, Object> missingDataLog = new LinkedHashMap<>(sourceEventLog);
    missingDataLog.remove("data");
    final Map<String, Object> missingDataReceipt = new LinkedHashMap<>(receipt);
    missingDataReceipt.put("logs", Arrays.asList(missingDataLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              missingDataReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logs[0].data");
    }
    assert threw : "Ethereum source-event validation must reject source logs without data";

    for (final String missingField :
        Arrays.asList("transactionHash", "blockHash", "blockNumber")) {
      final Map<String, Object> missingContextLog = new LinkedHashMap<>(sourceEventLog);
      missingContextLog.remove(missingField);
      final Map<String, Object> missingContextReceipt = new LinkedHashMap<>(receipt);
      missingContextReceipt.put("logs", Arrays.asList(missingContextLog));
      threw = false;
      try {
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                missingContextReceipt,
                block,
                beaconFinality,
                null,
                null,
                sourceBridgeEmitterAddress));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("receipt.logs[0]." + missingField);
      }
      assert threw : "Ethereum source-event validation must reject logs without " + missingField;
    }

    final Object[][] conflictingLogAliases =
        new Object[][] {
          {"transaction_hash", "0x" + repeat("ab", 32), "receipt.logs[0].transactionHash"},
          {"block_hash", "0x" + repeat("ac", 32), "receipt.logs[0].blockHash"},
          {"block_number", "0x1235", "receipt.logs[0].blockNumber"}
        };
    for (final Object[] conflict : conflictingLogAliases) {
      final Map<String, Object> conflictingLog = new LinkedHashMap<>(sourceEventLog);
      conflictingLog.put((String) conflict[0], conflict[1]);
      final Map<String, Object> conflictingLogReceipt = new LinkedHashMap<>(receipt);
      conflictingLogReceipt.put("logs", Arrays.asList(conflictingLog));
      threw = false;
      try {
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                conflictingLogReceipt,
                block,
                beaconFinality,
                null,
                null,
                sourceBridgeEmitterAddress));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains((String) conflict[2]);
      }
      assert threw : "Ethereum source-event validation must reject conflicting log aliases";
    }

    final Map<String, Object> driftedLogTransaction = new LinkedHashMap<>(sourceEventLog);
    driftedLogTransaction.put("transactionHash", "0x" + repeat("ab", 32));
    final Map<String, Object> driftedLogTransactionReceipt = new LinkedHashMap<>(receipt);
    driftedLogTransactionReceipt.put("logs", Arrays.asList(driftedLogTransaction));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              driftedLogTransactionReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash must match");
    }
    assert threw : "Ethereum source-event validation must reject log transaction hash drift";

    final Map<String, Object> driftedLogBlockHash = new LinkedHashMap<>(sourceEventLog);
    driftedLogBlockHash.put("blockHash", "0x" + repeat("ab", 32));
    final Map<String, Object> driftedLogBlockHashReceipt = new LinkedHashMap<>(receipt);
    driftedLogBlockHashReceipt.put("logs", Arrays.asList(driftedLogBlockHash));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              driftedLogBlockHashReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockHash must match");
    }
    assert threw : "Ethereum source-event validation must reject log block hash drift";

    final Map<String, Object> driftedLogBlockNumber = new LinkedHashMap<>(sourceEventLog);
    driftedLogBlockNumber.put("blockNumber", "0x1235");
    final Map<String, Object> driftedLogBlockNumberReceipt = new LinkedHashMap<>(receipt);
    driftedLogBlockNumberReceipt.put("logs", Arrays.asList(driftedLogBlockNumber));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              driftedLogBlockNumberReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockNumber must match");
    }
    assert threw : "Ethereum source-event validation must reject log block number drift";

    threw = false;
    try {
      sdk.submitInboundToIroha(new byte[] {0, 0});
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "Ethereum inbound submitter must reject zero proof bytes";
  }

  @SuppressWarnings("unchecked")
  private static void ethereumMainnetInboundProverReceivesCallbackEvidenceSnapshot() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("44", 20);
    final String receiptsRoot = "0x" + repeat("cc", 32);
    final String finalizedRoot = "0x" + repeat("dd", 32);
    final String syncCommitteeRoot = "0x" + repeat("aa", 32);
    final Map<String, Object> receiptNested =
        linkedMap("value", "keep", "bytes", new byte[] {(byte) 0xbb});
    final ArrayList<Object> receiptWitness = new ArrayList<>();
    receiptWitness.add(receiptNested);
    final Map<String, Object> blockWitness =
        linkedMap("value", "block", "bytes", new byte[] {(byte) 0xcc});
    final ArrayList<String> finalityBranchWitness = new ArrayList<>(ETHEREUM_FINALITY_BRANCH);
    final byte[] finalityBytes = new byte[] {(byte) 0xaa};
    final Map<String, Object> finalityWitness =
        linkedMap("branch", finalityBranchWitness, "bytes", finalityBytes);
    final ArrayList<Object> blockReceiptsWitness = new ArrayList<>();
    blockReceiptsWitness.add("receipt-list");
    final Map<String, Object> sourceEventLog =
        linkedMap(
            "address", sourceBridgeEmitterAddress,
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
            "data", "0x");
    final Map<String, Object> receipt =
        linkedMap(
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "status", "0x1",
            "logs", Collections.singletonList(sourceEventLog),
            "mutableWitness", receiptWitness);
    final Map<String, Object> block =
        linkedMap(
            "hash", blockHash,
            "number", "0x1234",
            "receiptsRoot", receiptsRoot,
            "mutableWitness", blockWitness);
    final Map<String, Object> beaconFinality =
        linkedMap(
            "executionBlockNumber", "0x1234",
            "executionBlockHash", blockHash,
            "executionReceiptsRoot", receiptsRoot,
            "finalizedHeaderRoot", finalizedRoot,
            "syncCommitteeRoot", syncCommitteeRoot,
            "beaconSlot", "0x20",
            "finalityBranch", ETHEREUM_FINALITY_BRANCH,
            "syncCommitteeBits", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
            "syncCommitteeSignature", "0x" + repeat("34", 96),
            "syncCommitteeParticipation", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION,
            "syncSignatureSlot", "65",
            "mutableWitness", finalityWitness);
    final Map<String, Object> blockReceipt = new LinkedHashMap<>(receipt);
    blockReceipt.put("mutableWitness", blockReceiptsWitness);
    final byte[] mutableReceiptProofNode = new byte[] {0x01, 0x02};
    final byte[] mutableReceiptProofBranch = repeatedByteArray(0x11, 32);
    final byte[] mutableInputBranch = new byte[] {0x44};
    final EthereumMainnetSccp.ReceiptProof receiptProof =
        new EthereumMainnetSccp.ReceiptProof(
            sourceEventDigest,
            "32",
            "4660",
            blockHash,
            receiptsRoot,
            finalizedRoot,
            syncCommitteeRoot,
            "0",
            Collections.singletonList(mutableReceiptProofNode),
            Collections.singletonList(mutableReceiptProofBranch));
    final String receiptProofHash =
        SourceSccpProofs.evmReceiptProofHash(
            receiptProof.sourceEventDigest(),
            receiptProof.beaconSlot(),
            receiptProof.executionBlockNumber(),
            receiptProof.executionBlockHash(),
            receiptProof.executionReceiptsRoot(),
            receiptProof.beaconFinalizedRoot(),
            receiptProof.syncCommitteeRoot(),
            receiptProof.receiptRootIndex(),
            receiptProof.receiptTrieProofNodes(),
            receiptProof.inclusionBranch());

    final byte[] proofBytes =
        new EthereumMainnetSccp(
                null,
                null,
                null,
                null,
                evidence -> {
                  receiptWitness.add("changed");
                  receiptNested.put("value", "changed");
                  ((byte[]) receiptNested.get("bytes"))[0] = 0x7f;
                  blockWitness.put("value", "changed");
                  ((byte[]) blockWitness.get("bytes"))[0] = 0x7e;
                  finalityBranchWitness.add("0x" + repeat("99", 32));
                  finalityBytes[0] = 0x7d;
                  finalityWitness.put("new", "changed");
                  blockReceiptsWitness.add("changed");
                  mutableReceiptProofNode[0] = 0x7c;
                  mutableReceiptProofBranch[0] = 0x7b;
                  mutableInputBranch[0] = 0x45;

                  final List<Object> receiptSnapshot =
                      (List<Object>) evidence.receipt().get("mutableWitness");
                  assert receiptSnapshot.size() == 1
                      : "Ethereum inbound callback receipt witness must be snapshotted";
                  final Map<String, Object> receiptNestedSnapshot =
                      (Map<String, Object>) receiptSnapshot.get(0);
                  assert "keep".equals(receiptNestedSnapshot.get("value"))
                      : "Ethereum inbound callback receipt map must be detached";
                  assert Arrays.equals(
                          new byte[] {(byte) 0xbb},
                          (byte[]) receiptNestedSnapshot.get("bytes"))
                      : "Ethereum inbound callback receipt bytes must be detached";

                  final Map<String, Object> blockSnapshot =
                      (Map<String, Object>) evidence.block().get("mutableWitness");
                  assert "block".equals(blockSnapshot.get("value"))
                      : "Ethereum inbound callback block map must be detached";
                  assert Arrays.equals(new byte[] {(byte) 0xcc}, (byte[]) blockSnapshot.get("bytes"))
                      : "Ethereum inbound callback block bytes must be detached";

                  final Map<String, Object> finalitySnapshot =
                      (Map<String, Object>) evidence.beaconFinality().get("mutableWitness");
                  final List<String> branchSnapshot =
                      (List<String>) finalitySnapshot.get("branch");
                  assert branchSnapshot.size() == ETHEREUM_FINALITY_BRANCH.size()
                      : "Ethereum inbound callback finality branch must be detached";
                  assert ETHEREUM_FINALITY_BRANCH.get(0).equals(branchSnapshot.get(0))
                      : "Ethereum inbound callback finality branch contents must be stable";
                  assert Arrays.equals(new byte[] {(byte) 0xaa}, (byte[]) finalitySnapshot.get("bytes"))
                      : "Ethereum inbound callback finality bytes must be detached";

                  final List<Map<String, Object>> blockReceiptsSnapshot = evidence.blockReceipts();
                  final List<Object> blockReceiptWitnessSnapshot =
                      (List<Object>) blockReceiptsSnapshot.get(0).get("mutableWitness");
                  assert blockReceiptWitnessSnapshot.equals(Collections.singletonList("receipt-list"))
                      : "Ethereum inbound callback block receipts must be detached";

                  assert Arrays.equals(new byte[] {0x44}, evidence.inclusionBranch().get(0))
                      : "Ethereum inbound callback inclusionBranch must be snapshotted";
                  assert Arrays.equals(
                          new byte[] {0x01, 0x02},
                          evidence.receiptProof().receiptTrieProofNodes().get(0))
                      : "Ethereum inbound callback trie nodes must be snapshotted";
                  assert Arrays.equals(
                          repeatedByteArray(0x11, 32),
                          evidence.receiptProof().inclusionBranch().get(0))
                      : "Ethereum inbound callback proof branches must be snapshotted";
                  assert receiptProofHash.equals(evidence.receiptProofHash())
                      : "Ethereum inbound callback must retain receiptProofHash";
                  return new byte[] {9, 8, 7};
                },
                null,
                null,
                null)
            .proveInboundToSora(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    receipt,
                    block,
                    beaconFinality,
                    receiptProof,
                    receiptProofHash,
                    null,
                    sourceBridgeEmitterAddress,
                    Collections.singletonList(blockReceipt),
                    Collections.singletonList(mutableInputBranch)));

    assert Arrays.equals(new byte[] {9, 8, 7}, proofBytes)
        : "Ethereum inbound callback snapshot test must return proof bytes";
  }

  @SuppressWarnings("unchecked")
  private static void ethereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("44", 20);
    final String receiptsRoot = "0x" + repeat("cc", 32);
    final String finalizedRoot = "0x" + repeat("dd", 32);
    final String syncCommitteeRoot = "0x" + repeat("aa", 32);
    final Map<String, Object> receiptNested =
        linkedMap("value", "keep", "bytes", new byte[] {(byte) 0xbb});
    final ArrayList<Object> receiptWitness = new ArrayList<>();
    receiptWitness.add(receiptNested);
    final Map<String, Object> blockWitness =
        linkedMap("value", "block", "bytes", new byte[] {(byte) 0xcc});
    final ArrayList<String> finalityBranchWitness = new ArrayList<>(ETHEREUM_FINALITY_BRANCH);
    final byte[] finalityBytes = new byte[] {(byte) 0xaa};
    final Map<String, Object> finalityWitness =
        linkedMap("branch", finalityBranchWitness, "bytes", finalityBytes);
    final Map<String, Object> sourceEventLog =
        linkedMap(
            "address", sourceBridgeEmitterAddress,
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
            "data", "0x");
    final Map<String, Object> receipt =
        linkedMap(
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "status", "0x1",
            "logs", Collections.singletonList(sourceEventLog),
            "mutableWitness", receiptWitness);
    final Map<String, Object> block =
        linkedMap(
            "hash", blockHash,
            "number", "0x1234",
            "receiptsRoot", receiptsRoot,
            "mutableWitness", blockWitness);
    final Map<String, Object> beaconFinality =
        linkedMap(
            "executionBlockNumber", "0x1234",
            "executionBlockHash", blockHash,
            "executionReceiptsRoot", receiptsRoot,
            "finalizedHeaderRoot", finalizedRoot,
            "syncCommitteeRoot", syncCommitteeRoot,
            "beaconSlot", "0x20",
            "finalityBranch", ETHEREUM_FINALITY_BRANCH,
            "syncCommitteeBits", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
            "syncCommitteeSignature", "0x" + repeat("34", 96),
            "syncCommitteeParticipation", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION,
            "syncSignatureSlot", "65",
            "mutableWitness", finalityWitness);
    final int[] consensusCalls = new int[] {0};
    final EthereumMainnetSccp.ConsensusProvider consensusProvider =
        (collectedReceipt, collectedBlock, collectedTransactionHash) -> {
          consensusCalls[0] += 1;
          assert txHash.equals(collectedTransactionHash)
              : "Ethereum collection consensus callback must receive transactionHash";
          assert collectedReceipt.get("mutableWitness") != receiptWitness
              : "Ethereum collection consensus callback must receive a receipt witness snapshot";
          final List<Object> receiptSnapshot =
              (List<Object>) collectedReceipt.get("mutableWitness");
          final Map<String, Object> receiptNestedSnapshot =
              (Map<String, Object>) receiptSnapshot.get(0);
          assert "keep".equals(receiptNestedSnapshot.get("value"))
              : "Ethereum collection consensus receipt map must be detached";
          assert Arrays.equals(new byte[] {(byte) 0xbb}, (byte[]) receiptNestedSnapshot.get("bytes"))
              : "Ethereum collection consensus receipt bytes must be detached";
          assert collectedBlock.get("mutableWitness") != blockWitness
              : "Ethereum collection consensus callback must receive a block witness snapshot";
          final Map<String, Object> blockSnapshot =
              (Map<String, Object>) collectedBlock.get("mutableWitness");
          assert "block".equals(blockSnapshot.get("value"))
              : "Ethereum collection consensus block map must be detached";
          assert Arrays.equals(new byte[] {(byte) 0xcc}, (byte[]) blockSnapshot.get("bytes"))
              : "Ethereum collection consensus block bytes must be detached";

          receiptWitness.add("changed");
          receiptNested.put("value", "changed");
          ((byte[]) receiptNested.get("bytes"))[0] = 0x7f;
          blockWitness.put("value", "changed");
          ((byte[]) blockWitness.get("bytes"))[0] = 0x7e;
          return beaconFinality;
        };

    final EthereumMainnetSccp.InboundEvidence evidence =
        new EthereumMainnetSccp(
                null,
                null,
                null,
                consensusProvider,
                null,
                null,
                null,
                sourceBridgeEmitterAddress)
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    receipt,
                    block,
                    null,
                    null));
    finalityBranchWitness.add("0x" + repeat("99", 32));
    finalityBytes[0] = 0x7d;
    finalityWitness.put("new", "changed");

    assert consensusCalls[0] == 1 : "Ethereum collection consensus callback must run once";
    final List<Object> receiptSnapshot = (List<Object>) evidence.receipt().get("mutableWitness");
    assert receiptSnapshot.size() == 1 : "Ethereum collection receipt witness must be snapshotted";
    final Map<String, Object> receiptNestedSnapshot = (Map<String, Object>) receiptSnapshot.get(0);
    assert "keep".equals(receiptNestedSnapshot.get("value"))
        : "Ethereum collection receipt snapshot must not see callback mutation";
    assert Arrays.equals(new byte[] {(byte) 0xbb}, (byte[]) receiptNestedSnapshot.get("bytes"))
        : "Ethereum collection receipt bytes must not see callback mutation";
    final Map<String, Object> blockSnapshot =
        (Map<String, Object>) evidence.block().get("mutableWitness");
    assert "block".equals(blockSnapshot.get("value"))
        : "Ethereum collection block snapshot must not see callback mutation";
    assert Arrays.equals(new byte[] {(byte) 0xcc}, (byte[]) blockSnapshot.get("bytes"))
        : "Ethereum collection block bytes must not see callback mutation";
    final Map<String, Object> finalitySnapshot =
        (Map<String, Object>) evidence.beaconFinality().get("mutableWitness");
    final List<String> branchSnapshot = (List<String>) finalitySnapshot.get("branch");
    assert branchSnapshot.size() == ETHEREUM_FINALITY_BRANCH.size()
        : "Ethereum collection finality branch must be snapshotted";
    assert ETHEREUM_FINALITY_BRANCH.get(0).equals(branchSnapshot.get(0))
        : "Ethereum collection finality branch contents must be stable";
    assert Arrays.equals(new byte[] {(byte) 0xaa}, (byte[]) finalitySnapshot.get("bytes"))
        : "Ethereum collection finality bytes must be snapshotted";
    assert !finalitySnapshot.containsKey("new")
        : "Ethereum collection finality snapshot must not see post-collection mutation";
  }

  @SuppressWarnings("unchecked")
  private static void bscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("44", 20);
    final String receiptsRoot = "0x" + repeat("cc", 32);
    final String validatorSetHash = "0x" + repeat("ab", 32);
    final String commitSealHash = "0x" + repeat("dd", 32);
    final Map<String, Object> receiptNested =
        linkedMap("value", "keep", "bytes", new byte[] {(byte) 0xbb});
    final ArrayList<Object> receiptWitness = new ArrayList<>();
    receiptWitness.add(receiptNested);
    final Map<String, Object> blockWitness =
        linkedMap("value", "block", "bytes", new byte[] {(byte) 0xcc});
    final ArrayList<Object> finalityBranchWitness = new ArrayList<>();
    finalityBranchWitness.add(validatorSetHash);
    final byte[] finalityBytes = new byte[] {(byte) 0xaa};
    final Map<String, Object> finalityWitness =
        linkedMap("branch", finalityBranchWitness, "bytes", finalityBytes);
    final Map<String, Object> sourceEventLog =
        linkedMap(
            "address", sourceBridgeEmitterAddress,
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
            "data", "0x");
    final Map<String, Object> receipt =
        linkedMap(
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "status", "0x1",
            "logs", Collections.singletonList(sourceEventLog),
            "mutableWitness", receiptWitness);
    final Map<String, Object> block =
        linkedMap(
            "hash", blockHash,
            "number", "0x1234",
            "receiptsRoot", receiptsRoot,
            "mutableWitness", blockWitness);
    final Map<String, Object> parliaFinality =
        linkedMap(
            "executionBlockNumber", "0x1234",
            "executionBlockHash", blockHash,
            "executionReceiptsRoot", receiptsRoot,
            "validatorEpoch", "0x24",
            "validatorSetHash", validatorSetHash,
            "commitSealHash", commitSealHash,
            "mutableWitness", finalityWitness);
    final int[] consensusCalls = new int[] {0};
    final BscMainnetSccp.ConsensusProvider consensusProvider =
        (collectedReceipt, collectedBlock, collectedTransactionHash) -> {
          consensusCalls[0] += 1;
          assert txHash.equals(collectedTransactionHash)
              : "BSC collection consensus callback must receive transactionHash";
          assert collectedReceipt.get("mutableWitness") != receiptWitness
              : "BSC collection consensus callback must receive a receipt witness snapshot";
          final List<Object> receiptSnapshot =
              (List<Object>) collectedReceipt.get("mutableWitness");
          final Map<String, Object> receiptNestedSnapshot =
              (Map<String, Object>) receiptSnapshot.get(0);
          assert "keep".equals(receiptNestedSnapshot.get("value"))
              : "BSC collection consensus receipt map must be detached";
          assert Arrays.equals(new byte[] {(byte) 0xbb}, (byte[]) receiptNestedSnapshot.get("bytes"))
              : "BSC collection consensus receipt bytes must be detached";
          assert collectedBlock.get("mutableWitness") != blockWitness
              : "BSC collection consensus callback must receive a block witness snapshot";
          final Map<String, Object> blockSnapshot =
              (Map<String, Object>) collectedBlock.get("mutableWitness");
          assert "block".equals(blockSnapshot.get("value"))
              : "BSC collection consensus block map must be detached";
          assert Arrays.equals(new byte[] {(byte) 0xcc}, (byte[]) blockSnapshot.get("bytes"))
              : "BSC collection consensus block bytes must be detached";

          receiptWitness.add("changed");
          receiptNested.put("value", "changed");
          ((byte[]) receiptNested.get("bytes"))[0] = 0x7f;
          blockWitness.put("value", "changed");
          ((byte[]) blockWitness.get("bytes"))[0] = 0x7e;
          return parliaFinality;
        };

    final BscMainnetSccp.InboundEvidence evidence =
        new BscMainnetSccp(
                null,
                null,
                null,
                consensusProvider,
                null,
                null,
                null,
                sourceBridgeEmitterAddress)
            .collectInboundEvidenceFromReceipt(
                new BscMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_BSC,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    receipt,
                    block,
                    (Map<String, Object>) null,
                    null));
    finalityBranchWitness.add("0x" + repeat("99", 32));
    finalityBytes[0] = 0x7d;
    finalityWitness.put("new", "changed");

    assert consensusCalls[0] == 1 : "BSC collection consensus callback must run once";
    final List<Object> receiptSnapshot = (List<Object>) evidence.receipt().get("mutableWitness");
    assert receiptSnapshot.size() == 1 : "BSC collection receipt witness must be snapshotted";
    final Map<String, Object> receiptNestedSnapshot = (Map<String, Object>) receiptSnapshot.get(0);
    assert "keep".equals(receiptNestedSnapshot.get("value"))
        : "BSC collection receipt snapshot must not see callback mutation";
    assert Arrays.equals(new byte[] {(byte) 0xbb}, (byte[]) receiptNestedSnapshot.get("bytes"))
        : "BSC collection receipt bytes must not see callback mutation";
    final Map<String, Object> blockSnapshot =
        (Map<String, Object>) evidence.block().get("mutableWitness");
    assert "block".equals(blockSnapshot.get("value"))
        : "BSC collection block snapshot must not see callback mutation";
    assert Arrays.equals(new byte[] {(byte) 0xcc}, (byte[]) blockSnapshot.get("bytes"))
        : "BSC collection block bytes must not see callback mutation";
    final Map<String, Object> finalitySnapshot =
        (Map<String, Object>) evidence.parliaFinality().get("mutableWitness");
    final List<Object> branchSnapshot = (List<Object>) finalitySnapshot.get("branch");
    assert branchSnapshot.equals(Collections.singletonList(validatorSetHash))
        : "BSC collection finality branch must be snapshotted";
    assert Arrays.equals(new byte[] {(byte) 0xaa}, (byte[]) finalitySnapshot.get("bytes"))
        : "BSC collection finality bytes must be snapshotted";
    assert !finalitySnapshot.containsKey("new")
        : "BSC collection finality snapshot must not see post-collection mutation";
  }

  private static void ethereumReceiptTrieProofBuilderUsesRlpTransactionIndexKeys() {
    final Map<String, Object> receipt =
        sampleEvmReceipt(0, "0x" + repeat("aa", 32), "0x" + repeat("bb", 32), "0x1234");
    final SourceSccpProofs.EvmReceiptTrieProof proof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(receipt), "0x0");

    assert "0x80".equals(SourceSccpProofs.evmReceiptTrieKey("0x0"))
        : "receipt trie key for index zero must be raw RLP 0x80";
    assert "0x01".equals(SourceSccpProofs.evmReceiptTrieKey("0x1"))
        : "single-byte RLP keys below 0x80 must encode as the byte itself";
    assert "0x8180".equals(SourceSccpProofs.evmReceiptTrieKey("0x80"))
        : "receipt trie keys must use RLP integer encoding";
    boolean threw = false;
    try {
      SourceSccpProofs.evmReceiptTrieKey("0x01");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "receipt trie keys must reject noncanonical JSON-RPC quantities";
    assert "0x80".equals(proof.receiptTrieKey) : "proof must expose the selected RLP key";
    assert proof.receiptRlp.equals("0x" + hexLower(SourceSccpProofs.canonicalEvmReceiptRlp(receipt)))
        : "proof must expose the canonical encoded target receipt";
    assert proof.receiptsRoot.matches("0x[0-9a-f]{64}") : "proof must derive a receiptsRoot";
    assert !proof.receiptTrieProofNodes().isEmpty() : "proof must include MPT nodes";

    final Map<String, Object> zeroTopicReceipt =
        sampleEvmReceipt(1, "0x" + repeat("ab", 32), "0x" + repeat("bb", 32), "0x1234");
    zeroTopicReceipt.put(
        "logs",
        Arrays.asList(
            linkedMap(
                "address", "0x" + repeat("12", 20),
                "topics", Arrays.asList("0x" + repeat("00", 32)),
                "data", "0x")));
    final SourceSccpProofs.EvmReceiptTrieProof zeroTopicProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(
            Arrays.asList(receipt, zeroTopicReceipt), "0x0");
    assert proof.receiptRlp.equals(zeroTopicProof.receiptRlp)
        : "generic Ethereum receipt RLP must allow zero log topics";
    final Map<String, Object> zeroAddressReceipt =
        sampleEvmReceipt(1, "0x" + repeat("ac", 32), "0x" + repeat("bb", 32), "0x1234");
    zeroAddressReceipt.put(
        "logs",
        Arrays.asList(
            linkedMap(
                "address", "0x" + repeat("00", 20),
                "topics", Arrays.asList("0x" + repeat("44", 32)),
                "data", "0x")));
    final SourceSccpProofs.EvmReceiptTrieProof zeroAddressProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(
            Arrays.asList(receipt, zeroAddressReceipt), "0x0");
    assert proof.receiptRlp.equals(zeroAddressProof.receiptRlp)
        : "generic Ethereum receipt RLP must allow zero log addresses";

    final Map<String, Object> wrongIndex = new LinkedHashMap<>(receipt);
    wrongIndex.put("transactionIndex", "0x1");
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(wrongIndex), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionIndex");
    }
    assert threw : "receipt proof builder must reject out-of-order block receipts";

    final Map<String, Object> conflictingIndex = new LinkedHashMap<>(receipt);
    conflictingIndex.put("transaction_index", "0x0");
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(conflictingIndex), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockReceipts[0].transactionIndex");
    }
    assert threw : "receipt proof builder must reject duplicate transactionIndex aliases";

    final Map<String, Object> conflictingHash = new LinkedHashMap<>(receipt);
    conflictingHash.put("transaction_hash", receipt.get("transactionHash"));
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(conflictingHash), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockReceipts[0].transactionHash");
    }
    assert threw : "receipt proof builder must reject duplicate transactionHash aliases";

    final Map<String, Object> conflictingGas = new LinkedHashMap<>(receipt);
    conflictingGas.put("cumulative_gas_used", "0x5208");
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(conflictingGas), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.cumulativeGasUsed");
    }
    assert threw : "receipt proof builder must reject duplicate cumulativeGasUsed aliases";

    final Map<String, Object> conflictingBloom = new LinkedHashMap<>(receipt);
    conflictingBloom.put("logs_bloom", "0x" + repeat("00", 256));
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(conflictingBloom), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logsBloom");
    }
    assert threw : "receipt proof builder must reject duplicate logsBloom aliases";

    final Map<String, Object> duplicateHashReceipt =
        sampleEvmReceipt(1, "0x" + repeat("aa", 32), "0x" + repeat("bb", 32), "0x1234");
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(
          Arrays.asList(receipt, duplicateHashReceipt), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash values must be unique");
    }
    assert threw : "receipt proof builder must reject duplicate block receipt transaction hashes";

    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(receipt), "0x1");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block receipt index");
    }
    assert threw : "receipt proof builder must reject out-of-range target indexes";

    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(new ArrayList<Map<String, Object>>(), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("non-empty");
    }
    assert threw : "receipt proof builder must reject empty block receipt lists";

    final List<Map<String, Object>> oversizedReceipts = new ArrayList<>();
    for (int index = 0; index < 4_097; index++) {
      oversizedReceipts.add(receipt);
    }
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(oversizedReceipts, "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("at most");
    }
    assert threw : "receipt proof builder must reject oversized block receipt lists";

    threw = false;
    final Map<String, Object> uppercaseBloomReceipt = new LinkedHashMap<>(receipt);
    uppercaseBloomReceipt.put("logsBloom", "0x" + repeat("AA", 256));
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(uppercaseBloomReceipt);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("lowercase");
    }
    assert threw : "receipt RLP must reject noncanonical uppercase hex";

    threw = false;
    final Map<String, Object> badType = new LinkedHashMap<>(receipt);
    badType.put("type", "0x80");
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(badType);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("typed receipt type");
    }
    assert threw : "typed receipt prefixes must fit one byte below 0x80";

    threw = false;
    final Map<String, Object> unsupportedType = new LinkedHashMap<>(receipt);
    unsupportedType.put("type", "0x7f");
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(unsupportedType);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("not supported");
    }
    assert threw : "unknown typed receipt prefixes must stay unsupported until their payload layout is admitted";

    final Map<String, Object> validReceiptLog =
        linkedMap(
            "address", "0x" + repeat("11", 20),
            "topics", Arrays.asList("0x" + repeat("22", 32)),
            "data", "0x");
    final Map<String, Object> removedLog = new LinkedHashMap<>(validReceiptLog);
    removedLog.put("removed", Boolean.TRUE);
    final Map<String, Object> removedLogReceipt = new LinkedHashMap<>(receipt);
    removedLogReceipt.put("logs", Arrays.asList(removedLog));
    threw = false;
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(removedLogReceipt);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("removed");
    }
    assert threw : "receipt RLP must reject removed logs";

    final Map<String, Object> tooManyTopicsLog = new LinkedHashMap<>(validReceiptLog);
    tooManyTopicsLog.put(
        "topics",
        Arrays.asList(
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32)));
    final Map<String, Object> tooManyTopicsReceipt = new LinkedHashMap<>(receipt);
    tooManyTopicsReceipt.put("logs", Arrays.asList(tooManyTopicsLog));
    threw = false;
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(tooManyTopicsReceipt);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("topics");
    }
    assert threw : "receipt RLP must reject logs with too many topics";
  }

  private static void ethereumInboundCollectionBuildsReceiptProofFromBlockReceipts() {
    final String txHash = "0x" + repeat("aa", 32);
    final String otherTxHash = "0x" + repeat("ab", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("12", 20);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final Map<String, Object> sourceEventLog =
        linkedMap(
            "address", sourceBridgeEmitterAddress,
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
            "data", "0x");
    final Map<String, Object> receipt =
        sampleEvmReceipt(0, txHash, blockHash, "0x1234", Arrays.asList(sourceEventLog));
    final Map<String, Object> otherReceipt =
        sampleEvmReceipt(1, otherTxHash, blockHash, "0x1234");
    final List<Map<String, Object>> blockReceipts = Arrays.asList(receipt, otherReceipt);
    final SourceSccpProofs.EvmReceiptTrieProof trieProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(blockReceipts, "0x0");
    final Map<String, Object> block =
        linkedMap("hash", blockHash, "number", "0x1234", "receiptsRoot", trieProof.receiptsRoot);
    final Map<String, Object> beaconFinality =
        linkedMap(
            "executionBlockNumber", "0x1234",
            "executionBlockHash", blockHash,
            "executionReceiptsRoot", trieProof.receiptsRoot,
            "finalizedHeaderRoot", "0x" + repeat("dd", 32),
            "syncCommitteeRoot", "0x" + repeat("cc", 32),
            "beaconSlot", "0x20",
            "syncCommitteeBits", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
            "syncCommitteeSignature", "0x" + repeat("34", 96),
            "syncCommitteeParticipation", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION,
            "syncSignatureSlot", "65");
    final List<byte[]> inclusionBranch = Arrays.asList(repeatedWord(0x44));
    final List<String> calls = new ArrayList<>();
    final EthereumMainnetSccp sdk =
        new EthereumMainnetSccp(
            null,
            null,
            (method, params) -> {
              calls.add(method);
              if ("eth_chainId".equals(method)) {
                return "0x1";
              }
              if ("eth_getBlockReceipts".equals(method)) {
                assert params.equals(Arrays.<Object>asList("0x1234"))
                    : "block receipt fetch must use the receipt block number";
                return blockReceipts;
              }
              throw new IllegalArgumentException("unexpected method " + method);
            },
            null,
            null);

    final EthereumMainnetSccp.InboundEvidence evidence =
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receipt,
                block,
                beaconFinality,
                null,
                null,
                null,
                sourceBridgeEmitterAddress,
                null,
                inclusionBranch));

    assert calls.equals(Arrays.asList("eth_chainId", "eth_getBlockReceipts"))
        : "collection must validate mainnet and fetch block receipts";
    assert sourceEventDigest.equals(evidence.sourceEventDigest())
        : "collection must validate the SCCP source event";
    assert evidence.blockReceipts().equals(blockReceipts)
        : "collection must retain block receipt evidence";
    final EthereumMainnetSccp.ReceiptProof receiptProof = evidence.receiptProof();
    assert receiptProof != null : "collection must auto-build receiptProof";
    assert receiptProof.sourceDomain() == EvmSccpProver.DOMAIN_ETH
        : "auto-built receiptProof must stay on ETH";
    assert "0".equals(receiptProof.receiptRootIndex())
        : "receiptRootIndex must be the transaction index";
    assert "32".equals(receiptProof.beaconSlot()) : "beacon slot must be normalized decimal";
    assert "4660".equals(receiptProof.executionBlockNumber())
        : "execution block number must be normalized decimal";
    assert trieProof.receiptsRoot.equals(receiptProof.executionReceiptsRoot())
        : "receipt proof must bind the computed receipt root";
    assert Arrays.equals(
        trieProof.receiptTrieProofNodes().get(0), receiptProof.receiptTrieProofNodes().get(0))
        : "receipt proof must carry generated MPT nodes";
    assert Arrays.equals(inclusionBranch.get(0), receiptProof.inclusionBranch().get(0))
        : "receipt proof must carry the app-supplied SSZ inclusion branch";
    final String expectedHash =
        SourceSccpProofs.evmReceiptProofHash(
            receiptProof.sourceEventDigest(),
            receiptProof.beaconSlot(),
            receiptProof.executionBlockNumber(),
            receiptProof.executionBlockHash(),
            receiptProof.executionReceiptsRoot(),
            receiptProof.beaconFinalizedRoot(),
            receiptProof.syncCommitteeRoot(),
            receiptProof.receiptRootIndex(),
            receiptProof.receiptTrieProofNodes(),
            receiptProof.inclusionBranch());
    assert expectedHash.equals(evidence.receiptProofHash())
        : "collection must derive receiptProofHash from the generated receiptProof";

    boolean threw = false;
    for (final String[] missingFinalityCase :
        new String[][] {
          {"finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot"},
          {"syncCommitteeRoot", "beaconFinality.syncCommitteeRoot"},
          {"beaconSlot", "beaconFinality.beaconSlot"}
        }) {
      final Map<String, Object> incompleteFinality = new LinkedHashMap<>(beaconFinality);
      incompleteFinality.remove(missingFinalityCase[0]);
      threw = false;
      try {
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receipt,
                block,
                incompleteFinality,
                null,
                null,
                null,
                sourceBridgeEmitterAddress,
                blockReceipts,
                inclusionBranch));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains(missingFinalityCase[1]);
      }
      assert threw : "collection must reject missing " + missingFinalityCase[1];
    }

    final String[][] receiptAliasConflicts =
        new String[][] {
          {"transaction_hash", "0x" + repeat("ac", 32), "receipt.transactionHash"},
          {"block_hash", "0x" + repeat("ac", 32), "receipt.blockHash"},
          {"block_number", "0x1235", "receipt.blockNumber"},
          {"transaction_index", "0x0", "receipt.transactionIndex"}
        };
    for (final String[] aliasCase : receiptAliasConflicts) {
      final Map<String, Object> conflictingReceipt = new LinkedHashMap<>(receipt);
      conflictingReceipt.put(aliasCase[0], aliasCase[1]);
      threw = false;
      try {
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                conflictingReceipt,
                block,
                beaconFinality,
                null,
                null,
                null,
                sourceBridgeEmitterAddress,
                blockReceipts,
                inclusionBranch));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains(aliasCase[2]);
      }
      assert threw : "collection must reject duplicate receipt alias " + aliasCase[2];
    }

    final String[][] blockNumberAliasConflicts =
        new String[][] {{"blockNumber", "0x1235"}, {"block_number", "0x1235"}};
    for (final String[] aliasCase : blockNumberAliasConflicts) {
      final Map<String, Object> conflictingBlock = new LinkedHashMap<>(block);
      conflictingBlock.put(aliasCase[0], aliasCase[1]);
      threw = false;
      try {
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receipt,
                conflictingBlock,
                beaconFinality,
                null,
                null,
                null,
                sourceBridgeEmitterAddress,
                blockReceipts,
                inclusionBranch));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("block.number");
      }
      assert threw : "collection must reject duplicate block number alias";
    }

    final Map<String, Object> conflictingReceiptsRootBlock = new LinkedHashMap<>(block);
    conflictingReceiptsRootBlock.put("receipts_root", "0x" + repeat("ac", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              conflictingReceiptsRootBlock,
              beaconFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              blockReceipts,
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.receiptsRoot");
    }
    assert threw : "collection must reject duplicate receiptsRoot aliases";

    final String[][] indexedReceiptAliasConflicts =
        new String[][] {
          {"block_hash", "0x" + repeat("ac", 32), "blockReceipts.blockHash"},
          {"block_number", "0x1235", "blockReceipts.blockNumber"}
        };
    for (final String[] aliasCase : indexedReceiptAliasConflicts) {
      final Map<String, Object> conflictingIndexedReceipt = new LinkedHashMap<>(receipt);
      conflictingIndexedReceipt.put(aliasCase[0], aliasCase[1]);
      threw = false;
      try {
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receipt,
                block,
                beaconFinality,
                null,
                null,
                null,
                sourceBridgeEmitterAddress,
                Arrays.asList(conflictingIndexedReceipt, otherReceipt),
                inclusionBranch));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains(aliasCase[2]);
      }
      assert threw : "collection must reject duplicate indexed receipt alias " + aliasCase[2];
    }

    final Map<String, Object> conflictingIndexedHashReceipt = new LinkedHashMap<>(receipt);
    conflictingIndexedHashReceipt.put("transaction_hash", receipt.get("transactionHash"));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              beaconFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              Arrays.asList(conflictingIndexedHashReceipt, otherReceipt),
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockReceipts[0].transactionHash");
    }
    assert threw : "collection must reject duplicate indexed receipt transactionHash aliases";

    threw = false;
    final Map<String, Object> wrongRootBlock = new LinkedHashMap<>(block);
    wrongRootBlock.put("receiptsRoot", "0x" + repeat("99", 32));
    final Map<String, Object> wrongRootFinality = new LinkedHashMap<>(beaconFinality);
    wrongRootFinality.put("executionReceiptsRoot", "0x" + repeat("99", 32));
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              wrongRootBlock,
              wrongRootFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              blockReceipts,
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("computed receipt trie root");
    }
    assert threw : "collection must reject receipt roots that do not match reconstructed block receipts";

    final Map<String, Object> mismatchedIndexedReceipt = new LinkedHashMap<>(receipt);
    mismatchedIndexedReceipt.put("logs", new ArrayList<Map<String, Object>>());
    final List<Map<String, Object>> mismatchedBlockReceipts =
        Arrays.asList(mismatchedIndexedReceipt, otherReceipt);
    final SourceSccpProofs.EvmReceiptTrieProof mismatchedReceiptProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(mismatchedBlockReceipts, "0x0");
    final Map<String, Object> mismatchedBlock = new LinkedHashMap<>(block);
    mismatchedBlock.put("receiptsRoot", mismatchedReceiptProof.receiptsRoot);
    final Map<String, Object> mismatchedFinality = new LinkedHashMap<>(beaconFinality);
    mismatchedFinality.put("executionReceiptsRoot", mismatchedReceiptProof.receiptsRoot);
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              mismatchedBlock,
              mismatchedFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              mismatchedBlockReceipts,
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt RLP");
    }
    assert threw : "collection must reject block receipts whose RLP differs from the source receipt";

    final Map<String, Object> blockHashDriftReceipt = new LinkedHashMap<>(receipt);
    blockHashDriftReceipt.put("blockHash", "0x" + repeat("99", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              beaconFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              Arrays.asList(blockHashDriftReceipt, otherReceipt),
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockHash");
    }
    assert threw : "collection must reject block receipt blockHash drift";

    final Map<String, Object> blockNumberDriftReceipt = new LinkedHashMap<>(receipt);
    blockNumberDriftReceipt.put("blockNumber", "0x1235");
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              beaconFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              Arrays.asList(blockNumberDriftReceipt, otherReceipt),
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockNumber");
    }
    assert threw : "collection must reject block receipt blockNumber drift";
  }

  private static void ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final Map<String, Object> receipt =
        linkedMap(
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "status", "0x1");
    final Map<String, Object> block =
        linkedMap(
            "hash", blockHash,
            "number", "0x1234",
            "receiptsRoot", "0x" + repeat("cc", 32),
            "beaconSlot", "64");
    final List<String> calls = new ArrayList<>();
    final List<Map<String, String>> headerCalls = new ArrayList<>();
    final EthereumMainnetSccp.BeaconRestTransport transport =
        (url, headers) -> {
          calls.add(url);
          headerCalls.add(headers);
          if ("https://beacon.example/eth/v1/beacon/headers/finalized".equals(url)) {
            return beaconResponse(beaconHeaderJson(false, true));
          }
          if ("https://beacon.example/eth/v1/beacon/headers/64".equals(url)) {
            return beaconResponse(beaconHeaderJson(false, true));
          }
          if ("https://beacon.example/eth/v1/beacon/blocks/64/root".equals(url)) {
            return beaconResponse(beaconBlockRootJson());
          }
          if ("https://beacon.example/eth/v2/beacon/blocks/64".equals(url)) {
            return beaconResponse(beaconBlockJson("64", blockHash, "4660", "0x" + repeat("cc", 32)));
          }
          if ("https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints"
              .equals(url)) {
            return beaconResponse(beaconCheckpointJson());
          }
          if ("https://beacon.example/eth/v1/beacon/light_client/finality_update".equals(url)) {
            return beaconResponse(beaconFinalityUpdateJson());
          }
          throw new IllegalArgumentException("unexpected Beacon REST URL " + url);
        };
    final EthereumMainnetSccp.BeaconRestConsensusProvider provider =
        new EthereumMainnetSccp.BeaconRestConsensusProvider(
            "https://beacon.example/eth/v1",
            "0x" + repeat("ee", 32),
            null,
            linkedStringMap("Authorization", "Bearer local"),
            true,
            transport);
    final EthereumMainnetSccp.InboundEvidence evidence =
        new EthereumMainnetSccp(null, null, null, provider, null, null)
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EthereumMainnetSccp.DOMAIN_ETH,
                    EthereumMainnetSccp.DOMAIN_SORA,
                    null,
                    receipt,
                    block,
                    null,
                    null));

    assert "4660".equals(evidence.beaconFinality().get("executionBlockNumber"));
    assert blockHash.equals(evidence.beaconFinality().get("executionBlockHash"));
    assert ("0x" + repeat("cc", 32)).equals(evidence.beaconFinality().get("executionReceiptsRoot"));
    assert BEACON_HEADER_ROOT_SLOT_64.equals(evidence.beaconFinality().get("finalizedHeaderRoot"));
    assert ("0x" + repeat("ee", 32)).equals(evidence.beaconFinality().get("syncCommitteeRoot"));
    assert "64".equals(evidence.beaconFinality().get("beaconSlot"));
    assert ETHEREUM_FINALITY_BRANCH.equals(evidence.beaconFinality().get("finalityBranch"));
    assert ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS.equals(
        evidence.beaconFinality().get("syncCommitteeBits"));
    assert ("0x" + repeat("34", 96)).equals(evidence.beaconFinality().get("syncCommitteeSignature"));
    assert ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_PARTICIPATION.equals(
        evidence.beaconFinality().get("syncCommitteeParticipation"));
    assert "65".equals(evidence.beaconFinality().get("syncSignatureSlot"));
    assert calls.equals(
        Arrays.asList(
            "https://beacon.example/eth/v1/beacon/headers/finalized",
            "https://beacon.example/eth/v1/beacon/headers/64",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update"));
    assert "Bearer local".equals(headerCalls.get(0).get("Authorization"));
  }

  private static void ethereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final Map<String, Object> receipt =
        linkedMap(
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "status", "0x1");
    final Map<String, Object> block =
        linkedMap(
            "hash", blockHash,
            "number", "0x1234",
            "receiptsRoot", "0x" + repeat("cc", 32),
            "timestamp", "0x364");
    final List<String> calls = new ArrayList<>();
    final EthereumMainnetSccp.BeaconRestTransport transport =
        (url, headers) -> {
          calls.add(url);
          if ("https://beacon.example/eth/v1/beacon/genesis".equals(url)) {
            return beaconResponse(beaconGenesisJson("100"));
          }
          if ("https://beacon.example/eth/v1/beacon/headers/finalized".equals(url)) {
            return beaconResponse(beaconHeaderJson(false, true));
          }
          if ("https://beacon.example/eth/v1/beacon/headers/64".equals(url)) {
            return beaconResponse(beaconHeaderJson(false, true));
          }
          if ("https://beacon.example/eth/v1/beacon/blocks/64/root".equals(url)) {
            return beaconResponse(beaconBlockRootJson());
          }
          if ("https://beacon.example/eth/v2/beacon/blocks/64".equals(url)) {
            return beaconResponse(beaconBlockJson("64", blockHash, "4660", "0x" + repeat("cc", 32)));
          }
          if ("https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints"
              .equals(url)) {
            return beaconResponse(beaconCheckpointJson());
          }
          if ("https://beacon.example/eth/v1/beacon/light_client/finality_update".equals(url)) {
            return beaconResponse(beaconFinalityUpdateJson());
          }
          throw new IllegalArgumentException("unexpected Beacon REST URL " + url);
        };
    final EthereumMainnetSccp.BeaconRestConsensusProvider provider =
        new EthereumMainnetSccp.BeaconRestConsensusProvider(
            "https://beacon.example/eth/v1",
            "0x" + repeat("ee", 32),
            null,
            java.util.Collections.emptyMap(),
            true,
            transport);
    final EthereumMainnetSccp.InboundEvidence evidence =
        new EthereumMainnetSccp(null, null, null, provider, null, null)
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EthereumMainnetSccp.DOMAIN_ETH,
                    EthereumMainnetSccp.DOMAIN_SORA,
                    null,
                    receipt,
                    block,
                    null,
                    null));

    assert BEACON_HEADER_ROOT_SLOT_64.equals(evidence.beaconFinality().get("finalizedHeaderRoot"));
    assert "64".equals(evidence.beaconFinality().get("beaconSlot"));
    assert calls.equals(
        Arrays.asList(
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/finalized",
            "https://beacon.example/eth/v1/beacon/headers/64",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update"));
  }

  private static void ethereumMainnetBeaconRestHttpTransportRejectsOversizedBodies()
      throws Exception {
    final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    server.createContext(
        "/oversized",
        exchange -> {
          final byte[] body = new byte[1024 * 1024 + 1];
          Arrays.fill(body, (byte) 0x7b);
          exchange.sendResponseHeaders(200, body.length);
          try (OutputStream output = exchange.getResponseBody()) {
            output.write(body);
          }
        });
    server.start();
    try {
      boolean threw = false;
      try {
        new EthereumMainnetSccp.BeaconRestHttpTransport()
            .get(
                "http://127.0.0.1:" + server.getAddress().getPort() + "/oversized",
                java.util.Collections.emptyMap());
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("response body must be at most");
      }
      assert threw : "Beacon REST HTTP transport must reject oversized response bodies";
    } finally {
      server.stop(0);
    }
  }

  private static void ethereumMainnetBeaconRestConsensusProviderRejectsUnsafeFinality() {
    final Map<String, Object> block =
        linkedMap(
            "hash", "0x" + repeat("bb", 32),
            "number", "0x1234",
            "receiptsRoot", "0x" + repeat("cc", 32));

    boolean threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, null, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requires block");
    }
    assert threw : "Beacon REST provider must require an execution block";

    threw = false;
    try {
      beaconRestProvider(
              new EthereumMainnetSccp.BeaconRestResponse(
                  503, "{}".getBytes(StandardCharsets.UTF_8), "Unavailable"),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("request failed 503 Unavailable");
    }
    assert threw : "Beacon REST provider must reject non-2xx header responses";

    threw = false;
    try {
      beaconRestProvider(
              new EthereumMainnetSccp.BeaconRestResponse(200, new byte[1024 * 1024 + 1]),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("response body must be at most");
    }
    assert threw : "Beacon REST provider must reject oversized header responses";

    final Map<String, Object> historicalBlock = new LinkedHashMap<>(block);
    historicalBlock.put("beaconSlot", "32");
    final EthereumMainnetSccp.BeaconRestTransport historicalTransport =
        (url, headers) -> {
          if ("https://beacon.example/eth/v1/beacon/headers/finalized".equals(url)) {
            return beaconResponse(beaconHeaderJson(false, true));
          }
          if ("https://beacon.example/eth/v1/beacon/headers/32".equals(url)) {
            return beaconResponse(beaconHeaderJson(false, true, "aa", "32"));
          }
          throw new IllegalArgumentException("unexpected Beacon REST URL " + url);
        };
    threw = false;
    try {
      new EthereumMainnetSccp.BeaconRestConsensusProvider(
              "https://beacon.example/eth/v1",
              "0x" + repeat("ee", 32),
              null,
              java.util.Collections.emptyMap(),
              true,
              historicalTransport)
          .collectFinalityEvidence(null, historicalBlock, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("historical target blocks require an ancestry proof");
    }
    assert threw : "Beacon REST provider must reject historical target slots without ancestry proof";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(true, true)),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("must not be execution optimistic");
    }
    assert threw : "Beacon REST provider must reject optimistic headers";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true)
                      .replace("\"execution_optimistic\":false", "\"execution_optimistic\":\"false\"")),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution_optimistic must be a boolean");
    }
    assert threw : "Beacon REST provider must reject malformed optimistic flags";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true).replace("\"finalized\":true", "\"finalized\":\"true\"")),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalized must be a boolean");
    }
    assert threw : "Beacon REST provider must reject malformed finalized flags";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true).replace("\"canonical\":true", "\"canonical\":\"true\"")),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical must be a boolean");
    }
    assert threw : "Beacon REST provider must reject malformed canonical flags";

    for (final String[] rootCase :
        new String[][] {
          {"parent_root", "01"},
          {"state_root", "02"},
          {"body_root", "03"},
        }) {
      threw = false;
      try {
        beaconRestProvider(
                beaconResponse(
                    beaconHeaderJson(false, true)
                        .replace(
                            "\"" + rootCase[0] + "\":\"0x" + repeat(rootCase[1], 32) + "\"",
                            "\"" + rootCase[0] + "\":\"0x\"")),
                beaconResponse(beaconCheckpointJson()),
                "0x" + repeat("ee", 32),
                null)
            .collectFinalityEvidence(null, block, null);
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains(rootCase[0]);
      }
      assert threw : "Beacon REST provider must reject malformed " + rootCase[0];
    }

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true)
                      .replace(
                          "\"signature\":\"0x" + repeat("12", 96) + "\"",
                          "\"signature\":\"0x" + repeat("12", 95) + "\"")),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("signature");
    }
    assert threw : "Beacon REST provider must reject malformed finalized header signatures";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockRootJson("99")),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalized block root must match finalized header root");
    }
    assert threw : "Beacon REST provider must reject finalized block root/header drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("65", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalized block slot must match finalized header slot");
    }
    assert threw : "Beacon REST provider must reject finalized block slot drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("99", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution payload block_hash must match block.hash");
    }
    assert threw : "Beacon REST provider must reject execution payload block-hash drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4661", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution payload block_number must match block.number");
    }
    assert threw : "Beacon REST provider must reject execution payload block-number drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("99", 32))),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution payload receipts_root must match block.receiptsRoot");
    }
    assert threw : "Beacon REST provider must reject execution payload receipts-root drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, false)),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("must be finalized");
    }
    assert threw : "Beacon REST provider must reject unfinalized headers";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson("99")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("checkpoint root must match");
    }
    assert threw : "Beacon REST provider must reject checkpoint/header mismatches";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockRootJson()),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              beaconResponse(beaconFinalityUpdateJson("64", "65", "0x" + repeat("00", 64))),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sync_committee_bits must contain at least one participant");
    }
    assert threw : "Beacon REST provider must reject empty sync committee aggregate bits";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockRootJson()),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              beaconResponse(beaconFinalityUpdateJson("64", "65", "0x01" + repeat("00", 63))),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sync_committee_bits must contain Ethereum sync committee supermajority");
    }
    assert threw : "Beacon REST provider must reject under-quorum sync committee aggregate bits";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockRootJson()),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              beaconResponse(
                  beaconFinalityUpdateJson(
                      "64",
                      "65",
                      ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
                      "0x" + repeat("34", 96),
                      false,
                      ETHEREUM_FINALITY_BRANCH)),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finality_branch");
    }
    assert threw : "Beacon REST provider must reject missing finality branch";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockRootJson()),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              beaconResponse(
                  beaconFinalityUpdateJson(
                      "64",
                      "65",
                      ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
                      "0x" + repeat("34", 96),
                      true,
                      ETHEREUM_FINALITY_BRANCH.subList(0, 5))),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finality_branch");
    }
    assert threw : "Beacon REST provider must reject malformed finality branch";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockRootJson()),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson()),
              beaconResponse(
                  beaconFinalityUpdateJson(
                      "64",
                      "65",
                      ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS,
                      "0x" + repeat("00", 96))),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sync_committee_signature must not be zero");
    }
    assert threw : "Beacon REST provider must reject zero sync committee aggregate signatures";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson()),
              null,
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requires syncCommitteeRoot or syncCommitteePayload");
    }
    assert threw : "Beacon REST provider must require local sync committee material";

    final List<byte[]> publicKeys = new ArrayList<>();
    final List<String> weights = new ArrayList<>();
    final List<byte[]> pops = new ArrayList<>();
    for (int index = 0; index < 512; index++) {
      publicKeys.add(indexedSyncCommitteeBytes(0x11, 48, index));
      weights.add("1");
      pops.add(indexedSyncCommitteeBytes(0x22, 96, index));
    }
    final byte[] syncCommitteePayload =
        SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(publicKeys, weights, pops);
    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson()),
              "0x" + repeat("ee", 32),
              syncCommitteePayload)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("syncCommitteeRoot must match syncCommitteePayload");
    }
    assert threw : "Beacon REST provider must reject sync committee root/payload mismatches";
  }

  private static void ethereumMainnetFacadeBuildsLocalAdmissionSubmission() {
    final EthereumMainnetSccp.LocalAdmissionSubmissionInput input =
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32));
    final EthereumMainnetSccp.LocalAdmissionSubmission submission =
        EthereumMainnetSccp.buildLocalAdmissionSubmission(input);
    final EthereumMainnetSccp.LocalAdmissionSubmission facadeSubmission =
        new EthereumMainnetSccp().buildLocalAdmission(input);

    assert EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1.equals(
            submission.platformPayload())
        : "Ethereum local admission platform payload must be local_admission";
    assert EthereumMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1.equals(
            submission.envelopeEncoding())
        : "Ethereum local admission must use the Norito envelope";
    assert EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1.equals(
            submission.verifierEntrypoint())
        : "Ethereum local admission must target SubmitBridgeProof";
    assert submission.sourceDomain() == EvmSccpProver.DOMAIN_ETH
        : "Ethereum local admission source must be ETH";
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_SORA
        : "Ethereum local admission target must be SORA";
    assert submission.arguments().isEmpty() : "Ethereum local admission must not add call arguments";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "Ethereum local admission must copy proof bytes";
    assert Arrays.equals(new byte[] {4, 5, 6}, submission.publicInputsBytes())
        : "Ethereum local admission must copy public input bytes";
    assert Arrays.equals(new byte[] {7, 8, 9}, submission.bundleBytes())
        : "Ethereum local admission must copy bundle bytes";
    assert Arrays.equals(new byte[] {10, 11, 12}, submission.envelopeBytes())
        : "Ethereum local admission must copy envelope bytes";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.localAdmission().proofBytes())
        : "Ethereum local admission payload must carry proof bytes";
    assert submission.envelopeHex().equals(facadeSubmission.envelopeHex())
        : "facade local admission helper must match static helper";

    input.proofBytes()[0] = 99;
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "Ethereum local admission must not expose mutable proof storage";

    boolean threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              EthereumMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("ETH -> SORA");
    }
    assert threw : "Ethereum local admission must reject wrong source domains";

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {0, 0},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be all zero");
    }
    assert threw : "Ethereum local admission must reject all-zero proof bytes";

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {0, 0},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "publicInputsBytes must not be all zero",
        "Ethereum local admission must reject all-zero public input bytes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {0, 0},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "bundleBytes must not be all zero",
        "Ethereum local admission must reject all-zero bundle bytes");

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[0],
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeBytes must not be empty");
    }
    assert threw : "Ethereum local admission must reject empty envelope bytes";

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {0, 0},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "envelopeBytes must not be all zero",
        "Ethereum local admission must reject all-zero envelope bytes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("00", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "statementHash must not be zero",
        "Ethereum local admission must reject zero statement hashes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("00", 32),
            "0x" + repeat("88", 32)),
        "sourceVerifierMaterialHash must not be zero",
        "Ethereum local admission must reject zero source material hashes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("00", 32)),
        "sourceAdapterEngineDeploymentHash must not be zero",
        "Ethereum local admission must reject zero source adapter deployment hashes");

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              EthereumMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              "abi_tuple_v1",
              EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeEncoding");
    }
    assert threw : "Ethereum local admission must reject stale metadata";

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              "debug-proof-family",
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofFamily");
    }
    assert threw : "Ethereum local admission must reject stale proof families";
  }

  private static void assertEthereumLocalAdmissionRejected(
      final EthereumMainnetSccp.LocalAdmissionSubmissionInput input,
      final String expectedMessagePart,
      final String assertionMessage) {
    boolean threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(input);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains(expectedMessagePart);
    }
    assert threw : assertionMessage;
  }

  private static void bscMainnetInboundFacadeUsesMainnetRpcAndRejectsDrift() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("44", 20);
    final Map<String, Object> sourceEventLog =
        linkedMap(
            "address", sourceBridgeEmitterAddress,
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
            "data", "0x");
    final Map<String, Object> receipt = linkedMap(
        "transactionHash", txHash,
        "blockHash", blockHash,
        "blockNumber", "0x1234",
        "status", "0x1",
        "logs", Collections.singletonList(sourceEventLog));
    final Map<String, Object> block = linkedMap(
        "hash", blockHash,
        "number", "0x1234",
        "receiptsRoot", "0x" + repeat("cc", 32));
    final BscMainnetSccp.ReceiptProof receiptProof =
        new BscMainnetSccp.ReceiptProof(
            sourceEventDigest,
            "36",
            "4660",
            blockHash,
            "0x" + repeat("cc", 32),
            "0x" + repeat("ab", 32),
            "0x" + repeat("dd", 32),
            "3",
            Arrays.asList(new byte[] {0x01}, new byte[] {0x02, 0x03}),
            Collections.singletonList(repeatedByteArray(0x11, 32)));
    final String receiptProofHash =
        SourceSccpProofs.bscReceiptProofHash(
            receiptProof.sourceEventDigest(),
            receiptProof.validatorEpoch(),
            receiptProof.blockNumber(),
            receiptProof.blockHash(),
            receiptProof.receiptsRoot(),
            receiptProof.validatorSetHash(),
            receiptProof.commitSealHash(),
            receiptProof.receiptRootIndex(),
            receiptProof.receiptTrieProofNodes(),
            receiptProof.inclusionBranch());
    final BscMainnetSccp.ParliaFinalityEvidence parliaFinalityEvidence =
        new BscMainnetSccp.ParliaFinalityEvidence(
            "0x1234",
            blockHash,
            "0x" + repeat("cc", 32),
            linkedMap(
                "validatorEpoch", "0x24",
                "validatorSetHash", "0x" + repeat("ab", 32),
                "commitSealHash", "0x" + repeat("dd", 32)));
    final Map<String, Object> parliaFinality = parliaFinalityEvidence.toMap();
    final ArrayList<String> calls = new ArrayList<>();
    final BscMainnetSccp sdk =
        new BscMainnetSccp(
            null,
            null,
            (method, params) -> {
              calls.add(method);
              if ("eth_chainId".equals(method)) {
                return "0x38";
              }
              if ("eth_getTransactionReceipt".equals(method)) {
                assert params.size() == 1 && txHash.equals(params.get(0))
                    : "receipt request must use the requested BSC tx hash";
                return receipt;
              }
              if ("eth_getBlockByHash".equals(method)) {
                assert params.size() == 2 && blockHash.equals(params.get(0))
                    : "block request must use BSC receipt block hash";
                return block;
              }
              throw new IllegalArgumentException("unexpected method " + method);
            },
            null,
            evidence -> {
              assert evidence.sourceDomain() == EvmSccpProver.DOMAIN_BSC
                  : "BSC inbound evidence must be BSC sourced";
              assert evidence.targetDomain() == EvmSccpProver.DOMAIN_SORA
                  : "BSC inbound evidence must target SORA";
              assert txHash.equals(evidence.transactionHash())
                  : "BSC inbound evidence must carry normalized tx hash";
              assert blockHash.equals(evidence.parliaFinality().get("executionBlockHash"))
                  : "BSC inbound evidence must carry bound finality block hash";
              assert receiptProofHash.equals(evidence.receiptProofHash())
                  : "BSC inbound evidence must derive receiptProofHash from receiptProof";
              assert receiptProof.blockHash().equals(evidence.receiptProof().blockHash())
                  : "BSC inbound evidence must retain receiptProof material";
              assert sourceEventDigest.equals(evidence.receiptProof().sourceEventDigest())
                  : "BSC inbound evidence must retain source-event transcript";
              assert sourceEventDigest.equals(evidence.sourceEventDigest())
                  : "BSC inbound evidence must carry receipt source-event digest";
              assert sourceBridgeEmitterAddress.equals(evidence.sourceBridgeEmitterAddress())
                  : "BSC inbound evidence must carry source bridge emitter";
              return new byte[] {1, 2, 3};
            },
            proof -> {
              assert Arrays.equals(new byte[] {1, 2, 3}, proof)
                  : "BSC inbound submitter must receive proof bytes";
              return "submitted";
            },
            null,
            sourceBridgeEmitterAddress);
    final BscMainnetSccp.InboundEvidence evidence =
        sdk.collectInboundEvidenceFromReceipt(
            BscMainnetSccp.InboundEvidence.withParliaFinalityEvidence(
                EvmSccpProver.DOMAIN_BSC,
                EvmSccpProver.DOMAIN_SORA,
                txHash,
                null,
                null,
                parliaFinalityEvidence,
                receiptProof,
                null));
    assert txHash.equals(evidence.transactionHash()) : "BSC evidence must retain tx hash";
    assert receipt.equals(evidence.receipt()) : "BSC evidence must carry receipt";
    assert block.equals(evidence.block()) : "BSC evidence must carry block";
    assert "4660".equals(evidence.parliaFinality().get("executionBlockNumber"))
        : "BSC evidence must normalize Parlia execution block number";
    assert blockHash.equals(evidence.parliaFinality().get("executionBlockHash"))
        : "BSC evidence must retain Parlia execution block hash";
    assert receiptProofHash.equals(evidence.receiptProofHash())
        : "BSC evidence must derive receiptProofHash from receiptProof";
    assert sourceEventDigest.equals(evidence.sourceEventDigest())
        : "BSC evidence must derive source event digest";
    assert sourceBridgeEmitterAddress.equals(evidence.sourceBridgeEmitterAddress())
        : "BSC evidence must retain source bridge emitter address";
    final byte[] mutableReceiptProofNode = receiptProof.receiptTrieProofNodes().get(0);
    final byte[] mutableReceiptProofBranch = receiptProof.inclusionBranch().get(0);
    mutableReceiptProofNode[0] = 0x7f;
    mutableReceiptProofBranch[0] = 0x7f;
    assert Arrays.equals(new byte[] {0x01}, evidence.receiptProof().receiptTrieProofNodes().get(0))
        : "BSC collection must snapshot receiptProof trie nodes";
    assert Arrays.equals(repeatedByteArray(0x11, 32), evidence.receiptProof().inclusionBranch().get(0))
        : "BSC collection must snapshot receiptProof inclusion branches";
    assert calls.equals(Arrays.asList("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"))
        : "BSC inbound collection must validate mainnet and fetch receipt/block";
    final BscMainnetSccp.InboundEvidence providerFinalityEvidence =
        new BscMainnetSccp(
                null,
                null,
                (method, params) -> {
                  if ("eth_chainId".equals(method)) {
                    return "0x38";
                  }
                  if ("eth_getTransactionReceipt".equals(method)) {
                    return receipt;
                  }
                  if ("eth_getBlockByHash".equals(method)) {
                    return block;
                  }
                  throw new IllegalArgumentException("unexpected method " + method);
                },
                (receiptInput, blockInput, transactionHashInput) -> parliaFinality,
                null,
                null,
                null,
                sourceBridgeEmitterAddress)
            .collectInboundEvidenceFromReceipt(
                new BscMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_BSC,
                    EvmSccpProver.DOMAIN_SORA,
                    txHash,
                    null,
                    null,
                    null,
                    null));
    assert blockHash.equals(providerFinalityEvidence.parliaFinality().get("executionBlockHash"))
        : "BSC consensus provider finality must be normalized";
    assert sourceEventDigest.equals(providerFinalityEvidence.sourceEventDigest())
        : "BSC consensus provider collection must validate source event";
    assert Arrays.equals(new byte[] {1, 2, 3}, sdk.proveInboundToSora(evidence))
        : "BSC inbound prover must receive validated evidence";
    assert "submitted".equals(sdk.submitInboundToIroha(new byte[] {1, 2, 3}))
        : "BSC inbound submitter must return caller result";

    boolean threw = false;
    final BscMainnetSccp.InboundEvidence receiptProofHashOnlyEvidence =
        new BscMainnetSccp()
            .collectInboundEvidenceFromReceipt(
                new BscMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_BSC,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    null,
                    null,
                    null,
                    receiptProofHash));
    assert receiptProofHash.equals(receiptProofHashOnlyEvidence.receiptProofHash())
        : "BSC inbound collection must accept hash-only receiptProofHash evidence";
    assert receiptProofHashOnlyEvidence.receiptProof() == null
        : "hash-only BSC evidence must not synthesize a receiptProof";

    try {
      new BscMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  receiptProof,
                  "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProofHash must match receiptProof");
    }
    assert threw : "BSC inbound collection must reject conflicting receiptProofHash";

    threw = false;
    try {
      new BscMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  new BscMainnetSccp.ReceiptProof(
                      EvmSccpProver.DOMAIN_ETH,
                      receiptProof.sourceEventDigest(),
                      receiptProof.validatorEpoch(),
                      receiptProof.blockNumber(),
                      receiptProof.blockHash(),
                      receiptProof.receiptsRoot(),
                      receiptProof.validatorSetHash(),
                      receiptProof.commitSealHash(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.sourceDomain");
    }
    assert threw : "BSC inbound collection must reject cross-lane receiptProof transcripts";

    threw = false;
    try {
      new BscMainnetSccp(null, null, (method, params) -> "0x1", null, null)
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 56");
    }
    assert threw : "BSC inbound collection must reject non-mainnet RPC";

    threw = false;
    try {
      new BscMainnetSccp(null, null, (method, params) -> "56", null, null)
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "BSC inbound collection must reject decimal eth_chainId RPC";

    threw = false;
    try {
      new BscMainnetSccp(null, null, (method, params) -> Long.valueOf(56L), null, null)
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "BSC inbound collection must reject numeric eth_chainId RPC";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be BSC");
    }
    assert threw : "BSC inbound collection must reject foreign source domains";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x0"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("status must be 0x1");
    }
    assert threw : "BSC inbound collection must reject failed receipts";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "BSC inbound collection must reject receipts without block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x0",
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "BSC inbound collection must reject zero receipt block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              txHash,
              linkedMap(
                  "transactionHash", "0x" + repeat("ab", 32),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash must match");
    }
    assert threw : "BSC inbound collection must reject receipt tx drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", "0x" + repeat("bc", 32),
                  "number", "0x1234",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.hash must match");
    }
    assert threw : "BSC inbound collection must reject block hash drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "BSC inbound collection must reject blocks without numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "number", "0x0",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "BSC inbound collection must reject zero block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "number", "0x1235",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number must match");
    }
    assert threw : "BSC inbound collection must reject block number drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash.toUpperCase(),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical lowercase");
    }
    assert threw : "BSC inbound collection must reject uppercase RPC hashes";

    threw = false;
    try {
      sdk.proveInboundToSora(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              null,
              null,
              null,
              "0x" + repeat("ee", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality");
    }
    assert threw : "BSC inbound proving must reject missing Parlia finality";

    final boolean[] calledWithHashOnly = new boolean[] {false};
    threw = false;
    try {
      new BscMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceInput -> {
                calledWithHashOnly[0] = true;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  parliaFinality,
                  receiptProofHash));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof");
    }
    assert threw : "BSC inbound proving must reject hash-only receipt proof evidence";
    assert !calledWithHashOnly[0] : "hash-only BSC evidence must not reach local prover callbacks";

    final boolean[] calledWithoutSourceEvent = new boolean[] {false};
    threw = false;
    try {
      new BscMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceInput -> {
                calledWithoutSourceEvent[0] = true;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  parliaFinality,
                  receiptProof,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt source event validation");
    }
    assert threw : "BSC inbound proving must require receipt source event validation";
    assert !calledWithoutSourceEvent[0]
        : "source-event-free BSC evidence must not reach local prover callbacks";

    threw = false;
    try {
      new BscMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceInput -> {
                throw new AssertionError("prover callback must not run with drifted receiptProof");
              },
              null)
          .proveInboundToSora(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  parliaFinality,
                  new BscMainnetSccp.ReceiptProof(
                      receiptProof.sourceEventDigest(),
                      receiptProof.validatorEpoch(),
                      receiptProof.blockNumber(),
                      receiptProof.blockHash(),
                      "0x" + repeat("99", 32),
                      receiptProof.validatorSetHash(),
                      receiptProof.commitSealHash(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.receiptsRoot");
    }
    assert threw : "BSC inbound collection must reject receiptProof receipts-root drift";

    final Map<String, Object> driftedSourceLog = new LinkedHashMap<>(sourceEventLog);
    driftedSourceLog.put(
        "topics",
        Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), "0x" + repeat("99", 32)));
    final Map<String, Object> driftedSourceReceipt = new LinkedHashMap<>(receipt);
    driftedSourceReceipt.put("logs", Collections.singletonList(driftedSourceLog));
    threw = false;
    try {
      new BscMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceInput -> {
                throw new AssertionError(
                    "prover callback must not run with drifted receipt source event");
              },
              null,
              null,
              sourceBridgeEmitterAddress)
          .proveInboundToSora(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  driftedSourceReceipt,
                  block,
                  parliaFinality,
                  receiptProof,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.sourceEventDigest");
    }
    assert threw : "BSC inbound collection must reject receiptProof source-event drift";

    final BscMainnetSccp sourceEventGuardSdk =
        new BscMainnetSccp(null, null, null, null, null, null, null, sourceBridgeEmitterAddress);
    final Map<String, Object> extraTopicBscSourceLog = new LinkedHashMap<>(sourceEventLog);
    extraTopicBscSourceLog.put(
        "topics",
        Arrays.asList(
            EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest, "0x" + repeat("66", 32)));
    final Map<String, Object> extraTopicBscSourceReceipt = new LinkedHashMap<>(receipt);
    extraTopicBscSourceReceipt.put("logs", Collections.singletonList(extraTopicBscSourceLog));
    threw = false;
    try {
      sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              extraTopicBscSourceReceipt,
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("exactly 2 topics");
    }
    assert threw : "BSC source-event validation must reject extra source-event topics";

    final Map<String, Object> nonEmptyDataBscSourceLog = new LinkedHashMap<>(sourceEventLog);
    nonEmptyDataBscSourceLog.put("data", "0x01");
    final Map<String, Object> nonEmptyDataBscSourceReceipt = new LinkedHashMap<>(receipt);
    nonEmptyDataBscSourceReceipt.put("logs", Collections.singletonList(nonEmptyDataBscSourceLog));
    threw = false;
    try {
      sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              nonEmptyDataBscSourceReceipt,
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("data must be 0x");
    }
    assert threw : "BSC source-event validation must reject non-empty source-event data";

    final Map<String, Object> zeroDigestBscSourceLog = new LinkedHashMap<>(sourceEventLog);
    zeroDigestBscSourceLog.put(
        "topics",
        Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), "0x" + repeat("00", 32)));
    final Map<String, Object> zeroDigestBscSourceReceipt = new LinkedHashMap<>(receipt);
    zeroDigestBscSourceReceipt.put("logs", Collections.singletonList(zeroDigestBscSourceLog));
    threw = false;
    try {
      sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              zeroDigestBscSourceReceipt,
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("digest must not be zero");
    }
    assert threw : "BSC source-event validation must reject zero source-event digests";

    final Map<String, Object> duplicateBscSourceReceipt = new LinkedHashMap<>(receipt);
    duplicateBscSourceReceipt.put("logs", Arrays.asList(sourceEventLog, sourceEventLog));
    threw = false;
    try {
      sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              duplicateBscSourceReceipt,
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("exactly one matching");
    }
    assert threw : "BSC source-event validation must reject duplicate source-event logs";

    final Map<String, Object> removedBscSourceLog = new LinkedHashMap<>(sourceEventLog);
    removedBscSourceLog.put("removed", Boolean.TRUE);
    final Map<String, Object> removedBscSourceReceipt = new LinkedHashMap<>(receipt);
    removedBscSourceReceipt.put("logs", Collections.singletonList(removedBscSourceLog));
    threw = false;
    try {
      sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              removedBscSourceReceipt,
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("removed logs");
    }
    assert threw : "BSC source-event validation must reject removed source-event logs";

    final Map<String, Object> missingBscSourceContextLog = new LinkedHashMap<>(sourceEventLog);
    missingBscSourceContextLog.remove("transactionHash");
    final Map<String, Object> missingBscSourceContextReceipt = new LinkedHashMap<>(receipt);
    missingBscSourceContextReceipt.put("logs", Collections.singletonList(missingBscSourceContextLog));
    threw = false;
    try {
      sourceEventGuardSdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              missingBscSourceContextReceipt,
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logs[0].transactionHash");
    }
    assert threw : "BSC source-event validation must reject missing log transaction context";

    final Map<String, Object> driftedFinalityHash = new LinkedHashMap<>(parliaFinality);
    driftedFinalityHash.put("executionBlockHash", "0x" + repeat("bc", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityHash,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality.executionBlockHash");
    }
    assert threw : "BSC inbound collection must reject Parlia block-hash drift";

    final Map<String, Object> driftedFinalityNumber = new LinkedHashMap<>(parliaFinality);
    driftedFinalityNumber.put("executionBlockNumber", "0x1235");
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityNumber,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality.executionBlockNumber");
    }
    assert threw : "BSC inbound collection must reject Parlia block-number drift";

    final Map<String, Object> driftedFinalityReceiptsRoot = new LinkedHashMap<>(parliaFinality);
    driftedFinalityReceiptsRoot.put("executionReceiptsRoot", "0x" + repeat("cd", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityReceiptsRoot,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality.executionReceiptsRoot");
    }
    assert threw : "BSC inbound collection must reject Parlia receipts-root drift";

    threw = false;
    try {
      sdk.submitInboundToIroha(new byte[] {0, 0});
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "BSC inbound submitter must reject zero proof bytes";
  }

  private static void mainnetFacadesSnapshotWitnessProviderInputs() {
    final byte[] ethBundleBytes = new byte[] {5, 6, 7};
    final byte[] ethSourceProofBytes = new byte[] {9, 10};
    final String ethNativeVerifierKeyHash =
        sha256Hex(nativeEvmProverArtifactBytes("java android verifier key v1"));
    final SourceSccpProofs.EvmDestinationBinding ethBinding =
        EthereumMainnetSccp.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            ethNativeVerifierKeyHash);
    final EvmSccpProver.ProofRequestInput ethInput =
        new EvmSccpProver.ProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
            ethBundleBytes,
            ethSourceProofBytes,
            repeat("56", 32),
            ethBinding);
    final EvmSccpProver.ProofRequest ethRequest =
        new EthereumMainnetSccp(
                input -> {
                  assert input.bundleBytes() != ethBundleBytes
                      : "Ethereum witness provider must receive bundle byte snapshot";
                  assert input.sourceProofBytes() != ethSourceProofBytes
                      : "Ethereum witness provider must receive source-proof byte snapshot";
                  input.bundleBytes()[0] = 0x7f;
                  input.sourceProofBytes()[0] = 0x7e;
                  return new EvmSccpProver.ProofRequestInput(
                      input.publicInputs(),
                      new byte[] {8, 8, 8},
                      new byte[] {9, 9},
                      input.statementHash(),
                      input.destinationBinding());
                },
                null)
            .buildOutboundProofRequest(ethInput);
    assert Arrays.equals(new byte[] {5, 6, 7}, ethBundleBytes)
        : "Ethereum facade must not let witness providers mutate app-owned bundle bytes";
    assert Arrays.equals(new byte[] {9, 10}, ethSourceProofBytes)
        : "Ethereum facade must not let witness providers mutate app-owned source proof bytes";
    assert Arrays.equals(new byte[] {8, 8, 8}, ethRequest.bundleBytes())
        : "Ethereum facade must use witness-resolved bundle bytes";
    assert Arrays.equals(new byte[] {9, 9}, ethRequest.sourceProofBytes())
        : "Ethereum facade must use witness-resolved source proof bytes";
    final EvmSccpProver.ProofRequest[] seenEthProofRequest =
        new EvmSccpProver.ProofRequest[1];
    final EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts ethNativeArtifacts =
        sampleVerifiedEthereumNativeEvmProverArtifacts(ethBinding.hash);
    final EvmSccpProver.ProofRequest directEthRequest =
        EthereumMainnetSccp.buildProofRequest(ethInput, ethNativeArtifacts.nativeProverBundle());
    final EvmSccpProver.ProofResult ethProofResult =
        new EthereumMainnetSccp(
            null,
            request -> {
              seenEthProofRequest[0] = request;
              assert request != directEthRequest
                  : "Ethereum proof engine must receive a callback request snapshot";
              assert request.requestHash().equals(directEthRequest.requestHash())
                  : "Ethereum callback request snapshot must preserve the request hash";
              final byte[] callbackBundleBytes = request.bundleBytes();
              final byte[] callbackSourceProofBytes = request.sourceProofBytes();
              callbackBundleBytes[0] = 0x7d;
              callbackSourceProofBytes[0] = 0x7c;
              return sampleGroth16ProofBytes();
            },
            null,
            null,
            null,
            null,
            null,
            null,
            ethNativeArtifacts,
            (fixture, expectedResult, artifacts) -> expectedResult,
            null)
        .proveOutboundToEthereum(ethInput);
    assert seenEthProofRequest[0] != null
        : "Ethereum proof engine must receive a callback request";
    assert Arrays.equals(new byte[] {5, 6, 7}, ethProofResult.bundleBytes())
        : "Ethereum proof result must keep the original bundle bytes";
    assert Arrays.equals(new byte[] {9, 10}, ethProofResult.sourceProofBytes())
        : "Ethereum proof result must keep the original source proof bytes";

    final byte[] bscBundleBytes = new byte[] {3, 4, 5};
    final byte[] bscSourceProofBytes = new byte[] {6, 7};
    final SourceSccpProofs.EvmDestinationBinding bscBinding =
        BscSccpProver.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    final EvmSccpProver.ProofRequestInput bscInput =
        new EvmSccpProver.ProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
            bscBundleBytes,
            bscSourceProofBytes,
            repeat("56", 32),
            bscBinding);
    final EvmSccpProver.ProofRequest bscRequest =
        new BscSccpProver(
                input -> {
                  assert input.bundleBytes() != bscBundleBytes
                      : "BSC witness provider must receive bundle byte snapshot";
                  assert input.sourceProofBytes() != bscSourceProofBytes
                      : "BSC witness provider must receive source-proof byte snapshot";
                  input.bundleBytes()[0] = 0x7f;
                  input.sourceProofBytes()[0] = 0x7e;
                  return new EvmSccpProver.ProofRequestInput(
                      input.publicInputs(),
                      new byte[] {4, 4, 4},
                      new byte[] {5, 5},
                      input.statementHash(),
                      input.destinationBinding());
                },
                null)
            .buildRequest(bscInput);
    assert Arrays.equals(new byte[] {3, 4, 5}, bscBundleBytes)
        : "BSC facade must not let witness providers mutate app-owned bundle bytes";
    assert Arrays.equals(new byte[] {6, 7}, bscSourceProofBytes)
        : "BSC facade must not let witness providers mutate app-owned source proof bytes";
    assert Arrays.equals(new byte[] {4, 4, 4}, bscRequest.bundleBytes())
        : "BSC facade must use witness-resolved bundle bytes";
    assert Arrays.equals(new byte[] {5, 5}, bscRequest.sourceProofBytes())
        : "BSC facade must use witness-resolved source proof bytes";
  }

  private static byte[] sampleGroth16ProofBytes() {
    return flattenGroth16ProofWords(sampleGroth16ProofWords());
  }

  private static byte[] sampleGroth16ProofBytes(final int wordIndex, final byte[] word) {
    final byte[][] words = sampleGroth16ProofWords();
    words[wordIndex] = Arrays.copyOf(word, word.length);
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithZeroB() {
    final byte[][] words = sampleGroth16ProofWords();
    words[6] = new byte[32];
    words[7] = new byte[32];
    words[8] = new byte[32];
    words[9] = new byte[32];
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithNonSubgroupB() {
    final byte[][] words = sampleGroth16ProofWords();
    words[6] = abiWord(0);
    words[7] = abiWord(1);
    words[8] = hexWord("0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8");
    words[9] = hexWord("07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2");
    return flattenGroth16ProofWords(words);
  }

  private static byte[][] sampleGroth16ProofWords() {
    return new byte[][] {
      abiWord(1),
      repeatedWord(0x11),
      abiWord(SolanaSccpProver.DOMAIN_SORA),
      repeatedWord(0x33),
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
    final byte[] out = new byte[EvmSccpProver.GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1];
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

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  private static String sha256Hex(final byte[] bytes) {
    try {
      return "0x" + hexLower(MessageDigest.getInstance("SHA-256").digest(bytes));
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 digest is unavailable", ex);
    }
  }

  private static byte[] nativeEvmProverArtifactBytes(final String label) {
    final byte[] labelBytes = label.getBytes(StandardCharsets.UTF_8);
    final byte[] bytes = new byte[256];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) ((index * 37 + labelBytes.length * 11) & 0xff);
    }
    System.arraycopy(labelBytes, 0, bytes, 0, Math.min(labelBytes.length, bytes.length));
    return bytes;
  }

  private static Map<String, Object> sampleEvmReceipt(
      final int transactionIndex,
      final String transactionHash,
      final String blockHash,
      final String blockNumber) {
    return sampleEvmReceipt(
        transactionIndex, transactionHash, blockHash, blockNumber, java.util.Collections.emptyList());
  }

  private static Map<String, Object> sampleEvmReceipt(
      final int transactionIndex,
      final String transactionHash,
      final String blockHash,
      final String blockNumber,
      final List<Map<String, Object>> logs) {
    return linkedMap(
        "transactionHash", transactionHash,
        "transactionIndex", "0x" + Integer.toString(transactionIndex, 16),
        "blockHash", blockHash,
        "blockNumber", blockNumber,
        "status", "0x1",
        "cumulativeGasUsed", "0x" + Long.toString(21_000L * (transactionIndex + 1L), 16),
        "logsBloom", "0x" + repeat("00", 256),
        "logs", logs);
  }

  private static EvmSccpProver.ProofRequest evmRequestWithBackend(
      final EvmSccpProver.ProofRequest request, final String backend) {
    return new EvmSccpProver.ProofRequest(
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

  private static EvmSccpProver.ProofResult evmResultWithEnvelopeHash(
      final EvmSccpProver.ProofResult result, final String envelopeHash) {
    return new EvmSccpProver.ProofResult(
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

  private static EvmSccpProver.ProofResult evmResultWithBundleBytes(
      final EvmSccpProver.ProofResult result, final byte[] bundleBytes) {
    return new EvmSccpProver.ProofResult(
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

  private static EvmSccpProver.ProofResult evmResultWithProofBase64(
      final EvmSccpProver.ProofResult result, final String proofBase64) {
    return new EvmSccpProver.ProofResult(
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

  private static EvmSccpProver.ProofRequest evmRequestWithRequestHash(
      final EvmSccpProver.ProofRequest request, final String requestHash) {
    return new EvmSccpProver.ProofRequest(
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

  private static EvmSccpProver.ProofRequest evmRequestWithDestinationBindingHash(
      final EvmSccpProver.ProofRequest request, final String destinationBindingHash) {
    return new EvmSccpProver.ProofRequest(
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
        destinationBindingHash,
        request.requestHash(),
        request.destinationBinding());
  }

  private static EvmSccpProver.ProofRequestInput sampleProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return sampleProofRequestInput(
        publicInputs, sourceProofBytes, statementHash, repeat("78", 32), SolanaSccpProver.DOMAIN_SORA);
  }

  private static EvmSccpProver.ProofRequestInput sampleProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final int sourceDomain) {
    return sampleProofRequestInput(
        publicInputs,
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain);
  }

  private static EvmSccpProver.ProofRequestInput sampleProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final String backend,
      final int sourceDomain) {
    return new EvmSccpProver.ProofRequestInput(
        publicInputs,
        new byte[] {5, 6, 7},
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        backend,
        sourceDomain);
  }

  private static EvmSccpProver.ProofRequestInput sampleProductionProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return sampleProductionProofRequestInput(
        publicInputs, new byte[] {5, 6, 7}, sourceProofBytes, statementHash);
  }

  private static EvmSccpProver.ProofRequestInput sampleProductionProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] bundleBytes,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return new EvmSccpProver.ProofRequestInput(
        publicInputs,
        bundleBytes,
        sourceProofBytes,
        statementHash,
        sampleDestinationBinding(publicInputs),
        EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        SolanaSccpProver.DOMAIN_SORA);
  }

  private static SourceSccpProofs.EvmDestinationBinding sampleDestinationBinding(
      final EvmSccpProver.PublicInputsInput publicInputs) {
    return SourceSccpProofs.evmDestinationBinding(
        EvmSccpProver.DOMAIN_SORA,
        publicInputs.targetDomain(),
        "0x" + repeat("33", 32),
        "0x" + repeat("11", 20),
        "0x" + repeat("22", 20),
        "0x" + repeat("bb", 32),
        "0x" + repeat("cc", 32));
  }

  private static EvmSccpProver.PublicInputsInput samplePublicInputs(final int targetDomain) {
    return samplePublicInputs(targetDomain, "19");
  }

  private static EvmSccpProver.PublicInputsInput samplePublicInputs(
      final int targetDomain, final String finalityHeight) {
    return new EvmSccpProver.PublicInputsInput(
        1,
        repeat("11", 32),
        repeat("22", 32),
        targetDomain,
        repeat("33", 32),
        finalityHeight,
        repeat("44", 32));
  }

  private static EthereumMainnetSccp.BeaconRestResponse beaconResponse(final String json) {
    return new EthereumMainnetSccp.BeaconRestResponse(200, json.getBytes(StandardCharsets.UTF_8));
  }

  private static EthereumMainnetSccp.BeaconRestConsensusProvider beaconRestProvider(
      final EthereumMainnetSccp.BeaconRestResponse header,
      final EthereumMainnetSccp.BeaconRestResponse checkpoint,
      final String syncCommitteeRoot,
      final byte[] syncCommitteePayload) {
    return beaconRestProvider(
        header,
        beaconResponse(beaconBlockRootJson()),
        beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
        checkpoint,
        beaconResponse(beaconFinalityUpdateJson()),
        syncCommitteeRoot,
        syncCommitteePayload);
  }

  private static EthereumMainnetSccp.BeaconRestConsensusProvider beaconRestProvider(
      final EthereumMainnetSccp.BeaconRestResponse header,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlockRoot,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlock,
      final EthereumMainnetSccp.BeaconRestResponse checkpoint,
      final String syncCommitteeRoot,
      final byte[] syncCommitteePayload) {
    return beaconRestProvider(
        header,
        finalizedBlockRoot,
        finalizedBlock,
        checkpoint,
        beaconResponse(beaconFinalityUpdateJson()),
        syncCommitteeRoot,
        syncCommitteePayload);
  }

  private static EthereumMainnetSccp.BeaconRestConsensusProvider beaconRestProvider(
      final EthereumMainnetSccp.BeaconRestResponse header,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlockRoot,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlock,
      final EthereumMainnetSccp.BeaconRestResponse checkpoint,
      final EthereumMainnetSccp.BeaconRestResponse finalityUpdate,
      final String syncCommitteeRoot,
      final byte[] syncCommitteePayload) {
    return new EthereumMainnetSccp.BeaconRestConsensusProvider(
        "https://beacon.example",
        syncCommitteeRoot,
        syncCommitteePayload,
        java.util.Collections.emptyMap(),
        true,
        (url, headers) -> {
          if (url.endsWith("/eth/v1/beacon/headers/finalized")) {
            return header;
          }
          if (url.endsWith("/eth/v1/beacon/blocks/finalized/root")) {
            return finalizedBlockRoot;
          }
          if (url.endsWith("/eth/v2/beacon/blocks/finalized")) {
            return finalizedBlock;
          }
          if (url.endsWith("/eth/v1/beacon/states/finalized/finality_checkpoints")) {
            return checkpoint;
          }
          if (url.endsWith("/eth/v1/beacon/light_client/finality_update")) {
            return finalityUpdate;
          }
          throw new IllegalArgumentException("unexpected Beacon REST URL " + url);
        });
  }

  private static EthereumMainnetSccp.BeaconRestConsensusProvider beaconRestProvider(
      final EthereumMainnetSccp.BeaconRestResponse header,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlock,
      final EthereumMainnetSccp.BeaconRestResponse checkpoint,
      final String syncCommitteeRoot,
      final byte[] syncCommitteePayload) {
    return beaconRestProvider(
        header,
        beaconResponse(beaconBlockRootJson()),
        finalizedBlock,
        checkpoint,
        beaconResponse(beaconFinalityUpdateJson()),
        syncCommitteeRoot,
        syncCommitteePayload);
  }

  private static String beaconHeaderJson(
      final boolean executionOptimistic, final boolean finalized) {
    return beaconHeaderJsonWithRoot(
        executionOptimistic, finalized, BEACON_HEADER_ROOT_SLOT_64, "64");
  }

  private static String beaconHeaderJson(
      final boolean executionOptimistic,
      final boolean finalized,
      final String rootByte,
      final String slot) {
    return beaconHeaderJsonWithRoot(
        executionOptimistic, finalized, "0x" + repeat(rootByte, 32), slot);
  }

  private static String beaconHeaderJsonWithRoot(
      final boolean executionOptimistic,
      final boolean finalized,
      final String root,
      final String slot) {
    return "{"
        + "\"execution_optimistic\":"
        + executionOptimistic
        + ",\"finalized\":"
        + finalized
        + ",\"data\":{"
        + "\"root\":\""
        + root
        + "\",\"canonical\":true,"
        + "\"header\":{\"message\":{"
        + "\"slot\":\""
        + slot
        + "\","
        + "\"proposer_index\":\"1\","
        + "\"parent_root\":\"0x"
        + repeat("01", 32)
        + "\",\"state_root\":\"0x"
        + repeat("02", 32)
        + "\",\"body_root\":\"0x"
        + repeat("03", 32)
        + "\"},\"signature\":\"0x"
        + repeat("12", 96)
        + "\"}}}";
  }

  private static String beaconCheckpointJson() {
    return beaconCheckpointJsonWithRoot(BEACON_HEADER_ROOT_SLOT_64);
  }

  private static String beaconCheckpointJson(final String rootByte) {
    return beaconCheckpointJsonWithRoot("0x" + repeat(rootByte, 32));
  }

  private static String beaconCheckpointJsonWithRoot(final String root) {
    return "{"
        + "\"execution_optimistic\":false,"
        + "\"finalized\":true,"
        + "\"data\":{\"finalized\":{\"root\":\""
        + root
        + "\",\"epoch\":\"2\"}}}";
  }

  private static String beaconBlockRootJson() {
    return beaconBlockRootJsonWithRoot(BEACON_HEADER_ROOT_SLOT_64);
  }

  private static String beaconBlockRootJson(final String rootByte) {
    return beaconBlockRootJsonWithRoot("0x" + repeat(rootByte, 32));
  }

  private static String beaconBlockRootJsonWithRoot(final String root) {
    return "{"
        + "\"execution_optimistic\":false,"
        + "\"finalized\":true,"
        + "\"data\":{\"root\":\""
        + root
        + "\"}}";
  }

  private static String beaconBlockJson(
      final String slot,
      final String blockHash,
      final String blockNumber,
      final String receiptsRoot) {
    return "{"
        + "\"execution_optimistic\":false,"
        + "\"finalized\":true,"
        + "\"data\":{\"message\":{"
        + "\"slot\":\""
        + slot
        + "\",\"body\":{\"execution_payload\":{"
        + "\"block_hash\":\""
        + blockHash
        + "\",\"block_number\":\""
        + blockNumber
        + "\",\"receipts_root\":\""
        + receiptsRoot
        + "\"}}}}}";
  }

  private static String beaconGenesisJson(final String genesisTime) {
    return "{"
        + "\"data\":{"
        + "\"genesis_time\":\""
        + genesisTime
        + "\",\"genesis_validators_root\":\"0x"
        + repeat("ab", 32)
        + "\",\"genesis_fork_version\":\"0x00000000\"}}";
  }

  private static String beaconFinalityUpdateJson() {
    return beaconFinalityUpdateJson(
        "64", "65", ETHEREUM_SYNC_COMMITTEE_SUPERMAJORITY_BITS);
  }

  private static String beaconFinalityUpdateJson(
      final String slot, final String signatureSlot, final String syncCommitteeBits) {
    return beaconFinalityUpdateJson(slot, signatureSlot, syncCommitteeBits, "0x" + repeat("34", 96));
  }

  private static String beaconFinalityUpdateJson(
      final String slot,
      final String signatureSlot,
      final String syncCommitteeBits,
      final String syncCommitteeSignature) {
    return beaconFinalityUpdateJson(
        slot, signatureSlot, syncCommitteeBits, syncCommitteeSignature, true, ETHEREUM_FINALITY_BRANCH);
  }

  private static String beaconFinalityUpdateJson(
      final String slot,
      final String signatureSlot,
      final String syncCommitteeBits,
      final String syncCommitteeSignature,
      final boolean includeFinalityBranch,
      final List<String> finalityBranch) {
    final String finalityBranchField =
        includeFinalityBranch
            ? "\"finality_branch\":[" + quotedJsonArray(finalityBranch) + "],"
            : "";
    return "{"
        + "\"execution_optimistic\":false,"
        + "\"data\":{"
        + "\"finalized_header\":{\"beacon\":{"
        + "\"slot\":\""
        + slot
        + "\",\"proposer_index\":\"1\","
        + "\"parent_root\":\"0x"
        + repeat("01", 32)
        + "\",\"state_root\":\"0x"
        + repeat("02", 32)
        + "\",\"body_root\":\"0x"
        + repeat("03", 32)
        + "\"}},"
        + finalityBranchField
        + "\"sync_aggregate\":{"
        + "\"sync_committee_bits\":\""
        + syncCommitteeBits
        + "\",\"sync_committee_signature\":\""
        + syncCommitteeSignature
        + "\"},"
        + "\"signature_slot\":\""
        + signatureSlot
        + "\"}}";
  }

  private static List<String> ethereumFinalityBranch() {
    final List<String> branch = new ArrayList<>();
    for (int index = 0; index < 6; index++) {
      branch.add("0x" + repeat(String.format("%02x", 0x50 + index), 32));
    }
    return Collections.unmodifiableList(branch);
  }

  private static String quotedJsonArray(final List<String> values) {
    final StringBuilder out = new StringBuilder();
    for (int index = 0; index < values.size(); index++) {
      if (index > 0) {
        out.append(',');
      }
      out.append('"').append(values.get(index)).append('"');
    }
    return out.toString();
  }

  private static Map<String, String> linkedStringMap(final String key, final String value) {
    final LinkedHashMap<String, String> out = new LinkedHashMap<>();
    out.put(key, value);
    return out;
  }

  private static Map<String, Object> linkedMap(final Object... entries) {
    if (entries.length % 2 != 0) {
      throw new IllegalArgumentException("linkedMap requires key/value pairs");
    }
    final LinkedHashMap<String, Object> out = new LinkedHashMap<>();
    for (int index = 0; index < entries.length; index += 2) {
      out.put((String) entries[index], entries[index + 1]);
    }
    return out;
  }

  private static EvmSccpProver.EthereumMainnetNativeEvmProverBundle
      sampleEthereumNativeEvmProverBundle(
          final String destinationBindingHash,
          final boolean noWasm,
          final boolean remoteProverRequired) {
    final String proofArtifactHash = "0x" + repeat("91", 32);
    final String provingKeyHash = "0x" + repeat("92", 32);
    final List<EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact> artifacts =
        new ArrayList<>();
    int index = 1;
    for (final Map.Entry<String, String> entry :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.entrySet()) {
      artifacts.add(
          new EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact(
              entry.getKey(),
              entry.getValue(),
              proofArtifactHash,
              provingKeyHash,
              "artifacts/eth-mainnet/" + entry.getKey() + "-implementation.bin",
              "0x" + repeat(String.format("%02x", index), 32)));
      index++;
    }
    return new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
        EvmSccpProver.NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
        EvmSccpProver.DOMAIN_ETH,
        "eth",
        EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        "artifacts/eth-mainnet/proof-artifact.bin",
        proofArtifactHash,
        "artifacts/eth-mainnet/proving-key.bin",
        provingKeyHash,
        "artifacts/eth-mainnet/verifier-key.bin",
        "0x" + repeat("cc", 32),
        destinationBindingHash,
        noWasm,
        remoteProverRequired,
        "pure-typescript",
        artifacts,
        sampleEthereumNativeAuditHashes());
  }

  private static Map<String, String> sampleEthereumNativeAuditHashes() {
    final LinkedHashMap<String, String> auditHashes = new LinkedHashMap<>();
    auditHashes.put("circuit_security_audit", "0x" + repeat("a1", 32));
    auditHashes.put("native_implementation_audit", "0x" + repeat("a2", 32));
    auditHashes.put("reproducible_build_attestation", "0x" + repeat("a3", 32));
    auditHashes.put("cross_sdk_fixture_parity", "0x" + repeat("a4", 32));
    auditHashes.put("native_prover_self_test", "0x" + repeat("a5", 32));
    auditHashes.put("no_wasm_no_remote_scan", "0x" + repeat("a6", 32));
    return auditHashes;
  }

  private static EvmSccpProver.EthereumMainnetNativeEvmProverArtifacts
      sampleVerifiedEthereumNativeEvmProverArtifacts(final String destinationBindingHash) {
    final byte[] proofArtifactBytes = nativeEvmProverArtifactBytes("java android proof artifact v1");
    final byte[] provingKeyBytes = nativeEvmProverArtifactBytes("java android proving key v1");
    final byte[] verifierKeyBytes = nativeEvmProverArtifactBytes("java android verifier key v1");
    final byte[] implementationBytes =
        nativeEvmProverArtifactBytes("java android implementation artifact v1");
    final String proofArtifactHash = sha256Hex(proofArtifactBytes);
    final String provingKeyHash = sha256Hex(provingKeyBytes);
    final String verifierKeyHash = sha256Hex(verifierKeyBytes);
    final String implementationHash = sha256Hex(implementationBytes);
    final ArrayList<EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact>
        artifacts = new ArrayList<>();
    int index = 0;
    for (final Map.Entry<String, String> entry :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.entrySet()) {
      index++;
      artifacts.add(
          new EvmSccpProver.EthereumMainnetNativeEvmProverBundleSdkArtifact(
              entry.getKey(),
              entry.getValue(),
              proofArtifactHash,
              provingKeyHash,
              "java-android".equals(entry.getKey())
                  ? implementationHash
                  : "0x" + repeat(String.format("%02x", index), 32)));
    }
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle draftBundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            proofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            destinationBindingHash,
            artifacts,
            sampleEthereumNativeAuditHashes());
    final byte[] parityFixtureBytes =
        sampleEthereumNativeEvmProverParityFixtureJson(draftBundle).getBytes(StandardCharsets.UTF_8);
    final byte[] selfTestFixtureBytes =
        sampleEthereumNativeEvmProverSelfTestFixtureJson(draftBundle)
            .getBytes(StandardCharsets.UTF_8);
    final Map<String, String> auditHashes = sampleEthereumNativeAuditHashes();
    auditHashes.put("cross_sdk_fixture_parity", sha256Hex(parityFixtureBytes));
    auditHashes.put("native_prover_self_test", sha256Hex(selfTestFixtureBytes));
    final EvmSccpProver.EthereumMainnetNativeEvmProverBundle bundle =
        new EvmSccpProver.EthereumMainnetNativeEvmProverBundle(
            proofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            destinationBindingHash,
            artifacts,
            auditHashes);
    return bundle.verifiedArtifacts(
        proofArtifactBytes,
        provingKeyBytes,
        verifierKeyBytes,
        "java-android",
        implementationBytes,
        parityFixtureBytes,
        selfTestFixtureBytes);
  }

  private static String sampleEthereumNativeEvmProverBundleJson(
      final String destinationBindingHash) {
    return sampleEthereumNativeEvmProverBundleJson(destinationBindingHash, true, false);
  }

  private static String sampleEthereumNativeEvmProverBundleJson(
      final String destinationBindingHash,
      final boolean noWasm,
      final boolean remoteProverRequired) {
    return sampleEthereumNativeEvmProverBundleJson(
        destinationBindingHash,
        noWasm,
        remoteProverRequired,
        "artifacts/eth-mainnet/proof-artifact.bin");
  }

  private static String sampleEthereumNativeEvmProverBundleJson(
      final String destinationBindingHash,
      final boolean noWasm,
      final boolean remoteProverRequired,
      final String proofArtifact) {
    final String proofArtifactHash = "0x" + repeat("91", 32);
    final String provingKeyHash = "0x" + repeat("92", 32);
    final StringBuilder artifacts = new StringBuilder();
    int index = 1;
    for (final Map.Entry<String, String> entry :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.entrySet()) {
      if (artifacts.length() > 0) {
        artifacts.append(',');
      }
      artifacts
          .append("{")
          .append("\"sdk\":\"")
          .append(entry.getKey())
          .append("\",")
          .append("\"implementation\":\"")
          .append(entry.getValue())
          .append("\",")
          .append("\"prover_artifact_hash\":\"")
          .append(proofArtifactHash)
          .append("\",")
          .append("\"proving_key_hash\":\"")
          .append(provingKeyHash)
          .append("\",")
          .append("\"implementation_artifact\":\"artifacts/eth-mainnet/")
          .append(entry.getKey())
          .append("-implementation.bin\",")
          .append("\"implementation_hash\":\"0x")
          .append(repeat(String.format("%02x", index), 32))
          .append("\"}");
      index++;
    }
    return "{"
        + "\"schema\":\""
        + EvmSccpProver.NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1
        + "\","
        + "\"bundle_id\":\""
        + EvmSccpProver.ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1
        + "\","
        + "\"domain\":"
        + EvmSccpProver.DOMAIN_ETH
        + ","
        + "\"chain\":\"eth\","
        + "\"proof_backend\":\""
        + EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1
        + "\","
        + "\"proof_artifact\":\""
        + proofArtifact
        + "\","
        + "\"proof_artifact_hash\":\""
        + proofArtifactHash
        + "\","
        + "\"proving_key\":\"artifacts/eth-mainnet/proving-key.bin\","
        + "\"proving_key_hash\":\""
        + provingKeyHash
        + "\","
        + "\"verifier_key\":\"artifacts/eth-mainnet/verifier-key.bin\","
        + "\"verifier_key_hash\":\"0x"
        + repeat("cc", 32)
        + "\","
        + "\"destination_binding_hash\":\""
        + destinationBindingHash
        + "\","
        + "\"no_wasm\":"
        + noWasm
        + ","
        + "\"remote_prover_required\":"
        + remoteProverRequired
        + ","
        + "\"browser_implementation\":\"pure-typescript\","
        + "\"native_sdk_artifacts\":["
        + artifacts
        + "],"
        + "\"cross_sdk_fixture_parity_artifact\":\"artifacts/eth-mainnet/cross-sdk-fixture-parity.json\","
        + "\"native_prover_self_test_artifact\":\"artifacts/eth-mainnet/native-prover-self-test.json\","
        + "\"audit_hashes\":{"
        + "\"circuit_security_audit\":\"0x"
        + repeat("a1", 32)
        + "\",\"native_implementation_audit\":\"0x"
        + repeat("a2", 32)
        + "\",\"reproducible_build_attestation\":\"0x"
        + repeat("a3", 32)
        + "\",\"cross_sdk_fixture_parity\":\"0x"
        + repeat("a4", 32)
        + "\",\"native_prover_self_test\":\"0x"
        + repeat("a5", 32)
        + "\",\"no_wasm_no_remote_scan\":\"0x"
        + repeat("a6", 32)
        + "\"}"
        + "}";
  }

  private static String sampleEthereumNativeEvmProverParityFixtureJson(
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
    return sampleEthereumNativeEvmProverParityFixtureJson(nativeProverBundle, null);
  }

  private static String sampleEthereumNativeEvmProverParityFixtureJson(
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle,
      final String javaAndroidCalldataHash) {
    final String defaultCalldataHash = "0x" + repeat("d3", 32);
    final StringBuilder publicSignalWords = new StringBuilder();
    for (int index = 0; index < 9; index++) {
      if (publicSignalWords.length() > 0) {
        publicSignalWords.append(',');
      }
      publicSignalWords
          .append("\"0x")
          .append(repeat(String.format("%02x", index + 0x10), 32))
          .append("\"");
    }
    final StringBuilder sdkResults = new StringBuilder();
    for (final String sdk :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keySet()) {
      if (sdkResults.length() > 0) {
        sdkResults.append(',');
      }
      final String calldataHash =
          "java-android".equals(sdk) && javaAndroidCalldataHash != null
              ? javaAndroidCalldataHash
              : defaultCalldataHash;
      sdkResults
          .append("\"")
          .append(sdk)
          .append("\":{")
          .append("\"receipt_proof_hash\":\"0x")
          .append(repeat("d1", 32))
          .append("\",")
          .append("\"source_proof_hash\":\"0x")
          .append(repeat("d2", 32))
          .append("\",")
          .append("\"destination_binding_hash\":\"")
          .append(nativeProverBundle.destinationBindingHash())
          .append("\",")
          .append("\"public_signal_words\":[")
          .append(publicSignalWords)
          .append("],")
          .append("\"calldata_hash\":\"")
          .append(calldataHash)
          .append("\",")
          .append("\"torii_submit_payload_hash\":\"0x")
          .append(repeat("d4", 32))
          .append("\"}");
    }
    return "{"
        + "\"schema\":\""
        + EvmSccpProver.ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1
        + "\","
        + "\"domain\":"
        + EvmSccpProver.DOMAIN_ETH
        + ","
        + "\"chain\":\"eth\","
        + "\"proof_backend\":\""
        + EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1
        + "\","
        + "\"proof_artifact_hash\":\""
        + nativeProverBundle.proofArtifactHash()
        + "\","
        + "\"proving_key_hash\":\""
        + nativeProverBundle.provingKeyHash()
        + "\","
        + "\"verifier_key_hash\":\""
        + nativeProverBundle.verifierKeyHash()
        + "\","
        + "\"destination_binding_hash\":\""
        + nativeProverBundle.destinationBindingHash()
        + "\","
        + "\"receipt_proof_hash\":\"0x"
        + repeat("d1", 32)
        + "\","
        + "\"source_proof_hash\":\"0x"
        + repeat("d2", 32)
        + "\","
        + "\"public_signal_words\":["
        + publicSignalWords
        + "],"
        + "\"calldata_hash\":\""
        + defaultCalldataHash
        + "\","
        + "\"torii_submit_payload_hash\":\"0x"
        + repeat("d4", 32)
        + "\","
        + "\"sdk_results\":{"
        + sdkResults
        + "}"
        + "}";
  }

  private static String sampleEthereumNativeEvmProverSelfTestFixtureJson(
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
    return sampleEthereumNativeEvmProverSelfTestFixtureJson(nativeProverBundle, null);
  }

  private static String sampleEthereumNativeEvmProverSelfTestFixtureJson(
      final EvmSccpProver.EthereumMainnetNativeEvmProverBundle nativeProverBundle,
      final String javaAndroidProofHash) {
    final String defaultProofHash = "0x" + repeat("e4", 32);
    final StringBuilder publicSignalWords = new StringBuilder();
    for (int index = 0; index < 9; index++) {
      if (publicSignalWords.length() > 0) {
        publicSignalWords.append(',');
      }
      publicSignalWords
          .append("\"0x")
          .append(repeat(String.format("%02x", index + 0x20), 32))
          .append("\"");
    }
    final StringBuilder sdkResults = new StringBuilder();
    for (final String sdk :
        EvmSccpProver.ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keySet()) {
      if (sdkResults.length() > 0) {
        sdkResults.append(',');
      }
      final String proofHash =
          "java-android".equals(sdk) && javaAndroidProofHash != null
              ? javaAndroidProofHash
              : defaultProofHash;
      sdkResults
          .append("\"")
          .append(sdk)
          .append("\":{")
          .append("\"request_hash\":\"0x")
          .append(repeat("e1", 32))
          .append("\",")
          .append("\"witness_hash\":\"0x")
          .append(repeat("e2", 32))
          .append("\",")
          .append("\"source_proof_hash\":\"0x")
          .append(repeat("e3", 32))
          .append("\",")
          .append("\"proof_hash\":\"")
          .append(proofHash)
          .append("\",")
          .append("\"public_signal_words\":[")
          .append(publicSignalWords)
          .append("],")
          .append("\"calldata_hash\":\"0x")
          .append(repeat("e5", 32))
          .append("\",")
          .append("\"torii_submit_payload_hash\":\"0x")
          .append(repeat("e6", 32))
          .append("\"}");
    }
    return "{"
        + "\"schema\":\""
        + EvmSccpProver.ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1
        + "\","
        + "\"domain\":"
        + EvmSccpProver.DOMAIN_ETH
        + ","
        + "\"chain\":\"eth\","
        + "\"proof_backend\":\""
        + EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1
        + "\","
        + "\"proof_artifact_hash\":\""
        + nativeProverBundle.proofArtifactHash()
        + "\","
        + "\"proving_key_hash\":\""
        + nativeProverBundle.provingKeyHash()
        + "\","
        + "\"verifier_key_hash\":\""
        + nativeProverBundle.verifierKeyHash()
        + "\","
        + "\"destination_binding_hash\":\""
        + nativeProverBundle.destinationBindingHash()
        + "\","
        + "\"request_hash\":\"0x"
        + repeat("e1", 32)
        + "\","
        + "\"witness_hash\":\"0x"
        + repeat("e2", 32)
        + "\","
        + "\"source_proof_hash\":\"0x"
        + repeat("e3", 32)
        + "\","
        + "\"proof_hash\":\""
        + defaultProofHash
        + "\","
        + "\"public_signal_words\":["
        + publicSignalWords
        + "],"
        + "\"calldata_hash\":\"0x"
        + repeat("e5", 32)
        + "\","
        + "\"torii_submit_payload_hash\":\"0x"
        + repeat("e6", 32)
        + "\","
        + "\"sdk_results\":{"
        + sdkResults
        + "}"
        + "}";
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }

  private static byte[] repeatedByteArray(final int value, final int count) {
    final byte[] out = new byte[count];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static byte[] indexedSyncCommitteeBytes(final int value, final int count, final int index) {
    final byte[] out = repeatedByteArray(value, count);
    out[count - 2] = (byte) ((index >>> 8) & 0xff);
    out[count - 1] = (byte) (index & 0xff);
    return out;
  }
}
