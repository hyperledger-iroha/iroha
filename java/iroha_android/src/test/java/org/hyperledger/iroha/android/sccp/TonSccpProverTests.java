package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.hyperledger.iroha.android.crypto.Blake2b;

public final class TonSccpProverTests {
  private TonSccpProverTests() {}

  public static void main(final String[] args) {
    derivesTonRouteCanaryEvidenceHash();
    buildsTonMessageBodyBoc();
    derivesTonBocRootHashFromOrdinaryCells();
    derivesTonShardProofHashFromWitnessMaterial();
    buildsTonShardStateOpenVerifyProofRequestFromWitnessMaterial();
    buildsTonFullLightClientAuditRoleProofRequests();
    derivesTonValidatorSetTransitionHashesFromWitnessMaterial();
    derivesTonMasterchainConfigProofHashesFromWitnessMaterial();
    derivesTonMasterchainBlockMessageAndSignatureHashesFromWitnessMaterial();
    callbackRequestSnapshotCopiesTonProofRequestBytes();
    proverRequiresLinkedProofEngine();
    proverRejectsNonProductionInputBeforeLinkedProofEngine();
    proverWrapsExternalProofBytes();
    proverResolvesWitnessProviderBeforeBuildingRequest();
    proofRequestBindsRelayContextAndDeployment();
    proofRequestHashMatchesCrossSdkVector();
    proofRequestRejectsNoncanonicalOrMismatchedBundleBytes();
    System.out.println("[IrohaAndroid] TON SCCP prover tests passed.");
  }

  private static void derivesTonRouteCanaryEvidenceHash() {
    final TonSccpProver.RouteCanaryEvidenceInput evidence = sampleRouteCanaryEvidence();

    assert TonSccpProver.canonicalRouteCanaryEvidenceBytes(evidence).length == 358
        : "TON route canary transcript length must match Rust";
    assert TonSccpProver.routeCanaryEvidenceHash(evidence)
        .equals("0xf128e8405017b9ca7733bb10d43eeaf783e38d39740a3455aa353c76655c6942")
        : "TON route canary hash must match Rust/script vector";

    boolean wrongDestinationBindingThrew = false;
    try {
      TonSccpProver.routeCanaryEvidenceHash(
          sampleRouteCanaryEvidence(
              "0x" + repeat("78", 32),
              null,
              "0:" + repeat("11", 32),
              "active",
              "123456789",
              "0x" + repeat("44", 32)));
    } catch (final IllegalArgumentException ex) {
      wrongDestinationBindingThrew =
          ex.getMessage().contains("destinationBindingHash must match canonical TON");
    }
    assert wrongDestinationBindingThrew : "TON canary must reject wrong destination binding";

    boolean wrongWorkchainThrew = false;
    try {
      TonSccpProver.routeCanaryEvidenceHash(
          sampleRouteCanaryEvidence(
              null,
              null,
              "1:" + repeat("11", 32),
              "active",
              "123456789",
              "0x" + repeat("44", 32)));
    } catch (final IllegalArgumentException ex) {
      wrongWorkchainThrew = ex.getMessage().contains("verifierContractAddress workchain");
    }
    assert wrongWorkchainThrew : "TON canary must reject non-basechain accounts";

    boolean inactiveAccountThrew = false;
    try {
      TonSccpProver.routeCanaryEvidenceHash(
          sampleRouteCanaryEvidence(
              null,
              null,
              "0:" + repeat("11", 32),
              "uninit",
              "123456789",
              "0x" + repeat("44", 32)));
    } catch (final IllegalArgumentException ex) {
      inactiveAccountThrew = ex.getMessage().contains("accountStatus must be active");
    }
    assert inactiveAccountThrew : "TON canary must reject inactive accounts";

    boolean paddedLtThrew = false;
    try {
      TonSccpProver.routeCanaryEvidenceHash(
          sampleRouteCanaryEvidence(
              null,
              null,
              "0:" + repeat("11", 32),
              "active",
              "0123",
              "0x" + repeat("44", 32)));
    } catch (final IllegalArgumentException ex) {
      paddedLtThrew = ex.getMessage().contains("lastTransactionLt must be a positive decimal");
    }
    assert paddedLtThrew : "TON canary must reject padded last transaction LTs";

    boolean mismatchedCodeRootThrew = false;
    try {
      TonSccpProver.routeCanaryEvidenceHash(
          sampleRouteCanaryEvidence(
              null,
              null,
              "0:" + repeat("11", 32),
              "active",
              "123456789",
              "0x" + repeat("45", 32)));
    } catch (final IllegalArgumentException ex) {
      mismatchedCodeRootThrew = ex.getMessage().contains("verifierCodeBocRootHash");
    }
    assert mismatchedCodeRootThrew : "TON canary must reject mismatched verifier BoC root";

    final String canonicalDestinationBindingHash =
        SourceSccpProofs.destinationBindingHash(TonSccpProver.DOMAIN_TON);
    assertThrows(
        () ->
            TonSccpProver.routeCanaryEvidenceHash(
                sampleRouteCanaryEvidenceWithGovernedHashes(
                    canonicalDestinationBindingHash, canonicalDestinationBindingHash, null, null)),
        "TON route canary governed hashes");
    assertThrows(
        () ->
            TonSccpProver.routeCanaryEvidenceHash(
                sampleRouteCanaryEvidenceWithGovernedHashes(null, null, "0x" + repeat("31", 32), null)),
        "TON route canary governed hashes");
    assertThrows(
        () ->
            TonSccpProver.routeCanaryEvidenceHash(
                sampleRouteCanaryEvidenceWithGovernedHashes(null, null, null, "0x" + repeat("31", 32))),
        "TON route canary governed hashes");
    assertThrows(
        () ->
            TonSccpProver.routeCanaryEvidenceHash(
                sampleRouteCanaryEvidenceWithGovernedHashes(null, canonicalDestinationBindingHash, canonicalDestinationBindingHash, null)),
        "TON route canary governed hashes");
    assertThrows(
        () ->
            TonSccpProver.routeCanaryEvidenceHash(
                sampleRouteCanaryEvidenceWithGovernedHashes(null, canonicalDestinationBindingHash, null, canonicalDestinationBindingHash)),
        "TON route canary governed hashes");
    assertThrows(
        () ->
            TonSccpProver.routeCanaryEvidenceHash(
                sampleRouteCanaryEvidenceWithGovernedHashes(null, null, null, "0x" + repeat("33", 32))),
        "TON route canary governed hashes");
  }

  private static void buildsTonMessageBodyBoc() {
    final byte[] body = TonSccpProver.buildMessageBodyBoc(sampleMessageBodyInput());
    final byte[] magic = {(byte) 0xb5, (byte) 0xee, (byte) 0x9c, 0x72};
    assert Arrays.equals(magic, Arrays.copyOfRange(body, 0, 4))
        : "TON message body must be a BOC";
    assert body.length
            > TonSccpProver.canonicalPublicInputsBytes(samplePublicInputs()).length
        : "BOC must carry refs beyond public inputs";

    final TonSccpProver.SubmissionDestinationBindingInput destinationBinding =
        new TonSccpProver.SubmissionDestinationBindingInput("sora:ton", repeat("78", 32));
    final TonSccpProver.SubmissionManifestInput manifest =
        new TonSccpProver.SubmissionManifestInput(
            1,
            SolanaSccpProver.DOMAIN_SORA,
            TonSccpProver.DOMAIN_TON,
            "RecursiveZk",
            "CryptographicProof",
            "TonContract",
            "TonContract",
            TonSccpProver.STARK_FRI_PROOF_FAMILY_V1,
            TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
            "sccp-message-v1",
            "sccp-registry-v1",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
            destinationBinding);
    final byte[] metadata =
        TonSccpProver.canonicalSubmissionMetadataBytes(
            new TonSccpProver.SubmissionMetadataInput(
                manifest, null, repeat("78", 32), samplePublicInputs(), repeat("bb", 32)));
    assert metadata.length > TonSccpProver.canonicalPublicInputsBytes(samplePublicInputs()).length
        : "TON submission metadata must include manifest and public inputs";
    boolean mismatchedMetadataBindingThrew = false;
    try {
      TonSccpProver.canonicalSubmissionMetadataBytes(
          new TonSccpProver.SubmissionMetadataInput(
              manifest, null, repeat("56", 32), samplePublicInputs(), repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      mismatchedMetadataBindingThrew = ex.getMessage().contains("destinationBindingHash");
    }
    assert mismatchedMetadataBindingThrew
        : "TON metadata must reject root/metadata destination binding mismatches";
    boolean wrongManifestThrew = false;
    try {
      TonSccpProver.canonicalSubmissionMetadataBytes(
          new TonSccpProver.SubmissionMetadataInput(
              new TonSccpProver.SubmissionManifestInput(
                  1,
                  SolanaSccpProver.DOMAIN_SORA,
                  SolanaSccpProver.DOMAIN_SOLANA,
                  "RecursiveZk",
                  "CryptographicProof",
                  "TonContract",
                  "TonContract",
                  TonSccpProver.STARK_FRI_PROOF_FAMILY_V1,
                  TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
                  "sccp-message-v1",
                  "sccp-registry-v1",
                  "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                  destinationBinding),
              null,
              null,
              samplePublicInputs(),
              repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      wrongManifestThrew = ex.getMessage().contains("counterpartyDomain");
    }
    assert wrongManifestThrew : "TON metadata must reject lane-foreign manifests";

    final TonSccpProver.Submission submission =
        TonSccpProver.buildSubmission(sampleMessageBodyInput());
    assert submission.version() == 1 : "submission version must be v1";
    assert TonSccpProver.MESSAGE_BODY_BOC_V1.equals(submission.envelopeEncoding())
        : "submission encoding must be TON BOC";
    assert "internal_message".equals(submission.submissionKind())
        : "submission kind must match TON template";
    assert "op::submit_sccp_message_proof".equals(submission.verifierEntrypoint())
        : "submission entrypoint must match TON template";
    assert Arrays.equals(body, submission.messageBodyBoc()) : "submission body must match BOC";
    assert submission.messageBodyBocHex().startsWith("0xb5ee9c72")
        : "submission hex must expose BOC magic";
    assert submission.arguments().size() == 1 : "TON submission must expose one argument";
    assert "message_body_boc".equals(submission.arguments().get(0).key())
        : "TON submission argument key must match template";
    assert "ton_boc".equals(submission.arguments().get(0).encoding())
        : "TON submission argument encoding must match template";
    assert submission.messageBodyBocHex().equals(submission.arguments().get(0).bytesHex())
        : "TON submission argument bytes must match BOC hex";
    assert Arrays.equals(body, submission.envelopeBytes()) : "TON envelope bytes must match BOC";
    assert submission.messageBodyBocHex().equals(submission.envelopeHex())
        : "TON envelope hex must match BOC hex";
    final byte[] exposedBody = submission.messageBodyBoc();
    exposedBody[0] = 0;
    assert Arrays.equals(body, submission.messageBodyBoc())
        : "TON submission BOC getter must return defensive copies";
    final byte[] exposedEnvelope = submission.envelopeBytes();
    exposedEnvelope[0] = 0;
    assert Arrays.equals(body, submission.envelopeBytes())
        : "TON submission envelope getter must return defensive copies";

    final byte[] proofBytes = {1, 2};
    final byte[] bundleBytes = sampleTonBundleBytes();
    final byte[] expectedBundleBytes = Arrays.copyOf(bundleBytes, bundleBytes.length);
    final byte[] metadataBytes = {5, 6};
    final TonSccpProver.ProofResult copiedResult =
        sampleMessageProofResult(
            samplePublicInputs(), proofBytes, bundleBytes, repeat("bb", 32), repeat("56", 32));
    final TonSccpProver.MessageBodyInput copiedInput =
        new TonSccpProver.MessageBodyInput(copiedResult, bundleBytes, metadataBytes);
    proofBytes[0] = 9;
    bundleBytes[0] = 9;
    metadataBytes[0] = 9;
    copiedInput.proofBytes()[0] = 9;
    copiedInput.bundleBytes()[0] = 9;
    copiedInput.metadataBytes()[0] = 9;
    assert Arrays.equals(new byte[] {1, 2}, copiedInput.proofBytes())
        : "TON message-body proof bytes must be defensive copies";
    assert Arrays.equals(expectedBundleBytes, copiedInput.bundleBytes())
        : "TON message-body bundle bytes must be defensive copies";
    assert Arrays.equals(new byte[] {5, 6}, copiedInput.metadataBytes())
        : "TON message-body metadata bytes must be defensive copies";

    boolean threw = false;
    try {
      TonSccpProver.buildSubmission(sampleMessageBodyInput(new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes");
    }
    assert threw : "empty TON submission bundle bytes must be rejected";

    boolean zeroBundleThrew = false;
    try {
      TonSccpProver.buildSubmission(sampleMessageBodyInput(new byte[] {0, 0}));
    } catch (final IllegalArgumentException ex) {
      zeroBundleThrew = ex.getMessage().contains("bundleBytes must not be all zero");
    }
    assert zeroBundleThrew : "all-zero TON submission bundle bytes must be rejected";

    final byte[] oversizedBundle = new byte[TonSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedBundle, (byte) 1);
    boolean oversizedBundleThrew = false;
    try {
      TonSccpProver.buildSubmission(sampleMessageBodyInput(oversizedBundle));
    } catch (final IllegalArgumentException ex) {
      oversizedBundleThrew = ex.getMessage().contains("bundleBytes must be at most");
    }
    assert oversizedBundleThrew : "oversized TON submission bundle bytes must be rejected";

    boolean zeroProofThrew = false;
    try {
      TonSccpProver.buildSubmission(sampleMessageBodyInput(new byte[] {0, 0}, sampleTonBundleBytes()));
    } catch (final IllegalArgumentException ex) {
      zeroProofThrew = ex.getMessage().contains("all zero");
    }
    assert zeroProofThrew : "all-zero TON submission proof bytes must be rejected";

    final byte[] oversizedTonMessageProof = new byte[4096 * 127];
    Arrays.fill(oversizedTonMessageProof, (byte) 1);
    boolean oversizedTonMessageThrew = false;
    try {
      TonSccpProver.buildSubmission(
          sampleMessageBodyInput(oversizedTonMessageProof, sampleTonBundleBytes()));
    } catch (final IllegalArgumentException ex) {
      oversizedTonMessageThrew = ex.getMessage().contains("TON BOC contains too many cells");
    }
    assert oversizedTonMessageThrew : "oversized TON message-body BOC must be rejected";

    boolean zeroStatementHashThrew = false;
    try {
      TonSccpProver.buildSubmission(
          sampleMessageBodyInput(
              samplePublicInputs(),
              new byte[] {1, 2, 3, 4},
              sampleTonBundleBytes(),
              repeat("00", 32),
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      zeroStatementHashThrew = ex.getMessage().contains("statementHash must not be zero");
    }
    assert zeroStatementHashThrew : "zero TON submission statement hash must be rejected";

    boolean zeroDestinationBindingHashThrew = false;
    try {
      TonSccpProver.buildSubmission(
          sampleMessageBodyInput(
              samplePublicInputs(),
              new byte[] {1, 2, 3, 4},
              sampleTonBundleBytes(),
              repeat("bb", 32),
              repeat("00", 32)));
    } catch (final IllegalArgumentException ex) {
      zeroDestinationBindingHashThrew =
          ex.getMessage().contains("destinationBindingHash must not be zero");
    }
    assert zeroDestinationBindingHashThrew
        : "zero TON submission destination binding hash must be rejected";

    boolean wrongTargetDomainThrew = false;
    try {
      TonSccpProver.buildSubmission(
          sampleMessageBodyInput(
              new TonSccpProver.PublicInputsInput(
                  1,
                  repeat("dd", 32),
                  repeat("ee", 32),
                  SolanaSccpProver.DOMAIN_SOLANA,
                  repeat("12", 32),
                  "19",
                  repeat("aa", 32)),
              new byte[] {1, 2, 3, 4},
              sampleTonBundleBytes(),
              repeat("bb", 32),
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      wrongTargetDomainThrew = ex.getMessage().contains("targetDomain must be TON");
    }
    assert wrongTargetDomainThrew : "TON submissions must reject non-TON target domains";
  }

  private static void derivesTonBocRootHashFromOrdinaryCells() {
    final byte[] boc = hexBytes("b5ee9c720101020100070001020101000202");
    final byte[] checkedBoc = hexBytes("b5ee9c724101020100070001020101000202be1c1df5");
    final String rootHash =
        "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe";

    assert TonSccpProver.bocRootHashes(boc).equals(Collections.singletonList(rootHash))
        : "ordinary TON BoC root hash must match Rust";
    assert TonSccpProver.bocSingleRootHash(boc).equals(rootHash)
        : "single-root helper must return the root hash";
    assert TonSccpProver.bocSingleRootHash(checkedBoc).equals(rootHash)
        : "CRC32C-checked TON BoC root hash must match";

    final byte[] badCrc = checkedBoc.clone();
    badCrc[badCrc.length - 1] ^= 0x01;
    boolean badCrcThrew = false;
    try {
      TonSccpProver.bocSingleRootHash(badCrc);
    } catch (final IllegalArgumentException ex) {
      badCrcThrew = true;
    }
    assert badCrcThrew : "invalid TON BoC CRC32C must be rejected";

    final byte[] changedChild = boc.clone();
    changedChild[changedChild.length - 1] ^= 0x01;
    assert !TonSccpProver.bocSingleRootHash(changedChild).equals(rootHash)
        : "child data must affect the BoC root hash";

    final byte[] cyclicRef = boc.clone();
    cyclicRef[14] = 0;
    boolean cyclicRefThrew = false;
    try {
      TonSccpProver.bocSingleRootHash(cyclicRef);
    } catch (final IllegalArgumentException ex) {
      cyclicRefThrew = true;
    }
    assert cyclicRefThrew : "cyclic TON BoC refs must be rejected";

    final byte[] explicitHashDescriptor = boc.clone();
    explicitHashDescriptor[11] = (byte) (explicitHashDescriptor[11] | 0x10);
    boolean explicitHashDescriptorThrew = false;
    try {
      TonSccpProver.bocSingleRootHash(explicitHashDescriptor);
    } catch (final IllegalArgumentException ex) {
      explicitHashDescriptorThrew = true;
    }
    assert explicitHashDescriptorThrew : "explicit TON BoC descriptor hashes must be rejected";

    final byte[] invalidPartialData = boc.clone();
    invalidPartialData[16] = 1;
    invalidPartialData[17] = 0;
    boolean invalidPartialDataThrew = false;
    try {
      TonSccpProver.bocSingleRootHash(invalidPartialData);
    } catch (final IllegalArgumentException ex) {
      invalidPartialDataThrew = true;
    }
    assert invalidPartialDataThrew : "invalid TON BoC partial-byte padding must be rejected";

    final byte[] prunedBranchBoc =
        hexBytes(
            "b5ee9c72010101010026002848010149725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe0001");
    final String prunedBranchRootHash =
        "0xcc9095f882fb62a27bb19ad4aa84e19571a3283988ae40b75e238ad240cf1a96";
    assert TonSccpProver.bocSingleRootHash(prunedBranchBoc).equals(prunedBranchRootHash)
        : "TON pruned branch BoC root hash must match Rust";

    final byte[] legacyPrunedProofBoc =
        hexBytes(
            "b5ee9c7201010601005f0022012001052201620203284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0040004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001");
    final String legacyPrunedProofRootHash =
        "0x9c769b035b601b0ddc098e9b148d9bdab0761c14bfe310ac090962ba1f39739a";
    assert TonSccpProver.bocSingleRootHash(legacyPrunedProofBoc)
            .equals(legacyPrunedProofRootHash)
        : "TON legacy pruned-branch proof BoC root hash must match Rust";

    final byte[] merkleProofBoc =
        hexBytes(
            "b5ee9c7201010301002d0009460349725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe00010101020102000202");
    final String merkleProofRootHash =
        "0xe749bc5225cabbe3fa78fc12d74a734c365379bc0d302123dcf7bfa2ee3fbd21";
    assert TonSccpProver.bocSingleRootHash(merkleProofBoc).equals(merkleProofRootHash)
        : "TON Merkle proof BoC root hash must match Rust";

    final byte[] mismatchedMerkleProof = merkleProofBoc.clone();
    mismatchedMerkleProof[14] ^= 0x01;
    boolean mismatchedMerkleProofThrew = false;
    try {
      TonSccpProver.bocSingleRootHash(mismatchedMerkleProof);
    } catch (final IllegalArgumentException ex) {
      mismatchedMerkleProofThrew = true;
    }
    assert mismatchedMerkleProofThrew : "mismatched TON Merkle proof cells must be rejected";

    final byte[] hashmapBoc =
        hexBytes(
            "b5ee9c72010109010028000101c001020120020702016203050103a0c004000403090103a0c0060004006f0101de08000403e7");
    final String hashmapValueHash =
        "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419";
    assert hashmapValueHash.equals(
            TonSccpProver.hashmapECellRefValueHash(hashmapBoc, new byte[] {17}, 8))
        : "TON HashmapE value hash must match Rust";
    assert TonSccpProver.hashmapECellRefValueHash(hashmapBoc, new byte[] {18}, 8) == null
        : "absent TON HashmapE key must return null";
    boolean badHashmapKeyThrew = false;
    try {
      TonSccpProver.hashmapECellRefValueHash(hashmapBoc, new byte[] {17}, 7);
    } catch (final IllegalArgumentException ex) {
      badHashmapKeyThrew = true;
    }
    assert badHashmapKeyThrew : "non-canonical TON HashmapE key must be rejected";

    final byte[] hashmapDirectProofBoc =
        hexBytes(
            "b5ee9c72010107010063002101c00122012002062201620304284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0050004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001");
    assert hashmapValueHash.equals(
            TonSccpProver.hashmapECellRefValueHash(hashmapDirectProofBoc, new byte[] {17}, 8))
        : "TON HashmapE direct proof value hash must match Rust";
    assert TonSccpProver.hashmapECellRefValueHash(hashmapDirectProofBoc, new byte[] {1}, 8) == null
        : "pruned TON HashmapE selected path must fail closed";

    final byte[] hashmapMerkleProofBoc =
        hexBytes(
            "b5ee9c72010108010089000101c001094603e714f85374c2c336ed499a5a35e6c4f87441184532e7c23be795ce71b457f1bf00030222012003072201620405284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0060004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001");
    assert hashmapValueHash.equals(
            TonSccpProver.hashmapECellRefValueHash(hashmapMerkleProofBoc, new byte[] {17}, 8))
        : "TON HashmapE Merkle proof value hash must match Rust";

    final byte[] shardAccountsBoc =
        hexBytes(
            "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000");
    final byte[] shardAccountKey = new byte[32];
    shardAccountKey[0] = 17;
    final byte[] absentShardAccountKey = new byte[32];
    absentShardAccountKey[0] = 18;
    assert hashmapValueHash.equals(
            TonSccpProver.shardAccountsLastTransactionHash(
                shardAccountsBoc, shardAccountKey, 256))
        : "TON ShardAccounts last transaction hash must match Rust";
    assert new TonSccpProver.ShardAccountLastTransaction(
            hashmapValueHash, BigInteger.valueOf(7))
        .equals(TonSccpProver.shardAccountsLastTransaction(shardAccountsBoc, shardAccountKey, 256))
        : "TON ShardAccounts last transaction identity must match Rust";
    assert TonSccpProver.shardAccountsLastTransactionHash(shardAccountsBoc, absentShardAccountKey, 256)
            == null
        : "absent TON ShardAccounts key must return null";
    boolean badShardAccountsKeyBitsThrew = false;
    try {
      TonSccpProver.shardAccountsLastTransactionHash(shardAccountsBoc, new byte[] {17}, 8);
    } catch (final IllegalArgumentException ex) {
      badShardAccountsKeyBitsThrew = true;
    }
    assert badShardAccountsKeyBitsThrew : "short TON ShardAccounts keys must be rejected";

    final byte[] shardStateProofBoc =
        hexBytes(
            "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000");
    assert "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270"
        .equals(TonSccpProver.shardStateProofRootHash(shardStateProofBoc))
        : "TON shard-state proof root must match Rust";
    assert "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3"
        .equals(TonSccpProver.shardStateAccountsRootHash(shardStateProofBoc))
        : "TON shard-state accounts root must match Rust";
    final byte[] badShardStateTag = shardStateProofBoc.clone();
    final int tagOffset = indexOfBytes(badShardStateTag, hexBytes("9023afe2"));
    assert tagOffset >= 0 : "test shard-state fixture must contain the expected tag";
    badShardStateTag[tagOffset] ^= 0x01;
    boolean badShardStateTagThrew = false;
    try {
      TonSccpProver.shardStateAccountsRootHash(badShardStateTag);
    } catch (final IllegalArgumentException ex) {
      badShardStateTagThrew = true;
    }
    assert badShardStateTagThrew : "invalid TON ShardStateUnsplit tag must be rejected";
    final int shardIdentOffset = tagOffset + 8;
    final byte[] badShardIdentTag = shardStateProofBoc.clone();
    badShardIdentTag[shardIdentOffset] |= (byte) 0x80;
    boolean badShardIdentTagThrew = false;
    try {
      TonSccpProver.shardStateAccountsRootHash(badShardIdentTag);
    } catch (final IllegalArgumentException ex) {
      badShardIdentTagThrew = true;
    }
    assert badShardIdentTagThrew : "invalid TON ShardIdent tag must be rejected";
    final byte[] badShardIdentPrefixLen = shardStateProofBoc.clone();
    badShardIdentPrefixLen[shardIdentOffset] = 0x3d;
    boolean badShardIdentPrefixLenThrew = false;
    try {
      TonSccpProver.shardStateAccountsRootHash(badShardIdentPrefixLen);
    } catch (final IllegalArgumentException ex) {
      badShardIdentPrefixLenThrew = true;
    }
    assert badShardIdentPrefixLenThrew : "invalid TON ShardIdent prefix length must be rejected";
    final byte[] basechainCustom = shardStateProofBoc.clone();
    basechainCustom[tagOffset + 45] |= (byte) 0x40;
    boolean basechainCustomThrew = false;
    try {
      TonSccpProver.shardStateAccountsRootHash(basechainCustom);
    } catch (final IllegalArgumentException ex) {
      basechainCustomThrew = ex.getMessage().contains("custom");
    }
    assert basechainCustomThrew : "TON basechain shard-state custom refs must be rejected";
  }

  private static void derivesTonShardProofHashFromWitnessMaterial() {
    final byte[] branch = repeatedByte((byte) 0xee, 32);
    final byte[] shardStateBranch = repeatedByte((byte) 0x12, 32);
    final byte[] bytes =
        TonSccpProver.canonicalShardProofBytes(
            repeat("34", 32),
            "19",
            repeat("aa", 32),
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            repeat("cc", 32),
            repeat("dd", 32),
            "7",
            "0",
            Collections.singletonList(shardStateBranch),
            Collections.singletonList(branch));

    assert bytes.length == 309
        : "TON shard proof transcript must have the expected fixed length";
    assert bytes[0] == 1 : "TON shard proof transcript version must be first";

    final String hash =
        TonSccpProver.shardProofHash(
            repeat("34", 32),
            "19",
            repeat("aa", 32),
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            repeat("cc", 32),
            repeat("dd", 32),
            "7",
            "0",
            Collections.singletonList(shardStateBranch),
            Collections.singletonList(branch));
    final String changed =
        TonSccpProver.shardProofHash(
            repeat("34", 32),
            "19",
            repeat("aa", 32),
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            repeat("cc", 32),
            repeat("dd", 32),
            "7",
            "0",
            Collections.singletonList(shardStateBranch),
            Collections.singletonList(repeatedByte((byte) 0x12, 32)));
    final String changedShardState =
        TonSccpProver.shardProofHash(
            repeat("34", 32),
            "19",
            repeat("aa", 32),
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            repeat("cc", 32),
            repeat("dd", 32),
            "7",
            "0",
            Collections.singletonList(repeatedByte((byte) 0xee, 32)),
            Collections.singletonList(branch));
    assert "0x09c63ca1185b537f0a37b7b248600a0992e5b7ed64ace9d1d437db7caae00686".equals(hash)
        : "TON shard proof hash must match Rust";
    assert !hash.equals(changed) : "TON shard proof hash must bind inclusion branch";
    assert !hash.equals(changedShardState) : "TON shard proof hash must bind shard-state branch";
    boolean zeroSourceEventDigestThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("00", 32),
          "19",
          repeat("aa", 32),
          0,
          "9223372036854775808",
          "7",
          repeat("bb", 32),
          repeat("bc", 32),
          repeat("cc", 32),
          repeat("dd", 32),
          "7",
          "0",
          Collections.singletonList(shardStateBranch),
          Collections.singletonList(branch));
    } catch (final IllegalArgumentException ex) {
      zeroSourceEventDigestThrew = ex.getMessage().contains("sourceEventDigest must not be zero");
    }
    assert zeroSourceEventDigestThrew : "zero TON source event digest must be rejected";

    final byte[] shardAccountsBoc =
        hexBytes(
            "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000");
    final byte[] shardStateProofBoc =
        hexBytes(
            "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000");
    final byte[] shardAccountKey = new byte[32];
    shardAccountKey[0] = 17;
    final String shardStateRoot =
        "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270";
    final String shardAccountsRoot =
        "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3";
    final byte[] dictionaryBytes =
        TonSccpProver.canonicalShardProofBytes(
            repeat("34", 32),
            "19",
            repeat("aa", 32),
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            shardStateRoot,
            "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            "7",
            "0",
            Collections.emptyList(),
            Collections.singletonList(branch),
            shardAccountsRoot,
            256,
            shardAccountKey,
            shardAccountsBoc,
            shardStateProofBoc);
    final String dictionaryHash =
        TonSccpProver.shardProofHash(
            repeat("34", 32),
            "19",
            repeat("aa", 32),
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            shardStateRoot,
            "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            "7",
            "0",
            Collections.emptyList(),
            Collections.singletonList(branch),
            shardAccountsRoot,
            256,
            shardAccountKey,
            shardAccountsBoc,
            shardStateProofBoc);
    assert dictionaryBytes.length == 662 : "TON dictionary shard proof transcript length must match Rust";
    assert "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf"
        .equals(dictionaryHash) : "TON dictionary shard proof hash must match Rust";
    assert !dictionaryHash.equals(hash) : "TON dictionary fields must change the shard proof hash";

    boolean wrongTransactionLtThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
          0,
          "9223372036854775808",
          "7",
          repeat("bb", 32),
          repeat("bc", 32),
          shardStateRoot,
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "8",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          shardStateProofBoc);
    } catch (final IllegalArgumentException ex) {
      wrongTransactionLtThrew = ex.getMessage().contains("transaction lt");
    }
    assert wrongTransactionLtThrew : "mismatched TON dictionary value logical time must be rejected";

    boolean unusedShardStateBranchThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          shardStateRoot,
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.singletonList(shardStateBranch),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          shardStateProofBoc);
    } catch (final IllegalArgumentException ex) {
      unusedShardStateBranchThrew = true;
    }
    assert unusedShardStateBranchThrew : "dictionary TON shard-state branches must be empty";

    boolean wrongShardStateRootThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          repeat("66", 32),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          shardStateProofBoc);
    } catch (final IllegalArgumentException ex) {
      wrongShardStateRootThrew = true;
    }
    assert wrongShardStateRootThrew : "mismatched TON shard-state root must be rejected";

    boolean wrongTransactionRootThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          shardStateRoot,
          repeat("66", 32),
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          shardStateProofBoc);
    } catch (final IllegalArgumentException ex) {
      wrongTransactionRootThrew = true;
    }
    assert wrongTransactionRootThrew : "mismatched TON dictionary value hash must be rejected";

    boolean wrongDictionaryRootThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          shardStateRoot,
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          repeat("66", 32),
          256,
          shardAccountKey,
          shardAccountsBoc,
          shardStateProofBoc);
    } catch (final IllegalArgumentException ex) {
      wrongDictionaryRootThrew = true;
    }
    assert wrongDictionaryRootThrew : "mismatched TON accounts root must be rejected";

    boolean zeroDictionaryRootThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          repeat("cc", 32),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.singletonList(shardStateBranch),
          Collections.singletonList(branch),
          repeat("00", 32),
          256,
          shardAccountKey,
          shardAccountsBoc,
          shardStateProofBoc);
    } catch (final IllegalArgumentException ex) {
      zeroDictionaryRootThrew = true;
    }
    assert zeroDictionaryRootThrew : "zero TON shard-state dictionary root must be rejected";

    final byte[] wrongGlobalIdProofBoc = shardStateProofBoc.clone();
    final int wrongGlobalIdTagOffset = indexOfBytes(wrongGlobalIdProofBoc, hexBytes("9023afe2"));
    assert wrongGlobalIdTagOffset >= 0 : "test shard-state fixture must contain the expected tag";
    Arrays.fill(wrongGlobalIdProofBoc, wrongGlobalIdTagOffset + 4, wrongGlobalIdTagOffset + 8, (byte) 0);
    assert shardAccountsRoot.equals(TonSccpProver.shardStateAccountsRootHash(wrongGlobalIdProofBoc))
        : "wrong-global-id fixture must keep the accounts root";
    boolean wrongGlobalIdThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          TonSccpProver.shardStateProofRootHash(wrongGlobalIdProofBoc),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          wrongGlobalIdProofBoc);
    } catch (final IllegalArgumentException ex) {
      wrongGlobalIdThrew = ex.getMessage().contains("global_id");
    }
    assert wrongGlobalIdThrew : "TON shard-state global_id must be mainnet";

    final byte[] wrongWorkchainIdProofBoc = shardStateProofBoc.clone();
    final int wrongWorkchainIdTagOffset =
        indexOfBytes(wrongWorkchainIdProofBoc, hexBytes("9023afe2"));
    assert wrongWorkchainIdTagOffset >= 0 : "test shard-state fixture must contain the expected tag";
    final int wrongWorkchainShardIdentOffset = wrongWorkchainIdTagOffset + 8;
    Arrays.fill(
        wrongWorkchainIdProofBoc,
        wrongWorkchainShardIdentOffset + 1,
        wrongWorkchainShardIdentOffset + 5,
        (byte) 0xff);
    assert shardAccountsRoot.equals(TonSccpProver.shardStateAccountsRootHash(wrongWorkchainIdProofBoc))
        : "wrong-workchain-id fixture must keep the accounts root";
    boolean wrongWorkchainIdThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          TonSccpProver.shardStateProofRootHash(wrongWorkchainIdProofBoc),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          wrongWorkchainIdProofBoc);
    } catch (final IllegalArgumentException ex) {
      wrongWorkchainIdThrew = ex.getMessage().contains("workchain_id");
    }
    assert wrongWorkchainIdThrew : "TON shard-state workchain_id must be basechain";

    final byte[] zeroGenUtimeProofBoc = shardStateProofBoc.clone();
    final int zeroGenUtimeTagOffset = indexOfBytes(zeroGenUtimeProofBoc, hexBytes("9023afe2"));
    assert zeroGenUtimeTagOffset >= 0 : "test shard-state fixture must contain the expected tag";
    Arrays.fill(zeroGenUtimeProofBoc, zeroGenUtimeTagOffset + 29, zeroGenUtimeTagOffset + 33, (byte) 0);
    boolean zeroGenUtimeThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          TonSccpProver.shardStateProofRootHash(zeroGenUtimeProofBoc),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          zeroGenUtimeProofBoc);
    } catch (final IllegalArgumentException ex) {
      zeroGenUtimeThrew = ex.getMessage().contains("gen_utime");
    }
    assert zeroGenUtimeThrew : "TON shard-state generation time must be non-zero";

    final byte[] futureMinRefMcSeqnoProofBoc = shardStateProofBoc.clone();
    final int futureMinRefMcSeqnoTagOffset =
        indexOfBytes(futureMinRefMcSeqnoProofBoc, hexBytes("9023afe2"));
    assert futureMinRefMcSeqnoTagOffset >= 0 : "test shard-state fixture must contain the expected tag";
    futureMinRefMcSeqnoProofBoc[futureMinRefMcSeqnoTagOffset + 44] = 0x14;
    boolean futureMinRefMcSeqnoThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          TonSccpProver.shardStateProofRootHash(futureMinRefMcSeqnoProofBoc),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          futureMinRefMcSeqnoProofBoc);
    } catch (final IllegalArgumentException ex) {
      futureMinRefMcSeqnoThrew = ex.getMessage().contains("min_ref_mc_seqno");
    }
    assert futureMinRefMcSeqnoThrew : "TON shard-state min_ref_mc_seqno must not exceed masterchain seqno";

    final byte[] mismatchedShardPrefixProofBoc = shardStateProofBoc.clone();
    final int mismatchedTagOffset = indexOfBytes(mismatchedShardPrefixProofBoc, hexBytes("9023afe2"));
    assert mismatchedTagOffset >= 0 : "test shard-state fixture must contain the expected tag";
    final int mismatchedShardIdentOffset = mismatchedTagOffset + 8;
    mismatchedShardPrefixProofBoc[mismatchedShardIdentOffset] = 0x08;
    mismatchedShardPrefixProofBoc[mismatchedShardIdentOffset + 5] = 0x12;
    assert shardAccountsRoot.equals(
            TonSccpProver.shardStateAccountsRootHash(mismatchedShardPrefixProofBoc))
        : "mismatched-prefix fixture must keep the accounts root";
    boolean mismatchedShardPrefixThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "1333065489701666816",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          TonSccpProver.shardStateProofRootHash(mismatchedShardPrefixProofBoc),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.emptyList(),
          Collections.singletonList(branch),
          shardAccountsRoot,
          256,
          shardAccountKey,
          shardAccountsBoc,
          mismatchedShardPrefixProofBoc);
    } catch (final IllegalArgumentException ex) {
      mismatchedShardPrefixThrew = ex.getMessage().contains("ShardIdent prefix");
    }
    assert mismatchedShardPrefixThrew : "TON ShardAccounts key must match proven ShardIdent prefix";

    boolean badDictionaryKeyThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          repeat("cc", 32),
          "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
          "7",
          "0",
          Collections.singletonList(shardStateBranch),
          Collections.singletonList(branch),
          shardAccountsRoot,
          7,
          new byte[] {17},
          shardAccountsBoc,
          shardStateProofBoc);
    } catch (final IllegalArgumentException ex) {
      badDictionaryKeyThrew = true;
    }
    assert badDictionaryKeyThrew : "non-canonical TON shard-state dictionary key must be rejected";

    boolean threw = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          repeat("cc", 32),
          repeat("dd", 32),
          "7",
          "0",
          Collections.singletonList(shardStateBranch),
          Collections.singletonList(new byte[] {1, 2, 3}));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("inclusionBranch[0]");
    }
    assert threw : "malformed TON inclusion branch must be rejected";

    boolean oversizedBranchThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          repeat("cc", 32),
          repeat("dd", 32),
          "7",
          "0",
          Collections.singletonList(shardStateBranch),
          Collections.nCopies(65, repeatedByte((byte) 0xee, 32)));
    } catch (final IllegalArgumentException ex) {
      oversizedBranchThrew = ex.getMessage().contains("at most 64");
    }
    assert oversizedBranchThrew : "oversized TON inclusion branch must be rejected";

    boolean badShardStateBranchThrew = false;
    try {
      TonSccpProver.canonicalShardProofBytes(
          repeat("34", 32),
          "19",
          repeat("aa", 32),
           0,
           "9223372036854775808",
           "7",
           repeat("bb", 32),
           repeat("bc", 32),
          repeat("cc", 32),
          repeat("dd", 32),
          "7",
          "0",
          Collections.singletonList(new byte[] {1, 2, 3}),
          Collections.singletonList(branch));
    } catch (final IllegalArgumentException ex) {
      badShardStateBranchThrew = ex.getMessage().contains("inclusionBranch[0]");
    }
    assert badShardStateBranchThrew : "malformed TON shard-state branch must be rejected";
  }

  private static void buildsTonShardStateOpenVerifyProofRequestFromWitnessMaterial() {
    final TonSccpProver.ShardStateProofRequestInput input = sampleShardStateProofRequestInput();
    final byte[] statement = TonSccpProver.canonicalShardStateProofPublicInputsBytes(input);
    final byte[] witness = TonSccpProver.canonicalShardStateWitnessCommitmentBytes(input);
    final byte[] context = TonSccpProver.canonicalShardStateVerificationContextBytes(input);
    final byte[] schema = TonSccpProver.shardStateOpenVerifySchemaDescriptor(input);
    final String publicInputsHash = TonSccpProver.shardStateProofPublicInputsHash(input);
    final List<List<String>> columns = TonSccpProver.shardStatePublicInputColumns(input);
    final TonSccpProver.ShardStateProofRequest request =
        TonSccpProver.buildShardStateProofRequest(input);

    assert statement.length == 603 : "TON shard-state statement length must match Rust";
    assert "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19"
        .equals(publicInputsHash) : "TON shard-state public input hash must match Rust";
    assert witness.length == 480 : "TON shard-state witness length must match Rust";
    assert context.length == 467 : "TON shard-state context length must match Rust";
    assert schema.length == 436 : "TON shard-state schema length must match Rust";
    assert TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(request.circuitId())
        : "TON shard-state request must name the circuit";
    assert "stark-fri-v1".equals(request.proofFamily())
        : "TON shard-state request must name the proof family";
    assert "0x27e44edc7d124906a8176e94557996c3"
        .equals(request.fastpqPublicInputs().dsid()) : "TON shard-state FastPQ dsid must match Rust";
    assert publicInputsHash.equals(request.fastpqPublicInputs().txSetHash())
        : "TON shard-state FastPQ tx_set_hash must bind public inputs";
    assert publicInputsHash.equals(request.shardStateProofPublicInputsHash())
        : "TON shard-state request must expose the public input hash";
    assert publicInputsHash.equals(columns.get(15).get(0))
        : "TON shard-state public columns must include the public input hash";
    assert publicInputsHash.equals(request.publicInputColumns().get(15).get(0))
        : "TON shard-state request columns must include the public input hash";
    assert "sccp:ton:shard-state:v1:statement".equals(request.fastpqTransitions().get(0).key())
        : "TON shard-state statement transition key must match Rust";
    assert "sccp:ton:shard-state:v1:witness".equals(request.fastpqTransitions().get(1).key())
        : "TON shard-state witness transition key must match Rust";
    assert "sccp:ton:shard-state:v1:context".equals(request.fastpqTransitions().get(2).key())
        : "TON shard-state context transition key must match Rust";
    assert Arrays.equals(statement, request.statementBytes())
        : "TON shard-state request must carry statement bytes";
    assert Arrays.equals(witness, request.witnessCommitmentBytes())
        : "TON shard-state request must carry witness bytes";
    assert Arrays.equals(context, request.verificationContextBytes())
        : "TON shard-state request must carry context bytes";
    assert Arrays.equals(schema, request.schemaDescriptor())
        : "TON shard-state request must carry schema bytes";

    final TonSccpProver.ValidatorSetTransitionProofInput transitionProof =
        sampleValidatorSetTransitionProofInput();
    final TonSccpProver.ShardStateProofRequestInput transitionBoundInput =
        sampleShardStateProofRequestInput(Collections.singletonList(transitionProof));
    final byte[] tamperedTransitionSignature = repeatedByte((byte) 0xab, 64);
    tamperedTransitionSignature[0] = (byte) 0xaa;
    final TonSccpProver.ValidatorSetTransitionProofInput tamperedTransitionProof =
        sampleValidatorSetTransitionProofInput(
            Arrays.asList(tamperedTransitionSignature, repeatedByte((byte) 0xcd, 64)));
    assert !Arrays.equals(
            TonSccpProver.canonicalShardStateProofPublicInputsBytes(transitionBoundInput),
            TonSccpProver.canonicalShardStateProofPublicInputsBytes(
                sampleShardStateProofRequestInput(
                    Collections.singletonList(tamperedTransitionProof))))
        : "TON shard-state public inputs must bind nested transition signature bytes";

    final byte[] exposedStatement = request.statementBytes();
    exposedStatement[0] = 9;
    assert Arrays.equals(statement, request.statementBytes())
        : "TON shard-state request byte getters must be defensive copies";

    boolean templateSourceStateThrew = false;
    try {
      TonSccpProver.buildShardStateProofRequest(
          sampleShardStateProofRequestInput(
              "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
              "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f"));
    } catch (final IllegalArgumentException ex) {
      templateSourceStateThrew = ex.getMessage().contains("TON template verifier hash");
    }
    assert templateSourceStateThrew
        : "TON shard-state request must reject the template source-state verifier hash";

    boolean mismatchedTransactionRootThrew = false;
    try {
      TonSccpProver.shardStateProofPublicInputsHash(
          sampleShardStateProofRequestInput(repeat("66", 32)));
    } catch (final IllegalArgumentException ex) {
      mismatchedTransactionRootThrew = true;
    }
    assert mismatchedTransactionRootThrew
        : "TON shard-state proof request must reject mismatched transaction roots";
  }

  private static void buildsTonFullLightClientAuditRoleProofRequests() {
    final TonSccpProver.FullLightClientAuditProofInput input =
        sampleFullLightClientAuditProofInput();
    final TonSccpProver.FullLightClientAuditProofRequests requests =
        TonSccpProver.buildFullLightClientAuditProofRequests(input);
    final String shardStateProofPublicInputsHash =
        TonSccpProver.shardStateProofPublicInputsHash(input.shardState());
    final String shardStateVerificationProofHash =
        TonSccpProver.shardStateVerificationProofHash(input.shardStateVerificationProof());
    boolean badShardStateVerificationProofVersionThrew = false;
    try {
      TonSccpProver.canonicalSourceStateVerificationProofBytes(
          new TonSccpProver.SourceStateVerificationProof(
              0,
              "stark-fri-v1",
              TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
              new byte[] {1, 2, 3}));
    } catch (final IllegalArgumentException ex) {
      badShardStateVerificationProofVersionThrew = ex.getMessage().contains("version");
    }
    assert badShardStateVerificationProofVersionThrew
        : "TON source-state verification proof version must be v1";
    boolean badShardStateVerificationProofFamilyThrew = false;
    try {
      TonSccpProver.canonicalSourceStateVerificationProofBytes(
          new TonSccpProver.SourceStateVerificationProof(
              1,
              "debug-proof-family",
              TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
              new byte[] {1, 2, 3}));
    } catch (final IllegalArgumentException ex) {
      badShardStateVerificationProofFamilyThrew = ex.getMessage().contains("stark-fri-v1");
    }
    assert badShardStateVerificationProofFamilyThrew
        : "TON source-state verification proof family must be stark-fri-v1";
    boolean nullProofFamilyThrew = false;
    try {
      new TonSccpProver.SourceStateVerificationProof(
          1,
          null,
          TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
          new byte[] {1, 2, 3});
    } catch (final NullPointerException ex) {
      nullProofFamilyThrew = "proofFamily".equals(ex.getMessage());
    }
    assert nullProofFamilyThrew : "TON source-state proof family must be required";
    boolean nullCircuitIdThrew = false;
    try {
      new TonSccpProver.SourceStateVerificationProof(
          1,
          "stark-fri-v1",
          null,
          new byte[] {1, 2, 3});
    } catch (final NullPointerException ex) {
      nullCircuitIdThrew = "circuitId".equals(ex.getMessage());
    }
    assert nullCircuitIdThrew : "TON source-state proof circuit id must be required";
    final byte[] sourceStateProofBytes = {4, 5, 6};
    final TonSccpProver.SourceStateVerificationProof copiedProof =
        new TonSccpProver.SourceStateVerificationProof(sourceStateProofBytes);
    sourceStateProofBytes[0] = 0;
    copiedProof.proofBytes()[1] = 0;
    assert Arrays.equals(new byte[] {4, 5, 6}, copiedProof.proofBytes())
        : "TON source-state proof bytes must be defensive copies";
    assert "BAUG".equals(copiedProof.proofBase64())
        : "TON source-state proof base64 must be derived from stored proof bytes";
    final List<TonSccpProver.FullLightClientAuditProofRequest> allRequests =
        Arrays.asList(
            requests.masterchainConfig(),
            requests.validatorSetTransition(),
            requests.shardAccountsDictionary());
    final java.util.HashSet<String> circuitIds = new java.util.HashSet<String>();

    assert TonSccpProver.MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
        .equals(requests.masterchainConfig().circuitId())
        : "TON masterchain config audit request must name its circuit";
    assert TonSccpProver.VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1
        .equals(requests.validatorSetTransition().circuitId())
        : "TON validator-set transition audit request must name its circuit";
    assert TonSccpProver.SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1
        .equals(requests.shardAccountsDictionary().circuitId())
        : "TON shard accounts audit request must name its circuit";
    for (final TonSccpProver.FullLightClientAuditProofRequest request : allRequests) {
      circuitIds.add(request.circuitId());
    }
    assert circuitIds.size() == 3 : "TON audit role circuits must be distinct";
    assert "masterchain_config".equals(requests.masterchainConfig().role())
        : "TON masterchain config audit request role must use its wire id";
    assert "validator_set_transition".equals(requests.validatorSetTransition().role())
        : "TON validator-set transition audit request role must use its wire id";
    assert "shard_accounts_dictionary".equals(requests.shardAccountsDictionary().role())
        : "TON shard accounts audit request role must use its wire id";
    assert TonSccpProver.canonicalFullLightClientAuditStatementBytes(
            input, TonSccpProver.FullLightClientAuditRole.MASTERCHAIN_CONFIG)
        .length > 0 : "TON audit statement bytes must be exposed";

    for (final TonSccpProver.FullLightClientAuditProofRequest request : allRequests) {
      final TonSccpProver.FullLightClientAuditRole role;
      switch (request.roleCode()) {
        case 1:
          role = TonSccpProver.FullLightClientAuditRole.MASTERCHAIN_CONFIG;
          break;
        case 2:
          role = TonSccpProver.FullLightClientAuditRole.VALIDATOR_SET_TRANSITION;
          break;
        case 3:
          role = TonSccpProver.FullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY;
          break;
        default:
          throw new AssertionError("unexpected TON audit role");
      }
      assert request.version() == 1 : "TON audit request version must be v1";
      assert "stark-fri-v1".equals(request.proofFamily())
          : "TON audit request proof family must be STARK/FRI";
      assert "fastpq-lane-balanced".equals(request.parameterSet())
          : "TON audit request parameter set must match OpenVerify";
      assert request.sourceDomain() == TonSccpProver.DOMAIN_TON
          : "TON audit request source domain must be TON";
      assert "19".equals(request.masterchainSeqno())
          : "TON audit request must expose masterchain seqno";
      assert "7".equals(request.shardSeqno())
          : "TON audit request must expose shard seqno";
      assert TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1
          .equals(request.sourceStateVerifierId())
          : "TON audit request must bind the shard-state verifier";
      assert input.fullLightClientGateHash().equals(request.fullLightClientGateHash())
          : "TON audit request must bind the full light-client gate";
      assert shardStateProofPublicInputsHash.equals(request.shardStateProofPublicInputsHash())
          : "TON audit request must bind shard-state public inputs";
      assert shardStateVerificationProofHash.equals(request.shardStateVerificationProofHash())
          : "TON audit request must bind the source-state proof hash";
      assert TonSccpProver.fullLightClientAuditStatementHash(input, role)
          .equals(request.auditStatementHash())
          : "TON audit request must expose the role statement hash";
      assert request.fastpqTransitions().size() == 3
          : "TON audit request must carry three FastPQ metadata writes";
      final ArrayList<String> keys = new ArrayList<String>();
      for (final TonSccpProver.FullLightClientAuditFastpqTransition transition :
          request.fastpqTransitions()) {
        keys.add(transition.key());
        assert transition.key().startsWith("0x") : "TON audit FastPQ key must be hex";
      }
      final ArrayList<String> sortedKeys = new ArrayList<String>(keys);
      Collections.sort(sortedKeys);
      assert keys.equals(sortedKeys) : "TON audit FastPQ transitions must be deterministic";
    }

    assert requests.masterchainConfig().publicInputColumns().size() == 17
        : "TON masterchain config audit columns must match OpenVerify";
    assert requests.validatorSetTransition().publicInputColumns().size() == 16
        : "TON validator-set transition audit columns must match OpenVerify";
    assert requests.shardAccountsDictionary().publicInputColumns().size() == 17
        : "TON shard accounts audit columns must match OpenVerify";
    assert TonSccpProver.fullLightClientAuditPublicInputColumns(
            input, TonSccpProver.FullLightClientAuditRole.MASTERCHAIN_CONFIG)
        .equals(requests.masterchainConfig().publicInputColumns())
        : "TON audit columns must be exposed exactly";
    assert Arrays.equals(
            TonSccpProver.fullLightClientAuditOpenVerifySchemaDescriptor(
                input, TonSccpProver.FullLightClientAuditRole.MASTERCHAIN_CONFIG),
            requests.masterchainConfig().schemaDescriptor())
        : "TON audit schema descriptor must be exposed exactly";
    assert input.shardState().masterchainConfigRoot()
        .equals(requests.masterchainConfig().fastpqPublicInputs().oldRoot())
        : "TON masterchain config audit FastPQ roots must bind config root";
    assert input.shardState().sourceTrustAnchorHash()
        .equals(requests.validatorSetTransition().fastpqPublicInputs().oldRoot())
        : "TON validator-set audit FastPQ roots must bind source trust anchor";
    assert input.shardState().transactionRoot()
        .equals(requests.shardAccountsDictionary().fastpqPublicInputs().newRoot())
        : "TON shard accounts audit FastPQ roots must bind transaction root";

    final TonSccpProver.ShardStateProofRequest shardRequest =
        TonSccpProver.buildShardStateProofRequest(input.shardState());
    final TonSccpProver.SourceStateVerificationProof wrappedShard =
        TonSccpProver.wrapSourceStateVerificationProof(new byte[] {9, 8, 7}, shardRequest);
    assert TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(wrappedShard.circuitId())
        : "TON wrapped shard-state proof must retain the shard-state circuit";
    assert Arrays.equals(new byte[] {9, 8, 7}, wrappedShard.proofBytes())
        : "TON wrapped shard-state proof must retain proof bytes";
    assert "CQgH".equals(wrappedShard.proofBase64())
        : "TON wrapped shard-state proof must expose proof base64";
    wrappedShard.proofBytes()[0] = 0;
    assert Arrays.equals(new byte[] {9, 8, 7}, wrappedShard.proofBytes())
        : "TON wrapped shard-state proof bytes must remain defensive copies";
    assert "CQgH".equals(wrappedShard.proofBase64())
        : "TON wrapped shard-state proof base64 must stay bound to stored proof bytes";
    assert TonSccpProver.canonicalSourceStateVerificationProofBytes(wrappedShard).length > 0
        : "TON wrapped shard-state proof must canonicalize";
    final TonSccpProver.SourceStateVerificationProof wrappedAudit =
        TonSccpProver.wrapSourceStateVerificationProof(
            new byte[] {1, 2, 3}, requests.masterchainConfig());
    assert TonSccpProver.MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1.equals(
            wrappedAudit.circuitId())
        : "TON wrapped audit proof must retain its role circuit";
    assert "AQID".equals(wrappedAudit.proofBase64())
        : "TON wrapped audit proof must expose proof base64";
    assert TonSccpProver.canonicalSourceStateVerificationProofBytes(wrappedAudit).length > 0
        : "TON wrapped audit proof must canonicalize";
    boolean allZeroWrappedProofThrew = false;
    try {
      TonSccpProver.wrapSourceStateVerificationProof(new byte[] {0, 0}, shardRequest);
    } catch (final IllegalArgumentException ex) {
      allZeroWrappedProofThrew = ex.getMessage().contains("all zero");
    }
    assert allZeroWrappedProofThrew : "TON source-state wrappers must reject all-zero proofs";
    final byte[] oversizedSourceStateProofBytes =
        new byte[TonSccpProver.SOURCE_STATE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedSourceStateProofBytes, (byte) 1);
    boolean oversizedWrappedProofThrew = false;
    try {
      TonSccpProver.wrapSourceStateVerificationProof(
          oversizedSourceStateProofBytes, shardRequest);
    } catch (final IllegalArgumentException ex) {
      oversizedWrappedProofThrew = ex.getMessage().contains("proofBytes must be at most");
    }
    assert oversizedWrappedProofThrew
        : "TON source-state wrappers must reject oversized source-state proofs";
    final ArrayList<TonSccpProver.ShardStateFastpqTransition> tamperedShardTransitions =
        new ArrayList<TonSccpProver.ShardStateFastpqTransition>(
            shardRequest.fastpqTransitions());
    final TonSccpProver.ShardStateFastpqTransition firstShardTransition =
        tamperedShardTransitions.get(0);
    tamperedShardTransitions.set(
        0,
        new TonSccpProver.ShardStateFastpqTransition(
            firstShardTransition.key(),
            firstShardTransition.operation(),
            firstShardTransition.oldValue(),
            "0x00"));
    final TonSccpProver.ShardStateProofRequest tamperedShardRequest =
        new TonSccpProver.ShardStateProofRequest(
            shardRequest.version(),
            shardRequest.proofFamily(),
            shardRequest.circuitId(),
            shardRequest.parameterSet(),
            shardRequest.sourceDomain(),
            shardRequest.masterchainSeqno(),
            shardRequest.shardSeqno(),
            shardRequest.sourceStateVerifierId(),
            shardRequest.sourceStateVerifierHash(),
            shardRequest.shardStateProofPublicInputsHash(),
            shardRequest.statementBytes(),
            shardRequest.witnessCommitmentBytes(),
            shardRequest.verificationContextBytes(),
            shardRequest.schemaDescriptor(),
            shardRequest.publicInputColumns(),
            shardRequest.fastpqPublicInputs(),
            tamperedShardTransitions);
    boolean tamperedShardTransitionThrew = false;
    try {
      TonSccpProver.wrapSourceStateVerificationProof(new byte[] {9, 8, 7}, tamperedShardRequest);
    } catch (final IllegalArgumentException ex) {
      tamperedShardTransitionThrew =
          ex.getMessage().contains("canonical TON source-state request");
    }
    assert tamperedShardTransitionThrew
        : "TON source-state wrappers must reject tampered shard-state FastPQ transitions";
    final TonSccpProver.ShardStateFastpqPublicInputs tamperedShardFastpqInputs =
        new TonSccpProver.ShardStateFastpqPublicInputs(
            "0x" + repeat("00", 16),
            shardRequest.fastpqPublicInputs().slot(),
            shardRequest.fastpqPublicInputs().oldRoot(),
            shardRequest.fastpqPublicInputs().newRoot(),
            shardRequest.fastpqPublicInputs().permRoot(),
            shardRequest.fastpqPublicInputs().txSetHash());
    final TonSccpProver.ShardStateProofRequest tamperedShardDsidRequest =
        new TonSccpProver.ShardStateProofRequest(
            shardRequest.version(),
            shardRequest.proofFamily(),
            shardRequest.circuitId(),
            shardRequest.parameterSet(),
            shardRequest.sourceDomain(),
            shardRequest.masterchainSeqno(),
            shardRequest.shardSeqno(),
            shardRequest.sourceStateVerifierId(),
            shardRequest.sourceStateVerifierHash(),
            shardRequest.shardStateProofPublicInputsHash(),
            shardRequest.statementBytes(),
            shardRequest.witnessCommitmentBytes(),
            shardRequest.verificationContextBytes(),
            shardRequest.schemaDescriptor(),
            shardRequest.publicInputColumns(),
            tamperedShardFastpqInputs,
            shardRequest.fastpqTransitions());
    boolean tamperedShardDsidThrew = false;
    try {
      TonSccpProver.wrapSourceStateVerificationProof(
          new byte[] {9, 8, 7}, tamperedShardDsidRequest);
    } catch (final IllegalArgumentException ex) {
      tamperedShardDsidThrew = ex.getMessage().contains("fastpqPublicInputs.dsid");
    }
    assert tamperedShardDsidThrew
        : "TON source-state wrappers must reject tampered shard-state FastPQ DSIDs";
    final ArrayList<TonSccpProver.FullLightClientAuditFastpqTransition> tamperedAuditTransitions =
        new ArrayList<TonSccpProver.FullLightClientAuditFastpqTransition>(
            requests.masterchainConfig().fastpqTransitions());
    final TonSccpProver.FullLightClientAuditFastpqTransition firstAuditTransition =
        tamperedAuditTransitions.get(0);
    tamperedAuditTransitions.set(
        0,
        new TonSccpProver.FullLightClientAuditFastpqTransition(
            firstAuditTransition.key(),
            firstAuditTransition.operation(),
            firstAuditTransition.oldValue(),
            "0x00"));
    final TonSccpProver.FullLightClientAuditProofRequest tamperedAuditRequest =
        new TonSccpProver.FullLightClientAuditProofRequest(
            requests.masterchainConfig().version(),
            requests.masterchainConfig().proofFamily(),
            requests.masterchainConfig().circuitId(),
            requests.masterchainConfig().parameterSet(),
            requests.masterchainConfig().role(),
            requests.masterchainConfig().roleCode(),
            requests.masterchainConfig().sourceDomain(),
            requests.masterchainConfig().masterchainSeqno(),
            requests.masterchainConfig().shardSeqno(),
            requests.masterchainConfig().verifierId(),
            requests.masterchainConfig().verifierHash(),
            requests.masterchainConfig().sourceStateVerifierId(),
            requests.masterchainConfig().sourceStateVerifierHash(),
            requests.masterchainConfig().sourceVerifierMaterialHash(),
            requests.masterchainConfig().sourceAdapterDeploymentHash(),
            requests.masterchainConfig().fullLightClientGateHash(),
            requests.masterchainConfig().shardStateProofPublicInputsHash(),
            requests.masterchainConfig().shardStateVerificationProofHash(),
            requests.masterchainConfig().auditStatementHash(),
            requests.masterchainConfig().statementBytes(),
            requests.masterchainConfig().verificationContextBytes(),
            requests.masterchainConfig().schemaDescriptor(),
            requests.masterchainConfig().publicInputColumns(),
            requests.masterchainConfig().fastpqPublicInputs(),
            tamperedAuditTransitions);
    boolean tamperedAuditTransitionThrew = false;
    try {
      TonSccpProver.wrapSourceStateVerificationProof(new byte[] {9, 8, 7}, tamperedAuditRequest);
    } catch (final IllegalArgumentException ex) {
      tamperedAuditTransitionThrew =
          ex.getMessage().contains("canonical TON source-state request");
    }
    assert tamperedAuditTransitionThrew
        : "TON source-state wrappers must reject tampered audit FastPQ transitions";
    final TonSccpProver.FullLightClientAuditFastpqPublicInputs tamperedAuditFastpqInputs =
        new TonSccpProver.FullLightClientAuditFastpqPublicInputs(
            requests.masterchainConfig().fastpqPublicInputs().dsid(),
            requests.masterchainConfig().fastpqPublicInputs().slot(),
            requests.masterchainConfig().fastpqPublicInputs().oldRoot(),
            requests.masterchainConfig().fastpqPublicInputs().newRoot(),
            requests.masterchainConfig().fastpqPublicInputs().permRoot(),
            "0x" + repeat("aa", 32));
    final TonSccpProver.FullLightClientAuditProofRequest tamperedAuditTxRequest =
        new TonSccpProver.FullLightClientAuditProofRequest(
            requests.masterchainConfig().version(),
            requests.masterchainConfig().proofFamily(),
            requests.masterchainConfig().circuitId(),
            requests.masterchainConfig().parameterSet(),
            requests.masterchainConfig().role(),
            requests.masterchainConfig().roleCode(),
            requests.masterchainConfig().sourceDomain(),
            requests.masterchainConfig().masterchainSeqno(),
            requests.masterchainConfig().shardSeqno(),
            requests.masterchainConfig().verifierId(),
            requests.masterchainConfig().verifierHash(),
            requests.masterchainConfig().sourceStateVerifierId(),
            requests.masterchainConfig().sourceStateVerifierHash(),
            requests.masterchainConfig().sourceVerifierMaterialHash(),
            requests.masterchainConfig().sourceAdapterDeploymentHash(),
            requests.masterchainConfig().fullLightClientGateHash(),
            requests.masterchainConfig().shardStateProofPublicInputsHash(),
            requests.masterchainConfig().shardStateVerificationProofHash(),
            requests.masterchainConfig().auditStatementHash(),
            requests.masterchainConfig().statementBytes(),
            requests.masterchainConfig().verificationContextBytes(),
            requests.masterchainConfig().schemaDescriptor(),
            requests.masterchainConfig().publicInputColumns(),
            tamperedAuditFastpqInputs,
            requests.masterchainConfig().fastpqTransitions());
    boolean tamperedAuditTxThrew = false;
    try {
      TonSccpProver.wrapSourceStateVerificationProof(
          new byte[] {9, 8, 7}, tamperedAuditTxRequest);
    } catch (final IllegalArgumentException ex) {
      tamperedAuditTxThrew = ex.getMessage().contains("fastpqPublicInputs.txSetHash");
    }
    assert tamperedAuditTxThrew
        : "TON source-state wrappers must reject tampered audit FastPQ tx-set hashes";

    final boolean[] preflightCallbackInvoked = new boolean[] {false};
    final TonSccpProver.SourceStateProver preflightCheckingProver =
        new TonSccpProver.SourceStateProver(
            request -> {
              preflightCallbackInvoked[0] = true;
              return new byte[] {9, 8, 7};
            },
            request -> {
              preflightCallbackInvoked[0] = true;
              return new byte[] {9, 8, 7};
            });
    boolean shardPreflightThrew = false;
    try {
      preflightCheckingProver.proveShardState(tamperedShardRequest);
    } catch (final IllegalArgumentException ex) {
      shardPreflightThrew = ex.getMessage().contains("canonical TON source-state request");
    }
    assert shardPreflightThrew
        : "TON shard-state prover must reject malformed requests before callback";
    assert !preflightCallbackInvoked[0]
        : "TON shard-state prover must not invoke callback for malformed requests";
    boolean auditPreflightThrew = false;
    try {
      preflightCheckingProver.proveFullLightClientAudit(tamperedAuditRequest);
    } catch (final IllegalArgumentException ex) {
      auditPreflightThrew = ex.getMessage().contains("canonical TON source-state request");
    }
    assert auditPreflightThrew
        : "TON audit prover must reject malformed requests before callback";
    assert !preflightCallbackInvoked[0]
        : "TON audit prover must not invoke callback for malformed requests";

    final TonSccpProver.SourceStateProver oversizedCallbackProver =
        new TonSccpProver.SourceStateProver(
            request -> oversizedSourceStateProofBytes,
            null);
    boolean oversizedCallbackProofThrew = false;
    try {
      oversizedCallbackProver.proveShardState(shardRequest);
    } catch (final IllegalArgumentException ex) {
      oversizedCallbackProofThrew = ex.getMessage().contains("proofBytes must be at most");
    }
    assert oversizedCallbackProofThrew
        : "TON source-state prover must reject oversized callback proof bytes";

    final StringBuilder seenRoles = new StringBuilder();
    final TonSccpProver.SourceStateProver sourceStateProver =
        new TonSccpProver.SourceStateProver(
            request -> {
              seenRoles.append("shard_state");
              assert TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(request.circuitId())
                  : "TON shard proof engine must receive the shard-state request";
              return new byte[] {9, 8, 7};
            },
            request -> {
              if (seenRoles.length() > 0) {
                seenRoles.append(",");
              }
              seenRoles.append(request.role());
              return new byte[] {9, 8, 7};
            });
    final TonSccpProver.SourceStateVerificationProof linkedShardProof =
        sourceStateProver.proveShardState(input.shardState());
    final TonSccpProver.FullLightClientAuditProofs linkedProofs =
        sourceStateProver.proveFullLightClientAudit(input);
    assert TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(linkedShardProof.circuitId())
        : "TON source-state prover must wrap shard-state proof bytes";
    assert "CQgH".equals(linkedShardProof.proofBase64())
        : "TON source-state prover must expose shard-state proof base64";
    assert "shard_state,masterchain_config,validator_set_transition,shard_accounts_dictionary"
        .equals(seenRoles.toString())
        : "TON source-state prover must pass canonical role ids to the linked engine";
    assert TonSccpProver.VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1.equals(
            linkedProofs.validatorSetTransition().circuitId())
        : "TON linked validator-set proof must retain role circuit";
    assert TonSccpProver.SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1.equals(
            linkedProofs.shardAccountsDictionary().circuitId())
        : "TON linked shard-accounts proof must retain role circuit";
    assert Arrays.equals(new byte[] {9, 8, 7}, linkedProofs.shardAccountsDictionary().proofBytes())
        : "TON linked role proof must retain proof bytes";
    assert "CQgH".equals(linkedProofs.shardAccountsDictionary().proofBase64())
        : "TON source-state prover must expose audit proof base64";
    final boolean[] sawSnapshots = new boolean[] {false, false};
    final TonSccpProver.SourceStateProver snapshotCheckingProver =
        new TonSccpProver.SourceStateProver(
            request -> {
              sawSnapshots[0] = true;
              assert request != shardRequest
                  : "TON shard-state proof engine must receive a request snapshot";
              assert Arrays.equals(shardRequest.statementBytes(), request.statementBytes())
                  : "TON shard-state snapshot must preserve statement bytes";
              assert Arrays.equals(
                      shardRequest.witnessCommitmentBytes(), request.witnessCommitmentBytes())
                  : "TON shard-state snapshot must preserve witness bytes";
              return new byte[] {9, 8, 7};
            },
            request -> {
              sawSnapshots[1] = true;
              assert request != requests.masterchainConfig()
                  : "TON audit proof engine must receive a request snapshot";
              assert Arrays.equals(
                      requests.masterchainConfig().statementBytes(), request.statementBytes())
                  : "TON audit snapshot must preserve statement bytes";
              return new byte[] {9, 8, 7};
            });
    snapshotCheckingProver.proveShardState(shardRequest);
    snapshotCheckingProver.proveFullLightClientAudit(requests.masterchainConfig());
    assert sawSnapshots[0] : "TON shard-state snapshot regression must run";
    assert sawSnapshots[1] : "TON audit snapshot regression must run";

    final ArrayList<List<String>> mutableColumns =
        new ArrayList<List<String>>(shardRequest.publicInputColumns());
    final ArrayList<String> mutableFirstColumn =
        new ArrayList<String>(mutableColumns.get(0));
    mutableColumns.set(0, mutableFirstColumn);
    final TonSccpProver.ShardStateProofRequest columnSnapshot =
        new TonSccpProver.ShardStateProofRequest(
            shardRequest.version(),
            shardRequest.proofFamily(),
            shardRequest.circuitId(),
            shardRequest.parameterSet(),
            shardRequest.sourceDomain(),
            shardRequest.masterchainSeqno(),
            shardRequest.shardSeqno(),
            shardRequest.sourceStateVerifierId(),
            shardRequest.sourceStateVerifierHash(),
            shardRequest.shardStateProofPublicInputsHash(),
            shardRequest.statementBytes(),
            shardRequest.witnessCommitmentBytes(),
            shardRequest.verificationContextBytes(),
            shardRequest.schemaDescriptor(),
            mutableColumns,
            shardRequest.fastpqPublicInputs(),
            shardRequest.fastpqTransitions());
    final String firstColumnValue = columnSnapshot.publicInputColumns().get(0).get(0);
    mutableFirstColumn.set(0, "0x" + repeat("ff", 32));
    assert firstColumnValue.equals(columnSnapshot.publicInputColumns().get(0).get(0))
        : "TON shard-state public input columns must be deep copied for Android callbacks";
    boolean immutableNestedColumnThrew = false;
    try {
      columnSnapshot.publicInputColumns().get(0).set(0, "0x" + repeat("ee", 32));
    } catch (final UnsupportedOperationException ex) {
      immutableNestedColumnThrew = true;
    }
    assert immutableNestedColumnThrew
        : "TON shard-state public input columns must expose immutable nested lists";

    boolean missingSourceStateProverThrew = false;
    try {
      new TonSccpProver.SourceStateProver().proveFullLightClientAudit(input);
    } catch (final IllegalStateException ex) {
      missingSourceStateProverThrew = ex.getMessage().contains("source-state prover is not linked");
    }
    assert missingSourceStateProverThrew
        : "TON source-state prover must require a linked proof engine";

    boolean duplicateRoleThrew = false;
    try {
      TonSccpProver.buildFullLightClientAuditProofRequests(
          fullLightClientAuditInputWithVerifierHashes(
              input,
              input.tonMasterchainConfigVerifierHash(),
              input.tonMasterchainConfigVerifierHash(),
              input.tonShardAccountsDictionaryVerifierHash()));
    } catch (final IllegalArgumentException ex) {
      duplicateRoleThrew = ex.getMessage().contains("role-separated");
    }
    assert duplicateRoleThrew : "TON audit proof requests must reject duplicate role hashes";

    boolean reusedSourceStateThrew = false;
    try {
      TonSccpProver.buildFullLightClientAuditProofRequests(
          fullLightClientAuditInputWithVerifierHashes(
              input,
              "0x" + repeat("d4", 32),
              input.tonValidatorSetTransitionVerifierHash(),
              input.tonShardAccountsDictionaryVerifierHash()));
    } catch (final IllegalArgumentException ex) {
      reusedSourceStateThrew = ex.getMessage().contains("source-adapter material");
    }
    assert reusedSourceStateThrew
        : "TON audit proof requests must reject source-state hash reuse";

    boolean reusedRequestHashThrew = false;
    try {
      TonSccpProver.buildFullLightClientAuditProofRequests(
          fullLightClientAuditInputWithVerifierHashes(
              input,
              TonSccpProver.fullLightClientAuditStatementHash(
                  input, TonSccpProver.FullLightClientAuditRole.MASTERCHAIN_CONFIG),
              input.tonValidatorSetTransitionVerifierHash(),
              input.tonShardAccountsDictionaryVerifierHash()));
    } catch (final IllegalArgumentException ex) {
      reusedRequestHashThrew = ex.getMessage().contains("request-bound hashes");
    }
    assert reusedRequestHashThrew
        : "TON audit proof requests must reject request-bound hash reuse";

    boolean reusedTemplateMaterialThrew = false;
    try {
      TonSccpProver.buildFullLightClientAuditProofRequests(
          fullLightClientAuditInputWithVerifierHashes(
              input,
              "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
              input.tonValidatorSetTransitionVerifierHash(),
              input.tonShardAccountsDictionaryVerifierHash()));
    } catch (final IllegalArgumentException ex) {
      reusedTemplateMaterialThrew = ex.getMessage().contains("template material");
    }
    assert reusedTemplateMaterialThrew
        : "TON audit proof requests must reject template source-material hash reuse";

    boolean staleProofHashThrew = false;
    try {
      TonSccpProver.buildFullLightClientAuditProofRequests(
          fullLightClientAuditInputWithShardStateVerificationProofHash(
              input, "0x" + repeat("aa", 32)));
    } catch (final IllegalArgumentException ex) {
      staleProofHashThrew = ex.getMessage().contains("shardStateVerificationProofHash");
    }
    assert staleProofHashThrew : "TON audit proof requests must reject stale source proof hashes";

    boolean mismatchedConfigProofThrew = false;
    try {
      TonSccpProver.buildFullLightClientAuditProofRequests(
          sampleFullLightClientAuditProofInput("0x" + repeat("aa", 32)));
    } catch (final IllegalArgumentException ex) {
      mismatchedConfigProofThrew = ex.getMessage().contains("masterchainConfigProofHash");
    }
    assert mismatchedConfigProofThrew
        : "TON audit proof requests must reject mismatched config proof material";
  }

  private static void derivesTonValidatorSetTransitionHashesFromWitnessMaterial() {
    final List<byte[]> validatorPublicKeys =
        Arrays.asList(repeatedByte((byte) 0x11, 32), repeatedByte((byte) 0x22, 32));
    final List<String> validatorWeights = Arrays.asList("1", "2");
    final String validatorSetHash =
        "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938";
    final byte[] nextValidatorSetPayload =
        TonSccpProver.canonicalValidatorSetPayloadBytes(
            Arrays.asList(repeatedByte((byte) 0x33, 32), repeatedByte((byte) 0x44, 32)),
            Arrays.asList("3", "4"));
    final String nextValidatorSetHash =
        "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f";
    final String nextValidatorSetPayloadHash =
        "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983";
    final String transitionMessageHash =
        "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19";
    final String transitionSignatureHash =
        "0xd784461f68495981c2c00e60316dc9353ea4b5be3bc261b26feadc7c83c4f6a7";
    final TonSccpProver.ValidatorSignatureProofInput signatureProof =
        new TonSccpProver.ValidatorSignatureProofInput(
            "3",
            "3",
            transitionMessageHash,
            validatorPublicKeys,
            validatorWeights,
            new byte[] {0x03},
            Arrays.asList(repeatedByte((byte) 0xab, 64), repeatedByte((byte) 0xcd, 64)));

    assert TonSccpProver.canonicalValidatorSetBytes(validatorPublicKeys, validatorWeights).length
            == 85
        : "TON validator-set transcript length must match Rust";
    assert validatorSetHash.equals(TonSccpProver.validatorSetHash(validatorPublicKeys, validatorWeights))
        : "TON validator-set hash must match Rust";
    assert nextValidatorSetPayloadHash.equals(
            TonSccpProver.validatorSetPayloadHash(nextValidatorSetPayload))
        : "TON next validator-set payload hash must match Rust";
    assert nextValidatorSetHash.equals(
            TonSccpProver.validatorSetHashFromPayload(nextValidatorSetPayload))
        : "TON next validator-set hash must match Rust";
    assert TonSccpProver.canonicalValidatorSetTransitionMessageBytes(
                TonSccpProver.DOMAIN_TON,
                "7",
                "8",
                "19",
                -1,
                "9223372036854775808",
                repeat("aa", 32),
                repeat("a5", 32),
                validatorSetHash,
                nextValidatorSetHash,
                nextValidatorSetPayloadHash,
                repeat("cc", 32))
            .length
        == 233 : "TON transition message length must match Rust";
    assert transitionMessageHash.equals(
            TonSccpProver.validatorSetTransitionMessageHash(
                TonSccpProver.DOMAIN_TON,
                "7",
                "8",
                "19",
                -1,
                "9223372036854775808",
                repeat("aa", 32),
                repeat("a5", 32),
                validatorSetHash,
                nextValidatorSetHash,
                nextValidatorSetPayloadHash,
                repeat("cc", 32)))
        : "TON transition message hash must match Rust";
    assert TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
                1,
                TonSccpProver.DOMAIN_TON,
                "7",
                "8",
                "19",
                -1,
                "9223372036854775808",
                repeat("aa", 32),
                repeat("a5", 32),
                validatorSetHash,
                nextValidatorSetHash,
                nextValidatorSetPayload,
                nextValidatorSetPayloadHash,
                repeat("cc", 32),
                transitionMessageHash,
                signatureProof)
            .length
        == 676 : "TON transition signature length must match Rust";
    assert transitionSignatureHash.equals(
            TonSccpProver.validatorSetTransitionSignatureHash(
                1,
                TonSccpProver.DOMAIN_TON,
                "7",
                "8",
                "19",
                -1,
                "9223372036854775808",
                repeat("aa", 32),
                repeat("a5", 32),
                validatorSetHash,
                nextValidatorSetHash,
                nextValidatorSetPayload,
                nextValidatorSetPayloadHash,
                repeat("cc", 32),
                transitionMessageHash,
                signatureProof))
        : "TON transition signature hash must match Rust";

    boolean wrongParentHashThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
          1,
          TonSccpProver.DOMAIN_TON,
          "7",
          "8",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          repeat("dd", 32),
          nextValidatorSetHash,
          nextValidatorSetPayload,
          nextValidatorSetPayloadHash,
          repeat("cc", 32),
          transitionMessageHash,
          signatureProof);
    } catch (final IllegalArgumentException ex) {
      wrongParentHashThrew = ex.getMessage().contains("parentValidatorSetHash");
    }
    assert wrongParentHashThrew : "TON parent validator-set hash must match signature proof";

    boolean wrongTransitionMessageHashThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
          1,
          TonSccpProver.DOMAIN_TON,
          "7",
          "8",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          nextValidatorSetHash,
          nextValidatorSetPayload,
          nextValidatorSetPayloadHash,
          repeat("cc", 32),
          repeat("dd", 32),
          signatureProof);
    } catch (final IllegalArgumentException ex) {
      wrongTransitionMessageHashThrew = ex.getMessage().contains("transitionMessageHash");
    }
    assert wrongTransitionMessageHashThrew : "TON transition message hash must match fields";

    boolean skippedTransitionSeqnoThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionMessageBytes(
          TonSccpProver.DOMAIN_TON,
          "7",
          "9",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          nextValidatorSetHash,
          nextValidatorSetPayloadHash,
          repeat("cc", 32));
    } catch (final IllegalArgumentException ex) {
      skippedTransitionSeqnoThrew = ex.getMessage().contains("toValidatorSetSeqno");
    }
    assert skippedTransitionSeqnoThrew : "TON validator-set transition seqnos must be adjacent";

    boolean wrongSignedBlockMessageHashThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
          1,
          TonSccpProver.DOMAIN_TON,
          "7",
          "8",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          nextValidatorSetHash,
          nextValidatorSetPayload,
          nextValidatorSetPayloadHash,
          repeat("cc", 32),
          transitionMessageHash,
          new TonSccpProver.ValidatorSignatureProofInput(
              "3",
              "3",
              repeat("dd", 32),
              validatorPublicKeys,
              validatorWeights,
              new byte[] {0x03},
              Arrays.asList(repeatedByte((byte) 0xab, 64), repeatedByte((byte) 0xcd, 64))));
    } catch (final IllegalArgumentException ex) {
      wrongSignedBlockMessageHashThrew = ex.getMessage().contains("blockMessageHash");
    }
    assert wrongSignedBlockMessageHashThrew : "TON transition signatures must sign the transition message";

    boolean zeroWeightThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetBytes(validatorPublicKeys, Arrays.asList("1", "0"));
    } catch (final IllegalArgumentException ex) {
      zeroWeightThrew = ex.getMessage().contains("must not be zero");
    }
    assert zeroWeightThrew : "zero TON validator weight must be rejected";

    boolean zeroKeyThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetBytes(
          Arrays.asList(new byte[32], validatorPublicKeys.get(1)), validatorWeights);
    } catch (final IllegalArgumentException ex) {
      zeroKeyThrew = ex.getMessage().contains("must not be zero");
    }
    assert zeroKeyThrew : "zero TON validator public key must be rejected";

    final byte[] zeroKeyValidatorSetPayload =
        TonSccpProver.canonicalValidatorSetPayloadBytes(validatorPublicKeys, validatorWeights);
    Arrays.fill(zeroKeyValidatorSetPayload, 5, 37, (byte) 0);
    boolean zeroKeyPayloadThrew = false;
    try {
      TonSccpProver.validatorSetHashFromPayload(zeroKeyValidatorSetPayload);
    } catch (final IllegalArgumentException ex) {
      zeroKeyPayloadThrew = ex.getMessage().contains("must not be zero");
    }
    assert zeroKeyPayloadThrew : "zero TON validator public key payload must be rejected";

    final List<byte[]> oversizedValidatorPublicKeys = new ArrayList<byte[]>();
    final List<String> oversizedValidatorWeights = new ArrayList<String>();
    for (int index = 0; index < 1025; index++) {
      final byte[] publicKey = new byte[32];
      publicKey[0] = (byte) 0x80;
      publicKey[28] = (byte) (index & 0xff);
      publicKey[29] = (byte) ((index >>> 8) & 0xff);
      publicKey[30] = (byte) ((index >>> 16) & 0xff);
      publicKey[31] = (byte) ((index >>> 24) & 0xff);
      oversizedValidatorPublicKeys.add(publicKey);
      oversizedValidatorWeights.add("1");
    }
    boolean oversizedValidatorSetThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetBytes(
          oversizedValidatorPublicKeys, oversizedValidatorWeights);
    } catch (final IllegalArgumentException ex) {
      oversizedValidatorSetThrew = ex.getMessage().contains("same-length arrays");
    }
    assert oversizedValidatorSetThrew : "oversized TON validator set must be rejected";

    final byte[] oversizedValidatorSetPayload = new byte[5 + 1025 * 40];
    oversizedValidatorSetPayload[0] = 1;
    oversizedValidatorSetPayload[1] = 1;
    oversizedValidatorSetPayload[2] = 4;
    for (int index = 0; index < 1025; index++) {
      final int offset = 5 + index * 40;
      oversizedValidatorSetPayload[offset] = (byte) 0x80;
      oversizedValidatorSetPayload[offset + 28] = (byte) (index & 0xff);
      oversizedValidatorSetPayload[offset + 29] = (byte) ((index >>> 8) & 0xff);
      oversizedValidatorSetPayload[offset + 30] = (byte) ((index >>> 16) & 0xff);
      oversizedValidatorSetPayload[offset + 31] = (byte) ((index >>> 24) & 0xff);
      oversizedValidatorSetPayload[offset + 32] = 1;
    }
    boolean oversizedValidatorSetPayloadThrew = false;
    try {
      TonSccpProver.validatorSetHashFromPayload(oversizedValidatorSetPayload);
    } catch (final IllegalArgumentException ex) {
      oversizedValidatorSetPayloadThrew = ex.getMessage().contains("validator count");
    }
    assert oversizedValidatorSetPayloadThrew : "oversized TON validator-set payload must be rejected";

    boolean badSignatureCountThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
          1,
          TonSccpProver.DOMAIN_TON,
          "7",
          "8",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          nextValidatorSetHash,
          nextValidatorSetPayload,
          nextValidatorSetPayloadHash,
          repeat("cc", 32),
          transitionMessageHash,
          new TonSccpProver.ValidatorSignatureProofInput(
              "3",
              "3",
              transitionMessageHash,
              validatorPublicKeys,
              validatorWeights,
              new byte[] {0x03},
              Collections.singletonList(new byte[64])));
    } catch (final IllegalArgumentException ex) {
      badSignatureCountThrew = ex.getMessage().contains("signatures length");
    }
    assert badSignatureCountThrew : "TON signature count must match signer bitmap";

    boolean insufficientSignedWeightThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
          1,
          TonSccpProver.DOMAIN_TON,
          "7",
          "8",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          nextValidatorSetHash,
          nextValidatorSetPayload,
          nextValidatorSetPayloadHash,
          repeat("cc", 32),
          transitionMessageHash,
          new TonSccpProver.ValidatorSignatureProofInput(
              "3",
              "1",
              transitionMessageHash,
              validatorPublicKeys,
              validatorWeights,
              new byte[] {0x01},
              Collections.singletonList(new byte[64])));
    } catch (final IllegalArgumentException ex) {
      insufficientSignedWeightThrew = ex.getMessage().contains("two thirds");
    }
    assert insufficientSignedWeightThrew : "TON signed weight must exceed two thirds";

    boolean badSignatureThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
          1,
          TonSccpProver.DOMAIN_TON,
          "7",
          "8",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          nextValidatorSetHash,
          nextValidatorSetPayload,
          nextValidatorSetPayloadHash,
          repeat("cc", 32),
          transitionMessageHash,
          new TonSccpProver.ValidatorSignatureProofInput(
              "3",
              "3",
              transitionMessageHash,
              validatorPublicKeys,
              validatorWeights,
              new byte[] {0x03},
              Arrays.asList(new byte[63], new byte[64])));
    } catch (final IllegalArgumentException ex) {
      badSignatureThrew = ex.getMessage().contains("64 bytes");
    }
    assert badSignatureThrew : "malformed TON transition signature must be rejected";

    boolean zeroSignatureThrew = false;
    try {
      TonSccpProver.canonicalValidatorSetTransitionSignatureBytes(
          1,
          TonSccpProver.DOMAIN_TON,
          "7",
          "8",
          "19",
          -1,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          nextValidatorSetHash,
          nextValidatorSetPayload,
          nextValidatorSetPayloadHash,
          repeat("cc", 32),
          transitionMessageHash,
          new TonSccpProver.ValidatorSignatureProofInput(
              "3",
              "3",
              transitionMessageHash,
              validatorPublicKeys,
              validatorWeights,
              new byte[] {0x03},
              Arrays.asList(new byte[64], repeatedByte((byte) 0x01, 64))));
    } catch (final IllegalArgumentException ex) {
      zeroSignatureThrew = ex.getMessage().contains("all zero");
    }
    assert zeroSignatureThrew : "all-zero TON transition signatures must be rejected";
  }

  private static void derivesTonMasterchainConfigProofHashesFromWitnessMaterial() {
    final List<byte[]> validatorPublicKeys =
        Arrays.asList(repeatedByte((byte) 0x11, 32), repeatedByte((byte) 0x22, 32));
    final List<String> validatorWeights = Arrays.asList("1", "2");
    final String validatorSetHash =
        "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938";
    final String validatorSetPayloadHash =
        "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0";
    final String configLeafHash =
        "0xed92ba8082850092da7cc296a2184cc4576877aaee08c72748d96ea449b16e39";
    final String configRoot =
        "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af";
    final String configValueHash =
        "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50";
    final byte[] configDictionaryProofBoc =
        hexBytes(
            "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0");
    final String configProofHash =
        "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c";
    final byte[] validatorSetPayload =
        TonSccpProver.canonicalValidatorSetPayloadBytes(validatorPublicKeys, validatorWeights);

    assert validatorSetPayloadHash.equals(TonSccpProver.validatorSetPayloadHash(validatorSetPayload))
        : "TON validator-set payload hash must match Rust";
    assert Arrays.equals(
            validatorSetPayload,
            TonSccpProver.configValidatorSetPayloadFromProofBoc(configDictionaryProofBoc))
        : "TON config dictionary proof must decode the active ValidatorSet payload";
    assert validatorSetPayloadHash.equals(
            TonSccpProver.configValidatorSetPayloadHashFromProofBoc(configDictionaryProofBoc))
        : "TON config dictionary proof must hash the decoded active ValidatorSet payload";
    assert TonSccpProver.canonicalMasterchainConfigLeafBytes(
                TonSccpProver.DOMAIN_TON,
                "19",
                repeat("aa", 32),
                repeat("cc", 32),
                validatorSetHash,
                validatorSetPayloadHash)
            .length
        == 141 : "TON masterchain config leaf transcript length must match Rust";
    assert configLeafHash.equals(
            TonSccpProver.masterchainConfigLeafHash(
                TonSccpProver.DOMAIN_TON,
                "19",
                repeat("aa", 32),
                repeat("cc", 32),
                validatorSetHash,
                validatorSetPayloadHash))
        : "TON masterchain config leaf hash must match Rust";
    boolean badConfigLeafVersionThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigLeafBytes(
          0,
          TonSccpProver.DOMAIN_TON,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          validatorSetHash,
          validatorSetPayloadHash);
    } catch (final IllegalArgumentException ex) {
      badConfigLeafVersionThrew = ex.getMessage().contains("version");
    }
    assert badConfigLeafVersionThrew : "TON masterchain config leaf version must be v1";
    assert configRoot.equals(TonSccpProver.hashmapEProofRootHash(configDictionaryProofBoc))
        : "TON config dictionary proof root must match Rust";
    assert configValueHash.equals(
            TonSccpProver.hashmapECellRefValueHash(
                configDictionaryProofBoc,
                new byte[] {0, 0, 0, (byte) TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM},
                TonSccpProver.CONFIG_PARAM_KEY_BITS))
        : "TON config dictionary proof must open config param 34";
    assert TonSccpProver.canonicalMasterchainConfigProofBytes(
                TonSccpProver.DOMAIN_TON,
                "19",
                repeat("aa", 32),
                repeat("cc", 32),
                configRoot,
                validatorSetHash,
                validatorSetPayloadHash,
                configLeafHash,
                Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
                configValueHash,
                configDictionaryProofBoc,
                Collections.emptyList())
            .length
        == 411 : "TON masterchain config proof transcript length must match Rust";
    assert configProofHash.equals(
            TonSccpProver.masterchainConfigProofHash(
                TonSccpProver.DOMAIN_TON,
                "19",
                repeat("aa", 32),
                repeat("cc", 32),
                configRoot,
                validatorSetHash,
                validatorSetPayloadHash,
                configLeafHash,
                Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
                configValueHash,
                configDictionaryProofBoc,
                Collections.emptyList()))
        : "TON masterchain config proof hash must match Rust";

    boolean badConfigParamThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigProofBytes(
          TonSccpProver.DOMAIN_TON,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          configRoot,
          validatorSetHash,
          validatorSetPayloadHash,
          configLeafHash,
          "0",
          configValueHash,
          configDictionaryProofBoc,
          Collections.emptyList());
    } catch (final IllegalArgumentException ex) {
      badConfigParamThrew = ex.getMessage().contains("config param 34");
    }
    assert badConfigParamThrew : "TON masterchain config proof must use config param 34";

    boolean badPayloadHashThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigProofBytes(
          TonSccpProver.DOMAIN_TON,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          configRoot,
          validatorSetHash,
          repeat("ee", 32),
          configLeafHash,
          Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
          configValueHash,
          configDictionaryProofBoc,
          Collections.emptyList());
    } catch (final IllegalArgumentException ex) {
      badPayloadHashThrew = ex.getMessage().contains("ValidatorSet");
    }
    assert badPayloadHashThrew : "TON config proof must bind decoded ValidatorSet payload hash";

    boolean badConfigLeafHashThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigProofBytes(
          TonSccpProver.DOMAIN_TON,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          configRoot,
          validatorSetHash,
          validatorSetPayloadHash,
          repeat("ee", 32),
          Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
          configValueHash,
          configDictionaryProofBoc,
          Collections.emptyList());
    } catch (final IllegalArgumentException ex) {
      badConfigLeafHashThrew = ex.getMessage().contains("configLeafHash");
    }
    assert badConfigLeafHashThrew : "TON config proof must bind its config leaf hash";

    final String wrongValidatorSetHash = repeat("ee", 32);
    final String wrongValidatorSetLeafHash =
        TonSccpProver.masterchainConfigLeafHash(
            TonSccpProver.DOMAIN_TON,
            "19",
            repeat("aa", 32),
            repeat("cc", 32),
            wrongValidatorSetHash,
            validatorSetPayloadHash);
    boolean badValidatorSetHashThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigProofBytes(
          TonSccpProver.DOMAIN_TON,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          configRoot,
          wrongValidatorSetHash,
          validatorSetPayloadHash,
          wrongValidatorSetLeafHash,
          Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
          configValueHash,
          configDictionaryProofBoc,
          Collections.emptyList());
    } catch (final IllegalArgumentException ex) {
      badValidatorSetHashThrew = ex.getMessage().contains("validatorSetHash");
    }
    assert badValidatorSetHashThrew : "TON config proof must bind the decoded validator-set hash";

    boolean badSourceDomainThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigProofBytes(
          3,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          configRoot,
          validatorSetHash,
          validatorSetPayloadHash,
          configLeafHash,
          Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
          configValueHash,
          configDictionaryProofBoc,
          Collections.emptyList());
    } catch (final IllegalArgumentException ex) {
      badSourceDomainThrew = ex.getMessage().contains("sourceDomain");
    }
    assert badSourceDomainThrew : "TON config proof helpers must stay domain-specific";

    boolean badBranchThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigProofBytes(
          TonSccpProver.DOMAIN_TON,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          configRoot,
          validatorSetHash,
          validatorSetPayloadHash,
          configLeafHash,
          Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
          configValueHash,
          configDictionaryProofBoc,
          Collections.singletonList(new byte[] {1, 2, 3}));
    } catch (final IllegalArgumentException ex) {
      badBranchThrew = ex.getMessage().contains("configInclusionBranch")
          || ex.getMessage().contains("inclusionBranch[0]");
    }
    assert badBranchThrew : "malformed TON masterchain config branch must be rejected";

    boolean oversizedBranchThrew = false;
    try {
      TonSccpProver.canonicalMasterchainConfigProofBytes(
          TonSccpProver.DOMAIN_TON,
          "19",
          repeat("aa", 32),
          repeat("cc", 32),
          configRoot,
          validatorSetHash,
          validatorSetPayloadHash,
          configLeafHash,
          Long.toString(TonSccpProver.CURRENT_VALIDATOR_SET_CONFIG_PARAM),
          configValueHash,
          configDictionaryProofBoc,
          Collections.nCopies(65, repeatedByte((byte) 0xee, 32)));
    } catch (final IllegalArgumentException ex) {
      oversizedBranchThrew = ex.getMessage().contains("configInclusionBranch")
          || ex.getMessage().contains("at most 64");
    }
    assert oversizedBranchThrew : "oversized TON masterchain config branch must be rejected";
  }

  private static void derivesTonMasterchainBlockMessageAndSignatureHashesFromWitnessMaterial() {
    final List<byte[]> validatorPublicKeys =
        Arrays.asList(repeatedByte((byte) 0x11, 32), repeatedByte((byte) 0x22, 32));
    final List<String> validatorWeights = Arrays.asList("1", "2");
    final String validatorSetHash =
        TonSccpProver.validatorSetHash(validatorPublicKeys, validatorWeights);
    final String configRoot =
        "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af";
    final String configProofHash =
        "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c";
    final String blockMessageHash =
        TonSccpProver.masterchainBlockMessageHash(
            TonSccpProver.DOMAIN_TON,
            "19",
            -1,
            "9223372036854775808",
            repeat("aa", 32),
            repeat("a5", 32),
            validatorSetHash,
            configRoot,
            configProofHash,
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            repeat("cc", 32),
            repeat("dd", 32),
            repeat("ee", 32));
    final TonSccpProver.ValidatorSignatureProofInput signatureProof =
        new TonSccpProver.ValidatorSignatureProofInput(
            "3",
            "3",
            blockMessageHash,
            validatorPublicKeys,
            validatorWeights,
            new byte[] {0x03},
            Arrays.asList(repeatedByte((byte) 0xab, 64), repeatedByte((byte) 0xcd, 64)));

    assert TonSccpProver.canonicalMasterchainBlockMessageBytes(
                TonSccpProver.DOMAIN_TON,
                "19",
                -1,
                "9223372036854775808",
                repeat("aa", 32),
                repeat("a5", 32),
                validatorSetHash,
                configRoot,
                configProofHash,
                0,
                "9223372036854775808",
                "7",
                repeat("bb", 32),
                repeat("bc", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("ee", 32))
            .length
        == 365 : "TON masterchain block-message transcript length must match Rust";
    assert "0x0ca07d5072adb7db3d6a0f831294c7e119c451884aaa1afcbb23e0df0911d8bd"
            .equals(blockMessageHash)
        : "TON masterchain block-message hash must match Rust";
    assert TonSccpProver.canonicalMasterchainValidatorSignaturesBytes(
                signatureProof,
                validatorSetHash)
            .length
        == 322 : "TON masterchain validator-signature transcript length must match Rust";
    assert "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15"
        .equals(TonSccpProver.masterchainValidatorSignaturesHash(signatureProof, validatorSetHash))
        : "TON masterchain validator-signature hash must match Rust";

    boolean zeroSignatureThrew = false;
    try {
      TonSccpProver.canonicalMasterchainValidatorSignaturesBytes(
          new TonSccpProver.ValidatorSignatureProofInput(
              "3",
              "3",
              blockMessageHash,
              validatorPublicKeys,
              validatorWeights,
              new byte[] {0x03},
              Arrays.asList(new byte[64], repeatedByte((byte) 0x01, 64))),
          validatorSetHash);
    } catch (final IllegalArgumentException ex) {
      zeroSignatureThrew = ex.getMessage().contains("all zero");
    }
    assert zeroSignatureThrew : "all-zero TON masterchain signatures must be rejected";

    boolean badWorkchainThrew = false;
    try {
      TonSccpProver.canonicalMasterchainBlockMessageBytes(
          TonSccpProver.DOMAIN_TON,
          "19",
          0,
          "9223372036854775808",
          repeat("aa", 32),
          repeat("a5", 32),
          validatorSetHash,
          configRoot,
          configProofHash,
          0,
          "9223372036854775808",
          "7",
          repeat("bb", 32),
          repeat("bc", 32),
          repeat("cc", 32),
          repeat("dd", 32),
          repeat("ee", 32));
    } catch (final IllegalArgumentException ex) {
      badWorkchainThrew = ex.getMessage().contains("masterchainWorkchainId");
    }
    assert badWorkchainThrew : "TON masterchain workchain id must be checked";
  }

  private static void proverRequiresLinkedProofEngine() {
    boolean threw = false;
    try {
      new TonSccpProver().prove(sampleProofRequestInput());
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("not linked");
    }
    assert threw : "expected missing local prover to throw";
  }

  private static void callbackRequestSnapshotCopiesTonProofRequestBytes() {
    final TonSccpProver.ProofRequest request =
        TonSccpProver.buildProofRequest(sampleProofRequestInput(new byte[] {9, 10}));
    final TonSccpProver.ProofRequest snapshot = TonSccpProver.callbackRequestSnapshot(request);

    assert snapshot != request : "TON proof callback must receive a request snapshot";
    assert request.version() == snapshot.version() : "snapshot must preserve request version";
    assert request.backend().equals(snapshot.backend()) : "snapshot must preserve backend";
    assert request.sourceDomain() == snapshot.sourceDomain() : "snapshot must preserve source";
    assert request.targetDomain() == snapshot.targetDomain() : "snapshot must preserve target";
    assert request.publicInputs().equals(snapshot.publicInputs())
        : "snapshot must preserve public inputs";
    assert Arrays.equals(request.publicInputsBytes(), snapshot.publicInputsBytes())
        : "snapshot must preserve public input bytes";
    assert Arrays.equals(request.bundleBytes(), snapshot.bundleBytes())
        : "snapshot must preserve bundle bytes";
    assert Arrays.equals(request.sourceProofBytes(), snapshot.sourceProofBytes())
        : "snapshot must preserve source proof bytes";
    assert request.proofContext().equals(snapshot.proofContext())
        : "snapshot must preserve proof context";
    assert request.statementHash().equals(snapshot.statementHash())
        : "snapshot must preserve statement hash";
    assert request.destinationBindingHash().equals(snapshot.destinationBindingHash())
        : "snapshot must preserve destination binding hash";
    assert request.sourceStateVerifierId().equals(snapshot.sourceStateVerifierId())
        : "snapshot must preserve source-state verifier id";
    assert request.sourceStateVerifierHash().equals(snapshot.sourceStateVerifierHash())
        : "snapshot must preserve source-state verifier hash";
    assert request
            .sourceAdapterDeploymentBindingHash()
            .equals(snapshot.sourceAdapterDeploymentBindingHash())
        : "snapshot must preserve source adapter binding hash";
    assert request
            .sourceAdapterDeploymentBinding()
            .equals(snapshot.sourceAdapterDeploymentBinding())
        : "snapshot must preserve source adapter binding";
    assert request.requestHash().equals(snapshot.requestHash()) : "snapshot must preserve hash";

    final byte[] exposedPublicInputs = snapshot.publicInputsBytes();
    final byte[] exposedBundle = snapshot.bundleBytes();
    final byte[] exposedSourceProof = snapshot.sourceProofBytes();
    exposedPublicInputs[0] = (byte) (exposedPublicInputs[0] ^ 0x01);
    exposedBundle[0] = (byte) (exposedBundle[0] ^ 0x01);
    exposedSourceProof[0] = (byte) (exposedSourceProof[0] ^ 0x01);

    assert Arrays.equals(request.publicInputsBytes(), snapshot.publicInputsBytes())
        : "callback public input bytes must be immutable to caller mutation";
    assert Arrays.equals(request.bundleBytes(), snapshot.bundleBytes())
        : "callback bundle bytes must be immutable to caller mutation";
    assert Arrays.equals(request.sourceProofBytes(), snapshot.sourceProofBytes())
        : "callback source proof bytes must be immutable to caller mutation";
  }

  private static void proverRejectsNonProductionInputBeforeLinkedProofEngine() {
    final boolean[] invoked = new boolean[] {false};
    final TonSccpProver prover =
        new TonSccpProver(
            null,
            request -> {
              invoked[0] = true;
              return new byte[] {1, 2, 3, 4};
            });

    boolean threw = false;
    try {
      prover.prove(
          sampleProofRequestInput(
              new byte[] {9, 10},
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              SolanaSccpProver.ZERO_HASH_V1,
              repeat("aa", 32),
              repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceStateVerifierHash");
    }
    assert threw : "TON prover must reject non-production input";
    assert !invoked[0] : "TON proof engine must not see rejected inputs";
  }

  private static void proverWrapsExternalProofBytes() {
    final TonSccpProver.ProofRequest[] seenRequests = new TonSccpProver.ProofRequest[2];
    final int[] seenRequestCount = new int[] {0};
    final TonSccpProver prover =
        new TonSccpProver(
            null,
            request -> {
              seenRequests[seenRequestCount[0]++] = request;
              assert TonSccpProver.CONTRACT_PROOF_BACKEND_V1.equals(request.backend())
                  : "backend must be TON";
              assert ("0x" + repeat("56", 32)).equals(request.statementHash())
                  : "statement hash must be normalized";
              assert ("0x" + repeat("78", 32)).equals(request.destinationBindingHash())
                  : "destination binding hash must be normalized";
              return new byte[] {1, 2, 3, 4};
            });

    final TonSccpProver.ProofResult result =
        prover.prove(sampleProofRequestInput(new byte[] {9, 10}));
    final TonSccpProver.ProofResult omittedSourceResult = prover.prove(sampleProofRequestInput());
    assert Arrays.equals(new byte[] {1, 2, 3, 4}, result.proofBytes())
        : "proof bytes must be preserved";
    assert Arrays.equals(new byte[0], omittedSourceResult.sourceProofBytes())
        : "TON production proofs may omit source proof bytes";
    assert "AQIDBA==".equals(result.proofBase64()) : "proof base64 must be exposed";
    assert ("0x" + repeat("56", 32)).equals(result.statementHash())
        : "result must expose statement hash";
    assert ("0x" + repeat("78", 32)).equals(result.destinationBindingHash())
        : "result must expose destination binding hash";
    assert TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(
            result.sourceStateVerifierId())
        : "result must expose TON source-state verifier id";
    assert ("0x" + repeat("cc", 32)).equals(result.sourceStateVerifierHash())
        : "result must expose TON source-state verifier hash";
    assert result.requestHash().matches("0x[0-9a-f]{64}") : "request hash must be hex";
    assert result.envelopeHash().matches("0x[0-9a-f]{64}") : "envelope hash must be hex";

    final TonSccpProver.ProofRequest request =
        TonSccpProver.buildProofRequest(sampleProofRequestInput(new byte[] {9, 10}));
    final TonSccpProver.ProofRequest omittedSourceRequest =
        TonSccpProver.buildProofRequest(sampleProofRequestInput());
    assert seenRequestCount[0] == 2 : "proof engine must receive both TON callback requests";
    assert seenRequests[0] != request : "TON proof engine must receive a request snapshot";
    assert seenRequests[0].requestHash().equals(request.requestHash())
        : "TON callback snapshot must match the canonical request hash";
    assert Arrays.equals(seenRequests[0].publicInputsBytes(), request.publicInputsBytes())
        : "TON callback snapshot must copy public inputs";
    assert Arrays.equals(seenRequests[0].bundleBytes(), request.bundleBytes())
        : "TON callback snapshot must copy bundle bytes";
    assert Arrays.equals(seenRequests[0].sourceProofBytes(), request.sourceProofBytes())
        : "TON callback snapshot must copy source proof bytes";
    assert seenRequests[1] != omittedSourceRequest
        : "TON proof engine must receive an omitted-source request snapshot";
    assert seenRequests[1].requestHash().equals(omittedSourceRequest.requestHash())
        : "TON omitted-source callback snapshot must match canonical request";

    final TonSccpProver.MessageBodyInput submissionInput =
        new TonSccpProver.MessageBodyInput(
            result, sampleTonBundleBytes(), new byte[] {8, 9}, "7");
    assert result.publicInputs().equals(submissionInput.publicInputs())
        : "proof-result submission input must carry public inputs";
    assert Arrays.equals(result.proofBytes(), submissionInput.proofBytes())
        : "proof-result submission input must carry proof bytes";
    assert Arrays.equals(sampleTonBundleBytes(), result.bundleBytes())
        : "proof result must carry request bundle bytes";
    assert Arrays.equals(new byte[] {9, 10}, result.sourceProofBytes())
        : "proof result must carry request source proof bytes";
    assert result.proofContext().statementHash().equals(submissionInput.statementHash())
        : "proof-result submission input must carry statement hash";
    assert result.proofContext().destinationBindingHash().equals(submissionInput.destinationBindingHash())
        : "proof-result submission input must carry destination binding hash";
    final TonSccpProver.Submission submission = TonSccpProver.buildSubmission(submissionInput);
    assert "internal_message".equals(submission.submissionKind())
        : "proof-result TON submission must carry submission kind";
    assert "op::submit_sccp_message_proof".equals(submission.verifierEntrypoint())
        : "proof-result TON submission must carry entrypoint";
    final TonSccpProver.ProofResult omittedSourceProofResult =
        TonSccpProver.wrapProofResult(
            result.proofBytes(), TonSccpProver.buildProofRequest(sampleProofRequestInput()));
    final TonSccpProver.Submission omittedSourceSubmission =
        TonSccpProver.buildSubmission(
            new TonSccpProver.MessageBodyInput(
                omittedSourceProofResult, sampleTonBundleBytes()));
    assert Arrays.equals(new byte[0], omittedSourceProofResult.sourceProofBytes())
        : "TON submit-ready proof results may omit source proof bytes";
    assert omittedSourceSubmission.messageBodyBoc().length > 0
        : "TON omitted-source submission must emit a BOC body";
    boolean mismatchedBundleThrew = false;
    try {
      new TonSccpProver.MessageBodyInput(result, sampleTonBundleBytes(new byte[] {0x71, 0x73}));
    } catch (final IllegalArgumentException ex) {
      mismatchedBundleThrew = ex.getMessage().contains("proofResult.bundleBytes");
    }
    assert mismatchedBundleThrew
        : "proof-result TON submission input must reject mismatched bundle bytes";

    boolean tamperedResultBundleThrew = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithBundleBytes(result, sampleTonBundleBytes(new byte[] {0x71, 0x73})),
          sampleTonBundleBytes(new byte[] {0x71, 0x73}));
    } catch (final IllegalArgumentException ex) {
      tamperedResultBundleThrew = ex.getMessage().contains("requestHash");
    }
    assert tamperedResultBundleThrew
        : "proof-result TON submission input must reject request-hash bundle mismatches";

    boolean mismatchedProofBase64Threw = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithProofBase64(result, "AAAA"), sampleTonBundleBytes());
    } catch (final IllegalArgumentException ex) {
      mismatchedProofBase64Threw = ex.getMessage().contains("proofBase64");
    }
    assert mismatchedProofBase64Threw
        : "proof-result TON submission input must reject mismatched proof base64";

    boolean missingEnvelopeThrew = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithEnvelopeHash(result, SolanaSccpProver.ZERO_HASH_V1),
          sampleTonBundleBytes());
    } catch (final IllegalArgumentException ex) {
      missingEnvelopeThrew = ex.getMessage().contains("proofResult.envelopeHash");
    }
    assert missingEnvelopeThrew
        : "proof-result TON submission input must reject zero envelope hashes";

    boolean tamperedEnvelopeThrew = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithEnvelopeHash(result, "0x" + repeat("aa", 32)),
          sampleTonBundleBytes());
    } catch (final IllegalArgumentException ex) {
      tamperedEnvelopeThrew = ex.getMessage().contains("wrapped proof bytes");
    }
    assert tamperedEnvelopeThrew
        : "proof-result TON submission input must reject tampered envelope hashes";

    boolean mismatchedProofContextThrew = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithProofContext(
              result,
              new TonSccpProver.ProofContext(
                  result.proofContext().version(),
                  "0x" + repeat("99", 32),
                  result.proofContext().destinationBindingHash())),
          sampleTonBundleBytes());
    } catch (final IllegalArgumentException ex) {
      mismatchedProofContextThrew = ex.getMessage().contains("proofContext");
    }
    assert mismatchedProofContextThrew
        : "proof-result TON submission input must reject mismatched proof contexts";

    boolean wrongSourceStateVerifierThrew = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithSourceStateVerifierHash(result, SolanaSccpProver.ZERO_HASH_V1),
          sampleTonBundleBytes());
    } catch (final IllegalArgumentException ex) {
      wrongSourceStateVerifierThrew = ex.getMessage().contains("sourceStateVerifierHash");
    }
    assert wrongSourceStateVerifierThrew
        : "proof-result TON submission input must reject zero source-state verifier hashes";

    boolean wrongResultDeploymentBindingThrew = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithDeploymentBinding(
              result,
              new SolanaSccpProver.SourceAdapterDeploymentBinding(
                  result.sourceAdapterDeploymentBinding().version(),
                  result.sourceAdapterDeploymentBinding().sourceDomain(),
                  TonSccpProver.DOMAIN_TON,
                  result.sourceAdapterDeploymentBinding().sourceAdapterDeploymentHash(),
                  result.sourceAdapterDeploymentBinding().sourceAdapterDeploymentReceiptHash())),
          sampleTonBundleBytes());
    } catch (final IllegalArgumentException ex) {
      wrongResultDeploymentBindingThrew = ex.getMessage().contains("targetDomain");
    }
    assert wrongResultDeploymentBindingThrew
        : "proof-result TON submission input must reject mismatched deployment bindings";

    boolean threw = false;
    try {
      TonSccpProver.wrapProofResult(
          new byte[] {1}, tonRequestWithBackend(request, "debug-ton-backend"));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("backend must be ton-contract-v1");
    }
    assert threw : "TON proof result wrapper must reject wrong backends";

    boolean zeroProofThrew = false;
    try {
      TonSccpProver.wrapProofResult(new byte[] {0, 0}, request);
    } catch (final IllegalArgumentException ex) {
      zeroProofThrew = ex.getMessage().contains("all zero");
    }
    assert zeroProofThrew : "TON proof result wrapper must reject all-zero proof bytes";

    final byte[] oversizedProof =
        new byte[TonSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedProof, (byte) 1);
    boolean oversizedProofThrew = false;
    try {
      TonSccpProver.wrapProofResult(oversizedProof, request);
    } catch (final IllegalArgumentException ex) {
      oversizedProofThrew = ex.getMessage().contains("at most");
    }
    assert oversizedProofThrew : "TON proof result wrapper must reject oversized proof bytes";

    boolean canonicalThrew = false;
    try {
      TonSccpProver.wrapProofResult(
          new byte[] {1}, tonRequestWithDeploymentBindingHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      canonicalThrew = ex.getMessage().contains("canonical");
    }
    assert canonicalThrew : "TON proof result wrapper must reject non-canonical requests";

    final byte[] exposedProof = result.proofBytes();
    exposedProof[0] = 9;
    assert Arrays.equals(new byte[] {1, 2, 3, 4}, result.proofBytes())
        : "TON proof result bytes must be defensive copies";
  }

  private static void proverResolvesWitnessProviderBeforeBuildingRequest() {
    final boolean[] resolved = new boolean[] {false};
    final byte[] bundleBytes = sampleTonBundleBytes();
    final byte[] expectedBundleBytes = Arrays.copyOf(bundleBytes, bundleBytes.length);
    final TonSccpProver.ProofRequestInput userInput =
        new TonSccpProver.ProofRequestInput(
            samplePublicInputs(),
            bundleBytes,
            new byte[0],
            repeat("56", 32),
            repeat("78", 32),
            TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
            repeat("cc", 32),
            repeat("aa", 32),
            repeat("bb", 32),
            TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
            TonSccpProver.DOMAIN_TON);
    final TonSccpProver prover =
        new TonSccpProver(
            input -> {
              assert Arrays.equals(new byte[0], input.sourceProofBytes())
                  : "UI witness provider should receive unresolved request input";
              assert input.bundleBytes() != bundleBytes
                  : "UI witness provider must receive a byte snapshot";
              input.bundleBytes()[0] = 0x7f;
              resolved[0] = true;
              return new TonSccpProver.ProofRequestInput(
                  input.publicInputs(),
                  sampleTonBundleBytes(),
                  new byte[] {9, 10},
                  input.statementHash(),
                  input.destinationBindingHash(),
                  input.sourceStateVerifierId(),
                  input.sourceStateVerifierHash(),
                  input.sourceAdapterDeploymentHash(),
                  input.sourceAdapterDeploymentReceiptHash(),
                  input.backend(),
                  input.sourceDomain());
            },
            request -> {
              assert resolved[0] : "witness provider must run before proof engine";
              assert Arrays.equals(new byte[] {9, 10}, request.sourceProofBytes())
                  : "proof engine must receive provider-resolved source proof bytes";
              return new byte[] {1, 2, 3, 4};
            });

    final TonSccpProver.ProofResult result = prover.prove(userInput);

    assert Arrays.equals(new byte[] {9, 10}, result.sourceProofBytes())
        : "wrapped result must preserve provider-resolved source proof bytes";
    assert Arrays.equals(expectedBundleBytes, userInput.bundleBytes())
        : "UI-owned TON bundle bytes must not be mutated by witness provider";
    assert Arrays.equals(expectedBundleBytes, bundleBytes)
        : "UI-owned TON bundle array must not be mutated by witness provider";
  }

  private static TonSccpProver.ProofRequest tonRequestWithBackend(
      final TonSccpProver.ProofRequest request, final String backend) {
    return new TonSccpProver.ProofRequest(
        request.version(),
        backend,
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.sourceAdapterDeploymentBindingHash(),
        request.sourceAdapterDeploymentBinding(),
        request.requestHash());
  }

  private static TonSccpProver.ProofResult tonProofResultWithEnvelopeHash(
      final TonSccpProver.ProofResult result, final String envelopeHash) {
    return new TonSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceAdapterDeploymentBinding(),
        result.requestHash(),
        envelopeHash);
  }

  private static TonSccpProver.ProofResult tonProofResultWithProofBase64(
      final TonSccpProver.ProofResult result, final String proofBase64) {
    return new TonSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        proofBase64,
        result.publicInputs(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceAdapterDeploymentBinding(),
        result.requestHash(),
        result.envelopeHash());
  }

  private static TonSccpProver.ProofResult tonProofResultWithBundleBytes(
      final TonSccpProver.ProofResult result, final byte[] bundleBytes) {
    return new TonSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        bundleBytes,
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceAdapterDeploymentBinding(),
        result.requestHash(),
        result.envelopeHash());
  }

  private static TonSccpProver.ProofResult tonProofResultWithSourceProofBytes(
      final TonSccpProver.ProofResult result, final byte[] sourceProofBytes) {
    return new TonSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.bundleBytes(),
        sourceProofBytes,
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceAdapterDeploymentBinding(),
        result.requestHash(),
        result.envelopeHash());
  }

  private static TonSccpProver.ProofResult tonProofResultWithProofContext(
      final TonSccpProver.ProofResult result, final TonSccpProver.ProofContext proofContext) {
    return new TonSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        proofContext,
        result.statementHash(),
        result.destinationBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceAdapterDeploymentBinding(),
        result.requestHash(),
        result.envelopeHash());
  }

  private static TonSccpProver.ProofResult tonProofResultWithSourceStateVerifierHash(
      final TonSccpProver.ProofResult result, final String sourceStateVerifierHash) {
    return new TonSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.sourceStateVerifierId(),
        sourceStateVerifierHash,
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceAdapterDeploymentBinding(),
        result.requestHash(),
        result.envelopeHash());
  }

  private static TonSccpProver.ProofResult tonProofResultWithDeploymentBinding(
      final TonSccpProver.ProofResult result,
      final SolanaSccpProver.SourceAdapterDeploymentBinding deploymentBinding) {
    return new TonSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.sourceAdapterDeploymentBindingHash(),
        deploymentBinding,
        result.requestHash(),
        result.envelopeHash());
  }

  private static TonSccpProver.ProofRequest tonRequestWithDeploymentBindingHash(
      final TonSccpProver.ProofRequest request, final String deploymentBindingHash) {
    return new TonSccpProver.ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        deploymentBindingHash,
        request.sourceAdapterDeploymentBinding(),
        request.requestHash());
  }

  private static void proofRequestBindsRelayContextAndDeployment() {
    final TonSccpProver.ProofRequest request =
        TonSccpProver.buildProofRequest(
            sampleProofRequestInput(repeat("aa", 32), repeat("bb", 32)));
    assert ("0x" + repeat("56", 32)).equals(request.proofContext().statementHash())
        : "proof context must expose statement hash";
    assert ("0x" + repeat("78", 32)).equals(request.proofContext().destinationBindingHash())
        : "proof context must expose destination binding hash";
    assert TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(
            request.sourceStateVerifierId())
        : "source-state verifier id must default to TON shard-state verifier";
    assert ("0x" + repeat("cc", 32)).equals(request.sourceStateVerifierHash())
        : "source-state verifier hash must be normalized";
    assert request.sourceAdapterDeploymentBinding().sourceDomain() == TonSccpProver.DOMAIN_TON
        : "deployment binding source domain must be TON";
    assert request.sourceAdapterDeploymentBinding().targetDomain() == SolanaSccpProver.DOMAIN_SORA
        : "deployment binding target domain must be SORA";
    assert ("0x" + repeat("aa", 32))
        .equals(request.sourceAdapterDeploymentBinding().sourceAdapterDeploymentHash())
        : "deployment hash must be normalized";
    assert ("0x" + repeat("bb", 32))
        .equals(request.sourceAdapterDeploymentBinding().sourceAdapterDeploymentReceiptHash())
        : "deployment receipt hash must be normalized";
    assert SolanaSccpProver.sourceAdapterDeploymentBindingHash(
            request.sourceAdapterDeploymentBinding())
        .equals(request.sourceAdapterDeploymentBindingHash())
        : "deployment binding hash must match helper";
    assert request.requestHash().matches("0x[0-9a-f]{64}") : "request hash must be hex";
    final SolanaSccpProver.SourceAdapterDeploymentBinding deploymentBinding =
        new SolanaSccpProver.SourceAdapterDeploymentBinding(
            1,
            TonSccpProver.DOMAIN_TON,
            SolanaSccpProver.DOMAIN_SORA,
            repeat("aa", 32),
            repeat("bb", 32));
    final TonSccpProver.ProofRequest bindingRequest =
        TonSccpProver.buildProofRequest(
            new TonSccpProver.ProofRequestInput(
                samplePublicInputs(),
                sampleTonBundleBytes(),
                repeat("56", 32),
                repeat("78", 32),
                TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
                repeat("cc", 32),
                deploymentBinding));
    assert bindingRequest.requestHash().equals(request.requestHash())
        : "typed deployment binding constructor must match raw-hash constructor";
    boolean wrongBindingTargetThrew = false;
    try {
      new TonSccpProver.ProofRequestInput(
          samplePublicInputs(),
          sampleTonBundleBytes(),
          repeat("56", 32),
          repeat("78", 32),
          TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
          repeat("cc", 32),
          new SolanaSccpProver.SourceAdapterDeploymentBinding(
              1,
              TonSccpProver.DOMAIN_TON,
              TonSccpProver.DOMAIN_TON,
              repeat("aa", 32),
              repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      wrongBindingTargetThrew =
          ex.getMessage().contains("sourceAdapterDeploymentBinding.targetDomain must be SORA");
    }
    assert wrongBindingTargetThrew : "typed deployment binding must enforce SORA target";
    final TonSccpProver.ProofRequest sourceStateBoundRequest =
        TonSccpProver.buildProofRequest(
            sampleProofRequestInput(
                TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
                repeat("dd", 32),
                repeat("aa", 32),
                repeat("bb", 32)));
    assert !sourceStateBoundRequest.requestHash().equals(request.requestHash())
        : "request hash must bind source-state verifier hash";
    final TonSccpProver.ProofRequest splitBoundaryRequest =
        TonSccpProver.buildProofRequest(
            new TonSccpProver.ProofRequestInput(
                samplePublicInputs(),
                sampleTonBundleBytes(new byte[] {0x71}),
                new byte[] {0x72, 0x73},
                repeat("56", 32),
                repeat("78", 32),
                TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
                repeat("cc", 32),
                repeat("aa", 32),
                repeat("bb", 32),
                TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
                TonSccpProver.DOMAIN_TON));
    final TonSccpProver.ProofRequest shiftedSplitRequest =
        TonSccpProver.buildProofRequest(
            new TonSccpProver.ProofRequestInput(
                samplePublicInputs(),
                sampleTonBundleBytes(new byte[] {0x71, 0x72}),
                new byte[] {0x73},
                repeat("56", 32),
                repeat("78", 32),
                TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
                repeat("cc", 32),
                repeat("aa", 32),
                repeat("bb", 32),
                TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
                TonSccpProver.DOMAIN_TON));
    assert !splitBoundaryRequest.requestHash().equals(shiftedSplitRequest.requestHash())
        : "TON request hash must bind bundle/source-proof byte boundaries";

    boolean threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput(
              "debug-ton-state-verifier",
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceStateVerifierId must match TON");
    }
    assert threw : "nonzero TON source-state verifier hash must require the deployed profile";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput(
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              SolanaSccpProver.ZERO_HASH_V1,
              repeat("aa", 32),
              repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceStateVerifierHash must not be zero");
    }
    assert threw : "TON proof requests must reject a zero source-state verifier hash";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              new TonSccpProver.PublicInputsInput(
                  repeat("dd", 32),
                  " " + repeat("ee", 32),
                  repeat("12", 32),
                  "19",
                  repeat("aa", 32)),
              sampleTonBundleBytes(),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("payloadHash must be canonical hex");
    }
    assert threw : "TON proof requests must reject padded payload hashes";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              sampleTonBundleBytes(),
              new byte[0],
              repeat("56", 32) + "\n",
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("statementHash must be canonical hex");
    }
    assert threw : "TON proof requests must reject padded statement hashes";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              sampleTonBundleBytes(),
              new byte[0],
              repeat("00", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("statementHash must not be zero");
    }
    assert threw : "TON proof requests must reject zero statement hashes";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              sampleTonBundleBytes(),
              new byte[0],
              repeat("56", 32),
              repeat("00", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash must not be zero");
    }
    assert threw : "TON proof requests must reject zero destination binding hashes";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput("\n" + repeat("aa", 32), repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceAdapterDeploymentHash must be canonical hex");
    }
    assert threw : "TON proof requests must reject padded deployment hashes";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              new TonSccpProver.PublicInputsInput(
                  samplePublicInputs().version(),
                  samplePublicInputs().messageId(),
                  samplePublicInputs().payloadHash(),
                  samplePublicInputs().targetDomain(),
                  samplePublicInputs().commitmentRoot(),
                  "019",
                  samplePublicInputs().finalityBlockHash()),
              sampleTonBundleBytes(),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalityHeight must be a canonical unsigned integer");
    }
    assert threw : "TON proof requests must reject noncanonical finality heights";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput(
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
              repeat("aa", 32),
              repeat("bb", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("TON template verifier hash");
    }
    assert threw : "TON proof requests must reject the template source-state verifier hash";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput(repeat("aa", 32), SolanaSccpProver.ZERO_HASH_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("must both be zero or both be non-zero");
    }
    assert threw : "half-bound deployment material must be rejected";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput(
              SolanaSccpProver.ZERO_HASH_V1,
              SolanaSccpProver.ZERO_HASH_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requires non-zero source adapter deployment binding");
    }
    assert threw : "zero TON deployment binding must be rejected";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              new byte[0],
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes");
    }
    assert threw : "empty TON bundle bytes must be rejected";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              new byte[] {0, 0},
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes must not be all zero");
    }
    assert threw : "all-zero TON bundle bytes must be rejected";

    final byte[] oversizedRequestBundle =
        new byte[TonSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedRequestBundle, (byte) 1);
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              oversizedRequestBundle,
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes must be at most");
    }
    assert threw : "oversized TON bundle bytes must be rejected";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              sampleTonBundleBytes(),
              new byte[] {0, 0},
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must not be all zero");
    }
    assert threw : "all-zero TON source proof bytes must be rejected";

    final byte[] oversizedSourceProof =
        new byte[TonSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedSourceProof, (byte) 1);
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              samplePublicInputs(),
              sampleTonBundleBytes(),
              oversizedSourceProof,
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must be at most");
    }
    assert threw : "oversized TON source proof bytes must be rejected";
    assert TonSccpProver.buildProofRequest(sampleProofRequestInput()).sourceProofBytes().length == 0
        : "empty optional TON source proof bytes must remain valid";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput(
              SolanaSccpProver.ZERO_HASH_V1,
              SolanaSccpProver.ZERO_HASH_V1,
              SolanaSccpProver.DOMAIN_SOLANA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be TON");
    }
    assert threw : "TON proof requests must reject non-TON source domains";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          new TonSccpProver.ProofRequestInput(
              new TonSccpProver.PublicInputsInput(
                  1,
                  repeat("dd", 32),
                  repeat("ee", 32),
                  SolanaSccpProver.DOMAIN_SOLANA,
                  repeat("12", 32),
                  "19",
                  repeat("aa", 32)),
              sampleTonBundleBytes(),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
              repeat("cc", 32),
              repeat("aa", 32),
              repeat("bb", 32),
              TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("targetDomain must be TON");
    }
    assert threw : "TON proof requests must reject non-TON target domains";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          sampleProofRequestInput(
              SolanaSccpProver.ZERO_HASH_V1,
              SolanaSccpProver.ZERO_HASH_V1,
              "debug-ton-backend",
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("backend must be ton-contract-v1");
    }
    assert threw : "TON proof requests must reject non-contract backends";
  }

  private static void proofRequestRejectsNoncanonicalOrMismatchedBundleBytes() {
    boolean threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(samplePublicInputs(), new byte[] {5, 6, 7}, new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes.version must be 1");
    }
    assert threw : "TON proof requests must reject placeholder bundle bytes";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              samplePublicInputs(), sampleTonBundleFixture(BigInteger.valueOf(43L)).bundleBytes, new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes must match publicInputs");
    }
    assert threw : "TON proof requests must reject bundle/public-input drift";

    final byte[] bundle = sampleTonBundleBytes();
    final TestBundleRanges ranges = splitTestSccpMessageProofBundleBytes(bundle);
    final byte[] tamperedCommitment = Arrays.copyOf(ranges.commitment.bytes, ranges.commitment.bytes.length);
    tamperedCommitment[6] = (byte) (tamperedCommitment[6] ^ 0x01);
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              samplePublicInputs(),
              replaceTestSccpMessageProofBundleVec(bundle, ranges.commitment, tamperedCommitment),
              new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes.commitment must match payload");
    }
    assert threw : "TON proof requests must reject commitment/payload drift";

    final byte[] tamperedRoot = Arrays.copyOf(bundle, bundle.length);
    tamperedRoot[1] = (byte) (tamperedRoot[1] ^ 0x01);
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(samplePublicInputs(), tamperedRoot, new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes.commitment_root must match merkle proof");
    }
    assert threw : "TON proof requests must reject tampered Merkle roots";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              samplePublicInputs(),
              replaceTestSccpMessageProofBundleVec(
                  bundle, ranges.payload, concatTestBytes(ranges.payload.bytes, new byte[] {0x00})),
              new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes.payload must not contain trailing bytes");
    }
    assert threw : "TON proof requests must reject payload trailing bytes";

    final byte[] unsupportedPayload = Arrays.copyOf(ranges.payload.bytes, ranges.payload.bytes.length);
    unsupportedPayload[0] = 0x7f;
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              samplePublicInputs(),
              replaceTestSccpMessageProofBundleVec(bundle, ranges.payload, unsupportedPayload),
              new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("unsupported SCCP payload kind");
    }
    assert threw : "TON proof requests must reject unsupported SCCP payload kinds";

    final byte[] nulPrefixedName = new byte[32];
    final byte[] tokenName = "Token".getBytes(StandardCharsets.UTF_8);
    System.arraycopy(tokenName, 0, nulPrefixedName, 1, tokenName.length);
    final SampleTonBundleFixture nulPrefixedNameBundle =
        sampleTonTokenAddBundleFixture(nulPrefixedName, fixedTestAscii32("TOK"));
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              nulPrefixedNameBundle.publicInputs,
              nulPrefixedNameBundle.bundleBytes,
              new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes.payload.name");
    }
    assert threw : "TON proof requests must reject NUL-prefixed TokenAdd names";
    final byte[] nulPrefixedSymbol = new byte[32];
    final byte[] tokenSymbol = "TOK".getBytes(StandardCharsets.UTF_8);
    System.arraycopy(tokenSymbol, 0, nulPrefixedSymbol, 1, tokenSymbol.length);
    final SampleTonBundleFixture nulPrefixedSymbolBundle =
        sampleTonTokenAddBundleFixture(fixedTestAscii32("Token"), nulPrefixedSymbol);
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              nulPrefixedSymbolBundle.publicInputs,
              nulPrefixedSymbolBundle.bundleBytes,
              new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes.payload.symbol");
    }
    assert threw : "TON proof requests must reject NUL-prefixed TokenAdd symbols";

    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              samplePublicInputs(),
              replaceTestSccpMessageProofBundleVec(
                  bundle, ranges.merkleProof, concatTestBytes(ranges.merkleProof.bytes, new byte[] {0x00})),
              new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes.merkle_proof must not contain trailing bytes");
    }
    assert threw : "TON proof requests must reject Merkle proof trailing bytes";

    final SampleTonBundleFixture oneStep =
        sampleTonBundleFixture(
            Collections.singletonList(new TestMerkleStep(repeatedByte((byte) 0xab, 32), 1)));
    final TestBundleRanges oneStepRanges = splitTestSccpMessageProofBundleBytes(oneStep.bundleBytes);
    final byte[] badDirection =
        Arrays.copyOf(oneStepRanges.merkleProof.bytes, oneStepRanges.merkleProof.bytes.length);
    badDirection[4 + 32] = 2;
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              oneStep.publicInputs,
              replaceTestSccpMessageProofBundleVec(oneStep.bundleBytes, oneStepRanges.merkleProof, badDirection),
              new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sibling_is_left must be 0 or 1");
    }
    assert threw : "TON proof requests must reject invalid Merkle sibling directions";

    final SampleTonBundleFixture nonSoraFixture =
        sampleTonBundleFixture(
            SourceSccpProofs.DOMAIN_ETH,
            TonSccpProver.CODEC_EVM_HEX,
            "0x52908400098527886E0F7030069857D2E4169EE7");
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(nonSoraFixture.publicInputs, nonSoraFixture.bundleBytes, new byte[0]));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes required for non-SORA source bundle");
    }
    assert threw : "TON proof requests must require source proof bytes for non-SORA bundles";

    final TonSccpProver.ProofRequest nonSoraRequest =
        TonSccpProver.buildProofRequest(
            proofRequestInputWithBundle(
                nonSoraFixture.publicInputs, nonSoraFixture.bundleBytes, new byte[] {0x51, 0x52}));
    final TonSccpProver.ProofResult nonSoraResult =
        TonSccpProver.wrapProofResult(new byte[] {1, 2, 3, 4}, nonSoraRequest);
    threw = false;
    try {
      new TonSccpProver.MessageBodyInput(
          tonProofResultWithSourceProofBytes(nonSoraResult, new byte[0]), nonSoraFixture.bundleBytes);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes required for non-SORA source bundle");
    }
    assert threw : "TON proof-result submissions must reject stripped non-SORA source proofs";

    final String canonicalEip55Sender = "0x52908400098527886E0F7030069857D2E4169EE7";
    final String lowercaseRequiredEip55Sender = "0xde709f2102306220921060314715629080e2fb77";
    final SampleTonBundleFixture lowercaseRequiredEip55Source =
        sampleTonBundleFixture(
            SourceSccpProofs.DOMAIN_ETH,
            TonSccpProver.CODEC_EVM_HEX,
            lowercaseRequiredEip55Sender);
    TonSccpProver.buildProofRequest(
        proofRequestInputWithBundle(
            lowercaseRequiredEip55Source.publicInputs,
            lowercaseRequiredEip55Source.bundleBytes,
            new byte[] {9, 10}));
    final SampleTonBundleFixture noncanonicalEip55Source =
        sampleTonBundleFixture(
            SourceSccpProofs.DOMAIN_ETH,
            TonSccpProver.CODEC_EVM_HEX,
            canonicalEip55Sender.toLowerCase(java.util.Locale.ROOT));
    threw = false;
    try {
      TonSccpProver.buildProofRequest(
          proofRequestInputWithBundle(
              noncanonicalEip55Source.publicInputs,
              noncanonicalEip55Source.bundleBytes,
              new byte[] {9, 10}));
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("bundleBytes.payload.sender")
              && ex.getMessage().contains("EIP-55");
    }
    assert threw : "TON proof requests must reject noncanonical EIP-55 source senders";
    for (final String invalidSender :
        new String[] {
          lowercaseRequiredEip55Sender.toUpperCase(java.util.Locale.ROOT),
          "0X" + canonicalEip55Sender.substring(2),
          "0x52908400098527886E0F7030069857D2E4169EEZ"
        }) {
      final SampleTonBundleFixture invalidSource =
          sampleTonBundleFixture(
              SourceSccpProofs.DOMAIN_ETH,
              TonSccpProver.CODEC_EVM_HEX,
              invalidSender);
      threw = false;
      try {
        TonSccpProver.buildProofRequest(
            proofRequestInputWithBundle(
                invalidSource.publicInputs,
                invalidSource.bundleBytes,
                new byte[] {9, 10}));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("bundleBytes.payload.sender");
      }
      assert threw : "invalid TON EVM source sender must be rejected";
    }
  }

  private static void proofRequestHashMatchesCrossSdkVector() {
    final SampleTonBundleFixture fixture = sampleTonBundleFixture();
    final TonSccpProver.PublicInputsInput publicInputs = fixture.publicInputs;
    final TonSccpProver.ProofRequest request =
        TonSccpProver.buildProofRequest(
            new TonSccpProver.ProofRequestInput(
                publicInputs,
                fixture.bundleBytes,
                new byte[] {0x51, 0x52, 0x53},
                repeat("55", 32),
                repeat("66", 32),
                TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
                repeat("42", 32),
                new SolanaSccpProver.SourceAdapterDeploymentBinding(
                    1,
                    TonSccpProver.DOMAIN_TON,
                    SolanaSccpProver.DOMAIN_SORA,
                    repeat("aa", 32),
                    repeat("bb", 32)),
                TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
                TonSccpProver.DOMAIN_TON));
    final byte[] expectedPublicInputsBytes =
        hexBytes(
            "01"
                + "806384e356636c10ee3bbbb90674a80410a86be034616abb811586b21ac81fc4367a4f"
                + "9061f46a282eeeda95bc68c727888bde665bd89d0ebbc6dae266e3a264"
                + "04000000"
                + "377eb92928595d90759d66529f96acf34afd4ef64cd2327ab6f65876fb3cf93e"
                + "1300000000000000"
                + repeat("aa", 32));

    assert Arrays.equals(
            expectedPublicInputsBytes, TonSccpProver.canonicalPublicInputsBytes(publicInputs))
        : "TON public-input vector must match other SDKs";
    assert "0x7d35b186e3d49aed31693e33d33355fa8fa9032160c929f2c7fe260094f6ccdf"
        .equals(request.sourceAdapterDeploymentBindingHash())
        : "TON deployment binding hash must match other SDKs";
    assert "0x2a292741b8e8d8454699eda954592904e8260e6b8a41cc840f5d9c48732c3bbe"
        .equals(request.requestHash())
        : "TON proof request hash must match other SDKs";
    final TonSccpProver.ProofResult proofResult =
        TonSccpProver.wrapProofResult(
            new byte[] {
              (byte) 0x91, (byte) 0x92, (byte) 0x93, (byte) 0x94, (byte) 0x95
            },
            request);
    assert "0x9ed8e54d81c13a61939dedffb36c487f33d32a128ba95a0d29b33c5d25be6489"
        .equals(proofResult.envelopeHash())
        : "TON proof envelope hash must match other SDKs";
  }

  private static TonSccpProver.MessageBodyInput sampleMessageBodyInput() {
    return sampleMessageBodyInput(sampleTonBundleBytes());
  }

  private static TonSccpProver.MessageBodyInput sampleMessageBodyInput(final byte[] bundleBytes) {
    return sampleMessageBodyInput(new byte[] {1, 2, 3, 4}, bundleBytes);
  }

  private static TonSccpProver.MessageBodyInput sampleMessageBodyInput(
      final byte[] proofBytes, final byte[] bundleBytes) {
    return sampleMessageBodyInput(
        samplePublicInputs(), proofBytes, bundleBytes, repeat("bb", 32), repeat("56", 32));
  }

  private static TonSccpProver.MessageBodyInput sampleMessageBodyInput(
      final TonSccpProver.PublicInputsInput publicInputs,
      final byte[] proofBytes,
      final byte[] bundleBytes,
      final String statementHash,
      final String destinationBindingHash) {
    final TonSccpProver.ProofResult proofResult =
        sampleMessageProofResult(publicInputs, proofBytes, bundleBytes, statementHash, destinationBindingHash);
    return new TonSccpProver.MessageBodyInput(proofResult, bundleBytes, new byte[] {8, 9});
  }

  private static TonSccpProver.ProofResult sampleMessageProofResult(
      final TonSccpProver.PublicInputsInput publicInputs,
      final byte[] proofBytes,
      final byte[] bundleBytes,
      final String statementHash,
      final String destinationBindingHash) {
    final TonSccpProver.ProofRequest request =
        TonSccpProver.buildProofRequest(
            new TonSccpProver.ProofRequestInput(
                publicInputs,
                bundleBytes,
                new byte[] {9, 10},
                statementHash,
                destinationBindingHash,
                TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
                repeat("cc", 32),
                repeat("aa", 32),
                repeat("bb", 32),
                TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
                TonSccpProver.DOMAIN_TON));
    return TonSccpProver.wrapProofResult(proofBytes, request);
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput() {
    return sampleProofRequestInput(
        repeat("aa", 32), repeat("bb", 32));
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final byte[] sourceProofBytes) {
    return sampleProofRequestInput(
        sourceProofBytes,
        TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        repeat("cc", 32),
        repeat("aa", 32),
        repeat("bb", 32));
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash) {
    return sampleProofRequestInput(
        TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        repeat("cc", 32),
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
        TonSccpProver.DOMAIN_TON);
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash,
      final int sourceDomain) {
    return sampleProofRequestInput(
        TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        repeat("cc", 32),
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
        sourceDomain);
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash,
      final String backend,
      final int sourceDomain) {
    return sampleProofRequestInput(
        TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        repeat("cc", 32),
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        backend,
        sourceDomain);
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash) {
    return sampleProofRequestInput(
        new byte[0],
        sourceStateVerifierId,
        sourceStateVerifierHash,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash);
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash,
      final String backend,
      final int sourceDomain) {
    return sampleProofRequestInput(
        new byte[0],
        sourceStateVerifierId,
        sourceStateVerifierHash,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        backend,
        sourceDomain);
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final byte[] sourceProofBytes,
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash) {
    return sampleProofRequestInput(
        sourceProofBytes,
        sourceStateVerifierId,
        sourceStateVerifierHash,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
        TonSccpProver.DOMAIN_TON);
  }

  private static TonSccpProver.ProofRequestInput sampleProofRequestInput(
      final byte[] sourceProofBytes,
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash,
      final String backend,
      final int sourceDomain) {
    return new TonSccpProver.ProofRequestInput(
        samplePublicInputs(),
        sampleTonBundleBytes(),
        sourceProofBytes,
        repeat("56", 32),
        repeat("78", 32),
        sourceStateVerifierId,
        sourceStateVerifierHash,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        backend,
        sourceDomain);
  }

  private static TonSccpProver.PublicInputsInput samplePublicInputs() {
    return sampleTonBundleFixture().publicInputs;
  }

  private static byte[] sampleTonBundleBytes() {
    return sampleTonBundleFixture().bundleBytes;
  }

  private static byte[] sampleTonBundleBytes(final byte[] finalityProof) {
    return sampleTonBundleFixture(
            SolanaSccpProver.DOMAIN_SORA,
            TonSccpProver.CODEC_TEXT_UTF8,
            "alice@sora",
            327L,
            BigInteger.valueOf(42L),
            "sccp-ton-proof-request",
            Collections.emptyList(),
            finalityProof)
        .bundleBytes;
  }

  private static SampleTonBundleFixture sampleTonBundleFixture() {
    return sampleTonBundleFixture(
        SolanaSccpProver.DOMAIN_SORA,
        TonSccpProver.CODEC_TEXT_UTF8,
        "alice@sora",
        327L,
        BigInteger.valueOf(42L),
        "sccp-ton-proof-request",
        Collections.emptyList(),
        new byte[] {0x71, 0x72});
  }

  private static SampleTonBundleFixture sampleTonBundleFixture(final BigInteger amount) {
    return sampleTonBundleFixture(
        SolanaSccpProver.DOMAIN_SORA,
        TonSccpProver.CODEC_TEXT_UTF8,
        "alice@sora",
        327L,
        amount,
        "sccp-ton-proof-request",
        Collections.emptyList(),
        new byte[] {0x71, 0x72});
  }

  private static SampleTonBundleFixture sampleTonBundleFixture(
      final int sourceDomain, final int senderCodec, final String sender) {
    return sampleTonBundleFixture(
        sourceDomain,
        senderCodec,
        sender,
        327L,
        BigInteger.valueOf(42L),
        "sccp-ton-proof-request",
        Collections.emptyList(),
        new byte[] {0x71, 0x72});
  }

  private static SampleTonBundleFixture sampleTonBundleFixture(
      final List<TestMerkleStep> merkleProofSteps) {
    return sampleTonBundleFixture(
        SolanaSccpProver.DOMAIN_SORA,
        TonSccpProver.CODEC_TEXT_UTF8,
        "alice@sora",
        327L,
        BigInteger.valueOf(42L),
        "sccp-ton-proof-request",
        merkleProofSteps,
        new byte[] {0x71, 0x72});
  }

  private static SampleTonBundleFixture sampleTonBundleFixture(
      final int sourceDomain,
      final int senderCodec,
      final String sender,
      final long nonce,
      final BigInteger amount,
      final String routeId,
      final List<TestMerkleStep> merkleProofSteps,
      final byte[] finalityProof) {
    final ByteArrayOutputStream payloadBody = new ByteArrayOutputStream();
    payloadBody.write(1);
    writeTestU32Le(payloadBody, sourceDomain);
    writeTestU32Le(payloadBody, TonSccpProver.DOMAIN_TON);
    writeTestU64Le(payloadBody, BigInteger.valueOf(nonce));
    writeTestU32Le(payloadBody, SolanaSccpProver.DOMAIN_SORA);
    payloadBody.write(TonSccpProver.CODEC_TEXT_UTF8);
    writeTestBytes(payloadBody, "xor#ton".getBytes(StandardCharsets.UTF_8));
    writeTestU128Le(payloadBody, amount);
    payloadBody.write(senderCodec);
    writeTestBytes(payloadBody, sender.getBytes(StandardCharsets.UTF_8));
    payloadBody.write(TonSccpProver.CODEC_TON_RAW);
    writeTestBytes(payloadBody, ("0:" + repeat("12", 32)).getBytes(StandardCharsets.UTF_8));
    payloadBody.write(TonSccpProver.CODEC_TEXT_UTF8);
    writeTestBytes(payloadBody, routeId.getBytes(StandardCharsets.UTF_8));

    final byte[] payloadBodyBytes = payloadBody.toByteArray();
    final byte[] payloadBytes = concatTestBytes(new byte[] {0x02}, payloadBodyBytes);
    final String messageId =
        "0x" + hexLower(prefixedKeccakBytes("sccp:transfer:v1", payloadBodyBytes));
    final String payloadHash =
        "0x"
            + hexLower(
                Blake2b.digest256(
                    concatTestBytes(
                        "sccp:payload:v1".getBytes(StandardCharsets.UTF_8), payloadBytes)));

    final ByteArrayOutputStream commitment = new ByteArrayOutputStream();
    commitment.write(1);
    commitment.write(6);
    writeTestU32Le(commitment, TonSccpProver.DOMAIN_TON);
    writeTestRawBytes(commitment, hexBytes(messageId.substring(2)));
    writeTestRawBytes(commitment, hexBytes(payloadHash.substring(2)));
    final byte[] commitmentBytes = commitment.toByteArray();

    byte[] currentRoot =
        Blake2b.digest256(
            concatTestBytes("sccp:hub:leaf:v1".getBytes(StandardCharsets.UTF_8), commitmentBytes));
    final ByteArrayOutputStream merkleProof = new ByteArrayOutputStream();
    writeTestU32Le(merkleProof, merkleProofSteps.size());
    for (final TestMerkleStep step : merkleProofSteps) {
      if (step.sibling.length != 32) {
        throw new IllegalArgumentException("test Merkle sibling must be 32 bytes");
      }
      writeTestRawBytes(merkleProof, step.sibling);
      merkleProof.write(step.siblingIsLeft);
      currentRoot =
          Blake2b.digest256(
              concatTestBytes(
                  "sccp:hub:node:v1".getBytes(StandardCharsets.UTF_8),
                  step.siblingIsLeft == 1
                      ? concatTestBytes(step.sibling, currentRoot)
                      : concatTestBytes(currentRoot, step.sibling)));
    }
    final String commitmentRoot = "0x" + hexLower(currentRoot);

    final ByteArrayOutputStream bundle = new ByteArrayOutputStream();
    bundle.write(1);
    writeTestRawBytes(bundle, currentRoot);
    writeTestBytes(bundle, commitmentBytes);
    writeTestBytes(bundle, merkleProof.toByteArray());
    writeTestBytes(bundle, payloadBytes);
    writeTestBytes(bundle, finalityProof);

    return new SampleTonBundleFixture(
        new TonSccpProver.PublicInputsInput(
            1,
            messageId,
            payloadHash,
            TonSccpProver.DOMAIN_TON,
            commitmentRoot,
            "19",
            repeat("aa", 32)),
        bundle.toByteArray());
  }

  private static SampleTonBundleFixture sampleTonTokenAddBundleFixture(
      final byte[] name, final byte[] symbol) {
    if (name.length != 32 || symbol.length != 32) {
      throw new IllegalArgumentException("fixed token fields must be 32 bytes");
    }

    final int targetDomain = TonSccpProver.DOMAIN_TON;
    final ByteArrayOutputStream payloadBody = new ByteArrayOutputStream();
    payloadBody.write(1);
    writeTestU32Le(payloadBody, targetDomain);
    writeTestU64Le(payloadBody, BigInteger.valueOf(327L));
    writeTestRawBytes(payloadBody, hexBytes(repeat("11", 32)));
    payloadBody.write(18);
    writeTestRawBytes(payloadBody, name);
    writeTestRawBytes(payloadBody, symbol);

    final byte[] payloadBodyBytes = payloadBody.toByteArray();
    final byte[] payloadBytes = concatTestBytes(new byte[] {0x03}, payloadBodyBytes);
    final String messageId =
        "0x" + hexLower(prefixedKeccakBytes("sccp:token:add:v1", payloadBodyBytes));
    final String payloadHash =
        "0x"
            + hexLower(
                Blake2b.digest256(
                    concatTestBytes("sccp:payload:v1".getBytes(StandardCharsets.UTF_8), payloadBytes)));

    final ByteArrayOutputStream commitment = new ByteArrayOutputStream();
    commitment.write(1);
    commitment.write(1);
    writeTestU32Le(commitment, targetDomain);
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
    writeTestBytes(bundle, new byte[] {0x71, 0x72});

    return new SampleTonBundleFixture(
        new TonSccpProver.PublicInputsInput(
            1,
            messageId,
            payloadHash,
            targetDomain,
            commitmentRoot,
            "19",
            repeat("aa", 32)),
        bundle.toByteArray());
  }

  private static byte[] fixedTestAscii32(final String value) {
    final byte[] raw = value.getBytes(StandardCharsets.UTF_8);
    if (raw.length > 32) {
      throw new IllegalArgumentException("fixed token field is too long");
    }
    return Arrays.copyOf(raw, 32);
  }

  private static TonSccpProver.ProofRequestInput proofRequestInputWithBundle(
      final TonSccpProver.PublicInputsInput publicInputs,
      final byte[] bundleBytes,
      final byte[] sourceProofBytes) {
    return new TonSccpProver.ProofRequestInput(
        publicInputs,
        bundleBytes,
        sourceProofBytes,
        repeat("56", 32),
        repeat("78", 32),
        TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        repeat("cc", 32),
        repeat("aa", 32),
        repeat("bb", 32),
        TonSccpProver.CONTRACT_PROOF_BACKEND_V1,
        TonSccpProver.DOMAIN_TON);
  }

  private static TestBundleRanges splitTestSccpMessageProofBundleBytes(final byte[] bundleBytes) {
    int offset = 33;
    final TestBundleVecRange commitment = readTestCanonicalVecRange(bundleBytes, offset);
    offset = commitment.nextOffset;
    final TestBundleVecRange merkleProof = readTestCanonicalVecRange(bundleBytes, offset);
    offset = merkleProof.nextOffset;
    final TestBundleVecRange payload = readTestCanonicalVecRange(bundleBytes, offset);
    offset = payload.nextOffset;
    final TestBundleVecRange finalityProof = readTestCanonicalVecRange(bundleBytes, offset);
    return new TestBundleRanges(commitment, merkleProof, payload, finalityProof);
  }

  private static TestBundleVecRange readTestCanonicalVecRange(
      final byte[] bundleBytes, final int offset) {
    final int length = readTestU32Le(bundleBytes, offset);
    final int start = offset + 4;
    final int end = start + length;
    if (end > bundleBytes.length) {
      throw new IllegalArgumentException("test vector exceeds bundle length");
    }
    return new TestBundleVecRange(
        offset,
        start,
        end,
        Arrays.copyOfRange(bundleBytes, start, end),
        end);
  }

  private static byte[] replaceTestSccpMessageProofBundleVec(
      final byte[] bundleBytes, final TestBundleVecRange vecRange, final byte[] replacement) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(bundleBytes, 0, vecRange.lengthOffset);
    writeTestU32Le(out, replacement.length);
    writeTestRawBytes(out, replacement);
    out.write(bundleBytes, vecRange.bytesEnd, bundleBytes.length - vecRange.bytesEnd);
    return out.toByteArray();
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

  private static int readTestU32Le(final byte[] raw, final int offset) {
    if (offset + 4 > raw.length) {
      throw new IllegalArgumentException("test u32 is too short");
    }
    return (raw[offset] & 0xff)
        | ((raw[offset + 1] & 0xff) << 8)
        | ((raw[offset + 2] & 0xff) << 16)
        | ((raw[offset + 3] & 0xff) << 24);
  }

  private static TonSccpProver.ShardStateProofRequestInput sampleShardStateProofRequestInput() {
    return sampleShardStateProofRequestInput(
        "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419");
  }

  private static TonSccpProver.ShardStateProofRequestInput sampleShardStateProofRequestInput(
      final List<TonSccpProver.ValidatorSetTransitionProofInput> validatorSetTransitionProofs) {
    return sampleShardStateProofRequestInput(
        "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
        repeat("d4", 32),
        "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3",
        validatorSetTransitionProofs);
  }

  private static TonSccpProver.ShardStateProofRequestInput sampleShardStateProofRequestInput(
      final String transactionRoot) {
    return sampleShardStateProofRequestInput(transactionRoot, repeat("d4", 32));
  }

  private static TonSccpProver.ShardStateProofRequestInput sampleShardStateProofRequestInput(
      final String transactionRoot, final String sourceStateVerifierHash) {
    return sampleShardStateProofRequestInput(
        transactionRoot,
        sourceStateVerifierHash,
        "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3");
  }

  private static TonSccpProver.ShardStateProofRequestInput sampleShardStateProofRequestInput(
      final String transactionRoot,
      final String sourceStateVerifierHash,
      final String masterchainConfigProofHash) {
    return sampleShardStateProofRequestInput(
        transactionRoot,
        sourceStateVerifierHash,
        masterchainConfigProofHash,
        Collections.emptyList());
  }

  private static TonSccpProver.ShardStateProofRequestInput sampleShardStateProofRequestInput(
      final String transactionRoot,
      final String sourceStateVerifierHash,
      final String masterchainConfigProofHash,
      final List<TonSccpProver.ValidatorSetTransitionProofInput> validatorSetTransitionProofs) {
    final byte[] shardAccountKey = new byte[32];
    shardAccountKey[0] = 17;
    return new TonSccpProver.ShardStateProofRequestInput(
        TonSccpProver.DOMAIN_TON,
        "19",
        -1,
        "9223372036854775808",
        repeat("aa", 32),
        repeat("a5", 32),
        "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
        "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
        masterchainConfigProofHash,
        0,
        "9223372036854775808",
        "7",
        repeat("bb", 32),
        repeat("bc", 32),
        "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270",
        transactionRoot,
        "7",
        "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3",
        256,
        shardAccountKey,
        "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15",
        "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf",
        hexBytes(
            "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000"),
        hexBytes(
            "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000"),
        hexBytes(
            "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0"),
        validatorSetTransitionProofs,
        TonSccpProver.MAINNET_SHARD_STATE_VERIFIER_ID_V1,
        sourceStateVerifierHash,
        "sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1",
        "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
        "sccp:ton:consensus-verifier:masterchain-block-proof:v1",
        repeat("b2", 32),
        "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1",
        repeat("c3", 32),
        "sccp:ton:finality-policy:masterchain-finality:v1",
        repeat("c4", 32));
  }

  private static TonSccpProver.ValidatorSetTransitionProofInput
      sampleValidatorSetTransitionProofInput() {
    return sampleValidatorSetTransitionProofInput(
        Arrays.asList(repeatedByte((byte) 0xab, 64), repeatedByte((byte) 0xcd, 64)));
  }

  private static TonSccpProver.ValidatorSetTransitionProofInput
      sampleValidatorSetTransitionProofInput(final List<byte[]> signatures) {
    final String transitionMessageHash =
        "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19";
    final byte[] nextValidatorSetPayload =
        hexBytes(
            "0102000000"
                + repeat("33", 32)
                + "0300000000000000"
                + repeat("44", 32)
                + "0400000000000000");
    final TonSccpProver.ValidatorSignatureProofInput validatorSignatureProof =
        new TonSccpProver.ValidatorSignatureProofInput(
            1,
            "3",
            "3",
            transitionMessageHash,
            Arrays.asList(repeatedByte((byte) 0x11, 32), repeatedByte((byte) 0x22, 32)),
            Arrays.asList("1", "2"),
            new byte[] {0x03},
            signatures);
    return new TonSccpProver.ValidatorSetTransitionProofInput(
        1,
        TonSccpProver.DOMAIN_TON,
        "7",
        "8",
        "19",
        -1,
        "9223372036854775808",
        repeat("aa", 32),
        repeat("a5", 32),
        "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
        "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f",
        nextValidatorSetPayload,
        "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983",
        repeat("cc", 32),
        transitionMessageHash,
        TonSccpProver.validatorSetTransitionSignatureHash(
            1,
            TonSccpProver.DOMAIN_TON,
            "7",
            "8",
            "19",
            -1,
            "9223372036854775808",
            repeat("aa", 32),
            repeat("a5", 32),
            "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f",
            nextValidatorSetPayload,
            "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983",
            repeat("cc", 32),
            transitionMessageHash,
            validatorSignatureProof),
        validatorSignatureProof);
  }

  private static TonSccpProver.FullLightClientAuditProofInput sampleFullLightClientAuditProofInput() {
    return sampleFullLightClientAuditProofInput(
        "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3");
  }

  private static TonSccpProver.FullLightClientAuditProofInput sampleFullLightClientAuditProofInput(
      final String masterchainConfigProofHash) {
    final TonSccpProver.ShardStateProofRequestInput shardState =
        sampleShardStateProofRequestInput(
            "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            repeat("d4", 32),
            masterchainConfigProofHash);
    final String masterchainConfigVerifierHash = "0x" + repeat("b1", 32);
    final String validatorSetTransitionVerifierHash = "0x" + repeat("c2", 32);
    final String shardAccountsDictionaryVerifierHash = "0x" + repeat("d3", 32);
    final String validatorSetPayloadHash =
        "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0";
    final String configLeafHash =
        TonSccpProver.masterchainConfigLeafHash(
            TonSccpProver.DOMAIN_TON,
            shardState.masterchainSeqno(),
            shardState.masterchainBlockHash(),
            shardState.shardStateRoot(),
            shardState.validatorSetHash(),
            validatorSetPayloadHash);
    final String configValueHash =
        "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50";
    final String sourceVerifierMaterialHash =
        SourceSccpProofs.sourceVerifierMaterialHash(
            SourceSccpProofs.DOMAIN_TON,
            shardState.sourceTrustAnchorHash(),
            shardState.consensusVerifierHash(),
            shardState.messageInclusionVerifierHash(),
            shardState.finalityPolicyHash(),
            shardState.sourceStateVerifierHash(),
            null,
            null,
            null,
            null,
            null);
    final String deploymentReceiptHash = "0x" + repeat("aa", 32);
    final String sourceAdapterDeploymentHash =
        SourceSccpProofs.sourceAdapterEngineDeploymentHash(
            SourceSccpProofs.DOMAIN_TON,
            shardState.sourceTrustAnchorHash(),
            shardState.consensusVerifierHash(),
            shardState.messageInclusionVerifierHash(),
            shardState.finalityPolicyHash(),
            deploymentReceiptHash,
            SourceSccpProofs.DOMAIN_SORA,
            null,
            shardState.sourceStateVerifierHash(),
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            masterchainConfigVerifierHash,
            validatorSetTransitionVerifierHash,
            shardAccountsDictionaryVerifierHash);
    final String fullLightClientGateHash =
        SourceSccpProofs.tonFullLightClientGateHash(
            SourceSccpProofs.DOMAIN_TON,
            shardState.sourceTrustAnchorHash(),
            shardState.consensusVerifierHash(),
            shardState.messageInclusionVerifierHash(),
            shardState.finalityPolicyHash(),
            deploymentReceiptHash,
            masterchainConfigVerifierHash,
            validatorSetTransitionVerifierHash,
            shardAccountsDictionaryVerifierHash,
            SourceSccpProofs.DOMAIN_SORA,
            null,
            shardState.sourceStateVerifierHash(),
            null,
            null,
            null,
            null,
            null);
    return new TonSccpProver.FullLightClientAuditProofInput(
        shardState,
        new TonSccpProver.SourceStateVerificationProof(new byte[] {0x11, 0x22, 0x33, 0x44}),
        validatorSetPayloadHash,
        configLeafHash,
        configValueHash,
        sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash,
        fullLightClientGateHash,
        masterchainConfigVerifierHash,
        validatorSetTransitionVerifierHash,
        shardAccountsDictionaryVerifierHash);
  }

  private static TonSccpProver.FullLightClientAuditProofInput fullLightClientAuditInputWithVerifierHashes(
      final TonSccpProver.FullLightClientAuditProofInput input,
      final String masterchainConfigVerifierHash,
      final String validatorSetTransitionVerifierHash,
      final String shardAccountsDictionaryVerifierHash) {
    return new TonSccpProver.FullLightClientAuditProofInput(
        input.shardState(),
        input.shardStateVerificationProof(),
        input.validatorSetPayloadHash(),
        input.configLeafHash(),
        input.configValueHash(),
        input.sourceVerifierMaterialHash(),
        input.sourceAdapterDeploymentHash(),
        input.fullLightClientGateHash(),
        masterchainConfigVerifierHash,
        validatorSetTransitionVerifierHash,
        shardAccountsDictionaryVerifierHash,
        input.shardStateProofPublicInputsHash(),
        input.shardStateVerificationProofHash());
  }

  private static TonSccpProver.FullLightClientAuditProofInput
      fullLightClientAuditInputWithShardStateVerificationProofHash(
          final TonSccpProver.FullLightClientAuditProofInput input,
          final String shardStateVerificationProofHash) {
    return new TonSccpProver.FullLightClientAuditProofInput(
        input.shardState(),
        input.shardStateVerificationProof(),
        input.validatorSetPayloadHash(),
        input.configLeafHash(),
        input.configValueHash(),
        input.sourceVerifierMaterialHash(),
        input.sourceAdapterDeploymentHash(),
        input.fullLightClientGateHash(),
        input.tonMasterchainConfigVerifierHash(),
        input.tonValidatorSetTransitionVerifierHash(),
        input.tonShardAccountsDictionaryVerifierHash(),
        input.shardStateProofPublicInputsHash(),
        shardStateVerificationProofHash);
  }

  private static TonSccpProver.RouteCanaryEvidenceInput sampleRouteCanaryEvidence() {
    return sampleRouteCanaryEvidence(
        null,
        null,
        "0:" + repeat("11", 32),
        "active",
        "123456789",
        "0x" + repeat("44", 32));
  }

  private static TonSccpProver.RouteCanaryEvidenceInput sampleRouteCanaryEvidence(
      final String destinationBindingHash,
      final String expectedDestinationBindingHash,
      final String verifierContractAddress,
      final String accountStatus,
      final String lastTransactionLt,
      final String verifierCodeBocRootHash) {
    return new TonSccpProver.RouteCanaryEvidenceInput(
        "0x" + repeat("31", 32),
        destinationBindingHash == null
            ? SourceSccpProofs.destinationBindingHash(TonSccpProver.DOMAIN_TON)
            : destinationBindingHash,
        expectedDestinationBindingHash,
        "0x" + repeat("33", 32),
        "0x" + repeat("34", 32),
        verifierContractAddress,
        "0x" + repeat("44", 32),
        accountStatus,
        "0x" + repeat("55", 32),
        lastTransactionLt,
        "0x" + repeat("66", 32),
        verifierCodeBocRootHash);
  }

  private static TonSccpProver.RouteCanaryEvidenceInput sampleRouteCanaryEvidenceWithGovernedHashes(
      final String routeAllowlistHash,
      final String destinationBindingHash,
      final String sourceVerifierMaterialHash,
      final String sourceAdapterEngineDeploymentHash) {
    return new TonSccpProver.RouteCanaryEvidenceInput(
        routeAllowlistHash == null ? "0x" + repeat("31", 32) : routeAllowlistHash,
        destinationBindingHash == null
            ? SourceSccpProofs.destinationBindingHash(TonSccpProver.DOMAIN_TON)
            : destinationBindingHash,
        null,
        sourceVerifierMaterialHash == null ? "0x" + repeat("33", 32) : sourceVerifierMaterialHash,
        sourceAdapterEngineDeploymentHash == null
            ? "0x" + repeat("34", 32)
            : sourceAdapterEngineDeploymentHash,
        "0:" + repeat("11", 32),
        "0x" + repeat("44", 32),
        "active",
        "0x" + repeat("55", 32),
        "123456789",
        "0x" + repeat("66", 32),
        "0x" + repeat("44", 32));
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

  private static byte[] concatTestBytes(final byte[] left, final byte[] right) {
    final byte[] out = new byte[left.length + right.length];
    System.arraycopy(left, 0, out, 0, left.length);
    System.arraycopy(right, 0, out, left.length, right.length);
    return out;
  }

  private static byte[] prefixedKeccakBytes(final String prefix, final byte[] payload) {
    return keccak256(concatTestBytes(prefix.getBytes(StandardCharsets.UTF_8), payload));
  }

  private static byte[] keccak256(final byte[] input) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(input, 0, input.length);
    final byte[] out = new byte[32];
    digest.doFinal(out, 0);
    return out;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }

  private static byte[] repeatedByte(final byte value, final int count) {
    final byte[] out = new byte[count];
    Arrays.fill(out, value);
    return out;
  }

  private static byte[] hexBytes(final String hex) {
    if ((hex.length() & 1) != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[hex.length() / 2];
    for (int index = 0; index < out.length; index++) {
      final int hi = Character.digit(hex.charAt(index * 2), 16);
      final int lo = Character.digit(hex.charAt(index * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException("hex must be lowercase");
      }
      out[index] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static int indexOfBytes(final byte[] haystack, final byte[] needle) {
    for (int offset = 0; offset <= haystack.length - needle.length; offset++) {
      boolean matches = true;
      for (int index = 0; index < needle.length; index++) {
        matches = matches && haystack[offset + index] == needle[index];
      }
      if (matches) {
        return offset;
      }
    }
    return -1;
  }

  private static final class SampleTonBundleFixture {
    private final TonSccpProver.PublicInputsInput publicInputs;
    private final byte[] bundleBytes;

    private SampleTonBundleFixture(
        final TonSccpProver.PublicInputsInput publicInputs, final byte[] bundleBytes) {
      this.publicInputs = publicInputs;
      this.bundleBytes = bundleBytes;
    }
  }

  private static final class TestMerkleStep {
    private final byte[] sibling;
    private final int siblingIsLeft;

    private TestMerkleStep(final byte[] sibling, final int siblingIsLeft) {
      this.sibling = sibling;
      this.siblingIsLeft = siblingIsLeft;
    }
  }

  private static final class TestBundleRanges {
    private final TestBundleVecRange commitment;
    private final TestBundleVecRange merkleProof;
    private final TestBundleVecRange payload;
    private final TestBundleVecRange finalityProof;

    private TestBundleRanges(
        final TestBundleVecRange commitment,
        final TestBundleVecRange merkleProof,
        final TestBundleVecRange payload,
        final TestBundleVecRange finalityProof) {
      this.commitment = commitment;
      this.merkleProof = merkleProof;
      this.payload = payload;
      this.finalityProof = finalityProof;
    }
  }

  private static final class TestBundleVecRange {
    private final int lengthOffset;
    private final int bytesStart;
    private final int bytesEnd;
    private final byte[] bytes;
    private final int nextOffset;

    private TestBundleVecRange(
        final int lengthOffset,
        final int bytesStart,
        final int bytesEnd,
        final byte[] bytes,
        final int nextOffset) {
      this.lengthOffset = lengthOffset;
      this.bytesStart = bytesStart;
      this.bytesEnd = bytesEnd;
      this.bytes = bytes;
      this.nextOffset = nextOffset;
    }
  }
}
