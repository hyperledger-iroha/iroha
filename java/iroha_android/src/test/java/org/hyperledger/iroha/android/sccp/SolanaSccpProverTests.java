package org.hyperledger.iroha.android.sccp;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import org.junit.Test;

/** Tests for Solana SCCP local proof request helpers. */
public final class SolanaSccpProverTests {
  private static final String SOLANA_SIGNATURE_55 =
      "2hxGyn4y9Mjkii76BqmxVoNYbTs3tw97bmtZRXnDoZPAw7VZTWhhk1aV11DtFgYGVibPaty4PQLHVLaKrT24NxGU";
  private static final String SOLANA_SIGNATURE_01 =
      "2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2";
  private static final String SOLANA_ZERO_SIGNATURE = repeat("1", 64);
  private static final String SOLANA_PROGRAM_42 =
      "5TeWSsjg2gbxCyWVniXeCmwM7UtHTCK7svzJr5xYJzHf";
  private static final String SOLANA_PROGRAM_02 =
      "8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR";
  private static final String SOLANA_ZERO_PROGRAM = repeat("1", 32);
  private static final String SOLANA_MAINNET_GENESIS_PUBLIC_INPUT =
      "0x8dbaadfbc441ded0257a4700cd26d814b5a196be44b963454cff8dd9543f13b5";

  @Test
  public void derivesSolanaRouteCanaryEvidenceHash() {
    final SolanaSccpProver.RouteCanaryEvidenceInput evidence =
        sampleSolanaRouteCanaryEvidence("4321", "f0VMRgECAwQF");

    assertEquals(475, SolanaSccpProver.canonicalRouteCanaryEvidenceBytes(evidence).length);
    assertEquals(
        "0x77296e47d5681f97136dc79d66dbda4478c3c5ec80271bfd4f1f3b3dbb8e15ca",
        SolanaSccpProver.routeCanaryEvidenceHash(evidence));
    assertEquals(
        "BPFLoaderUpgradeab1e11111111111111111111111",
        SolanaSccpProver.UPGRADEABLE_LOADER_ID);

    final IllegalArgumentException slotMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.routeCanaryEvidenceHash(
                    sampleSolanaRouteCanaryEvidence("4322", "f0VMRgECAwQF")));
    assertTrue(slotMismatch.getMessage().contains("solanaExpectedProgramdataSlot"));
    final IllegalArgumentException nonElf =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.routeCanaryEvidenceHash(
                    sampleSolanaRouteCanaryEvidence("4321", "AQIDBA==")));
    assertTrue(nonElf.getMessage().contains("BPF ELF"));
    final IllegalArgumentException wrongDestinationBinding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.routeCanaryEvidenceHash(
                    sampleSolanaRouteCanaryEvidence(
                        "4321", "f0VMRgECAwQF", "0x" + repeat("78", 32), null)));
    assertTrue(
        wrongDestinationBinding
            .getMessage()
            .contains("destinationBindingHash must match canonical Solana destination binding"));
    final IllegalArgumentException wrongExpectedDestinationBinding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.routeCanaryEvidenceHash(
                    sampleSolanaRouteCanaryEvidence(
                        "4321", "f0VMRgECAwQF", null, "0x" + repeat("78", 32))));
    assertTrue(
        wrongExpectedDestinationBinding
            .getMessage()
            .contains("expectedDestinationBindingHash must match canonical Solana destination binding"));
  }

  @Test
  public void normalizesWitnessAndBuildsDeterministicRequest() {
    final SolanaSccpProver.WitnessInput input =
        sampleWitnessInput(repeat("cc", 32), new byte[0][], repeat("ab", 32), repeat("cd", 32));
    final SolanaSccpProver.Witness witness = SolanaSccpProver.normalizeWitness(input);

    assertEquals(SolanaSccpProver.DOMAIN_SOLANA, witness.sourceDomain());
    assertEquals(SolanaSccpProver.DOMAIN_SORA, witness.targetDomain());
    assertEquals("0x" + repeat("aa", 32), witness.bankHash());
    assertEquals("123456789", witness.finalizedSlot());
    assertTrue(witness.blockhash().matches("0x[0-9a-f]{64}"));

    final SolanaSccpProver.ProofRequest first = SolanaSccpProver.buildProofRequest(input);
    final SolanaSccpProver.ProofRequest second = SolanaSccpProver.buildProofRequest(input);
    final SolanaSccpProver.ProofRequest canonicalBlockhash =
        SolanaSccpProver.buildProofRequest(witnessInputWithBlockhash(input, witness.blockhash()));
    assertEquals(first.witnessHash(), second.witnessHash());
    assertEquals(first.witnessHash(), canonicalBlockhash.witnessHash());
    assertArrayEquals(
        SolanaSccpProver.canonicalWitnessBytes(first.witness()),
        SolanaSccpProver.canonicalWitnessBytes(canonicalBlockhash.witness()));
    assertTrue(first.witnessHash().matches("0x[0-9a-f]{64}"));
    assertEquals("0x" + repeat("aa", 32), first.publicInputs().bankHash());
    assertEquals("123456788", first.publicInputs().parentSlot());
    assertEquals("8", first.publicInputs().bankSignatureCount());
    assertEquals("0x" + repeat("99", 32), first.publicInputs().parentBankHash());
    assertEquals("0x" + repeat("77", 32), first.publicInputs().accountInclusionRoot());
    assertEquals("0x" + repeat("88", 32), first.publicInputs().accountsLtHashChecksum());
    assertTrue(first.publicInputs().accountsLtHashProofPublicInputsHash().matches("0x[0-9a-f]{64}"));
    assertEquals("0x" + repeat("bb", 32), first.publicInputs().transactionStatusRoot());
    assertEquals("0x" + repeat("cc", 32), first.publicInputs().messageProofHash());
    assertEquals("0x" + repeat("56", 32), first.publicInputs().statementHash());
    assertEquals("0x" + repeat("78", 32), first.publicInputs().destinationBindingHash());
    assertEquals("0x" + repeat("ab", 32), first.publicInputs().sourceAdapterDeploymentHash());
    assertEquals(
        "0x" + repeat("cd", 32),
        first.publicInputs().sourceAdapterDeploymentReceiptHash());
    assertEquals(
        SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
        first.sourceStateVerifierId());
    assertEquals(SolanaSccpProver.ZERO_HASH_V1, first.sourceStateVerifierHash());
    assertEquals(
        SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
        first.publicInputs().sourceStateVerifierId());
    assertEquals(SolanaSccpProver.ZERO_HASH_V1, first.publicInputs().sourceStateVerifierHash());
    assertEquals(
        SolanaSccpProver.sourceAdapterDeploymentBindingHash(
            first.sourceAdapterDeploymentBinding()),
        first.sourceAdapterDeploymentBindingHash());
    assertEquals(first.publicInputs().statementHash(), first.proofContext().statementHash());
    assertTrue(first.proofContextHash().matches("0x[0-9a-f]{64}"));
    assertTrue(first.sourceAdapterDeploymentBindingHash().matches("0x[0-9a-f]{64}"));
    assertTrue(SolanaSccpProver.canonicalProofContextBytes(first.proofContext()).length > 0);

    final SolanaSccpProver.WitnessInput changedDigest =
        new SolanaSccpProver.WitnessInput(
            SolanaSccpProver.DOMAIN_SORA,
            SolanaSccpProver.MAINNET_GENESIS_HASH,
            "123456789",
            "123456788",
            "8",
            repeat("99", 32),
            "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosg1kA",
            repeat("aa", 32),
            repeat("bb", 32),
            repeat("cc", 32),
            repeat("77", 32),
            repeat("88", 32),
            null,
            new byte[0],
            null,
            SOLANA_SIGNATURE_55,
            SOLANA_PROGRAM_42,
            repeat("dd", 32),
            repeat("ee", 32),
            repeat("12", 32),
            repeat("35", 32),
            repeat("56", 32),
            repeat("78", 32),
            new byte[0][],
            repeat("ab", 32),
            repeat("cd", 32));
    assertNotEquals(
        first.witnessHash(), SolanaSccpProver.buildProofRequest(changedDigest).witnessHash());
  }

  @Test
  public void rejectsNonSoraSolanaProofRequestTargetDomain() {
    final IllegalArgumentException error =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.buildProofRequest(retargetWitnessInput(4)));

    assertTrue(error.getMessage().contains("targetDomain must be SORA"));
  }

  @Test
  public void requiresCallerSuppliedSourceEventDigest() {
    assertThrows(
        NullPointerException.class,
            () ->
            SolanaSccpProver.normalizeWitness(
                new SolanaSccpProver.WitnessInput(
                    SolanaSccpProver.DOMAIN_SORA,
                    SolanaSccpProver.MAINNET_GENESIS_HASH,
                    "123456789",
                    "123456788",
                    "8",
                    repeat("99", 32),
                    "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosg1kA",
                    repeat("aa", 32),
                    repeat("bb", 32),
                    repeat("cc", 32),
                    repeat("77", 32),
                    repeat("88", 32),
                    null,
                    new byte[0],
                    null,
                    SOLANA_SIGNATURE_55,
                    SOLANA_PROGRAM_42,
                    repeat("dd", 32),
                    repeat("ee", 32),
                    repeat("12", 32),
                    null,
                    repeat("56", 32),
                    repeat("78", 32),
                    new byte[0][],
                    SolanaSccpProver.ZERO_HASH_V1,
                    SolanaSccpProver.ZERO_HASH_V1)));
  }

  @Test
  public void buildsMessageProofHashFromInclusionWitness() {
    final byte[][] branch = new byte[][] {repeatByte((byte) 0x56, 32)};
    final String transactionStatusRoot =
        SolanaSccpProver.transactionStatusRootFromBranch(
            repeat("34", 32), SOLANA_SIGNATURE_55, SOLANA_PROGRAM_42, branch);
    assertEquals(
        "0xb048ca31d8ad7b2a0d15cbeb81d536350743483d44dd93136e859df93d3863b2",
        transactionStatusRoot);
    final String hash =
        SolanaSccpProver.messageProofHash(
            repeat("34", 32), transactionStatusRoot, SOLANA_SIGNATURE_55, SOLANA_PROGRAM_42, branch);

    assertTrue(hash.matches("0x[0-9a-f]{64}"));
    assertTrue(
        SolanaSccpProver.canonicalTransactionStatusLeafBytes(
                repeat("34", 32), SOLANA_SIGNATURE_55, SOLANA_PROGRAM_42)
            .length
            > 0);
    assertEquals(
        "0x4e12efed6d53466de0596f05aa6cc767df1efd6a4d1549276c4ec8b69118515d",
        SolanaSccpProver.transactionStatusLeafHash(
            repeat("34", 32), SOLANA_SIGNATURE_55, SOLANA_PROGRAM_42));
    final IllegalArgumentException zeroLeafSignatureEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.transactionStatusLeafHash(
                    repeat("34", 32), SOLANA_ZERO_SIGNATURE, SOLANA_PROGRAM_42));
    assertTrue(zeroLeafSignatureEx.getMessage().contains("transactionSignature"));
    final IllegalArgumentException zeroLeafProgramEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.transactionStatusLeafHash(
                    repeat("34", 32), SOLANA_SIGNATURE_55, SOLANA_ZERO_PROGRAM));
    assertTrue(zeroLeafProgramEx.getMessage().contains("emitterProgramId"));
    assertTrue(
        SolanaSccpProver.canonicalMessageProofBytes(
                repeat("34", 32), transactionStatusRoot, SOLANA_SIGNATURE_55, SOLANA_PROGRAM_42, branch)
                .length
            > 0);
    assertNotEquals(
        hash,
        SolanaSccpProver.messageProofHash(
            repeat("34", 32), transactionStatusRoot, SOLANA_SIGNATURE_01, SOLANA_PROGRAM_42, branch));
    assertNotEquals(
        hash,
        SolanaSccpProver.messageProofHash(
            repeat("34", 32), transactionStatusRoot, SOLANA_SIGNATURE_55, SOLANA_PROGRAM_02, branch));
    final IllegalArgumentException zeroProofSignatureEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("34", 32),
                    transactionStatusRoot,
                    SOLANA_ZERO_SIGNATURE,
                    SOLANA_PROGRAM_42,
                    branch));
    assertTrue(zeroProofSignatureEx.getMessage().contains("transactionSignature"));
    final IllegalArgumentException zeroProofProgramEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("34", 32),
                    transactionStatusRoot,
                    SOLANA_SIGNATURE_55,
                    SOLANA_ZERO_PROGRAM,
                    branch));
    assertTrue(zeroProofProgramEx.getMessage().contains("emitterProgramId"));
    final IllegalArgumentException emptyBranchEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("34", 32),
                    transactionStatusRoot,
                    SOLANA_SIGNATURE_55,
                    SOLANA_PROGRAM_42,
                    new byte[][] {}));
    assertTrue(emptyBranchEx.getMessage().contains("inclusionBranch"));
    final IllegalArgumentException oversizedBranchEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("34", 32),
                    transactionStatusRoot,
                    SOLANA_SIGNATURE_55,
                    SOLANA_PROGRAM_42,
                    repeatedBranch(SolanaSccpProver.MAX_SOURCE_MERKLE_BRANCH_NODES + 1)));
    assertTrue(oversizedBranchEx.getMessage().contains("at most"));
    final IllegalArgumentException ex =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("34", 32),
                    transactionStatusRoot,
                    SOLANA_SIGNATURE_55,
                    SOLANA_PROGRAM_42,
                    new byte[][] {repeatByte((byte) 0xab, 31)}));
    assertTrue(ex.getMessage().contains("inclusionBranch[0]"));
    final IllegalArgumentException base58Ex =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("34", 32),
                    transactionStatusRoot,
                    "not-a-solana-signature",
                    SOLANA_PROGRAM_42,
                    branch));
    assertTrue(base58Ex.getMessage().contains("transactionSignature"));
  }

  @Test
  public void buildsEpochStakeRootForVoteWitnesses() {
    final byte[][] validatorPublicKeys =
        new byte[][] {repeatByte((byte) 0x11, 32), repeatByte((byte) 0x22, 32)};
    final String[] validatorStakes = new String[] {"1", "2"};

    assertEquals(432_000L, SolanaSccpProver.MAINNET_SLOTS_PER_EPOCH);
    assertEquals("2", SolanaSccpProver.mainnetEpochForSlot("864000"));
    assertEquals(
        134,
        SolanaSccpProver.canonicalEpochStakeRootBytes(
                "3", validatorPublicKeys, validatorStakes)
            .length);
    assertEquals(
        "0x1d86a5ecfac6e63bfcefdc1a3bfefd962a33e2a4cf65cd4e8518bcebea771f0a",
        SolanaSccpProver.epochStakeRoot("3", validatorPublicKeys, validatorStakes));

    final IllegalArgumentException ex =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.epochStakeRoot(
                    "3", new byte[][] {repeatByte((byte) 0x11, 31)}, new String[] {"1"}));
    assertTrue(ex.getMessage().contains("validatorPublicKeys[0]"));

    final IllegalArgumentException zeroKey =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.epochStakeRoot(
                    "3", new byte[][] {repeatByte((byte) 0x00, 32)}, new String[] {"1"}));
    assertTrue(zeroKey.getMessage().contains("validatorPublicKeys[0]"));

    final byte[][] oversizedValidatorPublicKeys =
        new byte[SolanaSccpProver.MAX_VALIDATORS + 1][32];
    final String[] oversizedValidatorStakes = new String[oversizedValidatorPublicKeys.length];
    Arrays.fill(oversizedValidatorStakes, "1");
    for (int i = 0; i < oversizedValidatorPublicKeys.length; i++) {
      final int value = i + 1;
      oversizedValidatorPublicKeys[i][30] = (byte) (value & 0xff);
      oversizedValidatorPublicKeys[i][31] = (byte) ((value >>> 8) & 0xff);
    }
    final IllegalArgumentException tooManyValidators =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.epochStakeRoot(
                    "3", oversizedValidatorPublicKeys, oversizedValidatorStakes));
    assertTrue(tooManyValidators.getMessage().contains("1..8192"));
  }

  @Test
  public void buildsStakeActivationHashForFinalityContext() {
    final byte[][] validatorPublicKeys =
        new byte[][] {repeatByte((byte) 0x11, 32), repeatByte((byte) 0x22, 32)};
    final String[] validatorStakes = new String[] {"1", "2"};
    final String[] activationEpochs = new String[] {"0", "2"};
    final String[] deactivationEpochs = new String[] {"18446744073709551615", "9"};

    assertEquals(
        165,
        SolanaSccpProver.canonicalStakeActivationBytes(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs)
            .length);
    assertEquals(
        "0xdb418c62a1aeb8ae15cb26e3a198d46890cefa3545df8e1921be2e83f57dabf3",
        SolanaSccpProver.stakeActivationHash(
            "3",
            validatorPublicKeys,
            validatorStakes,
            activationEpochs,
            deactivationEpochs));

    final IllegalArgumentException futureActivation =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeActivationHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    new String[] {"4", "2"},
                    deactivationEpochs));
    assertTrue(futureActivation.getMessage().contains("validatorActivationEpochs[0]"));

    final IllegalArgumentException currentEpochActivation =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeActivationHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    new String[] {"3", "2"},
                    deactivationEpochs));
    assertTrue(currentEpochActivation.getMessage().contains("validatorActivationEpochs[0]"));

    final IllegalArgumentException expired =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeActivationHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    activationEpochs,
                    new String[] {"18446744073709551615", "2"}));
    assertTrue(expired.getMessage().contains("validatorDeactivationEpochs[1]"));
    assertEquals(
        66,
        SolanaSccpProver.stakeActivationHash(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                new String[] {"18446744073709551615", "3"})
            .length());

    final IllegalArgumentException lengthMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeActivationHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    new String[] {"0"},
                    deactivationEpochs));
    assertTrue(lengthMismatch.getMessage().contains("validatorActivationEpochs"));
  }

  @Test
  public void buildsAccountOpeningHashForFinalityContext() {
    final byte[] address = repeatByte((byte) 0x31, 32);
    final String dataHash = "0x" + repeat("71", 32);

    assertEquals(
        122,
        SolanaSccpProver.canonicalAccountOpeningBytes(
                address,
                hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
                "1000000",
                "0",
                false,
                dataHash)
            .length);
    final String accountHash =
        SolanaSccpProver.accountOpeningHash(
            address,
            hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
            "1000000",
            "0",
            false,
            dataHash);
    assertTrue(accountHash.matches("0x[0-9a-f]{64}"));
    assertNotEquals(
        accountHash,
        SolanaSccpProver.accountOpeningHash(
            address,
            hexBytes(SolanaSccpProver.STAKE_PROGRAM_ID),
            "1000000",
            "0",
            false,
            dataHash));
    assertNotEquals(
        accountHash,
        SolanaSccpProver.accountOpeningHash(
            address,
            hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
            "1000000",
            "0",
            true,
            dataHash));

    final IllegalArgumentException zeroLamports =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.accountOpeningHash(
                    address,
                    hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
                    "0",
                    "0",
                    false,
                    dataHash));
    assertTrue(zeroLamports.getMessage().contains("lamports"));
  }

  @Test
  public void buildsOpenedAccountsLtHashContributionBindings() {
    final SolanaSccpProver.AccountOpeningInput voteOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x31, 32),
            hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
            "1000000",
            "0",
            false,
            "0x" + repeat("91", 32));
    final SolanaSccpProver.AccountOpeningInput stakeOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x32, 32),
            hexBytes(SolanaSccpProver.STAKE_PROGRAM_ID),
            "2000000",
            "0",
            false,
            "0x" + repeat("92", 32));
    final SolanaSccpProver.AccountOpeningInput stakeHistoryOpening =
        new SolanaSccpProver.AccountOpeningInput(
            hexBytes(SolanaSccpProver.STAKE_HISTORY_SYSVAR_ID),
            hexBytes(SolanaSccpProver.SYSVAR_PROGRAM_ID),
            "1",
            "0",
            false,
            "0x" + repeat("93", 32));
    final SolanaSccpProver.AccountOpeningInput unopenedOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x34, 32),
            hexBytes(SolanaSccpProver.STAKE_PROGRAM_ID),
            "3000000",
            "0",
            false,
            "0x" + repeat("94", 32));
    final byte[] voteRawData = new byte[] {1, 2, 3};
    final byte[] stakeRawData = new byte[] {4, 5, 6};
    final byte[] stakeHistoryRawData = new byte[] {7, 8, 9};
    final byte[] unopenedRawData = new byte[] {10, 11, 12};
    final byte[] voteLtHash = SolanaSccpProver.accountLtHash(voteOpening, voteRawData);
    final byte[] stakeLtHash = SolanaSccpProver.accountLtHash(stakeOpening, stakeRawData);
    final byte[] stakeHistoryLtHash =
        SolanaSccpProver.accountLtHash(stakeHistoryOpening, stakeHistoryRawData);
    final byte[] unopenedLtHash = SolanaSccpProver.accountLtHash(unopenedOpening, unopenedRawData);
    final byte[] accountsLtHash =
        SolanaSccpProver.accountsLtHashFromOpenings(
            new SolanaSccpProver.AccountOpeningInput[] {
              voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening
            },
            new byte[][] {voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData});
    final SolanaSccpProver.OpenedAccountsLtHashContributionsInput input =
        new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
            SolanaSccpProver.DOMAIN_SOLANA,
            "1296096",
            "0x" + repeat("77", 32),
            SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash,
            new SolanaSccpProver.AccountOpeningInput[] {voteOpening},
            new byte[][] {voteRawData},
            new byte[][] {voteLtHash},
            new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
            new byte[][] {stakeRawData},
            new byte[][] {stakeLtHash},
            stakeHistoryOpening,
            stakeHistoryRawData,
            stakeHistoryLtHash);

    assertArrayEquals(unopenedLtHash, SolanaSccpProver.openedAccountsLtHashResidual(input));
    assertEquals(
        SolanaSccpProver.accountsLtHashChecksum(unopenedLtHash),
        SolanaSccpProver.openedAccountsLtHashResidualChecksum(input));
    assertEquals(10_696, SolanaSccpProver.canonicalOpenedAccountsLtHashContributionsBytes(input).length);
    assertEquals(
        "0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9",
        SolanaSccpProver.openedAccountsLtHashContributionsHash(input));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            SolanaSccpProver.openedAccountsLtHashContributionsHash(
                new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
                    SolanaSccpProver.DOMAIN_SOLANA,
                    "1296096",
                    "0x" + repeat("77", 32),
                    "0x" + repeat("88", 32),
                    accountsLtHash,
                    new SolanaSccpProver.AccountOpeningInput[] {voteOpening},
                    new byte[][] {voteRawData},
                    new byte[][] {voteLtHash},
                    new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
                    new byte[][] {stakeRawData},
                    new byte[][] {stakeLtHash},
                    stakeHistoryOpening,
                    stakeHistoryRawData,
                    stakeHistoryLtHash)));
    final SolanaSccpProver.AccountOpeningInput duplicateStakeOpening =
        new SolanaSccpProver.AccountOpeningInput(
            voteOpening.address(),
            stakeOpening.owner(),
            stakeOpening.lamports(),
            stakeOpening.rentEpoch(),
            stakeOpening.executable(),
            stakeOpening.dataHash());
    final byte[] duplicateStakeLtHash =
        SolanaSccpProver.accountLtHash(duplicateStakeOpening, stakeRawData);
    final IllegalArgumentException duplicateOpened =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.openedAccountsLtHashContributionsHash(
                    new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
                        SolanaSccpProver.DOMAIN_SOLANA,
                        "1296096",
                        "0x" + repeat("77", 32),
                        SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
                        accountsLtHash,
                        new SolanaSccpProver.AccountOpeningInput[] {voteOpening},
                        new byte[][] {voteRawData},
                        new byte[][] {voteLtHash},
                        new SolanaSccpProver.AccountOpeningInput[] {duplicateStakeOpening},
                        new byte[][] {stakeRawData},
                        new byte[][] {duplicateStakeLtHash},
                        stakeHistoryOpening,
                        stakeHistoryRawData,
                        stakeHistoryLtHash)));
    assertTrue(duplicateOpened.getMessage().contains("opened account addresses"));
    final SolanaSccpProver.AccountOpeningInput zeroLamportsVoteOpening =
        new SolanaSccpProver.AccountOpeningInput(
            voteOpening.address(),
            voteOpening.owner(),
            "0",
            voteOpening.rentEpoch(),
            voteOpening.executable(),
            voteOpening.dataHash());
    final IllegalArgumentException zeroLamportsOpened =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.openedAccountsLtHashContributionsHash(
                    new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
                        SolanaSccpProver.DOMAIN_SOLANA,
                        "1296096",
                        "0x" + repeat("77", 32),
                        SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
                        accountsLtHash,
                        new SolanaSccpProver.AccountOpeningInput[] {zeroLamportsVoteOpening},
                        new byte[][] {voteRawData},
                        new byte[][] {new byte[2048]},
                        new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
                        new byte[][] {stakeRawData},
                        new byte[][] {stakeLtHash},
                        stakeHistoryOpening,
                        stakeHistoryRawData,
                        stakeHistoryLtHash)));
    assertTrue(zeroLamportsOpened.getMessage().contains("lamports"));
    final IllegalArgumentException oversizedVoteOpened =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.openedAccountsLtHashContributionsHash(
                    new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
                        SolanaSccpProver.DOMAIN_SOLANA,
                        "1296096",
                        "0x" + repeat("77", 32),
                        SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
                        accountsLtHash,
                        repeatedOpening(voteOpening, SolanaSccpProver.MAX_VALIDATORS + 1),
                        repeatedBytes(voteRawData, SolanaSccpProver.MAX_VALIDATORS + 1),
                        repeatedBytes(voteLtHash, SolanaSccpProver.MAX_VALIDATORS + 1),
                        new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
                        new byte[][] {stakeRawData},
                        new byte[][] {stakeLtHash},
                        stakeHistoryOpening,
                        stakeHistoryRawData,
                        stakeHistoryLtHash)));
    assertTrue(oversizedVoteOpened.getMessage().contains("validatorVoteAccountOpenings"));
  }

  @Test
  public void derivesAccountLtHashFromOpeningsAndRawData() {
    final SolanaSccpProver.AccountOpeningInput voteOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x31, 32),
            hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
            "1000000",
            "0",
            false,
            "0x" + repeat("91", 32));
    final byte[] voteRawData = new byte[] {1, 2, 3};
    final byte[] voteLtHash = SolanaSccpProver.accountLtHash(voteOpening, voteRawData);
    assertEquals(2048, voteLtHash.length);
    assertEquals(
        "0x56a868657e9113c76dc94321040b8f01a35ea4996c6fa235581510cd18be4bfe",
        SolanaSccpProver.accountsLtHashChecksum(voteLtHash));
    final byte[] maxRawData = new byte[65_536];
    for (int i = 0; i < maxRawData.length; i++) {
      maxRawData[i] = (byte) (i & 0xff);
    }
    final byte[] maxLtHash = SolanaSccpProver.accountLtHash(voteOpening, maxRawData);
    assertEquals(
        "0xc467c59f47747fdae4d87f8c79413ae24d3674ea3ca02aad0a1216a20d4fe147",
        SolanaSccpProver.accountsLtHashChecksum(maxLtHash));
    assertEquals(
        "c972db5d20a5a451a44daa674d0511382480d6e9060f750129723812e0e3c66a4"
            + "deddbb7975e2ff4d4c753aebcb703e61122d1ca1cfcd4f0c002a2cad30f4949",
        hexLower(Arrays.copyOfRange(maxLtHash, 0, 64)));
    assertEquals(
        "b4159fa2d334c4209bfb59997f7da42a56e2e921e0bbc4ebd916f3c55353b630"
            + "e26303b0af0b23e91870e9815f7ed6348395fbc7c0f07bf605da23589fa9fb51",
        hexLower(Arrays.copyOfRange(maxLtHash, maxLtHash.length - 64, maxLtHash.length)));
    final SolanaSccpProver.AccountOpeningInput zeroLamportsOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x33, 32),
            hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
            "0",
            "0",
            false,
            "0x" + repeat("94", 32));
    assertArrayEquals(
        new byte[2048],
        SolanaSccpProver.accountLtHash(zeroLamportsOpening, voteRawData));
    assertArrayEquals(
        voteLtHash,
        SolanaSccpProver.accountsLtHashFromOpenings(
            new SolanaSccpProver.AccountOpeningInput[] {voteOpening, zeroLamportsOpening},
            new byte[][] {voteRawData, voteRawData}));

    final SolanaSccpProver.AccountOpeningInput stakeOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x32, 32),
            hexBytes(SolanaSccpProver.STAKE_PROGRAM_ID),
            "2000000",
            "0",
            false,
            "0x" + repeat("92", 32));
    final SolanaSccpProver.AccountOpeningInput stakeHistoryOpening =
        new SolanaSccpProver.AccountOpeningInput(
            hexBytes(SolanaSccpProver.STAKE_HISTORY_SYSVAR_ID),
            hexBytes(SolanaSccpProver.SYSVAR_PROGRAM_ID),
            "1",
            "0",
            false,
            "0x" + repeat("93", 32));
    final byte[] stakeRawData = new byte[] {4, 5, 6};
    final byte[] stakeHistoryRawData = new byte[] {7, 8, 9};
    final byte[] stakeLtHash = SolanaSccpProver.accountLtHash(stakeOpening, stakeRawData);
    final byte[] stakeHistoryLtHash =
        SolanaSccpProver.accountLtHash(stakeHistoryOpening, stakeHistoryRawData);
    final byte[] unopenedLtHash = repeatByte((byte) 0x44, 2048);
    final byte[] openedLtHash =
        SolanaSccpProver.accountsLtHashFromOpenings(
            new SolanaSccpProver.AccountOpeningInput[] {
              voteOpening, stakeOpening, stakeHistoryOpening
            },
            new byte[][] {voteRawData, stakeRawData, stakeHistoryRawData});
    final byte[] accountsLtHash = addFullLtHash(openedLtHash, unopenedLtHash);
    final SolanaSccpProver.OpenedAccountsLtHashContributionsInput derivedInput =
        new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
            SolanaSccpProver.DOMAIN_SOLANA,
            "1296096",
            "0x" + repeat("77", 32),
            SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash,
            new SolanaSccpProver.AccountOpeningInput[] {voteOpening},
            new byte[][] {voteRawData},
            new byte[0][],
            new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
            new byte[][] {stakeRawData},
            new byte[0][],
            stakeHistoryOpening,
            stakeHistoryRawData,
            new byte[0]);
    final SolanaSccpProver.OpenedAccountsLtHashContributionsInput precomputedInput =
        new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
            SolanaSccpProver.DOMAIN_SOLANA,
            "1296096",
            "0x" + repeat("77", 32),
            SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash,
            new SolanaSccpProver.AccountOpeningInput[] {voteOpening},
            new byte[][] {voteRawData},
            new byte[][] {voteLtHash},
            new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
            new byte[][] {stakeRawData},
            new byte[][] {stakeLtHash},
            stakeHistoryOpening,
            stakeHistoryRawData,
            stakeHistoryLtHash);

    assertArrayEquals(unopenedLtHash, SolanaSccpProver.openedAccountsLtHashResidual(derivedInput));
    assertArrayEquals(
        SolanaSccpProver.canonicalOpenedAccountsLtHashContributionsBytes(precomputedInput),
        SolanaSccpProver.canonicalOpenedAccountsLtHashContributionsBytes(derivedInput));
    assertEquals(
        SolanaSccpProver.openedAccountsLtHashContributionsHash(precomputedInput),
        SolanaSccpProver.openedAccountsLtHashContributionsHash(derivedInput));
    final byte[] wrongVoteLtHash = Arrays.copyOf(voteLtHash, voteLtHash.length);
    wrongVoteLtHash[0] = (byte) (wrongVoteLtHash[0] ^ 1);
    final IllegalArgumentException badPrecomputed =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.openedAccountsLtHashContributionsHash(
                    new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
                        derivedInput.sourceDomain(),
                        derivedInput.finalizedSlot(),
                        derivedInput.accountInclusionRoot(),
                        derivedInput.accountsLtHashChecksum(),
                        derivedInput.accountsLtHash(),
                        derivedInput.validatorVoteAccountOpenings(),
                        derivedInput.validatorVoteAccountRawData(),
                        new byte[][] {wrongVoteLtHash},
                        derivedInput.validatorStakeAccountOpenings(),
                        derivedInput.validatorStakeAccountRawData(),
                        new byte[][] {stakeLtHash},
                        derivedInput.stakeHistorySysvarOpening(),
                        derivedInput.stakeHistorySysvarRawData(),
                        stakeHistoryLtHash)));
    assertTrue(badPrecomputed.getMessage().contains("validatorVoteAccountLtHashes[0]"));
  }

  @Test
  public void buildsAccountsLtHashSourceStateProofRequests() {
    final SolanaSccpProver.AccountOpeningInput voteOpening =
        new SolanaSccpProver.AccountOpeningInput(
            hexBytes(repeat("31", 32)),
            hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
            "1000000",
            "0",
            false,
            "0x" + repeat("91", 32));
    final SolanaSccpProver.AccountOpeningInput stakeOpening =
        new SolanaSccpProver.AccountOpeningInput(
            hexBytes(repeat("32", 32)),
            hexBytes(SolanaSccpProver.STAKE_PROGRAM_ID),
            "2000000",
            "0",
            false,
            "0x" + repeat("92", 32));
    final SolanaSccpProver.AccountOpeningInput stakeHistoryOpening =
        new SolanaSccpProver.AccountOpeningInput(
            hexBytes(SolanaSccpProver.STAKE_HISTORY_SYSVAR_ID),
            hexBytes(SolanaSccpProver.SYSVAR_PROGRAM_ID),
            "1",
            "0",
            false,
            "0x" + repeat("93", 32));
    final byte[] voteRawData = new byte[] {1, 2, 3};
    final byte[] stakeRawData = new byte[] {4, 5, 6};
    final byte[] stakeHistoryRawData = new byte[] {7, 8, 9};
    final byte[] voteLtHash = SolanaSccpProver.accountLtHash(voteOpening, voteRawData);
    final byte[] stakeLtHash = SolanaSccpProver.accountLtHash(stakeOpening, stakeRawData);
    final byte[] stakeHistoryLtHash =
        SolanaSccpProver.accountLtHash(stakeHistoryOpening, stakeHistoryRawData);
    final byte[] openedLtHash = addFullLtHash(addFullLtHash(voteLtHash, stakeLtHash), stakeHistoryLtHash);
    final byte[] unopenedLtHash = ltHash(4);
    final byte[] accountsLtHash =
        addFullLtHash(openedLtHash, unopenedLtHash);
    final SolanaSccpProver.OpenedAccountsLtHashContributionsInput opened =
        new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
            SolanaSccpProver.DOMAIN_SOLANA,
            "1296096",
            "0x" + repeat("77", 32),
            SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash,
            new SolanaSccpProver.AccountOpeningInput[] {voteOpening},
            new byte[][] {voteRawData},
            new byte[][] {voteLtHash},
            new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
            new byte[][] {stakeRawData},
            new byte[][] {stakeLtHash},
            stakeHistoryOpening,
            stakeHistoryRawData,
            stakeHistoryLtHash);
    final String bankHash =
        SolanaSccpProver.agaveBankHash(
            repeat("c0", 32), "8", repeat("42", 32), accountsLtHash);
    final byte[] zeroAccountsLtHash = new byte[2048];
    final String zeroAccountsLtHashChecksum =
        SolanaSccpProver.accountsLtHashChecksum(zeroAccountsLtHash);
    assertTrue(zeroAccountsLtHashChecksum.startsWith("0x"));
    final IllegalArgumentException zeroBankHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.agaveBankHash(
                    repeat("c0", 32), "8", repeat("42", 32), zeroAccountsLtHash));
    assertTrue(zeroBankHash.getMessage().contains("accountsLtHash"));
    final SolanaSccpProver.OpenedAccountsLtHashContributionsInput zeroOpened =
        new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
            opened.sourceDomain(),
            opened.finalizedSlot(),
            opened.accountInclusionRoot(),
            zeroAccountsLtHashChecksum,
            zeroAccountsLtHash,
            opened.validatorVoteAccountOpenings(),
            opened.validatorVoteAccountRawData(),
            opened.validatorVoteAccountLtHashes(),
            opened.validatorStakeAccountOpenings(),
            opened.validatorStakeAccountRawData(),
            opened.validatorStakeAccountLtHashes(),
            opened.stakeHistorySysvarOpening(),
            opened.stakeHistorySysvarRawData(),
            opened.stakeHistorySysvarAccountLtHash());
    final IllegalArgumentException zeroOpenedHash =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.openedAccountsLtHashContributionsHash(zeroOpened));
    assertTrue(zeroOpenedHash.getMessage().contains("accountsLtHash"));
    final SolanaSccpProver.OpenedAccountsLtHashContributionsInput zeroResidualOpened =
        new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
            opened.sourceDomain(),
            opened.finalizedSlot(),
            opened.accountInclusionRoot(),
            SolanaSccpProver.accountsLtHashChecksum(openedLtHash),
            openedLtHash,
            opened.validatorVoteAccountOpenings(),
            opened.validatorVoteAccountRawData(),
            opened.validatorVoteAccountLtHashes(),
            opened.validatorStakeAccountOpenings(),
            opened.validatorStakeAccountRawData(),
            opened.validatorStakeAccountLtHashes(),
            opened.stakeHistorySysvarOpening(),
            opened.stakeHistorySysvarRawData(),
            opened.stakeHistorySysvarAccountLtHash());
    final IllegalArgumentException zeroResidualHash =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.openedAccountsLtHashContributionsHash(zeroResidualOpened));
    assertTrue(zeroResidualHash.getMessage().contains("openedAccountsLtHashResidual"));
    final SolanaSccpProver.WitnessInput witness =
        new SolanaSccpProver.WitnessInput(
            SolanaSccpProver.DOMAIN_SORA,
            SolanaSccpProver.MAINNET_GENESIS_HASH,
            "1296096",
            "1296095",
            "8",
            repeat("c0", 32),
            repeat("42", 32),
            bankHash,
            repeat("bb", 32),
            repeat("cc", 32),
            opened.accountInclusionRoot(),
            opened.accountsLtHashChecksum(),
            null,
            new byte[0],
            accountsLtHash,
            SOLANA_SIGNATURE_55,
            SOLANA_PROGRAM_42,
            repeat("dd", 32),
            repeat("ee", 32),
            repeat("12", 32),
            repeat("34", 32),
            SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
            repeat("aa", 32),
            repeat("56", 32),
            repeat("78", 32),
            new byte[0][],
            SolanaSccpProver.ZERO_HASH_V1,
            SolanaSccpProver.ZERO_HASH_V1);

    final SolanaSccpProver.AccountsLtHashProofRequest request =
        SolanaSccpProver.buildAccountsLtHashProofRequest(witness, opened);
    final byte[] mismatchedWitnessLtHash = Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    mismatchedWitnessLtHash[0] ^= 0x01;
    final IllegalArgumentException mismatchedWitnessAccountsLtHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildAccountsLtHashProofRequest(
                    witnessWithAccountsLtHash(witness, mismatchedWitnessLtHash), opened));
    assertTrue(mismatchedWitnessAccountsLtHash.getMessage().contains("accountsLtHash"));

    assertEquals(1, request.version());
    assertEquals("stark-fri-v1", request.proofFamily());
    assertEquals(SolanaSccpProver.ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1, request.circuitId());
    assertEquals("fastpq-lane-balanced", request.parameterSet());
    assertEquals(SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1, request.sourceStateVerifierId());
    assertEquals("0x" + repeat("aa", 32), request.sourceStateVerifierHash());
    assertEquals(
        SolanaSccpProver.accountsLtHashProofPublicInputsHash(
            SolanaSccpProver.DOMAIN_SOLANA,
            witness.finalizedSlot(),
            witness.parentSlot(),
            witness.bankSignatureCount(),
            witness.parentBankHash(),
            witness.bankHash(),
            witness.blockhash(),
            witness.bankHashHardForkData(),
            witness.transactionStatusRoot(),
            witness.accountInclusionRoot(),
            witness.accountsLtHashChecksum(),
            witness.accountsLtHash()),
        request.accountsLtHashProofPublicInputsHash());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            SolanaSccpProver.canonicalAccountsLtHashProofPublicInputsBytes(
                SolanaSccpProver.DOMAIN_SOLANA,
                witness.finalizedSlot(),
                witness.parentSlot(),
                witness.bankSignatureCount(),
                witness.parentBankHash(),
                repeat("44", 32),
                witness.blockhash(),
                witness.bankHashHardForkData(),
                witness.transactionStatusRoot(),
                witness.accountInclusionRoot(),
                witness.accountsLtHashChecksum(),
                witness.accountsLtHash()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            SolanaSccpProver.accountsLtHashProofPublicInputsHash(
                SolanaSccpProver.DOMAIN_SOLANA,
                witness.finalizedSlot(),
                witness.parentSlot(),
                witness.bankSignatureCount(),
                witness.parentBankHash(),
                witness.bankHash(),
                witness.blockhash(),
                witness.bankHashHardForkData(),
                witness.transactionStatusRoot(),
                witness.accountInclusionRoot(),
                repeat("44", 32),
                witness.accountsLtHash()));
    assertEquals(
        SolanaSccpProver.openedAccountsLtHashContributionsHash(opened),
        request.openedAccountsLtHashContributionsHash());
    assertEquals(
        SolanaSccpProver.openedAccountsLtHashResidualChecksum(opened),
        request.openedAccountsLtHashResidualChecksum());
    assertArrayEquals(
        SolanaSccpProver.canonicalAccountsLtHashCommitmentBytes(witness, opened),
        request.accountCommitmentBytes());
    assertArrayEquals(
        SolanaSccpProver.canonicalAccountsLtHashVerificationContextBytes(witness, opened),
        request.verificationContextBytes());
    assertEquals(
        SolanaSccpProver.accountsLtHashPublicInputColumns(witness, opened),
        request.publicInputColumns());
    assertEquals(SOLANA_MAINNET_GENESIS_PUBLIC_INPUT, request.publicInputColumns().get(1).get(0));
    assertEquals(request.openedAccountsLtHashContributionsHash(), request.publicInputColumns().get(12).get(0));
    assertEquals(request.openedAccountsLtHashResidualChecksum(), request.publicInputColumns().get(13).get(0));
    assertTrue(
        new String(request.schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("opened_accounts_lt_hash_residual_checksum"));
    assertTrue(
        new String(request.schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("mainnet_genesis_hash"));
    assertTrue(
        new String(request.schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("source_state_verifier_id"));
    assertTrue(
        new String(request.schemaDescriptor(), StandardCharsets.UTF_8)
            .contains(SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1));
    assertTrue(
        new String(request.schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("source_state_verifier_hash"));
    assertTrue(containsBytes(request.schemaDescriptor(), hexBytes(request.sourceStateVerifierHash())));
    assertEquals(
        Arrays.asList(
            "sccp:solana:accounts-lt:v1:statement",
            "sccp:solana:accounts-lt:v1:accounts",
            "sccp:solana:accounts-lt:v1:opened-contributions",
            "sccp:solana:accounts-lt:v1:residual",
            "sccp:solana:accounts-lt:v1:context"),
        Arrays.asList(
            request.fastpqTransitions().get(0).key(),
            request.fastpqTransitions().get(1).key(),
            request.fastpqTransitions().get(2).key(),
            request.fastpqTransitions().get(3).key(),
            request.fastpqTransitions().get(4).key()));
    assertEquals("0x" + repeat("c0", 32), request.fastpqPublicInputs().oldRoot());
    assertEquals(bankHash, request.fastpqPublicInputs().newRoot());
    final SolanaSccpProver.SourceStateVerificationProof wrappedProof =
        SolanaSccpProver.wrapSourceStateVerificationProof(new byte[] {1, 2, 3}, request);
    assertEquals(request.version(), wrappedProof.version());
    assertEquals(request.proofFamily(), wrappedProof.proofFamily());
    assertEquals(request.circuitId(), wrappedProof.circuitId());
    assertArrayEquals(new byte[] {1, 2, 3}, wrappedProof.proofBytes());
    assertEquals("AQID", wrappedProof.proofBase64());
    final byte[] exposedProofBytes = wrappedProof.proofBytes();
    exposedProofBytes[0] = 9;
    assertArrayEquals(new byte[] {1, 2, 3}, wrappedProof.proofBytes());
    assertEquals("AQID", wrappedProof.proofBase64());
    final SolanaSccpProver.AccountsLtHashProofRequest[] seenAccountsRequest =
        new SolanaSccpProver.AccountsLtHashProofRequest[1];
    final SolanaSccpProver.SourceStateProver sourceStateProver =
        new SolanaSccpProver.SourceStateProver(
            linkedRequest -> {
              seenAccountsRequest[0] = linkedRequest;
              assertEquals(
                  SolanaSccpProver.ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
                  linkedRequest.circuitId());
              return new byte[] {1, 2, 3};
            },
            null);
    final SolanaSccpProver.SourceStateVerificationProof linkedProof =
        sourceStateProver.proveAccountsLtHash(witness, opened);
    assertEquals(request.circuitId(), seenAccountsRequest[0].circuitId());
    assertEquals(request.circuitId(), linkedProof.circuitId());
    assertArrayEquals(new byte[] {1, 2, 3}, linkedProof.proofBytes());
    assertEquals("AQID", linkedProof.proofBase64());
    seenAccountsRequest[0] = null;
    sourceStateProver.proveAccountsLtHash(request);
    assertTrue(seenAccountsRequest[0] != request);
    assertArrayEquals(request.statementBytes(), seenAccountsRequest[0].statementBytes());
    assertArrayEquals(
        request.accountCommitmentBytes(), seenAccountsRequest[0].accountCommitmentBytes());
    final IllegalStateException missingSourceStateProver =
        assertThrows(
            IllegalStateException.class,
            () -> new SolanaSccpProver.SourceStateProver().proveAccountsLtHash(request));
    assertTrue(missingSourceStateProver.getMessage().contains("source-state prover is not linked"));
    assertEquals(
        SolanaSccpProver.accountsLtHashProofHash(
            new SolanaSccpProver.SourceStateVerificationProof(
                request.circuitId(), new byte[] {1, 2, 3})),
        SolanaSccpProver.accountsLtHashProofHash(wrappedProof));
    final IllegalArgumentException allZeroWrappedProof =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.wrapSourceStateVerificationProof(new byte[] {0, 0}, request));
    assertTrue(allZeroWrappedProof.getMessage().contains("all zero"));
    final byte[] oversizedProofBytes =
        filledBytes(SolanaSccpProver.SOURCE_STATE_MAX_PROOF_BYTES + 1, 1);
    final IllegalArgumentException oversizedWrappedProof =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.wrapSourceStateVerificationProof(oversizedProofBytes, request));
    assertTrue(oversizedWrappedProof.getMessage().contains("at most"));
    final IllegalArgumentException oversizedCanonicalProof =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.canonicalSourceStateVerificationProofBytes(
                    new SolanaSccpProver.SourceStateVerificationProof(
                        request.circuitId(), oversizedProofBytes)));
    assertTrue(oversizedCanonicalProof.getMessage().contains("at most"));
    final IllegalArgumentException oversizedProofFamily =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.canonicalSourceStateVerificationProofBytes(
                    new SolanaSccpProver.SourceStateVerificationProof(
                        1,
                        repeat("x", SolanaSccpProver.SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
                        request.circuitId(),
                        new byte[] {1})));
    assertTrue(oversizedProofFamily.getMessage().contains("proofFamily"));
    final IllegalArgumentException oversizedCircuitId =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.canonicalSourceStateVerificationProofBytes(
                    new SolanaSccpProver.SourceStateVerificationProof(
                        repeat("x", SolanaSccpProver.SOURCE_STATE_MAX_PROOF_LABEL_BYTES + 1),
                        new byte[] {1})));
    assertTrue(oversizedCircuitId.getMessage().contains("circuitId"));
    final List<List<String>> wrongGenesisColumns = mutableStringColumns(request.publicInputColumns());
    wrongGenesisColumns.get(1).set(0, "0x" + repeat("aa", 32));
    final SolanaSccpProver.AccountsLtHashProofRequest wrongGenesisRequest =
        new SolanaSccpProver.AccountsLtHashProofRequest(
            request.version(),
            request.proofFamily(),
            request.circuitId(),
            request.parameterSet(),
            request.sourceDomain(),
            request.finalizedSlot(),
            request.parentSlot(),
            request.sourceStateVerifierId(),
            request.sourceStateVerifierHash(),
            request.accountsLtHashProofPublicInputsHash(),
            request.openedAccountsLtHashContributionsHash(),
            request.openedAccountsLtHashResidualChecksum(),
            request.statementBytes(),
            request.accountCommitmentBytes(),
            request.verificationContextBytes(),
            request.schemaDescriptor(),
            wrongGenesisColumns,
            request.fastpqPublicInputs(),
            request.fastpqTransitions());
    final IllegalArgumentException wrongGenesisError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1}, wrongGenesisRequest));
    assertTrue(wrongGenesisError.getMessage().contains("mainnet_genesis_hash"));
    final List<List<String>> wrongResidualColumns =
        mutableStringColumns(request.publicInputColumns());
    wrongResidualColumns.get(13).set(0, "0x" + repeat("cc", 32));
    final SolanaSccpProver.AccountsLtHashProofRequest wrongResidualRequest =
        new SolanaSccpProver.AccountsLtHashProofRequest(
            request.version(),
            request.proofFamily(),
            request.circuitId(),
            request.parameterSet(),
            request.sourceDomain(),
            request.finalizedSlot(),
            request.parentSlot(),
            request.sourceStateVerifierId(),
            request.sourceStateVerifierHash(),
            request.accountsLtHashProofPublicInputsHash(),
            request.openedAccountsLtHashContributionsHash(),
            request.openedAccountsLtHashResidualChecksum(),
            request.statementBytes(),
            request.accountCommitmentBytes(),
            request.verificationContextBytes(),
            request.schemaDescriptor(),
            wrongResidualColumns,
            request.fastpqPublicInputs(),
            request.fastpqTransitions());
    final IllegalArgumentException wrongResidualError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1}, wrongResidualRequest));
    assertTrue(
        wrongResidualError
            .getMessage()
            .contains("opened_accounts_lt_hash_residual_checksum"));
    final IllegalArgumentException staleAccountsHashError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1},
                    accountsLtHashRequest(
                        request, "0x" + repeat("cc", 32), null, null, null)));
    assertTrue(
        staleAccountsHashError
            .getMessage()
            .contains("accountsLtHashProofPublicInputsHash"));
    final IllegalArgumentException wrongAccountsDsidError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1},
                    accountsLtHashRequest(
                        request,
                        null,
                        null,
                        new SolanaSccpProver.AccountsLtHashFastpqPublicInputs(
                            "0x" + repeat("00", 16),
                            request.fastpqPublicInputs().slot(),
                            request.fastpqPublicInputs().oldRoot(),
                            request.fastpqPublicInputs().newRoot(),
                            request.fastpqPublicInputs().permRoot(),
                            request.fastpqPublicInputs().txSetHash()),
                        null)));
    assertTrue(wrongAccountsDsidError.getMessage().contains("request.fastpqPublicInputs.dsid"));
    final IllegalArgumentException wrongAccountsTxSetError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1},
                    accountsLtHashRequest(
                        request,
                        null,
                        null,
                        new SolanaSccpProver.AccountsLtHashFastpqPublicInputs(
                            request.fastpqPublicInputs().dsid(),
                            request.fastpqPublicInputs().slot(),
                            request.fastpqPublicInputs().oldRoot(),
                            request.fastpqPublicInputs().newRoot(),
                            request.fastpqPublicInputs().permRoot(),
                            "0x" + repeat("cc", 32)),
                        null)));
    assertTrue(wrongAccountsTxSetError.getMessage().contains("request.fastpqPublicInputs.txSetHash"));
    final List<SolanaSccpProver.AccountsLtHashFastpqTransition> wrongTransitions =
        new ArrayList<>(request.fastpqTransitions());
    final SolanaSccpProver.AccountsLtHashFastpqTransition firstTransition =
        wrongTransitions.get(0);
    wrongTransitions.set(
        0,
        new SolanaSccpProver.AccountsLtHashFastpqTransition(
            firstTransition.key(),
            firstTransition.operation(),
            firstTransition.oldValue(),
            new byte[] {0}));
    final SolanaSccpProver.AccountsLtHashProofRequest wrongTransitionRequest =
        new SolanaSccpProver.AccountsLtHashProofRequest(
            request.version(),
            request.proofFamily(),
            request.circuitId(),
            request.parameterSet(),
            request.sourceDomain(),
            request.finalizedSlot(),
            request.parentSlot(),
            request.sourceStateVerifierId(),
            request.sourceStateVerifierHash(),
            request.accountsLtHashProofPublicInputsHash(),
            request.openedAccountsLtHashContributionsHash(),
            request.openedAccountsLtHashResidualChecksum(),
            request.statementBytes(),
            request.accountCommitmentBytes(),
            request.verificationContextBytes(),
            request.schemaDescriptor(),
            request.publicInputColumns(),
            request.fastpqPublicInputs(),
            wrongTransitions);
    final IllegalArgumentException wrongTransitionError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1}, wrongTransitionRequest));
    assertTrue(wrongTransitionError.getMessage().contains("canonical Solana source-state request"));
    final List<SolanaSccpProver.AccountsLtHashFastpqTransition> wrongOldValueTransitions =
        new ArrayList<>(request.fastpqTransitions());
    final SolanaSccpProver.AccountsLtHashFastpqTransition oldValueTransition =
        wrongOldValueTransitions.get(0);
    wrongOldValueTransitions.set(
        0,
        new SolanaSccpProver.AccountsLtHashFastpqTransition(
            oldValueTransition.key(),
            oldValueTransition.operation(),
            new byte[] {0},
            oldValueTransition.newValue()));
    final SolanaSccpProver.AccountsLtHashProofRequest wrongOldValueRequest =
        new SolanaSccpProver.AccountsLtHashProofRequest(
            request.version(),
            request.proofFamily(),
            request.circuitId(),
            request.parameterSet(),
            request.sourceDomain(),
            request.finalizedSlot(),
            request.parentSlot(),
            request.sourceStateVerifierId(),
            request.sourceStateVerifierHash(),
            request.accountsLtHashProofPublicInputsHash(),
            request.openedAccountsLtHashContributionsHash(),
            request.openedAccountsLtHashResidualChecksum(),
            request.statementBytes(),
            request.accountCommitmentBytes(),
            request.verificationContextBytes(),
            request.schemaDescriptor(),
            request.publicInputColumns(),
            request.fastpqPublicInputs(),
            wrongOldValueTransitions);
    final IllegalArgumentException wrongOldValueError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1}, wrongOldValueRequest));
    assertTrue(wrongOldValueError.getMessage().contains("canonical Solana source-state request"));
    final SolanaSccpProver.AccountsLtHashProofRequest missingStatementRequest =
        new SolanaSccpProver.AccountsLtHashProofRequest(
            request.version(),
            request.proofFamily(),
            request.circuitId(),
            request.parameterSet(),
            request.sourceDomain(),
            request.finalizedSlot(),
            request.parentSlot(),
            request.sourceStateVerifierId(),
            request.sourceStateVerifierHash(),
            request.accountsLtHashProofPublicInputsHash(),
            request.openedAccountsLtHashContributionsHash(),
            request.openedAccountsLtHashResidualChecksum(),
            new byte[0],
            request.accountCommitmentBytes(),
            request.verificationContextBytes(),
            request.schemaDescriptor(),
            request.publicInputColumns(),
            request.fastpqPublicInputs(),
            request.fastpqTransitions());
    final IllegalArgumentException missingStatementError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {1}, missingStatementRequest));
    assertTrue(missingStatementError.getMessage().contains("request.statementBytes"));
    final boolean[] rejectedAccountsCallbackRan = new boolean[] {false};
    final SolanaSccpProver.SourceStateProver guardingAccountsProver =
        new SolanaSccpProver.SourceStateProver(
            requestForCallback -> {
              rejectedAccountsCallbackRan[0] = true;
              return new byte[] {1};
            },
            null);
    final IllegalArgumentException rejectedAccountsRequest =
        assertThrows(
            IllegalArgumentException.class,
            () -> guardingAccountsProver.proveAccountsLtHash(missingStatementRequest));
    assertTrue(rejectedAccountsRequest.getMessage().contains("request.statementBytes"));
    assertTrue(!rejectedAccountsCallbackRan[0]);

    final IllegalArgumentException zeroHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildAccountsLtHashProofRequest(
                    new SolanaSccpProver.WitnessInput(
                        witness.targetDomain(),
                        witness.mainnetGenesisHash(),
                        witness.finalizedSlot(),
                        witness.parentSlot(),
                        witness.bankSignatureCount(),
                        witness.parentBankHash(),
                        witness.blockhash(),
                        witness.bankHash(),
                        witness.transactionStatusRoot(),
                        witness.messageProofHash(),
                        witness.accountInclusionRoot(),
                        witness.accountsLtHashChecksum(),
                        witness.accountsLtHashProofPublicInputsHash(),
                        witness.bankHashHardForkData(),
                        witness.accountsLtHash(),
                        witness.transactionSignature(),
                        witness.emitterProgramId(),
                        witness.messageId(),
                        witness.payloadHash(),
                        witness.commitmentRoot(),
                        witness.sourceEventDigest(),
                        witness.sourceStateVerifierId(),
                        SolanaSccpProver.ZERO_HASH_V1,
                        witness.statementHash(),
                        witness.destinationBindingHash(),
                        witness.inclusionBranch(),
                        witness.sourceAdapterDeploymentHash(),
                        witness.sourceAdapterDeploymentReceiptHash()),
                    opened));
    assertTrue(zeroHash.getMessage().contains("sourceStateVerifierHash"));
    final IllegalArgumentException templateHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildAccountsLtHashProofRequest(
                    new SolanaSccpProver.WitnessInput(
                        witness.targetDomain(),
                        witness.mainnetGenesisHash(),
                        witness.finalizedSlot(),
                        witness.parentSlot(),
                        witness.bankSignatureCount(),
                        witness.parentBankHash(),
                        witness.blockhash(),
                        witness.bankHash(),
                        witness.transactionStatusRoot(),
                        witness.messageProofHash(),
                        witness.accountInclusionRoot(),
                        witness.accountsLtHashChecksum(),
                        witness.accountsLtHashProofPublicInputsHash(),
                        witness.bankHashHardForkData(),
                        witness.accountsLtHash(),
                        witness.transactionSignature(),
                        witness.emitterProgramId(),
                        witness.messageId(),
                        witness.payloadHash(),
                        witness.commitmentRoot(),
                        witness.sourceEventDigest(),
                        witness.sourceStateVerifierId(),
                        SolanaSccpProver.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
                        witness.statementHash(),
                        witness.destinationBindingHash(),
                        witness.inclusionBranch(),
                        witness.sourceAdapterDeploymentHash(),
                        witness.sourceAdapterDeploymentReceiptHash()),
                    opened));
    assertTrue(templateHash.getMessage().contains("Solana template verifier hash"));
    final IllegalArgumentException badBankHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildAccountsLtHashProofRequest(
                    new SolanaSccpProver.WitnessInput(
                        witness.targetDomain(),
                        witness.mainnetGenesisHash(),
                        witness.finalizedSlot(),
                        witness.parentSlot(),
                        witness.bankSignatureCount(),
                        witness.parentBankHash(),
                        witness.blockhash(),
                        repeat("cc", 32),
                        witness.transactionStatusRoot(),
                        witness.messageProofHash(),
                        witness.accountInclusionRoot(),
                        witness.accountsLtHashChecksum(),
                        witness.accountsLtHashProofPublicInputsHash(),
                        witness.bankHashHardForkData(),
                        witness.accountsLtHash(),
                        witness.transactionSignature(),
                        witness.emitterProgramId(),
                        witness.messageId(),
                        witness.payloadHash(),
                        witness.commitmentRoot(),
                        witness.sourceEventDigest(),
                        witness.sourceStateVerifierId(),
                        witness.sourceStateVerifierHash(),
                        witness.statementHash(),
                        witness.destinationBindingHash(),
                        witness.inclusionBranch(),
                        witness.sourceAdapterDeploymentHash(),
                        witness.sourceAdapterDeploymentReceiptHash()),
                    opened));
    assertTrue(badBankHash.getMessage().contains("bankHash"));
  }

  @Test
  public void buildsSolanaFullLightClientAuditRoleProofRequests() {
    final SolanaSccpProver.AccountOpeningInput voteOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x31, 32),
            hexBytes(SolanaSccpProver.VOTE_PROGRAM_ID),
            "1000000",
            "0",
            false,
            "0x" + repeat("91", 32));
    final SolanaSccpProver.AccountOpeningInput stakeOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x32, 32),
            hexBytes(SolanaSccpProver.STAKE_PROGRAM_ID),
            "2000000",
            "0",
            false,
            "0x" + repeat("92", 32));
    final SolanaSccpProver.AccountOpeningInput stakeHistoryOpening =
        new SolanaSccpProver.AccountOpeningInput(
            hexBytes(SolanaSccpProver.STAKE_HISTORY_SYSVAR_ID),
            hexBytes(SolanaSccpProver.SYSVAR_PROGRAM_ID),
            "1",
            "0",
            false,
            "0x" + repeat("93", 32));
    final SolanaSccpProver.AccountOpeningInput unopenedOpening =
        new SolanaSccpProver.AccountOpeningInput(
            repeatByte((byte) 0x34, 32),
            hexBytes(SolanaSccpProver.STAKE_PROGRAM_ID),
            "3000000",
            "0",
            false,
            "0x" + repeat("94", 32));
    final byte[] voteRawData = new byte[] {1, 2, 3};
    final byte[] stakeRawData = new byte[] {4, 5, 6};
    final byte[] stakeHistoryRawData = new byte[] {7, 8, 9};
    final byte[] unopenedRawData = new byte[] {10, 11, 12};
    final byte[] accountsLtHash =
        SolanaSccpProver.accountsLtHashFromOpenings(
            new SolanaSccpProver.AccountOpeningInput[] {
              voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening
            },
            new byte[][] {voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData});
    final String parentBankHash = repeat("c0", 32);
    final String blockhash = repeat("42", 32);
    final String bankHash =
        SolanaSccpProver.agaveBankHash(parentBankHash, "8", blockhash, accountsLtHash);
    final SolanaSccpProver.OpenedAccountsLtHashContributionsInput opened =
        new SolanaSccpProver.OpenedAccountsLtHashContributionsInput(
            SolanaSccpProver.DOMAIN_SOLANA,
            "1296096",
            "0x" + repeat("77", 32),
            SolanaSccpProver.accountsLtHashChecksum(accountsLtHash),
            accountsLtHash,
            new SolanaSccpProver.AccountOpeningInput[] {voteOpening},
            new byte[][] {voteRawData},
            new byte[0][],
            new SolanaSccpProver.AccountOpeningInput[] {stakeOpening},
            new byte[][] {stakeRawData},
            new byte[0][],
            stakeHistoryOpening,
            stakeHistoryRawData,
            new byte[0]);
    final String sourceStateVerifierHash = repeat("99", 32);
    final String sourceTrustAnchorHash = repeat("44", 32);
    final String consensusVerifierHash = repeat("55", 32);
    final String messageInclusionVerifierHash = repeat("66", 32);
    final String finalityPolicyHash = repeat("88", 32);
    final String deploymentReceiptHash = repeat("aa", 32);
    final String towerVerifierHash = repeat("b1", 32);
    final String accountsdbVerifierHash = repeat("c2", 32);
    final String bankVerifierHash = repeat("d3", 32);
    final String sourceVerifierMaterialHash =
        SourceSccpProofs.sourceVerifierMaterialHash(
            SourceSccpProofs.DOMAIN_SOL,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            null,
            null,
            null,
            null,
            null);
    final String sourceAdapterDeploymentHash =
        SourceSccpProofs.sourceAdapterEngineDeploymentHash(
            SourceSccpProofs.DOMAIN_SOL,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            SourceSccpProofs.DOMAIN_SORA,
            null,
            sourceStateVerifierHash,
            null,
            null,
            null,
            null,
            null,
            towerVerifierHash,
            accountsdbVerifierHash,
            bankVerifierHash,
            null,
            null,
            null);
    final String fullLightClientGateHash =
        SourceSccpProofs.solanaFullLightClientGateHash(
            SourceSccpProofs.DOMAIN_SOL,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            towerVerifierHash,
            accountsdbVerifierHash,
            bankVerifierHash,
            SourceSccpProofs.DOMAIN_SORA,
            null,
            sourceStateVerifierHash,
            null,
            null,
            null,
            null,
            null);
    final IllegalArgumentException duplicatedGateAuditHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SourceSccpProofs.solanaFullLightClientGateHash(
                    SourceSccpProofs.DOMAIN_SOL,
                    sourceTrustAnchorHash,
                    consensusVerifierHash,
                    messageInclusionVerifierHash,
                    finalityPolicyHash,
                    deploymentReceiptHash,
                    accountsdbVerifierHash,
                    accountsdbVerifierHash,
                    bankVerifierHash,
                    SourceSccpProofs.DOMAIN_SORA,
                    null,
                    sourceStateVerifierHash,
                    null,
                    null,
                    null,
                    null,
                    null));
    assertTrue(duplicatedGateAuditHash.getMessage().contains("role-separated"));
    final IllegalArgumentException reusedGateDeploymentReceiptHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SourceSccpProofs.solanaFullLightClientGateHash(
                    SourceSccpProofs.DOMAIN_SOL,
                    sourceTrustAnchorHash,
                    consensusVerifierHash,
                    messageInclusionVerifierHash,
                    finalityPolicyHash,
                    deploymentReceiptHash,
                    deploymentReceiptHash,
                    accountsdbVerifierHash,
                    bankVerifierHash,
                    SourceSccpProofs.DOMAIN_SORA,
                    null,
                    sourceStateVerifierHash,
                    null,
                    null,
                    null,
                    null,
                    null));
    assertTrue(reusedGateDeploymentReceiptHash.getMessage().contains("source-adapter material"));
    final SolanaSccpProver.WitnessInput witness =
        new SolanaSccpProver.WitnessInput(
            SolanaSccpProver.DOMAIN_SORA,
            SolanaSccpProver.MAINNET_GENESIS_HASH,
            opened.finalizedSlot(),
            "1296095",
            "8",
            parentBankHash,
            blockhash,
            bankHash,
            repeat("bb", 32),
            repeat("cc", 32),
            opened.accountInclusionRoot(),
            opened.accountsLtHashChecksum(),
            null,
            new byte[0],
            accountsLtHash,
            SOLANA_SIGNATURE_55,
            SOLANA_PROGRAM_42,
            repeat("dd", 32),
            repeat("ee", 32),
            repeat("12", 32),
            repeat("34", 32),
            SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
            sourceStateVerifierHash,
            repeat("56", 32),
            repeat("78", 32),
            new byte[0][],
            sourceAdapterDeploymentHash,
            deploymentReceiptHash);
    final SolanaSccpProver.FullLightClientAuditProofInput input =
        new SolanaSccpProver.FullLightClientAuditProofInput(
            witness,
            opened,
            new SolanaSccpProver.SourceStateVerificationProof(
                SolanaSccpProver.ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
                new byte[] {1, 2, 3, 4}),
            "1296065",
            towerVoteSlots(),
            null,
            repeat("13", 32),
            repeat("14", 32),
            repeat("15", 32),
            repeat("16", 32),
            repeat("17", 32),
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            towerVerifierHash,
            accountsdbVerifierHash,
            bankVerifierHash,
            null,
            sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash,
            fullLightClientGateHash,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null);

    final SolanaSccpProver.FullLightClientAuditProofRequests requests =
        SolanaSccpProver.buildFullLightClientAuditProofRequests(input);
    final byte[] mismatchedAuditWitnessLtHash = Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    mismatchedAuditWitnessLtHash[0] ^= 0x01;
    final IllegalArgumentException mismatchedAuditAccountsLtHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithWitness(
                        input, witnessWithAccountsLtHash(witness, mismatchedAuditWitnessLtHash))));
    assertTrue(mismatchedAuditAccountsLtHash.getMessage().contains("accountsLtHash"));
    final String requestHashReusedTowerVerifierHash = requests.towerReplay().auditStatementHash();
    final String requestHashReusedDeploymentHash =
        SourceSccpProofs.sourceAdapterEngineDeploymentHash(
            SourceSccpProofs.DOMAIN_SOL,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            SourceSccpProofs.DOMAIN_SORA,
            null,
            sourceStateVerifierHash,
            null,
            null,
            null,
            null,
            null,
            requestHashReusedTowerVerifierHash,
            accountsdbVerifierHash,
            bankVerifierHash,
            null,
            null,
            null);
    final String requestHashReusedGateHash =
        SourceSccpProofs.solanaFullLightClientGateHash(
            SourceSccpProofs.DOMAIN_SOL,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            requestHashReusedTowerVerifierHash,
            accountsdbVerifierHash,
            bankVerifierHash,
            SourceSccpProofs.DOMAIN_SORA,
            null,
            sourceStateVerifierHash,
            null,
            null,
            null,
            null,
            null);
    final SolanaSccpProver.FullLightClientAuditProofInput requestHashReusedInput =
        auditInputWithGateHash(
            auditInputWithSourceAdapterDeploymentHash(
                auditInputWithVerifierHashes(
                    auditInputWithWitness(
                        input, witnessWithDeploymentHash(witness, requestHashReusedDeploymentHash)),
                    requestHashReusedTowerVerifierHash,
                    accountsdbVerifierHash,
                    bankVerifierHash),
                requestHashReusedDeploymentHash),
            requestHashReusedGateHash);
    final IllegalArgumentException requestHashReused =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.buildTowerReplayProofRequest(requestHashReusedInput));
    assertTrue(requestHashReused.getMessage().contains("role-separated"));
    final List<List<String>> expectedTowerReplayColumns =
        Arrays.asList(
            col("0x0100000000000000000000000000000000000000000000000000000000000000"),
            col("0x0300000000000000000000000000000000000000000000000000000000000000"),
            col(SOLANA_MAINNET_GENESIS_PUBLIC_INPUT),
            col("0xe0c6130000000000000000000000000000000000000000000000000000000000"),
            col("0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"),
            col("0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3"),
            col("0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"),
            col("0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"),
            col("0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"),
            col("0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1"),
            col("0x0300000000000000000000000000000000000000000000000000000000000000"),
            col("0xc1c6130000000000000000000000000000000000000000000000000000000000"),
            col("0xdfc6130000000000000000000000000000000000000000000000000000000000"),
            col("0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"),
            col("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"),
            col("0x922a426e06d6263986a0c9ff0f956f5429288c9c1310cb67fbaf30918de58b40"),
            col("0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"),
            col("0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"),
            col("0x1313131313131313131313131313131313131313131313131313131313131313"),
            col("0x1414141414141414141414141414141414141414141414141414141414141414"),
            col("0x1515151515151515151515151515151515151515151515151515151515151515"),
            col("0x1616161616161616161616161616161616161616161616161616161616161616"),
            col("0x1717171717171717171717171717171717171717171717171717171717171717"),
            col("0x7777777777777777777777777777777777777777777777777777777777777777"));
    final List<List<String>> expectedAccountsdbColumns =
        Arrays.asList(
            col("0x0200000000000000000000000000000000000000000000000000000000000000"),
            col("0x0300000000000000000000000000000000000000000000000000000000000000"),
            col(SOLANA_MAINNET_GENESIS_PUBLIC_INPUT),
            col("0xe0c6130000000000000000000000000000000000000000000000000000000000"),
            col("0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"),
            col("0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0"),
            col("0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"),
            col("0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"),
            col("0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"),
            col("0xc2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"),
            col("0x0300000000000000000000000000000000000000000000000000000000000000"),
            col("0xc1c6130000000000000000000000000000000000000000000000000000000000"),
            col("0xdfc6130000000000000000000000000000000000000000000000000000000000"),
            col("0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"),
            col("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"),
            col("0x7777777777777777777777777777777777777777777777777777777777777777"),
            col("0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"),
            col("0xc1b7c880344a2551d0842848f68b8519027e8b228a4c92c4e754141821d63810"),
            col("0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9"),
            col("0x336bb79a5e96c331ddca555aedde346438de4ca1b227ae09f7faaa5e0e455be0"),
            col("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"));
    final List<List<String>> expectedBankForkChoiceColumns =
        Arrays.asList(
            col("0x0300000000000000000000000000000000000000000000000000000000000000"),
            col("0x0300000000000000000000000000000000000000000000000000000000000000"),
            col(SOLANA_MAINNET_GENESIS_PUBLIC_INPUT),
            col("0xe0c6130000000000000000000000000000000000000000000000000000000000"),
            col("0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"),
            col("0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8"),
            col("0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"),
            col("0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"),
            col("0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"),
            col("0xd3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3"),
            col("0x0300000000000000000000000000000000000000000000000000000000000000"),
            col("0xc1c6130000000000000000000000000000000000000000000000000000000000"),
            col("0xdfc6130000000000000000000000000000000000000000000000000000000000"),
            col("0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"),
            col("0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"),
            col("0xc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0"),
            col("0x46bf9f58208a9c61b931640824eb13d636d3af5b0268cce866c958367bd6a451"),
            col("0x4242424242424242424242424242424242424242424242424242424242424242"),
            col("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            col("0x7777777777777777777777777777777777777777777777777777777777777777"),
            col("0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"),
            col("0x0800000000000000000000000000000000000000000000000000000000000000"),
            col("0x1d2a51ef7c068fe46c9f588c252ce9cea8b66d87453bf73c9920005802e738bc"),
            col("0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"),
            col("0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"));

    assertEquals(
        SolanaSccpProver.TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
        requests.towerReplay().circuitId());
    assertEquals(
        "0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3",
        requests.towerReplay().auditStatementHash());
    assertEquals(777, requests.towerReplay().statementBytes().length);
    assertEquals(expectedTowerReplayColumns, requests.towerReplay().publicInputColumns());
    assertEquals(
        SolanaSccpProver.FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
        requests.fullAccountsdbLattice().circuitId());
    assertEquals(
        "0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0",
        requests.fullAccountsdbLattice().auditStatementHash());
    assertEquals(440, requests.fullAccountsdbLattice().statementBytes().length);
    assertEquals(
        expectedAccountsdbColumns, requests.fullAccountsdbLattice().publicInputColumns());
    assertEquals(
        SolanaSccpProver.BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
        requests.bankForkChoice().circuitId());
    assertEquals(
        "0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8",
        requests.bankForkChoice().auditStatementHash());
    assertEquals(509, requests.bankForkChoice().statementBytes().length);
    assertEquals(expectedBankForkChoiceColumns, requests.bankForkChoice().publicInputColumns());
    assertEquals(
        Arrays.asList(input.witnessInput().accountInclusionRoot()),
        requests.bankForkChoice().publicInputColumns().get(19));
    assertTrue(
        new String(requests.towerReplay().schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("mainnet_genesis_hash"));
    assertEquals(
        Arrays.asList("0x1515151515151515151515151515151515151515151515151515151515151515"),
        requests.towerReplay().publicInputColumns().get(20));
    assertEquals(
        Arrays.asList("0x1717171717171717171717171717171717171717171717171717171717171717"),
        requests.towerReplay().publicInputColumns().get(22));
    assertEquals(
        Arrays.asList(input.witnessInput().accountInclusionRoot()),
        requests.towerReplay().publicInputColumns().get(23));
    assertTrue(
        new String(requests.towerReplay().schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("stake_account_state_hash"));
    assertTrue(
        new String(requests.towerReplay().schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("stake_history_sysvar_account_hash"));
    assertTrue(
        new String(requests.towerReplay().schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("account_inclusion_root"));
    assertTrue(
        new String(requests.bankForkChoice().schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("account_inclusion_root"));
    assertTrue(
        new String(requests.bankForkChoice().schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("bank_hash_hard_fork_data_hash"));
    assertEquals(
        3,
        new HashSet<>(
                Arrays.asList(
                    requests.towerReplay().auditStatementHash(),
                    requests.fullAccountsdbLattice().auditStatementHash(),
                    requests.bankForkChoice().auditStatementHash()))
            .size());
    assertEquals(fullLightClientGateHash, requests.towerReplay().fullLightClientGateHash());
    assertEquals(
        SolanaSccpProver.fullLightClientAuditFinalityContextHash(input),
        requests.towerReplay().finalityContextHash());
    assertEquals(
        SolanaSccpProver.fullLightClientAuditVoteMessageHash(input),
        requests.towerReplay().voteMessageHash());
    assertEquals(
        SolanaSccpProver.accountsLtHashProofHash(input.accountsLtHashProof()),
        requests.towerReplay().accountsLtHashProofHash());
    for (final SolanaSccpProver.FullLightClientAuditProofRequest request :
        new SolanaSccpProver.FullLightClientAuditProofRequest[] {
          requests.towerReplay(), requests.fullAccountsdbLattice(), requests.bankForkChoice()
        }) {
      final SolanaSccpProver.SourceStateVerificationProof proofCapsule =
          SolanaSccpProver.wrapSourceStateVerificationProof(new byte[] {9, 8, 7}, request);
      assertEquals(request.version(), proofCapsule.version());
      assertEquals(request.proofFamily(), proofCapsule.proofFamily());
      assertEquals(request.circuitId(), proofCapsule.circuitId());
      assertArrayEquals(new byte[] {9, 8, 7}, proofCapsule.proofBytes());
      assertEquals("CQgH", proofCapsule.proofBase64());
      assertTrue(SolanaSccpProver.canonicalSourceStateVerificationProofBytes(proofCapsule).length > 0);
      assertThrows(
          IllegalArgumentException.class,
          () -> SolanaSccpProver.accountsLtHashProofHash(proofCapsule));
      final byte[] exposedProofBytes = proofCapsule.proofBytes();
      exposedProofBytes[0] = 1;
      assertArrayEquals(new byte[] {9, 8, 7}, proofCapsule.proofBytes());
      assertEquals("CQgH", proofCapsule.proofBase64());
    }
    final StringBuilder seenRoles = new StringBuilder();
    final SolanaSccpProver.SourceStateProver sourceStateProver =
        new SolanaSccpProver.SourceStateProver(
            null,
            request -> {
              if (seenRoles.length() > 0) {
                seenRoles.append(",");
              }
              seenRoles.append(request.role());
              return new byte[] {9, 8, 7};
            });
    final SolanaSccpProver.FullLightClientAuditProofs linkedProofs =
        sourceStateProver.proveFullLightClientAudit(input);
    assertEquals("tower_replay,full_accountsdb_lattice,bank_fork_choice", seenRoles.toString());
    assertEquals(
        SolanaSccpProver.TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
        linkedProofs.towerReplay().circuitId());
    assertEquals(
        SolanaSccpProver.FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
        linkedProofs.fullAccountsdbLattice().circuitId());
    assertEquals(
        SolanaSccpProver.BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
        linkedProofs.bankForkChoice().circuitId());
    assertEquals("CQgH", linkedProofs.bankForkChoice().proofBase64());
    final SolanaSccpProver.FullLightClientAuditProofRequest[] seenAuditRequest =
        new SolanaSccpProver.FullLightClientAuditProofRequest[1];
    final SolanaSccpProver.SourceStateProver snapshotCheckingProver =
        new SolanaSccpProver.SourceStateProver(
            null,
            request -> {
              seenAuditRequest[0] = request;
              return new byte[] {9, 8, 7};
            });
    snapshotCheckingProver.proveFullLightClientAudit(requests.towerReplay());
    assertTrue(seenAuditRequest[0] != requests.towerReplay());
    assertArrayEquals(requests.towerReplay().statementBytes(), seenAuditRequest[0].statementBytes());
    assertArrayEquals(
        requests.towerReplay().verificationContextBytes(),
        seenAuditRequest[0].verificationContextBytes());
    final IllegalStateException missingSourceStateProver =
        assertThrows(
            IllegalStateException.class,
            () -> new SolanaSccpProver.SourceStateProver().proveFullLightClientAudit(input));
    assertTrue(missingSourceStateProver.getMessage().contains("source-state prover is not linked"));
    final SolanaSccpProver.FullLightClientAuditProofRequest towerReplay =
        requests.towerReplay();
    final SolanaSccpProver.FullLightClientAuditProofRequest bankForkChoice =
        requests.bankForkChoice();
    final List<List<String>> wrongAuditGenesisColumns =
        mutableStringColumns(bankForkChoice.publicInputColumns());
    wrongAuditGenesisColumns.get(2).set(0, "0x" + repeat("aa", 32));
    final SolanaSccpProver.FullLightClientAuditProofRequest wrongAuditGenesisRequest =
        new SolanaSccpProver.FullLightClientAuditProofRequest(
            bankForkChoice.version(),
            bankForkChoice.proofFamily(),
            bankForkChoice.circuitId(),
            bankForkChoice.parameterSet(),
            bankForkChoice.role(),
            bankForkChoice.roleCode(),
            bankForkChoice.sourceDomain(),
            bankForkChoice.finalizedSlot(),
            bankForkChoice.verifierId(),
            bankForkChoice.verifierHash(),
            bankForkChoice.sourceStateVerifierId(),
            bankForkChoice.sourceStateVerifierHash(),
            bankForkChoice.sourceVerifierMaterialHash(),
            bankForkChoice.sourceAdapterDeploymentHash(),
            bankForkChoice.fullLightClientGateHash(),
            bankForkChoice.finalityContextHash(),
            bankForkChoice.voteMessageHash(),
            bankForkChoice.accountsLtHashProofHash(),
            bankForkChoice.auditStatementHash(),
            bankForkChoice.statementBytes(),
            bankForkChoice.verificationContextBytes(),
            bankForkChoice.schemaDescriptor(),
            wrongAuditGenesisColumns,
            bankForkChoice.fastpqPublicInputs(),
            bankForkChoice.fastpqTransitions());
    final IllegalArgumentException wrongAuditGenesisError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7}, wrongAuditGenesisRequest));
    assertTrue(wrongAuditGenesisError.getMessage().contains("mainnet_genesis_hash"));
    final List<List<String>> wrongAuditStatementColumns =
        mutableStringColumns(towerReplay.publicInputColumns());
    wrongAuditStatementColumns.get(5).set(0, "0x" + repeat("cc", 32));
    final SolanaSccpProver.FullLightClientAuditProofRequest wrongAuditStatementRequest =
        new SolanaSccpProver.FullLightClientAuditProofRequest(
            towerReplay.version(),
            towerReplay.proofFamily(),
            towerReplay.circuitId(),
            towerReplay.parameterSet(),
            towerReplay.role(),
            towerReplay.roleCode(),
            towerReplay.sourceDomain(),
            towerReplay.finalizedSlot(),
            towerReplay.verifierId(),
            towerReplay.verifierHash(),
            towerReplay.sourceStateVerifierId(),
            towerReplay.sourceStateVerifierHash(),
            towerReplay.sourceVerifierMaterialHash(),
            towerReplay.sourceAdapterDeploymentHash(),
            towerReplay.fullLightClientGateHash(),
            towerReplay.finalityContextHash(),
            towerReplay.voteMessageHash(),
            towerReplay.accountsLtHashProofHash(),
            towerReplay.auditStatementHash(),
            towerReplay.statementBytes(),
            towerReplay.verificationContextBytes(),
            towerReplay.schemaDescriptor(),
            wrongAuditStatementColumns,
            towerReplay.fastpqPublicInputs(),
            towerReplay.fastpqTransitions());
    final IllegalArgumentException wrongAuditStatementError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7}, wrongAuditStatementRequest));
    assertTrue(wrongAuditStatementError.getMessage().contains("audit_statement_hash"));
    final IllegalArgumentException staleAuditHashError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7},
                    fullLightClientAuditRequest(
                        towerReplay, "0x" + repeat("cc", 32), null, null, null)));
    assertTrue(staleAuditHashError.getMessage().contains("request.auditStatementHash"));
    final IllegalArgumentException wrongAuditDsidError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7},
                    fullLightClientAuditRequest(
                        towerReplay,
                        null,
                        null,
                        new SolanaSccpProver.FullLightClientAuditFastpqPublicInputs(
                            "0x" + repeat("00", 16),
                            towerReplay.fastpqPublicInputs().slot(),
                            towerReplay.fastpqPublicInputs().oldRoot(),
                            towerReplay.fastpqPublicInputs().newRoot(),
                            towerReplay.fastpqPublicInputs().permRoot(),
                            towerReplay.fastpqPublicInputs().txSetHash()),
                        null)));
    assertTrue(wrongAuditDsidError.getMessage().contains("request.fastpqPublicInputs.dsid"));
    final IllegalArgumentException wrongAuditTxSetError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7},
                    fullLightClientAuditRequest(
                        towerReplay,
                        null,
                        null,
                        new SolanaSccpProver.FullLightClientAuditFastpqPublicInputs(
                            towerReplay.fastpqPublicInputs().dsid(),
                            towerReplay.fastpqPublicInputs().slot(),
                            towerReplay.fastpqPublicInputs().oldRoot(),
                            towerReplay.fastpqPublicInputs().newRoot(),
                            towerReplay.fastpqPublicInputs().permRoot(),
                            "0x" + repeat("cc", 32)),
                        null)));
    assertTrue(wrongAuditTxSetError.getMessage().contains("request.fastpqPublicInputs.txSetHash"));
    final IllegalArgumentException reusedSourceStateVerifierError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7},
                    fullLightClientAuditRequestWithVerifierHash(
                        towerReplay, towerReplay.sourceStateVerifierHash())));
    assertTrue(reusedSourceStateVerifierError.getMessage().contains("role-separated"));
    final List<SolanaSccpProver.FullLightClientAuditFastpqTransition> wrongAuditTransitions =
        new ArrayList<>(towerReplay.fastpqTransitions());
    final SolanaSccpProver.FullLightClientAuditFastpqTransition firstAuditTransition =
        wrongAuditTransitions.get(0);
    wrongAuditTransitions.set(
        0,
        new SolanaSccpProver.FullLightClientAuditFastpqTransition(
            firstAuditTransition.key(),
            firstAuditTransition.operation(),
            firstAuditTransition.oldValue(),
            new byte[] {0}));
    final SolanaSccpProver.FullLightClientAuditProofRequest wrongAuditTransitionRequest =
        new SolanaSccpProver.FullLightClientAuditProofRequest(
            towerReplay.version(),
            towerReplay.proofFamily(),
            towerReplay.circuitId(),
            towerReplay.parameterSet(),
            towerReplay.role(),
            towerReplay.roleCode(),
            towerReplay.sourceDomain(),
            towerReplay.finalizedSlot(),
            towerReplay.verifierId(),
            towerReplay.verifierHash(),
            towerReplay.sourceStateVerifierId(),
            towerReplay.sourceStateVerifierHash(),
            towerReplay.sourceVerifierMaterialHash(),
            towerReplay.sourceAdapterDeploymentHash(),
            towerReplay.fullLightClientGateHash(),
            towerReplay.finalityContextHash(),
            towerReplay.voteMessageHash(),
            towerReplay.accountsLtHashProofHash(),
            towerReplay.auditStatementHash(),
            towerReplay.statementBytes(),
            towerReplay.verificationContextBytes(),
            towerReplay.schemaDescriptor(),
            towerReplay.publicInputColumns(),
            towerReplay.fastpqPublicInputs(),
            wrongAuditTransitions);
    final IllegalArgumentException wrongAuditTransitionError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7}, wrongAuditTransitionRequest));
    assertTrue(
        wrongAuditTransitionError
            .getMessage()
            .contains("canonical Solana source-state request"));
    final List<SolanaSccpProver.FullLightClientAuditFastpqTransition> wrongAuditOldValueTransitions =
        new ArrayList<>(towerReplay.fastpqTransitions());
    final SolanaSccpProver.FullLightClientAuditFastpqTransition oldValueAuditTransition =
        wrongAuditOldValueTransitions.get(0);
    wrongAuditOldValueTransitions.set(
        0,
        new SolanaSccpProver.FullLightClientAuditFastpqTransition(
            oldValueAuditTransition.key(),
            oldValueAuditTransition.operation(),
            new byte[] {0},
            oldValueAuditTransition.newValue()));
    final SolanaSccpProver.FullLightClientAuditProofRequest wrongAuditOldValueRequest =
        new SolanaSccpProver.FullLightClientAuditProofRequest(
            towerReplay.version(),
            towerReplay.proofFamily(),
            towerReplay.circuitId(),
            towerReplay.parameterSet(),
            towerReplay.role(),
            towerReplay.roleCode(),
            towerReplay.sourceDomain(),
            towerReplay.finalizedSlot(),
            towerReplay.verifierId(),
            towerReplay.verifierHash(),
            towerReplay.sourceStateVerifierId(),
            towerReplay.sourceStateVerifierHash(),
            towerReplay.sourceVerifierMaterialHash(),
            towerReplay.sourceAdapterDeploymentHash(),
            towerReplay.fullLightClientGateHash(),
            towerReplay.finalityContextHash(),
            towerReplay.voteMessageHash(),
            towerReplay.accountsLtHashProofHash(),
            towerReplay.auditStatementHash(),
            towerReplay.statementBytes(),
            towerReplay.verificationContextBytes(),
            towerReplay.schemaDescriptor(),
            towerReplay.publicInputColumns(),
            towerReplay.fastpqPublicInputs(),
            wrongAuditOldValueTransitions);
    final IllegalArgumentException wrongAuditOldValueError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7}, wrongAuditOldValueRequest));
    assertTrue(
        wrongAuditOldValueError
            .getMessage()
            .contains("canonical Solana source-state request"));
    final SolanaSccpProver.FullLightClientAuditProofRequest malformedAuditRequest =
        new SolanaSccpProver.FullLightClientAuditProofRequest(
            towerReplay.version(),
            towerReplay.proofFamily(),
            towerReplay.circuitId(),
            towerReplay.parameterSet(),
            towerReplay.role(),
            towerReplay.roleCode(),
            towerReplay.sourceDomain(),
            towerReplay.finalizedSlot(),
            towerReplay.verifierId(),
            towerReplay.verifierHash(),
            towerReplay.sourceStateVerifierId(),
            towerReplay.sourceStateVerifierHash(),
            towerReplay.sourceVerifierMaterialHash(),
            towerReplay.sourceAdapterDeploymentHash(),
            towerReplay.fullLightClientGateHash(),
            towerReplay.finalityContextHash(),
            towerReplay.voteMessageHash(),
            towerReplay.accountsLtHashProofHash(),
            towerReplay.auditStatementHash(),
            towerReplay.statementBytes(),
            towerReplay.verificationContextBytes(),
            towerReplay.schemaDescriptor(),
            Collections.emptyList(),
            towerReplay.fastpqPublicInputs(),
            towerReplay.fastpqTransitions());
    final IllegalArgumentException malformedAuditError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapSourceStateVerificationProof(
                    new byte[] {9, 8, 7}, malformedAuditRequest));
    assertTrue(malformedAuditError.getMessage().contains("request.publicInputColumns"));
    final boolean[] rejectedAuditCallbackRan = new boolean[] {false};
    final SolanaSccpProver.SourceStateProver guardingAuditProver =
        new SolanaSccpProver.SourceStateProver(
            null,
            requestForCallback -> {
              rejectedAuditCallbackRan[0] = true;
              return new byte[] {9, 8, 7};
            });
    final IllegalArgumentException rejectedAuditRequest =
        assertThrows(
            IllegalArgumentException.class,
            () -> guardingAuditProver.proveFullLightClientAudit(malformedAuditRequest));
    assertTrue(rejectedAuditRequest.getMessage().contains("request.publicInputColumns"));
    assertTrue(!rejectedAuditCallbackRan[0]);
    final IllegalArgumentException allZeroAccountsLtHashProof =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.accountsLtHashProofHash(
                    new SolanaSccpProver.SourceStateVerificationProof(
                        input.accountsLtHashProof().circuitId(), new byte[3])));
    assertTrue(allZeroAccountsLtHashProof.getMessage().contains("all zero"));
    final IllegalArgumentException wrongAccountsLtHashProofVersion =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.canonicalSourceStateVerificationProofBytes(
                    new SolanaSccpProver.SourceStateVerificationProof(
                        0,
                        "stark-fri-v1",
                        input.accountsLtHashProof().circuitId(),
                        new byte[] {1, 2, 3})));
    assertTrue(wrongAccountsLtHashProofVersion.getMessage().contains("version"));
    assertThrows(
        NullPointerException.class,
        () ->
            new SolanaSccpProver.SourceStateVerificationProof(
                1,
                null,
                input.accountsLtHashProof().circuitId(),
                new byte[] {1, 2, 3}));
    assertTrue(
        new String(requests.fullAccountsdbLattice().schemaDescriptor(), StandardCharsets.UTF_8)
            .contains("full_light_client_gate_hash"));
    assertTrue(
        requests.bankForkChoice().fastpqTransitions().stream()
            .allMatch(transition -> transition.key().startsWith("0x")));
    final IllegalArgumentException mismatchedGateHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithGateHash(input, "0x" + repeat("ab", 32))));
    assertTrue(mismatchedGateHash.getMessage().contains("fullLightClientGateHash"));
    final IllegalArgumentException mismatchedMaterialHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithSourceMaterialHash(input, "0x" + repeat("ab", 32))));
    assertTrue(mismatchedMaterialHash.getMessage().contains("sourceVerifierMaterialHash"));
    final IllegalArgumentException mismatchedDeploymentReceiptHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithDeploymentReceiptHash(input, "0x" + repeat("ab", 32))));
    assertTrue(
        mismatchedDeploymentReceiptHash.getMessage().contains("sourceAdapterDeploymentReceiptHash"));
    final IllegalArgumentException mismatchedWitnessDeploymentHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithWitness(
                        input,
                        witnessWithDeploymentHash(
                            input.witnessInput(), "0x" + repeat("ab", 32)))));
    assertTrue(mismatchedWitnessDeploymentHash.getMessage().contains("sourceAdapterDeploymentHash"));
    final IllegalArgumentException mismatchedWitnessDeploymentReceiptHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithWitness(
                        input,
                        witnessWithDeploymentReceiptHash(
                            input.witnessInput(), "0x" + repeat("ab", 32)))));
    assertTrue(
        mismatchedWitnessDeploymentReceiptHash
            .getMessage()
            .contains("sourceAdapterDeploymentReceiptHash"));
    final IllegalArgumentException duplicatedAuditHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithVerifierHashes(
                        input, accountsdbVerifierHash, accountsdbVerifierHash, bankVerifierHash)));
    assertTrue(duplicatedAuditHash.getMessage().contains("role-separated"));
    final IllegalArgumentException reusedSourceStateHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithVerifierHashes(
                        input, sourceStateVerifierHash, accountsdbVerifierHash, bankVerifierHash)));
    assertTrue(reusedSourceStateHash.getMessage().contains("source-adapter material"));
    final IllegalArgumentException reusedSourceTrustAnchorHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithVerifierHashes(
                        input, sourceTrustAnchorHash, accountsdbVerifierHash, bankVerifierHash)));
    assertTrue(reusedSourceTrustAnchorHash.getMessage().contains("source-adapter material"));
    final IllegalArgumentException reusedAdapterVerifierHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithVerifierHashes(
                        input,
                        SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_SOL),
                        accountsdbVerifierHash,
                        bankVerifierHash)));
    assertTrue(reusedAdapterVerifierHash.getMessage().contains("source-adapter material"));
    final SolanaSccpProver.FullLightClientAuditProofInput receiptReuseInput =
        auditInputWithWitness(
            auditInputWithDeploymentReceiptHash(input, towerVerifierHash),
            witnessWithDeploymentReceiptHash(input.witnessInput(), towerVerifierHash));
    final IllegalArgumentException reusedDeploymentReceiptHash =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.buildFullLightClientAuditProofRequests(receiptReuseInput));
    assertTrue(reusedDeploymentReceiptHash.getMessage().contains("source-adapter material"));
    final IllegalArgumentException reusedTemplateHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildFullLightClientAuditProofRequests(
                    auditInputWithVerifierHashes(
                        input,
                        "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
                        accountsdbVerifierHash,
                        bankVerifierHash)));
    assertTrue(reusedTemplateHash.getMessage().contains("template material"));
  }

  @Test
  public void buildsVoteAndStakeAccountDataHashes() {
    final String[] towerVoteSlots = towerVoteSlots();

    assertEquals(
        457,
        SolanaSccpProver.canonicalVoteAccountDataBytes(
                repeatByte((byte) 0x51, 32),
                repeatByte((byte) 0x61, 32),
                repeatByte((byte) 0x71, 32),
                repeatByte((byte) 0x81, 32),
                repeatByte((byte) 0x51, 32),
                "700",
                "10000",
                "123",
                new byte[0],
                "10",
                towerVoteSlots)
            .length);
    final String voteHash =
        SolanaSccpProver.voteAccountDataHash(
            repeatByte((byte) 0x51, 32),
            repeatByte((byte) 0x61, 32),
            repeatByte((byte) 0x71, 32),
            repeatByte((byte) 0x81, 32),
            repeatByte((byte) 0x51, 32),
            "700",
            "10000",
            "123",
            new byte[0],
            "10",
            towerVoteSlots);
    assertTrue(voteHash.matches("0x[0-9a-f]{64}"));
    assertNotEquals(
        voteHash,
        SolanaSccpProver.voteAccountDataHash(
            repeatByte((byte) 0x51, 32),
            repeatByte((byte) 0x62, 32),
            repeatByte((byte) 0x71, 32),
            repeatByte((byte) 0x81, 32),
            repeatByte((byte) 0x51, 32),
            "700",
            "10000",
            "123",
            new byte[0],
            "10",
            towerVoteSlots));

    final String[] badTowerSlots = Arrays.copyOf(towerVoteSlots, towerVoteSlots.length);
    badTowerSlots[0] = "10";
    final IllegalArgumentException badVoteSlot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataHash(
                    repeatByte((byte) 0x51, 32),
                    repeatByte((byte) 0x61, 32),
                    repeatByte((byte) 0x71, 32),
                    repeatByte((byte) 0x81, 32),
                    repeatByte((byte) 0x51, 32),
                    "700",
                    "10000",
                    "123",
                    new byte[0],
                    "10",
                    badTowerSlots));
    assertTrue(badVoteSlot.getMessage().contains("towerVoteSlots[0]"));

    assertEquals(
        154,
        SolanaSccpProver.canonicalStakeAccountDataBytes(
                repeatByte((byte) 0x81, 32),
                repeatByte((byte) 0x91, 32),
                repeatByte((byte) 0xa1, 32),
                "1000",
                "2",
                "9",
                "123",
                "1",
                sampleSolanaStakeWarmupCooldownRateBytes())
            .length);
    final String stakeHash =
        SolanaSccpProver.stakeAccountDataHash(
            repeatByte((byte) 0x81, 32),
            repeatByte((byte) 0x91, 32),
            repeatByte((byte) 0xa1, 32),
            "1000",
            "2",
            "9",
            "123",
            "1",
            sampleSolanaStakeWarmupCooldownRateBytes());
    assertTrue(stakeHash.matches("0x[0-9a-f]{64}"));
    assertNotEquals(
        stakeHash,
        SolanaSccpProver.stakeAccountDataHash(
            repeatByte((byte) 0x81, 32),
            repeatByte((byte) 0x91, 32),
            repeatByte((byte) 0xa2, 32),
            "1000",
            "2",
            "9",
            "123",
            "1",
            sampleSolanaStakeWarmupCooldownRateBytes()));
    assertTrue(
        SolanaSccpProver.stakeAccountDataHash(
                repeatByte((byte) 0x81, 32),
                repeatByte((byte) 0x91, 32),
                repeatByte((byte) 0xa1, 32),
                "1000",
                "2",
                "9",
                "123",
                "1",
                new byte[] {0, 0, 0, 0, 0, 0, (byte) 0xd0, 0x3f})
            .matches("0x[0-9a-f]{64}"));
    final IllegalArgumentException badWarmupCooldownRate =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountDataHash(
                    repeatByte((byte) 0x81, 32),
                    repeatByte((byte) 0x91, 32),
                    repeatByte((byte) 0xa1, 32),
                    "1000",
                    "2",
                    "9",
                    "123",
                    "1",
                    new byte[8]));
    assertTrue(badWarmupCooldownRate.getMessage().contains("warmupCooldownRateBytes"));
    assertNotEquals(
        stakeHash,
        SolanaSccpProver.stakeAccountDataHash(
            repeatByte((byte) 0x81, 32),
            repeatByte((byte) 0x91, 32),
            repeatByte((byte) 0xa1, 32),
            "1000",
            "2",
            "9",
            "123",
            "0",
            sampleSolanaStakeWarmupCooldownRateBytes()));

    final IllegalArgumentException badStakeEpoch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountDataHash(
                    repeatByte((byte) 0x81, 32),
                    repeatByte((byte) 0x91, 32),
                    repeatByte((byte) 0xa1, 32),
                    "1000",
                    "2",
                    "2",
                    "123",
                    "1",
                    sampleSolanaStakeWarmupCooldownRateBytes()));
    assertTrue(badStakeEpoch.getMessage().contains("deactivationEpoch"));
    final IllegalArgumentException badStakeFlags =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountDataHash(
                    repeatByte((byte) 0x81, 32),
                    repeatByte((byte) 0x91, 32),
                    repeatByte((byte) 0xa1, 32),
                    "1000",
                    "2",
                    "9",
                    "123",
                    "2",
                    sampleSolanaStakeWarmupCooldownRateBytes()));
    assertTrue(badStakeFlags.getMessage().contains("stakeFlags"));
    final IllegalArgumentException badWarmupCooldownBytes =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountDataHash(
                    repeatByte((byte) 0x81, 32),
                    repeatByte((byte) 0x91, 32),
                    repeatByte((byte) 0xa1, 32),
                    "1000",
                    "2",
                    "9",
                    "123",
                    "1",
                    new byte[7]));
    assertTrue(badWarmupCooldownBytes.getMessage().contains("warmupCooldownRateBytes"));
  }

  @Test
  public void buildsVoteAccountDataHashFromRawVoteState() {
    final byte[] rawV3 = sampleSolanaVoteStateAccount(true);
    final byte[] voteAccountAddress = repeatByte((byte) 0x81, 32);
    final SolanaSccpProver.ParsedVoteStateV1OrV3AccountData parsed =
        SolanaSccpProver.voteAccountDataFromRawVoteState(rawV3, "3", voteAccountAddress);
    assertArrayEquals(repeatByte((byte) 0x51, 32), parsed.nodePubkey());
    assertArrayEquals(repeatByte((byte) 0x61, 32), parsed.authorizedVoter());
    assertArrayEquals(repeatByte((byte) 0x71, 32), parsed.authorizedWithdrawer());
    assertArrayEquals(voteAccountAddress, parsed.inflationRewardsCollector());
    assertArrayEquals(repeatByte((byte) 0x51, 32), parsed.blockRevenueCollector());
    assertEquals("700", parsed.inflationRewardsCommissionBps());
    assertEquals("10000", parsed.blockRevenueCommissionBps());
    assertEquals("0", parsed.pendingDelegatorRewards());
    assertArrayEquals(new byte[0], parsed.blsPubkeyCompressed());
    assertEquals("10", parsed.rootSlot());
    final String[] expectedVoteSlots = new String[31];
    for (int i = 0; i < expectedVoteSlots.length; i++) {
      expectedVoteSlots[i] = Integer.toString(11 + i);
    }
    assertArrayEquals(expectedVoteSlots, parsed.towerVoteSlots());
    assertEquals(
        SolanaSccpProver.voteAccountDataHash(
            parsed.nodePubkey(),
            parsed.authorizedVoter(),
            parsed.authorizedWithdrawer(),
            parsed.inflationRewardsCollector(),
            parsed.blockRevenueCollector(),
            parsed.inflationRewardsCommissionBps(),
            parsed.blockRevenueCommissionBps(),
            parsed.pendingDelegatorRewards(),
            parsed.blsPubkeyCompressed(),
            parsed.rootSlot(),
            parsed.towerVoteSlots()),
        SolanaSccpProver.voteAccountDataHashFromRawVoteState(rawV3, "3", voteAccountAddress));
    assertEquals(
        SolanaSccpProver.voteAccountDataHashFromRawVoteState(rawV3, "3", voteAccountAddress),
        SolanaSccpProver.voteAccountDataHashFromRawVoteStateV1OrV3(
            rawV3, "3", voteAccountAddress));

    final byte[] rawV1 = sampleSolanaVoteStateAccount(false);
    assertArrayEquals(
        parsed.towerVoteSlots(),
        SolanaSccpProver.voteAccountDataFromRawVoteStateV1OrV3(rawV1, "3", voteAccountAddress)
            .towerVoteSlots());

    final SolanaSccpProver.ParsedVoteStateV1OrV3AccountData parsedV4 =
        SolanaSccpProver.voteAccountDataFromRawVoteState(
            sampleSolanaVoteStateV4Account(), "3", voteAccountAddress);
    assertArrayEquals(repeatByte((byte) 0x81, 32), parsedV4.inflationRewardsCollector());
    assertArrayEquals(repeatByte((byte) 0x91, 32), parsedV4.blockRevenueCollector());
    assertEquals("1234", parsedV4.inflationRewardsCommissionBps());
    assertEquals("9876", parsedV4.blockRevenueCommissionBps());
    assertEquals("456", parsedV4.pendingDelegatorRewards());
    assertArrayEquals(repeatByte((byte) 0xa5, 48), parsedV4.blsPubkeyCompressed());
    final int v4InflationCommissionBpsOffset = 4 + (4 * 32);
    final byte[] excessiveInflationCommissionV4 = sampleSolanaVoteStateV4Account();
    writeU16Le(excessiveInflationCommissionV4, v4InflationCommissionBpsOffset, 10_001);
    final IllegalArgumentException excessiveInflationCommission =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    excessiveInflationCommissionV4, "3", voteAccountAddress));
    assertTrue(
        excessiveInflationCommission.getMessage().contains("inflationRewardsCommissionBps"));
    final byte[] excessiveBlockCommissionV4 = sampleSolanaVoteStateV4Account();
    writeU16Le(excessiveBlockCommissionV4, v4InflationCommissionBpsOffset + 2, 10_001);
    final IllegalArgumentException excessiveBlockCommission =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    excessiveBlockCommissionV4, "3", voteAccountAddress));
    assertTrue(excessiveBlockCommission.getMessage().contains("blockRevenueCommissionBps"));
    final IllegalArgumentException allZeroBlsVoteData =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataHash(
                    parsedV4.nodePubkey(),
                    parsedV4.authorizedVoter(),
                    parsedV4.authorizedWithdrawer(),
                    parsedV4.inflationRewardsCollector(),
                    parsedV4.blockRevenueCollector(),
                    parsedV4.inflationRewardsCommissionBps(),
                    parsedV4.blockRevenueCommissionBps(),
                    parsedV4.pendingDelegatorRewards(),
                    new byte[48],
                    parsedV4.rootSlot(),
                    parsedV4.towerVoteSlots()));
    assertTrue(allZeroBlsVoteData.getMessage().contains("blsPubkeyCompressed"));
    final byte[] allZeroBlsV4 = sampleSolanaVoteStateV4Account();
    final int v4BlsPubkeyOffset = 4 + (4 * 32) + 2 + 2 + 8 + 1;
    Arrays.fill(allZeroBlsV4, v4BlsPubkeyOffset, v4BlsPubkeyOffset + 48, (byte) 0);
    final IllegalArgumentException allZeroRawBls =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    allZeroBlsV4, "3", voteAccountAddress));
    assertTrue(allZeroRawBls.getMessage().contains("blsPubkeyCompressed"));
    final SolanaSccpProver.ParsedVoteStateV1OrV3AccountData parsedV4FourAuthorized =
        SolanaSccpProver.voteAccountDataFromRawVoteState(
            sampleSolanaVoteStateV4Account(4), "3", voteAccountAddress);
    assertArrayEquals(repeatByte((byte) 0x62, 32), parsedV4FourAuthorized.authorizedVoter());

    final byte[] wrongVoteCount = Arrays.copyOf(rawV3, rawV3.length);
    writeU64Le(wrongVoteCount, 4 + 32 + 32 + 1, 30L);
    final IllegalArgumentException badVoteCount =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    wrongVoteCount, "3", voteAccountAddress));
    assertTrue(badVoteCount.getMessage().contains("31 active post-root slots"));

    final int voteEntryOffset = 4 + 32 + 32 + 1 + 8;
    final int firstVoteSlotOffset = voteEntryOffset + 1;
    final int firstConfirmationOffset = firstVoteSlotOffset + 8;
    final int secondVoteSlotOffset = voteEntryOffset + (1 + 8 + 4) + 1;
    final int rootOptionOffset = voteEntryOffset + (31 * (1 + 8 + 4));

    final byte[] wrongConfirmationCount = Arrays.copyOf(rawV3, rawV3.length);
    writeU32Le(wrongConfirmationCount, firstConfirmationOffset, 30);
    final IllegalArgumentException badConfirmationCount =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    wrongConfirmationCount, "3", voteAccountAddress));
    assertTrue(badConfirmationCount.getMessage().contains("invalid Tower confirmation count"));

    final byte[] repeatedVoteSlot = Arrays.copyOf(rawV3, rawV3.length);
    writeU64Le(repeatedVoteSlot, secondVoteSlotOffset, 11L);
    final IllegalArgumentException repeatedSlot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    repeatedVoteSlot, "3", voteAccountAddress));
    assertTrue(repeatedSlot.getMessage().contains("greater than the previous slot"));

    final byte[] noRoot = Arrays.copyOf(rawV3, rawV3.length);
    noRoot[rootOptionOffset] = 0;
    final IllegalArgumentException badRoot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    noRoot, "3", voteAccountAddress));
    assertTrue(badRoot.getMessage().contains("rooted vote state"));

    final byte[] rootOverlapsVoteStack = Arrays.copyOf(rawV3, rawV3.length);
    writeU64Le(rootOverlapsVoteStack, rootOptionOffset + 1, 11L);
    final IllegalArgumentException overlappingRoot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    rootOverlapsVoteStack, "3", voteAccountAddress));
    assertTrue(overlappingRoot.getMessage().contains("greater than the previous slot"));

    final byte[] badPriorVoters = Arrays.copyOf(rawV3, rawV3.length);
    final int priorVotersOffset = rootOptionOffset + 1 + 8 + 8 + (2 * (8 + 32));
    final byte[] zeroPriorVoterWithEpochBounds = Arrays.copyOf(rawV3, rawV3.length);
    writeU64Le(zeroPriorVoterWithEpochBounds, priorVotersOffset + 32, 1L);
    final IllegalArgumentException malformedZeroPriorVoter =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    zeroPriorVoterWithEpochBounds, "3", voteAccountAddress));
    assertTrue(malformedZeroPriorVoter.getMessage().contains("priorVoters[0]"));
    badPriorVoters[priorVotersOffset + (32 * (32 + 8 + 8)) + 8] = 2;
    final IllegalArgumentException malformedPriorVoters =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    badPriorVoters, "3", voteAccountAddress));
    assertTrue(malformedPriorVoters.getMessage().contains("priorVoters"));

    final int v4AuthorizedVotersOffset =
        4
            + 32
            + 32
            + 32
            + 32
            + 2
            + 2
            + 8
            + 1
            + 48
            + 8
            + (31 * (1 + 8 + 4))
            + 1
            + 8;
    final byte[] zeroFutureAuthorizedVoter = sampleSolanaVoteStateV4Account(4);
    final int fourthAuthorizedVoterKeyOffset =
        v4AuthorizedVotersOffset + 8 + (3 * (8 + 32)) + 8;
    Arrays.fill(
        zeroFutureAuthorizedVoter,
        fourthAuthorizedVoterKeyOffset,
        fourthAuthorizedVoterKeyOffset + 32,
        (byte) 0);
    final IllegalArgumentException malformedFutureAuthorizedVoter =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    zeroFutureAuthorizedVoter, "3", voteAccountAddress));
    assertTrue(
        malformedFutureAuthorizedVoter.getMessage().contains(
            "authorizedVoters[3].authorizedVoter"));
    final byte[] tooManyV4AuthorizedVoters = sampleSolanaVoteStateV4Account(5);
    final IllegalArgumentException malformedV4AuthorizedVoters =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    tooManyV4AuthorizedVoters, "3", voteAccountAddress));
    assertTrue(
        malformedV4AuthorizedVoters.getMessage().contains("1..4 entries for VoteStateV4"));

    final byte[] tooManyEpochCredits = sampleSolanaVoteStateV4Account();
    final int v4EpochCreditsOffset = v4AuthorizedVotersOffset + 8 + (2 * (8 + 32));
    writeU64Le(tooManyEpochCredits, v4EpochCreditsOffset, 65L);
    final IllegalArgumentException malformedEpochCredits =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    tooManyEpochCredits, "3", voteAccountAddress));
    assertTrue(malformedEpochCredits.getMessage().contains("epochCredits"));

    final int v3EpochCreditsOffset = priorVotersOffset + (32 * (32 + 8 + 8)) + 8 + 1;
    final byte[] futureEpochCredit = Arrays.copyOf(rawV3, rawV3.length);
    writeU64Le(futureEpochCredit, v3EpochCreditsOffset, 1L);
    writeU64Le(futureEpochCredit, v3EpochCreditsOffset + 8, 4L);
    writeU64Le(futureEpochCredit, v3EpochCreditsOffset + 16, 1L);
    final IllegalArgumentException malformedFutureEpochCredit =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    futureEpochCredit, "3", voteAccountAddress));
    assertTrue(malformedFutureEpochCredit.getMessage().contains("epochCredits"));

    final int lastTimestampSlotOffset =
        v3EpochCreditsOffset + 8;
    final byte[] futureLastTimestampSlot = Arrays.copyOf(rawV3, rawV3.length);
    writeU64Le(futureLastTimestampSlot, lastTimestampSlotOffset, 42L);
    final IllegalArgumentException malformedFutureLastTimestamp =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    futureLastTimestampSlot, "3", voteAccountAddress));
    assertTrue(malformedFutureLastTimestamp.getMessage().contains("lastTimestamp"));

    final byte[] negativeLastTimestamp = Arrays.copyOf(rawV3, rawV3.length);
    writeU64Le(negativeLastTimestamp, lastTimestampSlotOffset, 41L);
    writeU64Le(negativeLastTimestamp, lastTimestampSlotOffset + 8, -1L);
    final IllegalArgumentException malformedNegativeLastTimestamp =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    negativeLastTimestamp, "3", voteAccountAddress));
    assertTrue(malformedNegativeLastTimestamp.getMessage().contains("lastTimestamp"));

    final byte[] nonzeroPadding = Arrays.copyOf(rawV3, rawV3.length);
    nonzeroPadding[nonzeroPadding.length - 1] = 1;
    final IllegalArgumentException malformedPadding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    nonzeroPadding, "3", voteAccountAddress));
    assertTrue(malformedPadding.getMessage().contains("padding"));

    final IllegalArgumentException missingAuthorizedVoter =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.voteAccountDataFromRawVoteState(
                    rawV3, "0", voteAccountAddress));
    assertTrue(missingAuthorizedVoter.getMessage().contains("at or before epoch"));
  }

  @Test
  public void buildsStakeAccountDataHashFromRawStakeStateV2() {
    final byte[] raw = sampleSolanaStakeStateV2StakeAccount();
    final SolanaSccpProver.ParsedStakeStateV2StakeAccountData parsed =
        SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(raw);
    assertArrayEquals(repeatByte((byte) 0x81, 32), parsed.staker());
    assertArrayEquals(repeatByte((byte) 0x91, 32), parsed.withdrawer());
    assertArrayEquals(repeatByte((byte) 0xa1, 32), parsed.voterPubkey());
    assertEquals("1000", parsed.delegatedStake());
    assertEquals("2", parsed.activationEpoch());
    assertEquals("9", parsed.deactivationEpoch());
    assertArrayEquals(sampleSolanaStakeWarmupCooldownRateBytes(), parsed.warmupCooldownRateBytes());
    assertEquals("123", parsed.creditsObserved());
    assertEquals("1", parsed.stakeFlags());
    assertEquals(
        SolanaSccpProver.stakeAccountDataHash(
            parsed.staker(),
            parsed.withdrawer(),
            parsed.voterPubkey(),
            parsed.delegatedStake(),
            parsed.activationEpoch(),
            parsed.deactivationEpoch(),
            parsed.creditsObserved(),
            parsed.stakeFlags(),
            parsed.warmupCooldownRateBytes()),
        SolanaSccpProver.stakeAccountDataHashFromRawStakeStateV2(raw));

    final byte[] wrongVariant = Arrays.copyOf(raw, raw.length);
    writeU32Le(wrongVariant, 0, 1);
    final IllegalArgumentException badVariant =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(wrongVariant));
    assertTrue(badVariant.getMessage().contains("StakeStateV2::Stake"));

    final IllegalArgumentException shortData =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(Arrays.copyOf(raw, 199)));
    assertTrue(shortData.getMessage().contains("200-byte"));

    final byte[] hiddenPadding = Arrays.copyOf(raw, raw.length);
    hiddenPadding[197] = 1;
    final IllegalArgumentException badPadding =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(hiddenPadding));
    assertTrue(badPadding.getMessage().contains("padding"));

    final byte[] unknownFlags = Arrays.copyOf(raw, raw.length);
    unknownFlags[196] = 2;
    final IllegalArgumentException badFlags =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(unknownFlags));
    assertTrue(badFlags.getMessage().contains("StakeFlags"));

    final byte[] zeroVoter = Arrays.copyOf(raw, raw.length);
    Arrays.fill(zeroVoter, 124, 156, (byte) 0);
    final IllegalArgumentException badVoter =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(zeroVoter));
    assertTrue(badVoter.getMessage().contains("voterPubkey"));

    final byte[] zeroDelegation = Arrays.copyOf(raw, raw.length);
    writeU64Le(zeroDelegation, 156, 0L);
    final IllegalArgumentException badDelegation =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(zeroDelegation));
    assertTrue(badDelegation.getMessage().contains("delegatedStake"));

    final byte[] legacyWarmupCooldownRate = Arrays.copyOf(raw, raw.length);
    System.arraycopy(sampleSolanaLegacyStakeWarmupCooldownRateBytes(), 0, legacyWarmupCooldownRate, 180, 8);
    assertArrayEquals(
        sampleSolanaLegacyStakeWarmupCooldownRateBytes(),
        SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(legacyWarmupCooldownRate)
            .warmupCooldownRateBytes());

    final byte[] zeroWarmupCooldownRate = Arrays.copyOf(raw, raw.length);
    Arrays.fill(zeroWarmupCooldownRate, 180, 188, (byte) 0);
    final IllegalArgumentException badWarmupCooldownRateFromRaw =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(zeroWarmupCooldownRate));
    assertTrue(
        badWarmupCooldownRateFromRaw.getMessage().contains("warmupCooldownRateBytes"));

    final byte[] invalidEpochOrder = Arrays.copyOf(raw, raw.length);
    writeU64Le(invalidEpochOrder, 172, 2L);
    final IllegalArgumentException badEpoch =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeAccountDataFromRawStakeStateV2(invalidEpochOrder));
    assertTrue(badEpoch.getMessage().contains("deactivationEpoch"));
  }

  @Test
  public void buildsStakeAccountStateHashForFinalityContext() {
    final byte[][] validatorPublicKeys =
        new byte[][] {repeatByte((byte) 0x11, 32), repeatByte((byte) 0x22, 32)};
    final String[] validatorStakes = new String[] {"1", "2"};
    final String[] activationEpochs = new String[] {"0", "2"};
    final String[] deactivationEpochs = new String[] {"18446744073709551615", "9"};
    final byte[][] voteAccounts =
        new byte[][] {repeatByte((byte) 0x33, 32), repeatByte((byte) 0x44, 32)};
    final byte[][] stakeAccounts =
        new byte[][] {repeatByte((byte) 0x55, 32), repeatByte((byte) 0x66, 32)};
    final byte[][] voteAccountHashes =
        new byte[][] {repeatByte((byte) 0x77, 32), repeatByte((byte) 0x88, 32)};
    final byte[][] stakeAccountHashes =
        new byte[][] {repeatByte((byte) 0x99, 32), repeatByte((byte) 0xaa, 32)};

    assertEquals(
        437,
        SolanaSccpProver.canonicalStakeAccountStateBytes(
                "3",
                validatorPublicKeys,
                validatorStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes)
            .length);
    assertEquals(
        "0x34f6086dd8c1770770802be17b833ed7c973fdaa002c866c0462c33d6938f5b5",
        SolanaSccpProver.stakeAccountStateHash(
            "3",
            validatorPublicKeys,
            validatorStakes,
            activationEpochs,
            deactivationEpochs,
            voteAccounts,
            stakeAccounts,
            voteAccountHashes,
            stakeAccountHashes));

    final IllegalArgumentException lengthMismatch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountStateHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    activationEpochs,
                    deactivationEpochs,
                    new byte[][] {repeatByte((byte) 0x33, 32)},
                    stakeAccounts,
                    voteAccountHashes,
                    stakeAccountHashes));
    assertTrue(lengthMismatch.getMessage().contains("validatorVoteAccountAddresses"));

    final IllegalArgumentException duplicateVoteAccount =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountStateHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    activationEpochs,
                    deactivationEpochs,
                    new byte[][] {repeatByte((byte) 0x33, 32), repeatByte((byte) 0x33, 32)},
                    stakeAccounts,
                    voteAccountHashes,
                    stakeAccountHashes));
    assertTrue(duplicateVoteAccount.getMessage().contains("validatorVoteAccountAddresses"));

    final IllegalArgumentException sameVoteAndStake =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountStateHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    activationEpochs,
                    deactivationEpochs,
                    voteAccounts,
                    new byte[][] {repeatByte((byte) 0x55, 32), repeatByte((byte) 0x44, 32)},
                    voteAccountHashes,
                    stakeAccountHashes));
    assertTrue(sameVoteAndStake.getMessage().contains("validatorStakeAccountAddresses[1]"));

    final IllegalArgumentException crossRoleOverlap =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountStateHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    activationEpochs,
                    deactivationEpochs,
                    new byte[][] {repeatByte((byte) 0x66, 32), repeatByte((byte) 0x44, 32)},
                    stakeAccounts,
                    voteAccountHashes,
                    stakeAccountHashes));
    assertTrue(crossRoleOverlap.getMessage().contains("validatorVoteAccountAddresses[0]"));

    final IllegalArgumentException zeroVoteHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeAccountStateHash(
                    "3",
                    validatorPublicKeys,
                    validatorStakes,
                    activationEpochs,
                    deactivationEpochs,
                    voteAccounts,
                    stakeAccounts,
                    new byte[][] {repeatByte((byte) 0x77, 32), repeatByte((byte) 0x00, 32)},
                    stakeAccountHashes));
    assertTrue(zeroVoteHash.getMessage().contains("validatorVoteAccountHashes[1]"));
  }

  @Test
  public void buildsStakeHistoryHashForFinalityContext() {
    final byte[][] validatorPublicKeys =
        new byte[][] {repeatByte((byte) 0x11, 32), repeatByte((byte) 0x22, 32)};
    final String[] validatorEffectiveStakes = new String[] {"1", "2"};
    final String[] validatorDelegatedStakes = new String[] {"1", "3"};
    final String[] activationEpochs = new String[] {"0", "2"};
    final String[] deactivationEpochs = new String[] {"18446744073709551615", "9"};
    final byte[][] voteAccounts =
        new byte[][] {repeatByte((byte) 0x33, 32), repeatByte((byte) 0x44, 32)};
    final byte[][] stakeAccounts =
        new byte[][] {repeatByte((byte) 0x55, 32), repeatByte((byte) 0x66, 32)};
    final byte[][] voteAccountHashes =
        new byte[][] {repeatByte((byte) 0x77, 32), repeatByte((byte) 0x88, 32)};
    final byte[][] stakeAccountHashes =
        new byte[][] {repeatByte((byte) 0x99, 32), repeatByte((byte) 0xaa, 32)};
    final SolanaSccpProver.StakeHistoryEntry[] stakeHistoryEntries =
        new SolanaSccpProver.StakeHistoryEntry[] {
          new SolanaSccpProver.StakeHistoryEntry("2", "23", "3", "0"),
          new SolanaSccpProver.StakeHistoryEntry("3", "3", "1", "0")
        };

    assertEquals(
        249,
        SolanaSccpProver.canonicalStakeHistoryBytes(
                "3",
                validatorPublicKeys,
                validatorEffectiveStakes,
                validatorDelegatedStakes,
                activationEpochs,
                deactivationEpochs,
                voteAccounts,
                stakeAccounts,
                voteAccountHashes,
                stakeAccountHashes,
                stakeHistoryEntries)
            .length);
    assertEquals(
        "0xd75957eec3cf9f5b88076c8dc18e81c5debd627adfbed7e03e35443bcc4d14b6",
        SolanaSccpProver.stakeHistoryHash(
            "3",
            validatorPublicKeys,
            validatorEffectiveStakes,
            validatorDelegatedStakes,
            activationEpochs,
            deactivationEpochs,
            voteAccounts,
            stakeAccounts,
            voteAccountHashes,
            stakeAccountHashes,
            stakeHistoryEntries));

    final IllegalArgumentException delegatedTooSmall =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeHistoryHash(
                    "3",
                    validatorPublicKeys,
                    validatorEffectiveStakes,
                    new String[] {"0", "3"},
                    activationEpochs,
                    deactivationEpochs,
                    voteAccounts,
                    stakeAccounts,
                    voteAccountHashes,
                    stakeAccountHashes,
                    stakeHistoryEntries));
    assertTrue(delegatedTooSmall.getMessage().contains("validatorDelegatedStakes[0]"));

    final IllegalArgumentException wrongEffectiveStake =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeHistoryHash(
                    "3",
                    validatorPublicKeys,
                    new String[] {"1", "1"},
                    validatorDelegatedStakes,
                    activationEpochs,
                    deactivationEpochs,
                    voteAccounts,
                    stakeAccounts,
                    voteAccountHashes,
                    stakeAccountHashes,
                    stakeHistoryEntries));
    assertTrue(wrongEffectiveStake.getMessage().contains("validatorEffectiveStakes[1]"));

    final IllegalArgumentException extraSignedEffectiveStake =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeHistoryHash(
                    "3",
                    validatorPublicKeys,
                    validatorEffectiveStakes,
                    validatorDelegatedStakes,
                    activationEpochs,
                    deactivationEpochs,
                    voteAccounts,
                    stakeAccounts,
                    voteAccountHashes,
                    stakeAccountHashes,
                    new SolanaSccpProver.StakeHistoryEntry[] {
                      stakeHistoryEntries[0],
                      new SolanaSccpProver.StakeHistoryEntry("3", "4", "1", "0")
                    }));
    assertTrue(
        extraSignedEffectiveStake
            .getMessage()
            .contains("must equal replayed validator effective stake"));

    final IllegalArgumentException missingEpoch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeHistoryHash(
                    "3",
                    validatorPublicKeys,
                    validatorEffectiveStakes,
                    validatorDelegatedStakes,
                    activationEpochs,
                    deactivationEpochs,
                    voteAccounts,
                    stakeAccounts,
                    voteAccountHashes,
                    stakeAccountHashes,
                    new SolanaSccpProver.StakeHistoryEntry[] {stakeHistoryEntries[0]}));
    assertTrue(missingEpoch.getMessage().contains("stakeHistoryEntries"));
  }

  @Test
  public void buildsStakeHistorySysvarDataHash() {
    final SolanaSccpProver.StakeHistoryEntry[] stakeHistoryEntries =
        new SolanaSccpProver.StakeHistoryEntry[] {
          new SolanaSccpProver.StakeHistoryEntry("2", "10", "3", "1"),
          new SolanaSccpProver.StakeHistoryEntry("3", "12", "0", "0")
        };

    assertEquals(32, hexBytes(SolanaSccpProver.SYSVAR_PROGRAM_ID).length);
    assertEquals(32, hexBytes(SolanaSccpProver.STAKE_HISTORY_SYSVAR_ID).length);
    final byte[] canonical =
        SolanaSccpProver.canonicalStakeHistorySysvarDataBytes(stakeHistoryEntries);
    assertEquals(72, canonical.length);
    assertEquals(3, canonical[8] & 0xff);
    final String dataHash = SolanaSccpProver.stakeHistorySysvarDataHash(stakeHistoryEntries);
    assertTrue(dataHash.matches("0x[0-9a-f]{64}"));
    assertEquals(dataHash, SolanaSccpProver.stakeHistorySysvarDataHashFromRawData(canonical));
    assertNotEquals(
        dataHash,
        SolanaSccpProver.stakeHistorySysvarDataHash(
            new SolanaSccpProver.StakeHistoryEntry[] {
              stakeHistoryEntries[0], new SolanaSccpProver.StakeHistoryEntry("3", "13", "0", "0")
            }));

    final IllegalArgumentException unsorted =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.stakeHistorySysvarDataHash(
                    new SolanaSccpProver.StakeHistoryEntry[] {
                      stakeHistoryEntries[1], stakeHistoryEntries[0]
                    }));
    assertTrue(unsorted.getMessage().contains("strictly increasing epoch"));

    final IllegalArgumentException truncatedRaw =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeHistorySysvarDataHashFromRawData(Arrays.copyOf(canonical, 9)));
    assertTrue(truncatedRaw.getMessage().contains("bincode Vec"));

    final byte[] wrongCount = Arrays.copyOf(canonical, canonical.length);
    writeU64Le(wrongCount, 0, 3L);
    final IllegalArgumentException wrongCountError =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeHistorySysvarDataHashFromRawData(wrongCount));
    assertTrue(wrongCountError.getMessage().contains("1..512"));

    final byte[] ascendingRaw = Arrays.copyOf(canonical, canonical.length);
    final byte[] newestEntry = Arrays.copyOfRange(canonical, 8, 40);
    final byte[] oldestEntry = Arrays.copyOfRange(canonical, 40, 72);
    System.arraycopy(oldestEntry, 0, ascendingRaw, 8, oldestEntry.length);
    System.arraycopy(newestEntry, 0, ascendingRaw, 40, newestEntry.length);
    final IllegalArgumentException wrongOrderError =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.stakeHistorySysvarDataHashFromRawData(ascendingRaw));
    assertTrue(wrongOrderError.getMessage().contains("newest-first"));
  }

  @Test
  public void buildsTowerLockoutHashForFinalityContext() {
    final String finalizedSlot = "1296096";
    final String rootedSlot = "1296065";
    final String parentSlot = "1296095";
    final String parentBankHash = "0x" + repeat("33", 32);

    assertEquals(32L, SolanaSccpProver.TOWER_LOCKOUT_CONFIRMATION_DEPTH);
    assertEquals(31L, SolanaSccpProver.TOWER_VOTE_STACK_DEPTH);
    assertEquals(
        73,
        SolanaSccpProver.canonicalTowerLockoutBytes(
                finalizedSlot, rootedSlot, parentSlot, parentBankHash)
            .length);
    assertTrue(
        SolanaSccpProver.towerLockoutHash(finalizedSlot, rootedSlot, parentSlot, parentBankHash)
            .matches("0x[0-9a-f]{64}"));
    assertEquals(
        SolanaSccpProver.towerLockoutHash(finalizedSlot, rootedSlot, parentSlot, parentBankHash),
        SolanaSccpProver.towerLockoutHash(
            finalizedSlot, rootedSlot, parentSlot, parentBankHash, "3"));

    final IllegalArgumentException wrongEpoch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerLockoutHash(
                    finalizedSlot, rootedSlot, parentSlot, parentBankHash, "4"));
    assertTrue(wrongEpoch.getMessage().contains("epoch"));

    final IllegalArgumentException shallowRoot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerLockoutHash(
                    finalizedSlot, "1296066", parentSlot, parentBankHash));
    assertTrue(shallowRoot.getMessage().contains("rootedSlot"));

    final IllegalArgumentException indirectParent =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerLockoutHash(
                    finalizedSlot, rootedSlot, "1296094", parentBankHash));
    assertTrue(indirectParent.getMessage().contains("parentSlot"));

    final IllegalArgumentException zeroParentBank =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerLockoutHash(
                    finalizedSlot, rootedSlot, parentSlot, "0x" + repeat("00", 32)));
    assertTrue(zeroParentBank.getMessage().contains("parentBankHash"));
  }

  @Test
  public void buildsTowerReplayHashForFinalityContext() {
    final String finalizedSlot = "1296096";
    final String rootedSlot = "1296065";
    final String parentSlot = "1296095";
    final String bankForkHash = "0x" + repeat("a5", 32);
    final String[] towerVoteSlots = towerVoteSlots();

    assertEquals(
        573,
        SolanaSccpProver.canonicalTowerReplayBytes(
                finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots)
            .length);
    assertTrue(
        SolanaSccpProver.towerReplayHash(
                finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots)
            .matches("0x[0-9a-f]{64}"));
    assertEquals(
        SolanaSccpProver.towerReplayHash(
            finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots),
        SolanaSccpProver.towerReplayHash(
            finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots, "3"));
    assertNotEquals(
        SolanaSccpProver.towerReplayHash(
            finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots),
        SolanaSccpProver.towerReplayHash(
            finalizedSlot, rootedSlot, parentSlot, "0x" + repeat("a6", 32), towerVoteSlots));
    final IllegalArgumentException zeroBankForkHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerReplayHash(
                    finalizedSlot,
                    rootedSlot,
                    parentSlot,
                    "0x" + repeat("00", 32),
                    towerVoteSlots));
    assertTrue(zeroBankForkHash.getMessage().contains("bankForkHash"));

    final IllegalArgumentException wrongEpoch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerReplayHash(
                    finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots, "4"));
    assertTrue(wrongEpoch.getMessage().contains("epoch"));

    final String[] shortStack = Arrays.copyOfRange(towerVoteSlots, 1, towerVoteSlots.length);
    final IllegalArgumentException shortVoteStack =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerReplayHash(
                    finalizedSlot, rootedSlot, parentSlot, bankForkHash, shortStack));
    assertTrue(shortVoteStack.getMessage().contains("towerVoteSlots"));

    final String[] unsortedVoteSlots = Arrays.copyOf(towerVoteSlots, towerVoteSlots.length);
    final String first = unsortedVoteSlots[0];
    unsortedVoteSlots[0] = unsortedVoteSlots[1];
    unsortedVoteSlots[1] = first;
    final IllegalArgumentException unsortedStack =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerReplayHash(
                    finalizedSlot, rootedSlot, parentSlot, bankForkHash, unsortedVoteSlots));
    assertTrue(unsortedStack.getMessage().contains("strictly increasing"));

    final String[] wrongLastVoteSlots = Arrays.copyOf(towerVoteSlots, towerVoteSlots.length);
    wrongLastVoteSlots[wrongLastVoteSlots.length - 1] = "1296095";
    final IllegalArgumentException wrongLast =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.towerReplayHash(
                    finalizedSlot, rootedSlot, parentSlot, bankForkHash, wrongLastVoteSlots));
    assertTrue(wrongLast.getMessage().contains("last towerVoteSlots"));
  }

  @Test
  public void buildsAccountInclusionWitness() {
    final String finalizedSlot = "1296096";
    final byte[][] addresses =
        new byte[][] {
          repeatByte((byte) 0x31, 32), repeatByte((byte) 0x41, 32), repeatByte((byte) 0x51, 32)
        };
    final byte[][] owners =
        new byte[][] {
          repeatByte((byte) 0x61, 32), repeatByte((byte) 0x62, 32), repeatByte((byte) 0x63, 32)
        };
    final String[] lamports = new String[] {"1000000", "1000001", "1000002"};
    final String[] dataHashes =
        new String[] {"0x" + repeat("91", 32), "0x" + repeat("92", 32), "0x" + repeat("93", 32)};
    final byte[][] rawData =
        new byte[][] {
          repeatByte((byte) 0x01, 64), repeatByte((byte) 0x02, 64), repeatByte((byte) 0x03, 64)
        };
    final SolanaSccpProver.AccountOpeningInput[] openingInputs =
        new SolanaSccpProver.AccountOpeningInput[] {
          new SolanaSccpProver.AccountOpeningInput(
              addresses[0], owners[0], lamports[0], "0", false, dataHashes[0]),
          new SolanaSccpProver.AccountOpeningInput(
              addresses[1], owners[1], lamports[1], "0", false, dataHashes[1]),
          new SolanaSccpProver.AccountOpeningInput(
              addresses[2], owners[2], lamports[2], "0", false, dataHashes[2])
        };
    assertEquals(
        109,
        SolanaSccpProver.canonicalAccountInclusionLeafBytes(
                finalizedSlot,
                addresses[0],
                owners[0],
                lamports[0],
                "0",
                false,
                dataHashes[0],
                SolanaSccpProver.accountRawDataHash(rawData[0]))
            .length);

    final List<String> leaves =
        Arrays.asList(
            SolanaSccpProver.accountInclusionLeafHash(
                finalizedSlot, addresses[0], owners[0], lamports[0], "0", false, dataHashes[0], rawData[0]),
            SolanaSccpProver.accountInclusionLeafHash(
                finalizedSlot, addresses[1], owners[1], lamports[1], "0", false, dataHashes[1], rawData[1]),
            SolanaSccpProver.accountInclusionLeafHash(
                finalizedSlot, addresses[2], owners[2], lamports[2], "0", false, dataHashes[2], rawData[2]));
    assertEquals(
        65, SolanaSccpProver.canonicalAccountInclusionNodeBytes(leaves.get(0), leaves.get(1)).length);
    assertTrue(SolanaSccpProver.accountInclusionNodeHash(leaves.get(0), leaves.get(1)).startsWith("0x"));

    final SolanaSccpProver.AccountInclusionWitness witness =
        SolanaSccpProver.accountInclusionRootAndBranches(leaves);
    assertEquals(leaves.size(), witness.branches.size());
    assertEquals(
        witness.root,
        SolanaSccpProver.accountInclusionRootFromBranch(leaves.get(0), witness.branches.get(0)));
    assertEquals(
        witness.root,
        SolanaSccpProver.accountInclusionRootFromBranch(leaves.get(1), witness.branches.get(1)));
    final SolanaSccpProver.OpenedAccountInclusionWitness openedWitness =
        SolanaSccpProver.openedAccountInclusionWitness(
            new SolanaSccpProver.OpenedAccountInclusionWitnessInput(
                finalizedSlot,
                new SolanaSccpProver.AccountOpeningInput[] {openingInputs[0]},
                new byte[][] {rawData[0]},
                new SolanaSccpProver.AccountOpeningInput[] {openingInputs[1]},
                new byte[][] {rawData[1]},
                openingInputs[2],
                rawData[2],
                witness.root));
    assertEquals(witness.branches, openedWitness.branches);
    assertEquals(Arrays.asList(witness.branches.get(0)), openedWitness.validatorVoteAccountBranches);
    assertEquals(Arrays.asList(witness.branches.get(1)), openedWitness.validatorStakeAccountBranches);
    assertEquals(witness.branches.get(2), openedWitness.stakeHistorySysvarBranch);
    final SolanaSccpProver.AccountOpeningInput duplicateStakeOpening =
        new SolanaSccpProver.AccountOpeningInput(
            openingInputs[0].address(),
            openingInputs[1].owner(),
            openingInputs[1].lamports(),
            openingInputs[1].rentEpoch(),
            openingInputs[1].executable(),
            openingInputs[1].dataHash());
    final IllegalArgumentException duplicateOpenedAddress =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.openedAccountInclusionWitness(
                    new SolanaSccpProver.OpenedAccountInclusionWitnessInput(
                        finalizedSlot,
                        new SolanaSccpProver.AccountOpeningInput[] {openingInputs[0]},
                        new byte[][] {rawData[0]},
                        new SolanaSccpProver.AccountOpeningInput[] {duplicateStakeOpening},
                        new byte[][] {rawData[1]},
                        openingInputs[2],
                        rawData[2],
                        null)));
    assertTrue(duplicateOpenedAddress.getMessage().contains("opened account addresses"));
    final IllegalArgumentException mismatchedRoot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.openedAccountInclusionWitness(
                    new SolanaSccpProver.OpenedAccountInclusionWitnessInput(
                        finalizedSlot,
                        new SolanaSccpProver.AccountOpeningInput[] {openingInputs[0]},
                        new byte[][] {rawData[0]},
                        new SolanaSccpProver.AccountOpeningInput[] {openingInputs[1]},
                        new byte[][] {rawData[1]},
                        openingInputs[2],
                        rawData[2],
                        "0x" + repeat("77", 32))));
    assertTrue(mismatchedRoot.getMessage().contains("accountInclusionRoot"));
    final String mutatedLeaf =
        SolanaSccpProver.accountInclusionLeafHash(
            finalizedSlot,
            addresses[0],
            owners[0],
            lamports[0],
            "0",
            false,
            dataHashes[0],
            repeatByte((byte) 0x04, 64));
    assertNotEquals(
        witness.root,
        SolanaSccpProver.accountInclusionRootFromBranch(mutatedLeaf, witness.branches.get(0)));
    final IllegalArgumentException zeroLeaf =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.accountInclusionRootFromBranch("0x" + repeat("00", 32), Collections.emptyList()));
    assertTrue(zeroLeaf.getMessage().contains("leaf"));
    final IllegalArgumentException oversizedBranch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.accountInclusionRootFromBranch(
                    leaves.get(0),
                    Collections.nCopies(
                        SolanaSccpProver.MAX_SOURCE_MERKLE_BRANCH_NODES + 1,
                        "0x" + repeat("56", 32))));
    assertTrue(oversizedBranch.getMessage().contains("at most"));
    final IllegalArgumentException oversizedOpened =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.openedAccountInclusionWitness(
                    new SolanaSccpProver.OpenedAccountInclusionWitnessInput(
                        finalizedSlot,
                        repeatedOpening(openingInputs[0], SolanaSccpProver.MAX_VALIDATORS + 1),
                        repeatedBytes(rawData[0], SolanaSccpProver.MAX_VALIDATORS + 1),
                        new SolanaSccpProver.AccountOpeningInput[] {openingInputs[1]},
                        new byte[][] {rawData[1]},
                        openingInputs[2],
                        rawData[2],
                        null)));
    assertTrue(oversizedOpened.getMessage().contains("validatorVoteAccountOpenings"));
    assertThrows(IllegalArgumentException.class, () -> SolanaSccpProver.accountRawDataHash(new byte[0]));
    assertThrows(
        IllegalArgumentException.class,
        () -> SolanaSccpProver.accountInclusionRootAndBranches(Arrays.asList(leaves.get(0), leaves.get(0))));
  }

  @Test
  public void buildsBankForkHashForFinalityContext() {
    final String finalizedSlot = "1296096";
    final String parentSlot = "1296095";
    final String parentBankHash = "0x" + repeat("33", 32);
    final String bankSignatureCount = "8";
    final String blockhash = "0x" + repeat("55", 32);
    final byte[] accountsLtHash = repeatByte((byte) 0x99, 2048);
    final String bankHash =
        SolanaSccpProver.agaveBankHash(
            parentBankHash, bankSignatureCount, blockhash, accountsLtHash);
    final String transactionStatusRoot = "0x" + repeat("66", 32);
    final String accountInclusionRoot = "0x" + repeat("77", 32);
    final String accountsLtHashChecksum = SolanaSccpProver.accountsLtHashChecksum(accountsLtHash);

    assertEquals(
        229,
        SolanaSccpProver.canonicalBankForkBytes(
                finalizedSlot,
                parentSlot,
                bankSignatureCount,
                parentBankHash,
                bankHash,
                blockhash,
                accountsLtHash,
                new byte[0],
                transactionStatusRoot,
                accountInclusionRoot,
                accountsLtHashChecksum,
                null)
            .length);
    assertEquals(
        "0x8c496fb25a4499947e454a84f638211a84445748bc5242fbb6fb511edd82e531",
        SolanaSccpProver.bankForkHash(
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            bankHash,
            blockhash,
            accountsLtHash,
            new byte[0],
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum,
            null));
    assertEquals(
        SolanaSccpProver.bankForkHash(
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            bankHash,
            blockhash,
            accountsLtHash,
            new byte[0],
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum,
            null),
        SolanaSccpProver.bankForkHash(
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            bankHash,
            blockhash,
            accountsLtHash,
            new byte[0],
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum,
            "3"));
    final String publicInputsHash =
        SolanaSccpProver.accountsLtHashProofPublicInputsHash(
            SolanaSccpProver.DOMAIN_SOLANA,
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            bankHash,
            blockhash,
            new byte[0],
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum);
    assertTrue(publicInputsHash.matches("0x[0-9a-f]{64}"));
    assertTrue(
        SolanaSccpProver.canonicalAccountsLtHashProofPublicInputsBytes(
                SolanaSccpProver.DOMAIN_SOLANA,
                finalizedSlot,
                parentSlot,
                bankSignatureCount,
                parentBankHash,
                bankHash,
                blockhash,
                new byte[0],
                transactionStatusRoot,
                accountInclusionRoot,
                accountsLtHashChecksum)
            .length
            > 250);
    assertNotEquals(
        publicInputsHash,
        SolanaSccpProver.accountsLtHashProofPublicInputsHash(
            SolanaSccpProver.DOMAIN_SOLANA,
            finalizedSlot,
            parentSlot,
            "9",
            parentBankHash,
            bankHash,
            blockhash,
            new byte[0],
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum));

    final IllegalArgumentException wrongEpoch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    parentSlot,
                    bankSignatureCount,
                    parentBankHash,
                    bankHash,
                    blockhash,
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    accountInclusionRoot,
                    accountsLtHashChecksum,
                    "4"));
    assertTrue(wrongEpoch.getMessage().contains("epoch"));

    final IllegalArgumentException indirectParent =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    "1296094",
                    bankSignatureCount,
                    parentBankHash,
                    bankHash,
                    blockhash,
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    accountInclusionRoot,
                    accountsLtHashChecksum,
                    null));
    assertTrue(indirectParent.getMessage().contains("parentSlot"));

    final IllegalArgumentException zeroSignatureCount =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    parentSlot,
                    "0",
                    parentBankHash,
                    bankHash,
                    blockhash,
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    accountInclusionRoot,
                    accountsLtHashChecksum,
                    null));
    assertTrue(zeroSignatureCount.getMessage().contains("bankSignatureCount"));

    final IllegalArgumentException repeatedBankHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    parentSlot,
                    bankSignatureCount,
                    parentBankHash,
                    parentBankHash,
                    blockhash,
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    accountInclusionRoot,
                    accountsLtHashChecksum,
                    null));
    assertTrue(repeatedBankHash.getMessage().contains("bankHash"));

    final IllegalArgumentException wrongBankHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    parentSlot,
                    bankSignatureCount,
                    parentBankHash,
                    "0x" + repeat("44", 32),
                    blockhash,
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    accountInclusionRoot,
                    accountsLtHashChecksum,
                    null));
    assertTrue(wrongBankHash.getMessage().contains("bankHash"));

    final IllegalArgumentException zeroBlockhash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    parentSlot,
                    bankSignatureCount,
                    parentBankHash,
                    bankHash,
                    "0x" + repeat("00", 32),
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    accountInclusionRoot,
                    accountsLtHashChecksum,
                    null));
    assertTrue(zeroBlockhash.getMessage().contains("blockhash"));
    final IllegalArgumentException zeroAccountRoot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    parentSlot,
                    bankSignatureCount,
                    parentBankHash,
                    bankHash,
                    blockhash,
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    "0x" + repeat("00", 32),
                    accountsLtHashChecksum,
                    null));
    assertTrue(zeroAccountRoot.getMessage().contains("accountInclusionRoot"));
    final IllegalArgumentException zeroAccountsLtHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.bankForkHash(
                    finalizedSlot,
                    parentSlot,
                    bankSignatureCount,
                    parentBankHash,
                    bankHash,
                    blockhash,
                    accountsLtHash,
                    new byte[0],
                    transactionStatusRoot,
                    accountInclusionRoot,
                    "0x" + repeat("00", 32),
                    null));
    assertTrue(zeroAccountsLtHash.getMessage().contains("accountsLtHashChecksum"));
    final IllegalArgumentException hugeHardForkData =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.agaveBankHash(
                    parentBankHash,
                    bankSignatureCount,
                    blockhash,
                    accountsLtHash,
                    new byte[1025]));
    assertTrue(hugeHardForkData.getMessage().contains("bankHashHardForkData"));
  }

  @Test
  public void derivesAndValidatesMessageProofHashFromWitnessBranch() {
    final byte[][] branch = new byte[][] {repeatByte((byte) 0x56, 32)};
    final SolanaSccpProver.WitnessInput input = sampleWitnessInput("", branch);
    final String derived =
        SolanaSccpProver.messageProofHash(
            input.sourceEventDigest(),
            input.transactionStatusRoot(),
            input.transactionSignature(),
            input.emitterProgramId(),
            branch);
    final SolanaSccpProver.Witness witness = SolanaSccpProver.normalizeWitness(input);

    assertEquals(derived, witness.messageProofHash());
    assertEquals(1, witness.inclusionBranch().length);
    final IllegalArgumentException zeroDigest =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("00", 32),
                    input.transactionStatusRoot(),
                    input.transactionSignature(),
                    input.emitterProgramId(),
                    branch));
    assertTrue(zeroDigest.getMessage().contains("sourceEventDigest"));
    final IllegalArgumentException zeroRoot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    input.sourceEventDigest(),
                    repeat("00", 32),
                    input.transactionSignature(),
                    input.emitterProgramId(),
                    branch));
    assertTrue(zeroRoot.getMessage().contains("transactionStatusRoot"));
    assertTrue(
        SolanaSccpProver.canonicalWitnessBytes(input).length
            > SolanaSccpProver.canonicalWitnessBytes(sampleWitnessInput()).length);
    final IllegalArgumentException zeroWitnessSignatureEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.normalizeWitness(
                    sampleWitnessInputWithIdentity(
                        repeat("cc", 32),
                        new byte[0][],
                        SOLANA_ZERO_SIGNATURE,
                        SOLANA_PROGRAM_42)));
    assertTrue(zeroWitnessSignatureEx.getMessage().contains("transactionSignature"));
    final IllegalArgumentException zeroWitnessProgramEx =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.normalizeWitness(
                    sampleWitnessInputWithIdentity(
                        repeat("cc", 32),
                        new byte[0][],
                        SOLANA_SIGNATURE_55,
                        SOLANA_ZERO_PROGRAM)));
    assertTrue(zeroWitnessProgramEx.getMessage().contains("emitterProgramId"));

    final IllegalArgumentException ex =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.normalizeWitness(sampleWitnessInput(repeat("cc", 32), branch)));
    assertTrue(ex.getMessage().contains("messageProofHash"));
  }


  @Test
  public void proverRequiresLinkedProofEngine() {
    final IllegalStateException ex =
        assertThrows(
            IllegalStateException.class,
            () -> new SolanaSccpProver().prove(sampleProductionWitnessInput()));
    assertTrue(ex.getMessage().contains("not linked"));
  }

  @Test
  public void callbackRequestSnapshotCopiesSolanaWitnessBytes() {
    final SolanaSccpProver.ProofRequest request =
        SolanaSccpProver.buildProofRequest(sampleProductionWitnessInput());
    final SolanaSccpProver.ProofRequest snapshot =
        SolanaSccpProver.callbackRequestSnapshot(request);

    assertTrue("Solana proof callback must receive a request snapshot", snapshot != request);
    assertEquals(request.version(), snapshot.version());
    assertEquals(request.backend(), snapshot.backend());
    assertEquals(request.sourceDomain(), snapshot.sourceDomain());
    assertEquals(request.targetDomain(), snapshot.targetDomain());
    assertEquals(request.mainnetGenesisHash(), snapshot.mainnetGenesisHash());
    assertEquals(request.witnessHash(), snapshot.witnessHash());
    assertEquals(request.proofContextHash(), snapshot.proofContextHash());
    assertEquals(
        request.sourceAdapterDeploymentBindingHash(),
        snapshot.sourceAdapterDeploymentBindingHash());
    assertEquals(request.sourceStateVerifierId(), snapshot.sourceStateVerifierId());
    assertEquals(request.sourceStateVerifierHash(), snapshot.sourceStateVerifierHash());
    assertEquals(request.publicInputs(), snapshot.publicInputs());
    assertEquals(request.proofContext(), snapshot.proofContext());
    assertEquals(
        request.sourceAdapterDeploymentBinding(), snapshot.sourceAdapterDeploymentBinding());
    assertTrue(
        "Solana witness callback material must be copied",
        snapshot.witness() != request.witness());
    assertArrayEquals(
        SolanaSccpProver.canonicalWitnessBytes(request.witness()),
        SolanaSccpProver.canonicalWitnessBytes(snapshot.witness()));

    final byte[] exposedAccountsLtHash = snapshot.witness().accountsLtHash();
    assertTrue(exposedAccountsLtHash.length > 0);
    exposedAccountsLtHash[0] = (byte) (exposedAccountsLtHash[0] ^ 0x01);
    final byte[][] exposedBranch = snapshot.witness().inclusionBranch();
    assertTrue(exposedBranch.length > 0);
    exposedBranch[0][0] = (byte) (exposedBranch[0][0] ^ 0x01);

    assertArrayEquals(request.witness().accountsLtHash(), snapshot.witness().accountsLtHash());
    final byte[][] requestBranch = request.witness().inclusionBranch();
    final byte[][] snapshotBranch = snapshot.witness().inclusionBranch();
    assertEquals(requestBranch.length, snapshotBranch.length);
    for (int i = 0; i < requestBranch.length; i++) {
      assertArrayEquals(requestBranch[i], snapshotBranch[i]);
    }
  }

  @Test
  public void proverResolvesWitnessProviderBeforeBuildingRequest() {
    final boolean[] resolved = new boolean[] {false};
    final SolanaSccpProver.WitnessInput input = sampleProductionWitnessInput();
    final byte[] originalAccountsLtHash = input.accountsLtHash();
    final byte[] originalHardForkData = input.bankHashHardForkData();
    final byte[][] originalInclusionBranch = input.inclusionBranch();
    final String resolvedDestinationBindingHash =
        SourceSccpProofs.destinationBindingHash(SolanaSccpProver.DOMAIN_SOLANA);
    final SolanaSccpProver.WitnessInput resolvedInput =
        witnessInputWithDestinationBindingHash(input, resolvedDestinationBindingHash);
    final SolanaSccpProver.ProofRequest expectedRequest =
        SolanaSccpProver.buildProofRequest(resolvedInput);
    final SolanaSccpProver prover =
        new SolanaSccpProver(
            unresolved -> {
              assertEquals(repeat("78", 32), unresolved.destinationBindingHash());
              assertTrue(
                  "witness provider must receive a distinct witness input snapshot",
                  unresolved != input);
              final byte[] exposedAccountsLtHash = unresolved.accountsLtHash();
              exposedAccountsLtHash[0] = 0x7f;
              final byte[] exposedHardForkData = unresolved.bankHashHardForkData();
              if (exposedHardForkData.length > 0) {
                exposedHardForkData[0] = 0x7f;
              }
              final byte[][] exposedBranch = unresolved.inclusionBranch();
              exposedBranch[0][0] = 0x7f;
              resolved[0] = true;
              return witnessInputWithDestinationBindingHash(
                  unresolved, resolvedDestinationBindingHash);
            },
            request -> {
              assertTrue("witness provider must run before proof engine", resolved[0]);
              assertEquals(
                  resolvedDestinationBindingHash,
                  request.proofContext().destinationBindingHash());
              assertEquals(expectedRequest.proofContextHash(), request.proofContextHash());
              return new byte[] {1, 2, 3, 4};
            });

    final SolanaSccpProver.ProofResult result = prover.prove(input);

    assertEquals(expectedRequest.witnessHash(), result.witnessHash());
    assertEquals(expectedRequest.proofContextHash(), result.proofContextHash());
    assertArrayEquals(originalAccountsLtHash, input.accountsLtHash());
    assertArrayEquals(originalHardForkData, input.bankHashHardForkData());
    assertEquals(originalInclusionBranch.length, input.inclusionBranch().length);
    for (int i = 0; i < originalInclusionBranch.length; i++) {
      assertArrayEquals(originalInclusionBranch[i], input.inclusionBranch()[i]);
    }
  }

  @Test
  public void requiresProofContext() {
    final IllegalArgumentException statement =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.normalizeProofContext("", repeat("78", 32)));
    assertTrue(statement.getMessage().contains("statementHash"));

    final IllegalArgumentException zeroStatement =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.normalizeProofContext(
                    SolanaSccpProver.ZERO_HASH_V1, repeat("78", 32)));
    assertTrue(zeroStatement.getMessage().contains("statementHash"));

    final IllegalArgumentException binding =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.normalizeProofContext(repeat("56", 32), ""));
    assertTrue(binding.getMessage().contains("destinationBindingHash"));

    final IllegalArgumentException zeroBinding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.normalizeProofContext(
                    repeat("56", 32), SolanaSccpProver.ZERO_HASH_V1));
    assertTrue(zeroBinding.getMessage().contains("destinationBindingHash"));
  }

  @Test
  public void bindsSourceAdapterDeploymentContextForUiProvers() {
    final SolanaSccpProver.SourceAdapterDeploymentBinding zeroBinding =
        SolanaSccpProver.normalizeSourceAdapterDeploymentBinding(
            SolanaSccpProver.ZERO_HASH_V1, SolanaSccpProver.ZERO_HASH_V1);
    assertEquals(SolanaSccpProver.ZERO_HASH_V1, zeroBinding.sourceAdapterDeploymentHash());
    assertEquals(
        SolanaSccpProver.ZERO_HASH_V1, zeroBinding.sourceAdapterDeploymentReceiptHash());
    final IllegalArgumentException zeroRequest =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.buildProofRequest(sampleWitnessInput()));
    assertTrue(
        zeroRequest
            .getMessage()
            .contains("requires non-zero source adapter deployment binding"));

    final SolanaSccpProver.ProofRequest request =
        SolanaSccpProver.buildProofRequest(
            sampleWitnessInput(repeat("cc", 32), new byte[0][], repeat("ab", 32), repeat("cd", 32)));

    assertEquals("0x" + repeat("ab", 32), request.publicInputs().sourceAdapterDeploymentHash());
    assertEquals(
        "0x" + repeat("cd", 32),
        request.publicInputs().sourceAdapterDeploymentReceiptHash());
    assertEquals(
        73,
        SolanaSccpProver.canonicalSourceAdapterDeploymentBindingBytes(
                request.sourceAdapterDeploymentBinding())
            .length);
    assertEquals(
        SolanaSccpProver.sourceAdapterDeploymentBindingHash(
            request.sourceAdapterDeploymentBinding()),
        request.sourceAdapterDeploymentBindingHash());

    final IllegalArgumentException ex =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.normalizeSourceAdapterDeploymentBinding(
                    repeat("ab", 32), SolanaSccpProver.ZERO_HASH_V1));
    assertTrue(ex.getMessage().contains("must both be zero"));

    final IllegalArgumentException reusedRoleHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.normalizeSourceAdapterDeploymentBinding(
                    repeat("ab", 32), repeat("ab", 32)));
    assertTrue(reusedRoleHash.getMessage().contains("must differ"));
  }

  @Test
  public void rejectsUnexpectedSolanaSourceStateVerifierProfile() {
    final IllegalArgumentException ex =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.normalizeWitness(
                    sampleWitnessInput(
                        repeat("cc", 32),
                        new byte[0][],
                        "debug-solana-state-verifier",
                        repeat("ab", 32),
                        SolanaSccpProver.ZERO_HASH_V1,
                        SolanaSccpProver.ZERO_HASH_V1)));
    assertTrue(ex.getMessage().contains("AccountsDB verifier profile"));
  }

  @Test
  public void buildsSolanaProgramInstructionSubmission() {
    final String solanaDestinationBindingHash =
        SourceSccpProofs.destinationBindingHash(SolanaSccpProver.DOMAIN_SOLANA);
    final SolanaSccpProver.ProofRequest canonicalRequest =
        SolanaSccpProver.buildProofRequest(
            witnessInputWithDestinationBindingHash(
                sampleProductionWitnessInput(), solanaDestinationBindingHash));
    final SolanaSccpProver.ProofResult canonicalProofResult =
        SolanaSccpProver.wrapProofResult(new byte[] {1, 2, 3, 4}, canonicalRequest);
    final SolanaSccpProver.SubmissionPublicInputs canonicalPublicInputs =
        sampleSubmissionPublicInputs(canonicalRequest.publicInputs());
    final SolanaSccpProver.Submission submission =
        SolanaSccpProver.buildSubmission(
            new SolanaSccpProver.SubmissionInput(
                canonicalPublicInputs, canonicalProofResult, new byte[] {5, 6, 7}));

    assertEquals(SolanaSccpProver.BORSH_INSTRUCTION_V1, submission.envelopeEncoding());
    assertEquals("program_instruction", submission.submissionKind());
    assertEquals("submit_sccp_message_proof", submission.verifierEntrypoint());
    assertEquals("proof_bytes", submission.arguments().get(0).key());
    assertEquals("public_inputs", submission.arguments().get(1).key());
    assertEquals("bundle_bytes", submission.arguments().get(2).key());
    assertEquals("statement_hash", submission.arguments().get(3).key());
    assertEquals("destination_binding_hash", submission.arguments().get(4).key());
    assertEquals("proof_context_hash", submission.arguments().get(5).key());
    assertEquals(141, submission.publicInputsBytes().length);
    assertEquals(
        SolanaSccpProver.proofContextHash(
            SolanaSccpProver.normalizeProofContext(repeat("56", 32), solanaDestinationBindingHash)),
        submission.proofContextHash());
    assertEquals(submission.instructionDataHex(), submission.envelopeHex());
    assertEquals(
        "submit_sccp_message_proof",
        new String(Arrays.copyOfRange(submission.instructionData(), 4, 29), StandardCharsets.UTF_8));

    final SolanaSccpProver.Submission proofResultSubmission =
        SolanaSccpProver.buildSubmission(
            new SolanaSccpProver.SubmissionInput(
                canonicalPublicInputs, canonicalProofResult, new byte[] {5, 6, 7}));
    assertEquals(canonicalProofResult.proofContextHash(), proofResultSubmission.proofContextHash());
    final SolanaSccpProver.ProofResult uppercaseProofResult =
        proofResultWithPublicInputs(
            proofResultWithProofContext(
                canonicalProofResult,
                new SolanaSccpProver.ProofContext(
                    1,
                    upper(canonicalProofResult.proofContext().statementHash()),
                    upper(canonicalProofResult.proofContext().destinationBindingHash()))),
            publicInputsWithProofContext(
                canonicalProofResult.publicInputs(),
                upper(canonicalProofResult.publicInputs().statementHash()),
                upper(canonicalProofResult.publicInputs().destinationBindingHash())));
    final SolanaSccpProver.Submission normalizedMetadataSubmission =
        SolanaSccpProver.buildSubmission(
            new SolanaSccpProver.SubmissionInput(
                new SolanaSccpProver.SubmissionPublicInputs(
                    upper(canonicalPublicInputs.messageId()),
                    upper(canonicalPublicInputs.payloadHash()),
                    canonicalPublicInputs.targetDomain(),
                    upper(canonicalPublicInputs.commitmentRoot()),
                    canonicalPublicInputs.finalityHeight(),
                    upper(canonicalPublicInputs.finalityBlockHash())),
                uppercaseProofResult.proofBytes(),
                new byte[] {5, 6, 7},
                upper(canonicalProofResult.proofContext().statementHash()),
                upper(solanaDestinationBindingHash),
                upper(canonicalProofResult.proofContextHash()),
                uppercaseProofResult));
    assertEquals(
        canonicalProofResult.proofContextHash(), normalizedMetadataSubmission.proofContextHash());
    assertEquals(
        canonicalProofResult.proofContext().statementHash(),
        normalizedMetadataSubmission.statementHash());
    assertEquals(solanaDestinationBindingHash, normalizedMetadataSubmission.destinationBindingHash());

    final IllegalArgumentException missingEnvelope =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithEnvelopeHash(
                        canonicalProofResult, SolanaSccpProver.ZERO_HASH_V1),
                    new byte[] {5, 6, 7}));
    assertTrue(missingEnvelope.getMessage().contains("envelopeHash"));

    final IllegalArgumentException tamperedEnvelope =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithEnvelopeHash(
                        canonicalProofResult, "0x" + repeat("aa", 32)),
                    new byte[] {5, 6, 7}));
    assertTrue(tamperedEnvelope.getMessage().contains("wrapped proof bytes"));

    final IllegalArgumentException mismatchedProofContext =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithProofContextHash(
                        canonicalProofResult, "0x" + repeat("99", 32)),
                    new byte[] {5, 6, 7}));
    assertTrue(mismatchedProofContext.getMessage().contains("proofContextHash"));

    final IllegalArgumentException mismatchedProofResultVersion =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithVersion(canonicalProofResult, 2),
                    new byte[] {5, 6, 7}));
    assertTrue(mismatchedProofResultVersion.getMessage().contains("proofResult.version"));

    final IllegalArgumentException mismatchedProofBase64 =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithProofBase64(canonicalProofResult, "AAAA"),
                    new byte[] {5, 6, 7}));
    assertTrue(mismatchedProofBase64.getMessage().contains("proofBase64"));

    final IllegalArgumentException zeroWitnessHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithWitnessHash(
                        canonicalProofResult, SolanaSccpProver.ZERO_HASH_V1),
                    new byte[] {5, 6, 7}));
    assertTrue(zeroWitnessHash.getMessage().contains("proofResult.witnessHash"));

    final IllegalArgumentException mismatchedProofContextVersion =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithProofContext(
                        canonicalProofResult,
                        new SolanaSccpProver.ProofContext(
                            2,
                            canonicalProofResult.proofContext().statementHash(),
                            canonicalProofResult.proofContext().destinationBindingHash())),
                    new byte[] {5, 6, 7}));
    assertTrue(mismatchedProofContextVersion.getMessage().contains("proofContext.version"));

    final IllegalArgumentException mismatchedSourceVerifier =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithSourceStateVerifierHash(
                        canonicalProofResult,
                        SolanaSccpProver.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1),
                    new byte[] {5, 6, 7}));
    assertTrue(mismatchedSourceVerifier.getMessage().contains("template verifier"));

    final IllegalArgumentException mismatchedDeploymentBindingVersion =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithSourceAdapterDeploymentBinding(
                        canonicalProofResult,
                        new SolanaSccpProver.SourceAdapterDeploymentBinding(
                            2,
                            canonicalProofResult.sourceAdapterDeploymentBinding().sourceDomain(),
                            canonicalProofResult.sourceAdapterDeploymentBinding().targetDomain(),
                            canonicalProofResult
                                .sourceAdapterDeploymentBinding()
                                .sourceAdapterDeploymentHash(),
                            canonicalProofResult
                                .sourceAdapterDeploymentBinding()
                                .sourceAdapterDeploymentReceiptHash())),
                    new byte[] {5, 6, 7}));
    assertTrue(
        mismatchedDeploymentBindingVersion
            .getMessage()
            .contains("sourceAdapterDeploymentBinding.version"));

    final IllegalArgumentException mismatchedDeploymentBinding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithSourceAdapterDeploymentBinding(
                        canonicalProofResult,
                        new SolanaSccpProver.SourceAdapterDeploymentBinding(
                            canonicalProofResult.sourceAdapterDeploymentBinding().version(),
                            canonicalProofResult.sourceAdapterDeploymentBinding().sourceDomain(),
                            canonicalProofResult.sourceAdapterDeploymentBinding().targetDomain(),
                            "0x" + repeat("ee", 32),
                            canonicalProofResult
                                .sourceAdapterDeploymentBinding()
                                .sourceAdapterDeploymentReceiptHash())),
                    new byte[] {5, 6, 7}));
    assertTrue(
        mismatchedDeploymentBinding
            .getMessage()
            .contains("sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding"));

    final IllegalArgumentException mismatchedDeploymentPublicInputs =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithPublicInputs(
                        canonicalProofResult,
                        publicInputsWithSourceAdapterDeploymentHash(
                            canonicalProofResult.publicInputs(), "0x" + repeat("ee", 32))),
                    new byte[] {5, 6, 7}));
    assertTrue(
        mismatchedDeploymentPublicInputs
            .getMessage()
            .contains("publicInputs.sourceAdapterDeploymentHash"));

    final IllegalArgumentException mismatchedPublicInputSourceVerifierId =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithPublicInputs(
                        canonicalProofResult,
                        publicInputsWithSourceStateVerifier(
                            canonicalProofResult.publicInputs(),
                            "sccp:solana:wrong-source-state-verifier:v1",
                            canonicalProofResult.publicInputs().sourceStateVerifierHash())),
                    new byte[] {5, 6, 7}));
    assertTrue(
        mismatchedPublicInputSourceVerifierId
            .getMessage()
            .contains("publicInputs.sourceStateVerifierId"));

    final IllegalArgumentException mismatchedPublicInputSourceVerifierHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithPublicInputs(
                        canonicalProofResult,
                        publicInputsWithSourceStateVerifier(
                            canonicalProofResult.publicInputs(),
                            canonicalProofResult.publicInputs().sourceStateVerifierId(),
                            "0x" + repeat("dd", 32))),
                    new byte[] {5, 6, 7}));
    assertTrue(
        mismatchedPublicInputSourceVerifierHash
            .getMessage()
            .contains("publicInputs.sourceStateVerifierHash"));

    final IllegalArgumentException mismatchedPublicInputParentSlot =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithPublicInputs(
                        canonicalProofResult,
                        publicInputsWithParentSlot(
                            canonicalProofResult.publicInputs(),
                            canonicalProofResult.publicInputs().finalizedSlot())),
                    new byte[] {5, 6, 7}));
    assertTrue(
        mismatchedPublicInputParentSlot.getMessage().contains("publicInputs.parentSlot"));

    final IllegalArgumentException zeroPublicInputMessageProofHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    canonicalPublicInputs,
                    proofResultWithPublicInputs(
                        canonicalProofResult,
                        publicInputsWithMessageProofHash(
                            canonicalProofResult.publicInputs(),
                            SolanaSccpProver.ZERO_HASH_V1)),
                    new byte[] {5, 6, 7}));
    assertTrue(
        zeroPublicInputMessageProofHash.getMessage().contains("publicInputs.messageProofHash"));

    final IllegalArgumentException mismatchedMessage =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver.SubmissionInput(
                    new SolanaSccpProver.SubmissionPublicInputs(
                        repeat("aa", 32),
                        canonicalPublicInputs.payloadHash(),
                        canonicalPublicInputs.targetDomain(),
                        canonicalPublicInputs.commitmentRoot(),
                        canonicalPublicInputs.finalityHeight(),
                        canonicalPublicInputs.finalityBlockHash()),
                    canonicalProofResult,
                    new byte[] {5, 6, 7}));
    assertTrue(mismatchedMessage.getMessage().contains("messageId"));

    final IllegalArgumentException rawInput =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildSubmission(
                    new SolanaSccpProver.SubmissionInput(
                        sampleSubmissionPublicInputs(),
                        new byte[] {0, 0},
                        new byte[] {2},
                        repeat("56", 32),
                        repeat("78", 32),
                        null)));
    assertTrue(rawInput.getMessage().contains("proofResult"));

    final IllegalArgumentException wrongTarget =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildSubmission(
                    new SolanaSccpProver.SubmissionInput(
                        sampleSubmissionPublicInputs(SolanaSccpProver.DOMAIN_SORA),
                        new byte[] {1, 2},
                        new byte[] {5, 6, 7},
                        repeat("56", 32),
                        solanaDestinationBindingHash,
                        null)));
    assertTrue(wrongTarget.getMessage().contains("publicInputs.targetDomain"));

    final IllegalArgumentException wrongPublicInputVersion =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildSubmission(
                    new SolanaSccpProver.SubmissionInput(
                        sampleSubmissionPublicInputs(2, SolanaSccpProver.DOMAIN_SOLANA),
                        new byte[] {1, 2},
                        new byte[] {5, 6, 7},
                        repeat("56", 32),
                        solanaDestinationBindingHash,
                        null)));
    assertTrue(wrongPublicInputVersion.getMessage().contains("publicInputs.version"));

    final IllegalArgumentException wrongBinding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildSubmission(
                    new SolanaSccpProver.SubmissionInput(
                        canonicalPublicInputs,
                        canonicalProofResult.proofBytes(),
                        new byte[] {2},
                        canonicalProofResult.proofContext().statementHash(),
                        repeat("78", 32),
                        canonicalProofResult.proofContextHash(),
                        canonicalProofResult)));
    assertTrue(wrongBinding.getMessage().contains("destinationBindingHash"));

    final IllegalArgumentException zeroBundle =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildSubmission(
                    new SolanaSccpProver.SubmissionInput(
                        canonicalPublicInputs,
                        canonicalProofResult.proofBytes(),
                        new byte[] {0, 0},
                        canonicalProofResult.proofContext().statementHash(),
                        solanaDestinationBindingHash,
                        canonicalProofResult.proofContextHash(),
                        canonicalProofResult)));
    assertTrue(zeroBundle.getMessage().contains("bundleBytes must not be all zero"));

    final IllegalArgumentException oversizedBundle =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildSubmission(
                    new SolanaSccpProver.SubmissionInput(
                        canonicalPublicInputs,
                        canonicalProofResult.proofBytes(),
                        filledBytes(SolanaSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1, 1),
                        canonicalProofResult.proofContext().statementHash(),
                        solanaDestinationBindingHash,
                        canonicalProofResult.proofContextHash(),
                        canonicalProofResult)));
    assertTrue(oversizedBundle.getMessage().contains("bundleBytes must be at most"));

    final IllegalArgumentException ex =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.buildSubmission(
                    new SolanaSccpProver.SubmissionInput(
                        canonicalPublicInputs,
                        canonicalProofResult.proofBytes(),
                        new byte[] {2},
                        canonicalProofResult.proofContext().statementHash(),
                        solanaDestinationBindingHash,
                        repeat("cc", 32),
                        canonicalProofResult)));
    assertTrue(ex.getMessage().contains("proofContextHash"));
  }

  @Test
  public void proverWrapsExternalProofBytes() {
    final SolanaSccpProver.WitnessInput productionWitness = sampleProductionWitnessInput();
    final SolanaSccpProver.ProofRequest[] seenRequest = new SolanaSccpProver.ProofRequest[1];
    final SolanaSccpProver prover =
        new SolanaSccpProver(
            null,
            request -> {
              seenRequest[0] = request;
              assertEquals(SolanaSccpProver.RECURSIVE_PROOF_BACKEND_V1, request.backend());
              assertEquals("0x" + repeat("56", 32), request.proofContext().statementHash());
              return new byte[] {1, 2, 3, 4};
            });

    final SolanaSccpProver.ProofResult result = prover.prove(productionWitness);
    assertArrayEquals(new byte[] {1, 2, 3, 4}, result.proofBytes());
    assertEquals("AQIDBA==", result.proofBase64());
    final SolanaSccpProver.ProofRequest request =
        SolanaSccpProver.buildProofRequest(productionWitness);
    assertEquals(
        request.proofContextHash(), result.proofContextHash());
    assertTrue(result.envelopeHash().matches("0x[0-9a-f]{64}"));
    assertTrue(seenRequest[0] != request);
    assertEquals(request.proofContextHash(), seenRequest[0].proofContextHash());
    assertEquals(request.witnessHash(), seenRequest[0].witnessHash());
    assertEquals(
        request.sourceAdapterDeploymentBindingHash(),
        seenRequest[0].sourceAdapterDeploymentBindingHash());
    final IllegalArgumentException zeroProof =
        assertThrows(
            IllegalArgumentException.class,
            () -> SolanaSccpProver.wrapProofResult(new byte[] {0, 0}, request));
    assertTrue(zeroProof.getMessage().contains("all zero"));
    final IllegalArgumentException oversizedProof =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapProofResult(
                    filledBytes(SolanaSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1, 1),
                    request));
    assertTrue(oversizedProof.getMessage().contains("at most"));
    final IllegalArgumentException wrongBackend =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapProofResult(
                    new byte[] {1}, solanaRequestWithBackend(request, "debug-solana-backend")));
    assertTrue(wrongBackend.getMessage().contains("sccp-solana-recursive-mainnet-v1"));

    final IllegalArgumentException wrongContext =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.wrapProofResult(
                    new byte[] {1},
                    solanaRequestWithProofContextHash(request, "0x" + repeat("99", 32))));
    assertTrue(wrongContext.getMessage().contains("canonical"));

    final IllegalArgumentException wrongGenesis =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver(
                        null,
                        ignored -> {
                          throw new AssertionError("local prover should not be invoked");
                        })
                    .prove(sampleProductionWitnessInput("devnet")));
    assertTrue(wrongGenesis.getMessage().contains("mainnetGenesisHash"));

    final IllegalArgumentException missingAccountsLtHash =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver(
                        null,
                        ignored -> {
                          throw new AssertionError("local prover should not be invoked");
                        })
                    .prove(
                        sampleProductionWitnessInput(
                            SolanaSccpProver.MAINNET_GENESIS_HASH, null)));
    assertTrue(missingAccountsLtHash.getMessage().contains("accountsLtHash"));

    final IllegalArgumentException missingProductionBinding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver(
                        null,
                        ignored -> {
                          throw new AssertionError("local prover should not be invoked");
                        })
                    .prove(
                        sampleWitnessInput(
                            repeat("cc", 32),
                            new byte[0][],
                            repeat("ab", 32),
                            repeat("cd", 32))));
    assertTrue(missingProductionBinding.getMessage().contains("sourceStateVerifierHash"));
    final IllegalArgumentException templateProductionBinding =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver(
                        null,
                        ignored -> {
                          throw new AssertionError("local prover should not be invoked");
                        })
                    .prove(
                        sampleProductionWitnessInput(
                            SolanaSccpProver.MAINNET_GENESIS_HASH,
                            SolanaSccpProver.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
                            productionAccountsLtHash())));
    assertTrue(templateProductionBinding.getMessage().contains("Solana template verifier hash"));
    final IllegalArgumentException missingInclusionBranch =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                new SolanaSccpProver(
                        null,
                        ignored -> {
                          throw new AssertionError("local prover should not be invoked");
                        })
                    .prove(
                        sampleWitnessInput(
                            repeat("cc", 32),
                            new byte[0][],
                            SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
                            repeat("ef", 32),
                            repeat("ab", 32),
                            repeat("cd", 32))));
    assertTrue(missingInclusionBranch.getMessage().contains("inclusionBranch"));
  }

  private static SolanaSccpProver.ProofRequest solanaRequestWithBackend(
      final SolanaSccpProver.ProofRequest request, final String backend) {
    return new SolanaSccpProver.ProofRequest(
        request.version(),
        backend,
        request.sourceDomain(),
        request.targetDomain(),
        request.mainnetGenesisHash(),
        request.witnessHash(),
        request.proofContextHash(),
        request.sourceAdapterDeploymentBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.publicInputs(),
        request.witness(),
        request.proofContext(),
        request.sourceAdapterDeploymentBinding());
  }

  private static SolanaSccpProver.ProofRequest solanaRequestWithProofContextHash(
      final SolanaSccpProver.ProofRequest request, final String proofContextHash) {
    return new SolanaSccpProver.ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.mainnetGenesisHash(),
        request.witnessHash(),
        proofContextHash,
        request.sourceAdapterDeploymentBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.publicInputs(),
        request.witness(),
        request.proofContext(),
        request.sourceAdapterDeploymentBinding());
  }

  private static SolanaSccpProver.ProofResult proofResultWithEnvelopeHash(
      final SolanaSccpProver.ProofResult result, final String envelopeHash) {
    return copyProofResult(
        result, result.proofContextHash(), result.sourceStateVerifierHash(), envelopeHash);
  }

  private static SolanaSccpProver.ProofResult proofResultWithVersion(
      final SolanaSccpProver.ProofResult result, final int version) {
    return new SolanaSccpProver.ProofResult(
        version,
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.witnessHash(),
        result.proofContextHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.proofContext(),
        result.sourceAdapterDeploymentBinding(),
        result.envelopeHash());
  }

  private static SolanaSccpProver.ProofResult proofResultWithProofBase64(
      final SolanaSccpProver.ProofResult result, final String proofBase64) {
    return new SolanaSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        proofBase64,
        result.publicInputs(),
        result.witnessHash(),
        result.proofContextHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.proofContext(),
        result.sourceAdapterDeploymentBinding(),
        result.envelopeHash());
  }

  private static SolanaSccpProver.ProofResult proofResultWithProofContextHash(
      final SolanaSccpProver.ProofResult result, final String proofContextHash) {
    return copyProofResult(
        result, proofContextHash, result.sourceStateVerifierHash(), result.envelopeHash());
  }

  private static SolanaSccpProver.ProofResult proofResultWithWitnessHash(
      final SolanaSccpProver.ProofResult result, final String witnessHash) {
    return new SolanaSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        witnessHash,
        result.proofContextHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.proofContext(),
        result.sourceAdapterDeploymentBinding(),
        result.envelopeHash());
  }

  private static SolanaSccpProver.ProofResult proofResultWithProofContext(
      final SolanaSccpProver.ProofResult result,
      final SolanaSccpProver.ProofContext proofContext) {
    return new SolanaSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.witnessHash(),
        result.proofContextHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        proofContext,
        result.sourceAdapterDeploymentBinding(),
        result.envelopeHash());
  }

  private static SolanaSccpProver.ProofResult proofResultWithSourceStateVerifierHash(
      final SolanaSccpProver.ProofResult result, final String sourceStateVerifierHash) {
    return copyProofResult(
        result, result.proofContextHash(), sourceStateVerifierHash, result.envelopeHash());
  }

  private static SolanaSccpProver.ProofResult proofResultWithSourceAdapterDeploymentBinding(
      final SolanaSccpProver.ProofResult result,
      final SolanaSccpProver.SourceAdapterDeploymentBinding binding) {
    return new SolanaSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.witnessHash(),
        result.proofContextHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.proofContext(),
        binding,
        result.envelopeHash());
  }

  private static SolanaSccpProver.ProofResult proofResultWithPublicInputs(
      final SolanaSccpProver.ProofResult result,
      final SolanaSccpProver.PublicInputs publicInputs) {
    return new SolanaSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        publicInputs,
        result.witnessHash(),
        result.proofContextHash(),
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceStateVerifierId(),
        result.sourceStateVerifierHash(),
        result.proofContext(),
        result.sourceAdapterDeploymentBinding(),
        result.envelopeHash());
  }

  private static SolanaSccpProver.PublicInputs publicInputsWithSourceAdapterDeploymentHash(
      final SolanaSccpProver.PublicInputs inputs, final String sourceAdapterDeploymentHash) {
    return new SolanaSccpProver.PublicInputs(
        inputs.messageId(),
        inputs.payloadHash(),
        inputs.commitmentRoot(),
        inputs.finalizedSlot(),
        inputs.parentSlot(),
        inputs.bankSignatureCount(),
        inputs.parentBankHash(),
        inputs.blockhash(),
        inputs.bankHash(),
        inputs.transactionStatusRoot(),
        inputs.messageProofHash(),
        inputs.accountInclusionRoot(),
        inputs.accountsLtHashChecksum(),
        inputs.accountsLtHashProofPublicInputsHash(),
        inputs.sourceEventDigest(),
        inputs.sourceStateVerifierId(),
        inputs.sourceStateVerifierHash(),
        inputs.statementHash(),
        inputs.destinationBindingHash(),
        sourceAdapterDeploymentHash,
        inputs.sourceAdapterDeploymentReceiptHash(),
        inputs.sourceAdapterDeploymentBindingHash());
  }

  private static SolanaSccpProver.PublicInputs publicInputsWithSourceStateVerifier(
      final SolanaSccpProver.PublicInputs inputs,
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash) {
    return new SolanaSccpProver.PublicInputs(
        inputs.messageId(),
        inputs.payloadHash(),
        inputs.commitmentRoot(),
        inputs.finalizedSlot(),
        inputs.parentSlot(),
        inputs.bankSignatureCount(),
        inputs.parentBankHash(),
        inputs.blockhash(),
        inputs.bankHash(),
        inputs.transactionStatusRoot(),
        inputs.messageProofHash(),
        inputs.accountInclusionRoot(),
        inputs.accountsLtHashChecksum(),
        inputs.accountsLtHashProofPublicInputsHash(),
        inputs.sourceEventDigest(),
        sourceStateVerifierId,
        sourceStateVerifierHash,
        inputs.statementHash(),
        inputs.destinationBindingHash(),
        inputs.sourceAdapterDeploymentHash(),
        inputs.sourceAdapterDeploymentReceiptHash(),
        inputs.sourceAdapterDeploymentBindingHash());
  }

  private static SolanaSccpProver.PublicInputs publicInputsWithProofContext(
      final SolanaSccpProver.PublicInputs inputs,
      final String statementHash,
      final String destinationBindingHash) {
    return new SolanaSccpProver.PublicInputs(
        inputs.messageId(),
        inputs.payloadHash(),
        inputs.commitmentRoot(),
        inputs.finalizedSlot(),
        inputs.parentSlot(),
        inputs.bankSignatureCount(),
        inputs.parentBankHash(),
        inputs.blockhash(),
        inputs.bankHash(),
        inputs.transactionStatusRoot(),
        inputs.messageProofHash(),
        inputs.accountInclusionRoot(),
        inputs.accountsLtHashChecksum(),
        inputs.accountsLtHashProofPublicInputsHash(),
        inputs.sourceEventDigest(),
        inputs.sourceStateVerifierId(),
        inputs.sourceStateVerifierHash(),
        statementHash,
        destinationBindingHash,
        inputs.sourceAdapterDeploymentHash(),
        inputs.sourceAdapterDeploymentReceiptHash(),
        inputs.sourceAdapterDeploymentBindingHash());
  }

  private static SolanaSccpProver.PublicInputs publicInputsWithParentSlot(
      final SolanaSccpProver.PublicInputs inputs, final String parentSlot) {
    return new SolanaSccpProver.PublicInputs(
        inputs.messageId(),
        inputs.payloadHash(),
        inputs.commitmentRoot(),
        inputs.finalizedSlot(),
        parentSlot,
        inputs.bankSignatureCount(),
        inputs.parentBankHash(),
        inputs.blockhash(),
        inputs.bankHash(),
        inputs.transactionStatusRoot(),
        inputs.messageProofHash(),
        inputs.accountInclusionRoot(),
        inputs.accountsLtHashChecksum(),
        inputs.accountsLtHashProofPublicInputsHash(),
        inputs.sourceEventDigest(),
        inputs.sourceStateVerifierId(),
        inputs.sourceStateVerifierHash(),
        inputs.statementHash(),
        inputs.destinationBindingHash(),
        inputs.sourceAdapterDeploymentHash(),
        inputs.sourceAdapterDeploymentReceiptHash(),
        inputs.sourceAdapterDeploymentBindingHash());
  }

  private static SolanaSccpProver.PublicInputs publicInputsWithMessageProofHash(
      final SolanaSccpProver.PublicInputs inputs, final String messageProofHash) {
    return new SolanaSccpProver.PublicInputs(
        inputs.messageId(),
        inputs.payloadHash(),
        inputs.commitmentRoot(),
        inputs.finalizedSlot(),
        inputs.parentSlot(),
        inputs.bankSignatureCount(),
        inputs.parentBankHash(),
        inputs.blockhash(),
        inputs.bankHash(),
        inputs.transactionStatusRoot(),
        messageProofHash,
        inputs.accountInclusionRoot(),
        inputs.accountsLtHashChecksum(),
        inputs.accountsLtHashProofPublicInputsHash(),
        inputs.sourceEventDigest(),
        inputs.sourceStateVerifierId(),
        inputs.sourceStateVerifierHash(),
        inputs.statementHash(),
        inputs.destinationBindingHash(),
        inputs.sourceAdapterDeploymentHash(),
        inputs.sourceAdapterDeploymentReceiptHash(),
        inputs.sourceAdapterDeploymentBindingHash());
  }

  private static SolanaSccpProver.ProofResult copyProofResult(
      final SolanaSccpProver.ProofResult result,
      final String proofContextHash,
      final String sourceStateVerifierHash,
      final String envelopeHash) {
    return new SolanaSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.witnessHash(),
        proofContextHash,
        result.sourceAdapterDeploymentBindingHash(),
        result.sourceStateVerifierId(),
        sourceStateVerifierHash,
        result.proofContext(),
        result.sourceAdapterDeploymentBinding(),
        envelopeHash);
  }

  private static String upper(final String value) {
    return value.toUpperCase(Locale.ROOT);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput() {
    return sampleWitnessInput(repeat("cc", 32), new byte[0][]);
  }

  private static SolanaSccpProver.WitnessInput retargetWitnessInput(final int targetDomain) {
    final SolanaSccpProver.WitnessInput input = sampleWitnessInput();
    return new SolanaSccpProver.WitnessInput(
        targetDomain,
        input.mainnetGenesisHash(),
        input.finalizedSlot(),
        input.parentSlot(),
        input.bankSignatureCount(),
        input.parentBankHash(),
        input.blockhash(),
        input.bankHash(),
        input.transactionStatusRoot(),
        input.messageProofHash(),
        input.accountInclusionRoot(),
        input.accountsLtHashChecksum(),
        input.accountsLtHashProofPublicInputsHash(),
        input.bankHashHardForkData(),
        input.accountsLtHash(),
        input.transactionSignature(),
        input.emitterProgramId(),
        input.messageId(),
        input.payloadHash(),
        input.commitmentRoot(),
        input.sourceEventDigest(),
        input.sourceStateVerifierId(),
        input.sourceStateVerifierHash(),
        input.statementHash(),
        input.destinationBindingHash(),
        input.inclusionBranch(),
        input.sourceAdapterDeploymentHash(),
        input.sourceAdapterDeploymentReceiptHash());
  }

  private static SolanaSccpProver.WitnessInput witnessInputWithBlockhash(
      final SolanaSccpProver.WitnessInput input, final String blockhash) {
    return new SolanaSccpProver.WitnessInput(
        input.targetDomain(),
        input.mainnetGenesisHash(),
        input.finalizedSlot(),
        input.parentSlot(),
        input.bankSignatureCount(),
        input.parentBankHash(),
        blockhash,
        input.bankHash(),
        input.transactionStatusRoot(),
        input.messageProofHash(),
        input.accountInclusionRoot(),
        input.accountsLtHashChecksum(),
        input.accountsLtHashProofPublicInputsHash(),
        input.bankHashHardForkData(),
        input.accountsLtHash(),
        input.transactionSignature(),
        input.emitterProgramId(),
        input.messageId(),
        input.payloadHash(),
        input.commitmentRoot(),
        input.sourceEventDigest(),
        input.sourceStateVerifierId(),
        input.sourceStateVerifierHash(),
        input.statementHash(),
        input.destinationBindingHash(),
        input.inclusionBranch(),
        input.sourceAdapterDeploymentHash(),
        input.sourceAdapterDeploymentReceiptHash());
  }

  private static SolanaSccpProver.WitnessInput witnessInputWithDestinationBindingHash(
      final SolanaSccpProver.WitnessInput input, final String destinationBindingHash) {
    return new SolanaSccpProver.WitnessInput(
        input.targetDomain(),
        input.mainnetGenesisHash(),
        input.finalizedSlot(),
        input.parentSlot(),
        input.bankSignatureCount(),
        input.parentBankHash(),
        input.blockhash(),
        input.bankHash(),
        input.transactionStatusRoot(),
        input.messageProofHash(),
        input.accountInclusionRoot(),
        input.accountsLtHashChecksum(),
        input.accountsLtHashProofPublicInputsHash(),
        input.bankHashHardForkData(),
        input.accountsLtHash(),
        input.transactionSignature(),
        input.emitterProgramId(),
        input.messageId(),
        input.payloadHash(),
        input.commitmentRoot(),
        input.sourceEventDigest(),
        input.sourceStateVerifierId(),
        input.sourceStateVerifierHash(),
        input.statementHash(),
        destinationBindingHash,
        input.inclusionBranch(),
        input.sourceAdapterDeploymentHash(),
        input.sourceAdapterDeploymentReceiptHash());
  }

  private static SolanaSccpProver.WitnessInput sampleProductionWitnessInput() {
    return sampleProductionWitnessInput(SolanaSccpProver.MAINNET_GENESIS_HASH);
  }

  private static SolanaSccpProver.WitnessInput sampleProductionWitnessInput(
      final String mainnetGenesisHash) {
    return sampleProductionWitnessInput(mainnetGenesisHash, productionAccountsLtHash());
  }

  private static SolanaSccpProver.WitnessInput sampleProductionWitnessInput(
      final String mainnetGenesisHash, final byte[] accountsLtHash) {
    return sampleProductionWitnessInput(mainnetGenesisHash, repeat("ef", 32), accountsLtHash);
  }

  private static SolanaSccpProver.WitnessInput sampleProductionWitnessInput(
      final String mainnetGenesisHash,
      final String sourceStateVerifierHash,
      final byte[] accountsLtHash) {
    final byte[][] branch = new byte[][] {repeatByte((byte) 0x56, 32)};
    final String sourceEventDigest = repeat("34", 32);
    final String blockhash = repeat("9a", 32);
    final String transactionStatusRoot =
        SolanaSccpProver.transactionStatusRootFromBranch(
            sourceEventDigest, SOLANA_SIGNATURE_55, SOLANA_PROGRAM_42, branch);
    final String messageProofHash =
        SolanaSccpProver.messageProofHash(
            sourceEventDigest,
            transactionStatusRoot,
            SOLANA_SIGNATURE_55,
            SOLANA_PROGRAM_42,
            branch);
    final String accountsLtHashChecksum =
        accountsLtHash == null
            ? repeat("88", 32)
            : SolanaSccpProver.accountsLtHashChecksum(accountsLtHash);
    final String bankHash =
        accountsLtHash == null
            ? repeat("aa", 32)
            : SolanaSccpProver.agaveBankHash(
                repeat("99", 32), "8", blockhash, accountsLtHash);
    return sampleWitnessInput(
        messageProofHash,
        branch,
        SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
        sourceStateVerifierHash,
        repeat("ab", 32),
        repeat("cd", 32),
        SOLANA_SIGNATURE_55,
        SOLANA_PROGRAM_42,
        mainnetGenesisHash,
        blockhash,
        bankHash,
        accountsLtHashChecksum,
        accountsLtHash);
  }

  private static SolanaSccpProver.AccountsLtHashProofRequest accountsLtHashRequest(
      final SolanaSccpProver.AccountsLtHashProofRequest request,
      final String accountsLtHashProofPublicInputsHash,
      final List<List<String>> publicInputColumns,
      final SolanaSccpProver.AccountsLtHashFastpqPublicInputs fastpqPublicInputs,
      final List<SolanaSccpProver.AccountsLtHashFastpqTransition> fastpqTransitions) {
    return new SolanaSccpProver.AccountsLtHashProofRequest(
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.parameterSet(),
        request.sourceDomain(),
        request.finalizedSlot(),
        request.parentSlot(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        accountsLtHashProofPublicInputsHash == null
            ? request.accountsLtHashProofPublicInputsHash()
            : accountsLtHashProofPublicInputsHash,
        request.openedAccountsLtHashContributionsHash(),
        request.openedAccountsLtHashResidualChecksum(),
        request.statementBytes(),
        request.accountCommitmentBytes(),
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        publicInputColumns == null ? request.publicInputColumns() : publicInputColumns,
        fastpqPublicInputs == null ? request.fastpqPublicInputs() : fastpqPublicInputs,
        fastpqTransitions == null ? request.fastpqTransitions() : fastpqTransitions);
  }

  private static SolanaSccpProver.FullLightClientAuditProofRequest fullLightClientAuditRequest(
      final SolanaSccpProver.FullLightClientAuditProofRequest request,
      final String auditStatementHash,
      final List<List<String>> publicInputColumns,
      final SolanaSccpProver.FullLightClientAuditFastpqPublicInputs fastpqPublicInputs,
      final List<SolanaSccpProver.FullLightClientAuditFastpqTransition> fastpqTransitions) {
    return fullLightClientAuditRequest(
        request,
        request.verifierHash(),
        auditStatementHash,
        publicInputColumns,
        fastpqPublicInputs,
        fastpqTransitions);
  }

  private static SolanaSccpProver.FullLightClientAuditProofRequest
      fullLightClientAuditRequestWithVerifierHash(
          final SolanaSccpProver.FullLightClientAuditProofRequest request,
          final String verifierHash) {
    return fullLightClientAuditRequest(request, verifierHash, null, null, null, null);
  }

  private static SolanaSccpProver.FullLightClientAuditProofRequest fullLightClientAuditRequest(
      final SolanaSccpProver.FullLightClientAuditProofRequest request,
      final String verifierHash,
      final String auditStatementHash,
      final List<List<String>> publicInputColumns,
      final SolanaSccpProver.FullLightClientAuditFastpqPublicInputs fastpqPublicInputs,
      final List<SolanaSccpProver.FullLightClientAuditFastpqTransition> fastpqTransitions) {
    return new SolanaSccpProver.FullLightClientAuditProofRequest(
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.parameterSet(),
        request.role(),
        request.roleCode(),
        request.sourceDomain(),
        request.finalizedSlot(),
        request.verifierId(),
        verifierHash,
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.sourceVerifierMaterialHash(),
        request.sourceAdapterDeploymentHash(),
        request.fullLightClientGateHash(),
        request.finalityContextHash(),
        request.voteMessageHash(),
        request.accountsLtHashProofHash(),
        auditStatementHash == null ? request.auditStatementHash() : auditStatementHash,
        request.statementBytes(),
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        publicInputColumns == null ? request.publicInputColumns() : publicInputColumns,
        fastpqPublicInputs == null ? request.fastpqPublicInputs() : fastpqPublicInputs,
        fastpqTransitions == null ? request.fastpqTransitions() : fastpqTransitions);
  }

  private static SolanaSccpProver.RouteCanaryEvidenceInput sampleSolanaRouteCanaryEvidence(
      final String solanaProgramdataSlot, final String solanaProgramdataExecutableBase64) {
    return sampleSolanaRouteCanaryEvidence(
        solanaProgramdataSlot, solanaProgramdataExecutableBase64, null, null);
  }

  private static SolanaSccpProver.RouteCanaryEvidenceInput sampleSolanaRouteCanaryEvidence(
      final String solanaProgramdataSlot,
      final String solanaProgramdataExecutableBase64,
      final String destinationBindingHash,
      final String expectedDestinationBindingHash) {
    return new SolanaSccpProver.RouteCanaryEvidenceInput(
        "0x" + repeat("31", 32),
        destinationBindingHash == null
            ? SourceSccpProofs.destinationBindingHash(SolanaSccpProver.DOMAIN_SOLANA)
            : destinationBindingHash,
        expectedDestinationBindingHash,
        "0x" + repeat("33", 32),
        "0x" + repeat("34", 32),
        "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
        "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
        "finalized",
        SolanaSccpProver.UPGRADEABLE_LOADER_ID,
        SolanaSccpProver.UPGRADEABLE_LOADER_ID,
        true,
        "AgAAABERERERERERERERERERERERERERERERERERERERERER",
        "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2",
        solanaProgramdataSlot,
        "4321",
        "5000",
        "5001",
        "0x2b5f26278ea949463e97c1dc5e53a821b82515b405454a1b0e3cd652c3b00209",
        "AwAAAOEQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
        solanaProgramdataExecutableBase64);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput(
      final String messageProofHash, final byte[][] inclusionBranch) {
    return sampleWitnessInput(
        messageProofHash,
        inclusionBranch,
        SolanaSccpProver.ZERO_HASH_V1,
        SolanaSccpProver.ZERO_HASH_V1);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInputWithIdentity(
      final String messageProofHash,
      final byte[][] inclusionBranch,
      final String transactionSignature,
      final String emitterProgramId) {
    return sampleWitnessInput(
        messageProofHash,
        inclusionBranch,
        SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
        SolanaSccpProver.ZERO_HASH_V1,
        SolanaSccpProver.ZERO_HASH_V1,
        SolanaSccpProver.ZERO_HASH_V1,
        transactionSignature,
        emitterProgramId);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput(
      final String messageProofHash,
      final byte[][] inclusionBranch,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash) {
    return sampleWitnessInput(
        messageProofHash,
        inclusionBranch,
        SolanaSccpProver.MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
        SolanaSccpProver.ZERO_HASH_V1,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        SOLANA_SIGNATURE_55,
        SOLANA_PROGRAM_42);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput(
      final String messageProofHash,
      final byte[][] inclusionBranch,
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash) {
    return sampleWitnessInput(
        messageProofHash,
        inclusionBranch,
        sourceStateVerifierId,
        sourceStateVerifierHash,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        SOLANA_SIGNATURE_55,
        SOLANA_PROGRAM_42,
        SolanaSccpProver.MAINNET_GENESIS_HASH);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput(
      final String messageProofHash,
      final byte[][] inclusionBranch,
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash,
      final String transactionSignature,
      final String emitterProgramId) {
    return sampleWitnessInput(
        messageProofHash,
        inclusionBranch,
        sourceStateVerifierId,
        sourceStateVerifierHash,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        transactionSignature,
        emitterProgramId,
        SolanaSccpProver.MAINNET_GENESIS_HASH);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput(
      final String messageProofHash,
      final byte[][] inclusionBranch,
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash,
      final String transactionSignature,
      final String emitterProgramId,
      final String mainnetGenesisHash) {
    return sampleWitnessInput(
        messageProofHash,
        inclusionBranch,
        sourceStateVerifierId,
        sourceStateVerifierHash,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash,
        transactionSignature,
        emitterProgramId,
        mainnetGenesisHash,
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosg1kA",
        repeat("aa", 32),
        repeat("88", 32),
        null);
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput(
      final String messageProofHash,
      final byte[][] inclusionBranch,
      final String sourceStateVerifierId,
      final String sourceStateVerifierHash,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash,
      final String transactionSignature,
      final String emitterProgramId,
      final String mainnetGenesisHash,
      final String blockhash,
      final String bankHash,
      final String accountsLtHashChecksum,
      final byte[] accountsLtHash) {
    final String sourceEventDigest = repeat("34", 32);
    final String transactionStatusRoot =
        inclusionBranch.length == 0
            ? repeat("bb", 32)
            : SolanaSccpProver.transactionStatusRootFromBranch(
                sourceEventDigest, transactionSignature, emitterProgramId, inclusionBranch);
    return new SolanaSccpProver.WitnessInput(
        SolanaSccpProver.DOMAIN_SORA,
        mainnetGenesisHash,
        "123456789",
        "123456788",
        "8",
        repeat("99", 32),
        blockhash,
        bankHash,
        transactionStatusRoot,
        messageProofHash,
        repeat("77", 32),
        accountsLtHashChecksum,
        null,
        new byte[0],
        accountsLtHash,
        transactionSignature,
        emitterProgramId,
        repeat("dd", 32),
        repeat("ee", 32),
        repeat("12", 32),
        sourceEventDigest,
        sourceStateVerifierId,
        sourceStateVerifierHash,
        repeat("56", 32),
        repeat("78", 32),
        inclusionBranch,
        sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash);
  }

  private static byte[] productionAccountsLtHash() {
    final byte[] out = new byte[2048];
    for (int index = 0; index < out.length; index++) {
      out[index] = (byte) ((index % 251) + 1);
    }
    return out;
  }

  private static SolanaSccpProver.SubmissionPublicInputs sampleSubmissionPublicInputs() {
    return sampleSubmissionPublicInputs(SolanaSccpProver.DOMAIN_SOLANA);
  }

  private static SolanaSccpProver.SubmissionPublicInputs sampleSubmissionPublicInputs(
      final int targetDomain) {
    return sampleSubmissionPublicInputs(1, targetDomain);
  }

  private static SolanaSccpProver.SubmissionPublicInputs sampleSubmissionPublicInputs(
      final int version, final int targetDomain) {
    return new SolanaSccpProver.SubmissionPublicInputs(
        version,
        repeat("dd", 32),
        repeat("ee", 32),
        targetDomain,
        repeat("12", 32),
        "321",
        repeat("aa", 32));
  }

  private static SolanaSccpProver.SubmissionPublicInputs sampleSubmissionPublicInputs(
      final SolanaSccpProver.PublicInputs publicInputs) {
    return new SolanaSccpProver.SubmissionPublicInputs(
        publicInputs.messageId(),
        publicInputs.payloadHash(),
        SolanaSccpProver.DOMAIN_SOLANA,
        publicInputs.commitmentRoot(),
        publicInputs.finalizedSlot(),
        publicInputs.bankHash());
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }

  private static byte[] filledBytes(final int count, final int value) {
    final byte[] out = new byte[count];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static List<List<String>> mutableStringColumns(final List<List<String>> columns) {
    final List<List<String>> copied = new ArrayList<>();
    for (final List<String> column : columns) {
      copied.add(new ArrayList<>(column));
    }
    return copied;
  }

  private static byte[] hexBytes(final String value) {
    String hex = value;
    if (hex.startsWith("0x") || hex.startsWith("0X")) {
      hex = hex.substring(2);
    }
    final byte[] out = new byte[hex.length() / 2];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
    }
    return out;
  }

  private static boolean containsBytes(final byte[] haystack, final byte[] needle) {
    if (needle.length == 0) {
      return true;
    }
    for (int start = 0; start + needle.length <= haystack.length; start++) {
      boolean matches = true;
      for (int offset = 0; offset < needle.length; offset++) {
        if (haystack[start + offset] != needle[offset]) {
          matches = false;
          break;
        }
      }
      if (matches) {
        return true;
      }
    }
    return false;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      builder.append(String.format("%02x", value & 0xff));
    }
    return builder.toString();
  }

  private static byte[] ltHash(final int value) {
    final byte[] out = new byte[2048];
    out[0] = (byte) value;
    out[1] = (byte) (value >>> 8);
    return out;
  }

  private static byte[] addLtHash(final byte[] left, final byte[] right) {
    final byte[] out = Arrays.copyOf(left, left.length);
    final int mixed =
        (((out[0] & 0xff) | ((out[1] & 0xff) << 8))
                + ((right[0] & 0xff) | ((right[1] & 0xff) << 8)))
            & 0xffff;
    out[0] = (byte) mixed;
    out[1] = (byte) (mixed >>> 8);
    return out;
  }

  private static byte[] addFullLtHash(final byte[] left, final byte[] right) {
    final byte[] out = Arrays.copyOf(left, left.length);
    for (int index = 0; index < out.length; index += 2) {
      final int mixed =
          (((out[index] & 0xff) | ((out[index + 1] & 0xff) << 8))
                  + ((right[index] & 0xff) | ((right[index + 1] & 0xff) << 8)))
              & 0xffff;
      out[index] = (byte) mixed;
      out[index + 1] = (byte) (mixed >>> 8);
    }
    return out;
  }

  private static String[] towerVoteSlots() {
    final String[] slots = new String[31];
    for (int i = 0; i < slots.length; i++) {
      slots[i] = Long.toString(1_296_066L + i);
    }
    return slots;
  }

  private static byte[] sampleSolanaStakeStateV2StakeAccount() {
    final byte[] data = new byte[200];
    writeU32Le(data, 0, 2);
    Arrays.fill(data, 12, 44, (byte) 0x81);
    Arrays.fill(data, 44, 76, (byte) 0x91);
    Arrays.fill(data, 124, 156, (byte) 0xa1);
    writeU64Le(data, 156, 1_000L);
    writeU64Le(data, 164, 2L);
    writeU64Le(data, 172, 9L);
    System.arraycopy(sampleSolanaStakeWarmupCooldownRateBytes(), 0, data, 180, 8);
    writeU64Le(data, 188, 123L);
    data[196] = 1;
    return data;
  }

  private static byte[] sampleSolanaStakeWarmupCooldownRateBytes() {
    return new byte[] {
      0x0a, (byte) 0xd7, (byte) 0xa3, 0x70, 0x3d, 0x0a, (byte) 0xb7, 0x3f
    };
  }

  private static byte[] sampleSolanaLegacyStakeWarmupCooldownRateBytes() {
    return new byte[] {0, 0, 0, 0, 0, 0, (byte) 0xd0, 0x3f};
  }

  private static byte[] sampleSolanaVoteStateAccount(final boolean hasLatency) {
    final byte[] data = new byte[3_762];
    final int[] cursor = new int[] {0};
    writeU32Le(data, cursor[0], hasLatency ? 2 : 1);
    cursor[0] += 4;
    fill(data, cursor, (byte) 0x51, 32);
    fill(data, cursor, (byte) 0x71, 32);
    data[cursor[0]++] = 7;
    writeU64Le(data, cursor[0], SolanaSccpProver.TOWER_VOTE_STACK_DEPTH);
    cursor[0] += 8;
    for (int i = 0; i < SolanaSccpProver.TOWER_VOTE_STACK_DEPTH; i++) {
      if (hasLatency) {
        data[cursor[0]++] = 0;
      }
      writeU64Le(data, cursor[0], 11L + i);
      cursor[0] += 8;
      writeU32Le(data, cursor[0], (int) SolanaSccpProver.TOWER_VOTE_STACK_DEPTH - i);
      cursor[0] += 4;
    }
    data[cursor[0]++] = 1;
    writeU64Le(data, cursor[0], 10L);
    cursor[0] += 8;
    writeU64Le(data, cursor[0], 2L);
    cursor[0] += 8;
    writeU64Le(data, cursor[0], 1L);
    cursor[0] += 8;
    fill(data, cursor, (byte) 0x60, 32);
    writeU64Le(data, cursor[0], 3L);
    cursor[0] += 8;
    fill(data, cursor, (byte) 0x61, 32);
    return data;
  }

  private static byte[] sampleSolanaVoteStateV4Account() {
    return sampleSolanaVoteStateV4Account(2);
  }

  private static byte[] sampleSolanaVoteStateV4Account(final int authorizedVoterCount) {
    final byte[] data = new byte[3_762];
    final int[] cursor = new int[] {0};
    writeU32Le(data, cursor[0], 3);
    cursor[0] += 4;
    fill(data, cursor, (byte) 0x51, 32);
    fill(data, cursor, (byte) 0x71, 32);
    fill(data, cursor, (byte) 0x81, 32);
    fill(data, cursor, (byte) 0x91, 32);
    writeU16Le(data, cursor[0], 1_234);
    cursor[0] += 2;
    writeU16Le(data, cursor[0], 9_876);
    cursor[0] += 2;
    writeU64Le(data, cursor[0], 456L);
    cursor[0] += 8;
    data[cursor[0]++] = 1;
    fill(data, cursor, (byte) 0xa5, 48);
    writeU64Le(data, cursor[0], SolanaSccpProver.TOWER_VOTE_STACK_DEPTH);
    cursor[0] += 8;
    for (int i = 0; i < SolanaSccpProver.TOWER_VOTE_STACK_DEPTH; i++) {
      data[cursor[0]++] = 0;
      writeU64Le(data, cursor[0], 11L + i);
      cursor[0] += 8;
      writeU32Le(data, cursor[0], (int) SolanaSccpProver.TOWER_VOTE_STACK_DEPTH - i);
      cursor[0] += 4;
    }
    data[cursor[0]++] = 1;
    writeU64Le(data, cursor[0], 10L);
    cursor[0] += 8;
    writeU64Le(data, cursor[0], authorizedVoterCount);
    cursor[0] += 8;
    for (int i = 0; i < authorizedVoterCount; i++) {
      writeU64Le(data, cursor[0], i + 1L);
      cursor[0] += 8;
      fill(data, cursor, (byte) (0x60 + i), 32);
    }
    return data;
  }

  private static void fill(
      final byte[] data, final int[] cursor, final byte value, final int count) {
    Arrays.fill(data, cursor[0], cursor[0] + count, value);
    cursor[0] += count;
  }

  private static SolanaSccpProver.FullLightClientAuditProofInput auditInputWithVerifierHashes(
      final SolanaSccpProver.FullLightClientAuditProofInput input,
      final String towerVerifierHash,
      final String accountsdbVerifierHash,
      final String bankVerifierHash) {
    return new SolanaSccpProver.FullLightClientAuditProofInput(
        input.witnessInput(),
        input.openedAccounts(),
        input.accountsLtHashProof(),
        input.rootedSlot(),
        input.towerVoteSlots(),
        input.epoch(),
        input.epochStakeRoot(),
        input.stakeActivationHash(),
        input.stakeAccountStateHash(),
        input.stakeHistoryHash(),
        input.stakeHistorySysvarAccountHash(),
        input.sourceTrustAnchorHash(),
        input.consensusVerifierHash(),
        input.messageInclusionVerifierHash(),
        input.finalityPolicyHash(),
        input.sourceAdapterDeploymentReceiptHash(),
        towerVerifierHash,
        accountsdbVerifierHash,
        bankVerifierHash,
        input.adapterVerifierVkHash(),
        input.sourceVerifierMaterialHash(),
        input.sourceAdapterDeploymentHash(),
        input.fullLightClientGateHash(),
        input.finalityContextHash(),
        input.voteMessageHash(),
        input.accountsLtHashProofHash(),
        input.openedAccountsLtHashContributionsHash(),
        input.openedAccountsLtHashResidualChecksum(),
        input.towerLockoutHash(),
        input.towerReplayHash(),
        input.bankForkHash());
  }

  private static SolanaSccpProver.FullLightClientAuditProofInput auditInputWithGateHash(
      final SolanaSccpProver.FullLightClientAuditProofInput input,
      final String fullLightClientGateHash) {
    return new SolanaSccpProver.FullLightClientAuditProofInput(
        input.witnessInput(),
        input.openedAccounts(),
        input.accountsLtHashProof(),
        input.rootedSlot(),
        input.towerVoteSlots(),
        input.epoch(),
        input.epochStakeRoot(),
        input.stakeActivationHash(),
        input.stakeAccountStateHash(),
        input.stakeHistoryHash(),
        input.stakeHistorySysvarAccountHash(),
        input.sourceTrustAnchorHash(),
        input.consensusVerifierHash(),
        input.messageInclusionVerifierHash(),
        input.finalityPolicyHash(),
        input.sourceAdapterDeploymentReceiptHash(),
        input.solanaTowerReplayVerifierHash(),
        input.solanaFullAccountsdbLatticeVerifierHash(),
        input.solanaBankForkChoiceVerifierHash(),
        input.adapterVerifierVkHash(),
        input.sourceVerifierMaterialHash(),
        input.sourceAdapterDeploymentHash(),
        fullLightClientGateHash,
        input.finalityContextHash(),
        input.voteMessageHash(),
        input.accountsLtHashProofHash(),
        input.openedAccountsLtHashContributionsHash(),
        input.openedAccountsLtHashResidualChecksum(),
        input.towerLockoutHash(),
        input.towerReplayHash(),
        input.bankForkHash());
  }

  private static SolanaSccpProver.FullLightClientAuditProofInput auditInputWithSourceMaterialHash(
      final SolanaSccpProver.FullLightClientAuditProofInput input,
      final String sourceVerifierMaterialHash) {
    return new SolanaSccpProver.FullLightClientAuditProofInput(
        input.witnessInput(),
        input.openedAccounts(),
        input.accountsLtHashProof(),
        input.rootedSlot(),
        input.towerVoteSlots(),
        input.epoch(),
        input.epochStakeRoot(),
        input.stakeActivationHash(),
        input.stakeAccountStateHash(),
        input.stakeHistoryHash(),
        input.stakeHistorySysvarAccountHash(),
        input.sourceTrustAnchorHash(),
        input.consensusVerifierHash(),
        input.messageInclusionVerifierHash(),
        input.finalityPolicyHash(),
        input.sourceAdapterDeploymentReceiptHash(),
        input.solanaTowerReplayVerifierHash(),
        input.solanaFullAccountsdbLatticeVerifierHash(),
        input.solanaBankForkChoiceVerifierHash(),
        input.adapterVerifierVkHash(),
        sourceVerifierMaterialHash,
        input.sourceAdapterDeploymentHash(),
        input.fullLightClientGateHash(),
        input.finalityContextHash(),
        input.voteMessageHash(),
        input.accountsLtHashProofHash(),
        input.openedAccountsLtHashContributionsHash(),
        input.openedAccountsLtHashResidualChecksum(),
        input.towerLockoutHash(),
        input.towerReplayHash(),
        input.bankForkHash());
  }

  private static SolanaSccpProver.FullLightClientAuditProofInput auditInputWithSourceAdapterDeploymentHash(
      final SolanaSccpProver.FullLightClientAuditProofInput input,
      final String sourceAdapterDeploymentHash) {
    return new SolanaSccpProver.FullLightClientAuditProofInput(
        input.witnessInput(),
        input.openedAccounts(),
        input.accountsLtHashProof(),
        input.rootedSlot(),
        input.towerVoteSlots(),
        input.epoch(),
        input.epochStakeRoot(),
        input.stakeActivationHash(),
        input.stakeAccountStateHash(),
        input.stakeHistoryHash(),
        input.stakeHistorySysvarAccountHash(),
        input.sourceTrustAnchorHash(),
        input.consensusVerifierHash(),
        input.messageInclusionVerifierHash(),
        input.finalityPolicyHash(),
        input.sourceAdapterDeploymentReceiptHash(),
        input.solanaTowerReplayVerifierHash(),
        input.solanaFullAccountsdbLatticeVerifierHash(),
        input.solanaBankForkChoiceVerifierHash(),
        input.adapterVerifierVkHash(),
        input.sourceVerifierMaterialHash(),
        sourceAdapterDeploymentHash,
        input.fullLightClientGateHash(),
        input.finalityContextHash(),
        input.voteMessageHash(),
        input.accountsLtHashProofHash(),
        input.openedAccountsLtHashContributionsHash(),
        input.openedAccountsLtHashResidualChecksum(),
        input.towerLockoutHash(),
        input.towerReplayHash(),
        input.bankForkHash());
  }

  private static SolanaSccpProver.FullLightClientAuditProofInput auditInputWithDeploymentReceiptHash(
      final SolanaSccpProver.FullLightClientAuditProofInput input,
      final String sourceAdapterDeploymentReceiptHash) {
    return new SolanaSccpProver.FullLightClientAuditProofInput(
        input.witnessInput(),
        input.openedAccounts(),
        input.accountsLtHashProof(),
        input.rootedSlot(),
        input.towerVoteSlots(),
        input.epoch(),
        input.epochStakeRoot(),
        input.stakeActivationHash(),
        input.stakeAccountStateHash(),
        input.stakeHistoryHash(),
        input.stakeHistorySysvarAccountHash(),
        input.sourceTrustAnchorHash(),
        input.consensusVerifierHash(),
        input.messageInclusionVerifierHash(),
        input.finalityPolicyHash(),
        sourceAdapterDeploymentReceiptHash,
        input.solanaTowerReplayVerifierHash(),
        input.solanaFullAccountsdbLatticeVerifierHash(),
        input.solanaBankForkChoiceVerifierHash(),
        input.adapterVerifierVkHash(),
        input.sourceVerifierMaterialHash(),
        input.sourceAdapterDeploymentHash(),
        input.fullLightClientGateHash(),
        input.finalityContextHash(),
        input.voteMessageHash(),
        input.accountsLtHashProofHash(),
        input.openedAccountsLtHashContributionsHash(),
        input.openedAccountsLtHashResidualChecksum(),
        input.towerLockoutHash(),
        input.towerReplayHash(),
        input.bankForkHash());
  }

  private static SolanaSccpProver.FullLightClientAuditProofInput auditInputWithWitness(
      final SolanaSccpProver.FullLightClientAuditProofInput input,
      final SolanaSccpProver.WitnessInput witness) {
    return new SolanaSccpProver.FullLightClientAuditProofInput(
        witness,
        input.openedAccounts(),
        input.accountsLtHashProof(),
        input.rootedSlot(),
        input.towerVoteSlots(),
        input.epoch(),
        input.epochStakeRoot(),
        input.stakeActivationHash(),
        input.stakeAccountStateHash(),
        input.stakeHistoryHash(),
        input.stakeHistorySysvarAccountHash(),
        input.sourceTrustAnchorHash(),
        input.consensusVerifierHash(),
        input.messageInclusionVerifierHash(),
        input.finalityPolicyHash(),
        input.sourceAdapterDeploymentReceiptHash(),
        input.solanaTowerReplayVerifierHash(),
        input.solanaFullAccountsdbLatticeVerifierHash(),
        input.solanaBankForkChoiceVerifierHash(),
        input.adapterVerifierVkHash(),
        input.sourceVerifierMaterialHash(),
        input.sourceAdapterDeploymentHash(),
        input.fullLightClientGateHash(),
        input.finalityContextHash(),
        input.voteMessageHash(),
        input.accountsLtHashProofHash(),
        input.openedAccountsLtHashContributionsHash(),
        input.openedAccountsLtHashResidualChecksum(),
        input.towerLockoutHash(),
        input.towerReplayHash(),
        input.bankForkHash());
  }

  private static SolanaSccpProver.WitnessInput witnessWithDeploymentHash(
      final SolanaSccpProver.WitnessInput witness, final String sourceAdapterDeploymentHash) {
    return new SolanaSccpProver.WitnessInput(
        witness.targetDomain(),
        witness.mainnetGenesisHash(),
        witness.finalizedSlot(),
        witness.parentSlot(),
        witness.bankSignatureCount(),
        witness.parentBankHash(),
        witness.blockhash(),
        witness.bankHash(),
        witness.transactionStatusRoot(),
        witness.messageProofHash(),
        witness.accountInclusionRoot(),
        witness.accountsLtHashChecksum(),
        witness.accountsLtHashProofPublicInputsHash(),
        witness.bankHashHardForkData(),
        witness.accountsLtHash(),
        witness.transactionSignature(),
        witness.emitterProgramId(),
        witness.messageId(),
        witness.payloadHash(),
        witness.commitmentRoot(),
        witness.sourceEventDigest(),
        witness.sourceStateVerifierId(),
        witness.sourceStateVerifierHash(),
        witness.statementHash(),
        witness.destinationBindingHash(),
        witness.inclusionBranch(),
        sourceAdapterDeploymentHash,
        witness.sourceAdapterDeploymentReceiptHash());
  }

  private static SolanaSccpProver.WitnessInput witnessWithDeploymentReceiptHash(
      final SolanaSccpProver.WitnessInput witness,
      final String sourceAdapterDeploymentReceiptHash) {
    return new SolanaSccpProver.WitnessInput(
        witness.targetDomain(),
        witness.mainnetGenesisHash(),
        witness.finalizedSlot(),
        witness.parentSlot(),
        witness.bankSignatureCount(),
        witness.parentBankHash(),
        witness.blockhash(),
        witness.bankHash(),
        witness.transactionStatusRoot(),
        witness.messageProofHash(),
        witness.accountInclusionRoot(),
        witness.accountsLtHashChecksum(),
        witness.accountsLtHashProofPublicInputsHash(),
        witness.bankHashHardForkData(),
        witness.accountsLtHash(),
        witness.transactionSignature(),
        witness.emitterProgramId(),
        witness.messageId(),
        witness.payloadHash(),
        witness.commitmentRoot(),
        witness.sourceEventDigest(),
        witness.sourceStateVerifierId(),
        witness.sourceStateVerifierHash(),
        witness.statementHash(),
        witness.destinationBindingHash(),
        witness.inclusionBranch(),
        witness.sourceAdapterDeploymentHash(),
        sourceAdapterDeploymentReceiptHash);
  }

  private static SolanaSccpProver.WitnessInput witnessWithAccountsLtHash(
      final SolanaSccpProver.WitnessInput witness, final byte[] accountsLtHash) {
    return new SolanaSccpProver.WitnessInput(
        witness.targetDomain(),
        witness.mainnetGenesisHash(),
        witness.finalizedSlot(),
        witness.parentSlot(),
        witness.bankSignatureCount(),
        witness.parentBankHash(),
        witness.blockhash(),
        witness.bankHash(),
        witness.transactionStatusRoot(),
        witness.messageProofHash(),
        witness.accountInclusionRoot(),
        witness.accountsLtHashChecksum(),
        witness.accountsLtHashProofPublicInputsHash(),
        witness.bankHashHardForkData(),
        accountsLtHash,
        witness.transactionSignature(),
        witness.emitterProgramId(),
        witness.messageId(),
        witness.payloadHash(),
        witness.commitmentRoot(),
        witness.sourceEventDigest(),
        witness.sourceStateVerifierId(),
        witness.sourceStateVerifierHash(),
        witness.statementHash(),
        witness.destinationBindingHash(),
        witness.inclusionBranch(),
        witness.sourceAdapterDeploymentHash(),
        witness.sourceAdapterDeploymentReceiptHash());
  }

  private static void writeU32Le(final byte[] data, final int offset, final int value) {
    data[offset] = (byte) (value & 0xff);
    data[offset + 1] = (byte) ((value >>> 8) & 0xff);
    data[offset + 2] = (byte) ((value >>> 16) & 0xff);
    data[offset + 3] = (byte) ((value >>> 24) & 0xff);
  }

  private static void writeU16Le(final byte[] data, final int offset, final int value) {
    data[offset] = (byte) (value & 0xff);
    data[offset + 1] = (byte) ((value >>> 8) & 0xff);
  }

  private static void writeU64Le(final byte[] data, final int offset, final long value) {
    long working = value;
    for (int i = 0; i < 8; i++) {
      data[offset + i] = (byte) (working & 0xffL);
      working >>>= 8;
    }
  }

  private static byte[] repeatByte(final byte value, final int count) {
    final byte[] out = new byte[count];
    for (int i = 0; i < count; i++) {
      out[i] = value;
    }
    return out;
  }

  private static List<String> col(final String value) {
    return Collections.singletonList(value);
  }

  private static byte[][] repeatedBranch(final int count) {
    final byte[][] out = new byte[count][];
    for (int i = 0; i < count; i++) {
      out[i] = repeatByte((byte) 0x56, 32);
    }
    return out;
  }

  private static SolanaSccpProver.AccountOpeningInput[] repeatedOpening(
      final SolanaSccpProver.AccountOpeningInput opening, final int count) {
    final SolanaSccpProver.AccountOpeningInput[] out =
        new SolanaSccpProver.AccountOpeningInput[count];
    Arrays.fill(out, opening);
    return out;
  }

  private static byte[][] repeatedBytes(final byte[] value, final int count) {
    final byte[][] out = new byte[count][];
    for (int i = 0; i < count; i++) {
      out[i] = Arrays.copyOf(value, value.length);
    }
    return out;
  }
}
