package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.stream.Collectors;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.crypto.Blake3;

/** Solana SCCP proof request helpers for local-first Android proof generation. */
public final class SolanaSccpProver {
  public static final int DOMAIN_SORA = 0;
  public static final int DOMAIN_SOLANA = 3;
  public static final String RECURSIVE_PROOF_BACKEND_V1 =
      "sccp-solana-recursive-mainnet-v1";
  public static final String ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-solana-accounts-lt-hash-v1";
  public static final String TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-solana-tower-replay-v1";
  public static final String FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-solana-full-accountsdb-lattice-v1";
  public static final String BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-solana-bank-fork-choice-v1";
  public static final String UPGRADEABLE_LOADER_ID =
      "BPFLoaderUpgradeab1e11111111111111111111111";
  public static final String MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1 =
      "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1";
  public static final int SOURCE_STATE_MAX_PROOF_BYTES = 2 * 1024 * 1024;
  public static final int SOURCE_STATE_MAX_PROOF_LABEL_BYTES = 128;
  public static final int NATIVE_RECURSIVE_MAX_PROOF_BYTES = 2 * 1024 * 1024;
  public static final String TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1 =
      "0x6b4e4106bbb6b343ae1a4a36c9c68756d4454d2167c9b8b2ee3225e39fb0a48b";
  private static final Set<String> TEMPLATE_SOURCE_MATERIAL_HASHES_V1 =
      Collections.unmodifiableSet(
          new HashSet<>(
              Arrays.asList(
                  "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
                  "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
                  "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
                  TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
                  "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56")));
  public static final String MAINNET_TOWER_REPLAY_VERIFIER_ID_V1 =
      "sccp:sol:light-client:tower-replay-mainnet-beta:v1";
  public static final String MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1 =
      "sccp:sol:light-client:full-accountsdb-lattice-mainnet-beta:v1";
  public static final String MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1 =
      "sccp:sol:light-client:bank-fork-choice-mainnet-beta:v1";
  public static final String MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp";
  public static final long MAINNET_SLOTS_PER_EPOCH = 432_000L;
  public static final long TOWER_LOCKOUT_CONFIRMATION_DEPTH = 32L;
  public static final long TOWER_VOTE_STACK_DEPTH = TOWER_LOCKOUT_CONFIRMATION_DEPTH - 1L;
  public static final long TOWER_WARMUP_COOLDOWN_RATE_BPS = 900L;
  public static final int MAX_VALIDATORS = 8_192;
  public static final int MAX_SOURCE_MERKLE_BRANCH_NODES = 64;
  public static final String VOTE_PROGRAM_ID =
      "0x0761481d357474bb7c4d7624ebd3bdb3d8355e73d11043fc0da3538000000000";
  public static final String STAKE_PROGRAM_ID =
      "0x06a1d8179137542a983437bdfe2a7ab2557f535c8a78722b68a49dc000000000";
  public static final String SYSVAR_PROGRAM_ID =
      "0x06a7d5171875f729c73d93408f216120067ed88c76e08c287fc1946000000000";
  public static final String STAKE_HISTORY_SYSVAR_ID =
      "0x06a7d517193584d0feed9bb3431d13206be544281b57b8566cc5375ff4000000";
  public static final String BORSH_INSTRUCTION_V1 = "borsh_instruction_v1";
  public static final String ZERO_HASH_V1 =
      "0x0000000000000000000000000000000000000000000000000000000000000000";

  private static final String SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1 = "submit_sccp_message_proof";
  private static final String ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX_V1 =
      "sccp:solana:accounts-lt-proof-public-inputs:v1";
  private static final String ACCOUNTS_LT_HASH_OPENED_CONTRIBUTIONS_PREFIX_V1 =
      "sccp:solana:accounts-lt-opened-contributions:v1";
  private static final String MAINNET_GENESIS_HASH_PREFIX_V1 =
      "sccp:solana:mainnet-genesis:v1";
  private static final String BANK_HASH_HARD_FORK_DATA_PREFIX_V1 =
      "sccp:solana:bank-hash-hard-fork-data:v1";
  private static final String ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX_V1 =
      "sccp:solana:accounts-lt:fastpq:dsid:v1";
  private static final String ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1 = "fastpq-lane-balanced";
  private static final String ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY_V1 =
      "sccp:solana:accounts-lt:v1:statement";
  private static final String ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY_V1 =
      "sccp:solana:accounts-lt:v1:accounts";
  private static final String ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY_V1 =
      "sccp:solana:accounts-lt:v1:opened-contributions";
  private static final String ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY_V1 =
      "sccp:solana:accounts-lt:v1:residual";
  private static final String ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY_V1 =
      "sccp:solana:accounts-lt:v1:context";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1 =
      "sccp:solana:full-light-client-audit:fastpq:dsid:v1";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1 =
      "fastpq-lane-balanced";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1 =
      "sccp:solana:full-light-client-audit:v1:statement";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1 =
      "sccp:solana:full-light-client-audit:v1:context";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1 =
      "sccp:solana:full-light-client-audit:v1:gate";
  private static final String FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1 =
      "sccp:solana:full-light-client-audit:statement:v1";
  private static final String SOLANA_SOURCE_CHAIN_KEY_V1 = "sol";
  private static final int SOLANA_SOURCE_PROOF_PLAN_CODE_V1 = 3;
  private static final int SOLANA_FINALITY_MODEL_CODE_V1 = 3;
  private static final int SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG = 2;
  private static final int SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG = 3;
  private static final int SOLANA_PROGRAMDATA_METADATA_LEN = 45;
  private static final String SOLANA_ROUTE_CANARY_LIVE_PROGRAM_PREFIX_V1 =
      "iroha:sccp:solana-route-canary-live-program:v1";
  private static final int TRANSACTION_SIGNATURE_BYTES = 64;
  private static final int PROGRAM_ID_BYTES = 32;
  private static final int STAKE_STATE_V2_STAKE_ACCOUNT_DATA_LEN = 200;
  private static final int VOTE_STATE_ACCOUNT_DATA_LEN = 3_762;
  private static final int MAX_ACCOUNT_RAW_DATA_BYTES = 65_536;
  private static final int ACCOUNTS_LT_HASH_BYTES = 2_048;
  private static final int LT_HASH_ELEMENTS = 1_024;
  private static final int OPENED_LT_HASH_ROLE_VOTE = 1;
  private static final int OPENED_LT_HASH_ROLE_STAKE = 2;
  private static final int OPENED_LT_HASH_ROLE_STAKE_HISTORY_SYSVAR = 3;
  private static final int MAX_BANK_HARD_FORK_HASH_DATA_BYTES = 1_024;
  private static final int BLS_PUBLIC_KEY_COMPRESSED_LEN = 48;
  private static final int VOTE_STATE_V1_14_11_DISCRIMINANT = 1;
  private static final int VOTE_STATE_V3_DISCRIMINANT = 2;
  private static final int VOTE_STATE_V4_DISCRIMINANT = 3;
  private static final int VOTE_STATE_PRIOR_VOTERS = 32;
  private static final int VOTE_STATE_V4_AUTHORIZED_VOTERS = 4;
  private static final int VOTE_STATE_MAX_EPOCH_CREDITS = 64;
  private static final int STAKE_STATE_V2_STAKE_DISCRIMINANT = 2;
  private static final int STAKE_STATE_V2_STAKER_OFFSET = 12;
  private static final int STAKE_STATE_V2_WITHDRAWER_OFFSET = 44;
  private static final int STAKE_STATE_V2_VOTER_PUBKEY_OFFSET = 124;
  private static final int STAKE_STATE_V2_DELEGATED_STAKE_OFFSET = 156;
  private static final int STAKE_STATE_V2_ACTIVATION_EPOCH_OFFSET = 164;
  private static final int STAKE_STATE_V2_DEACTIVATION_EPOCH_OFFSET = 172;
  private static final int STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_OFFSET = 180;
  private static final int STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_BYTES = 8;
  private static final byte[] STAKE_STATE_V2_LEGACY_WARMUP_COOLDOWN_RATE_BYTES =
      new byte[] {0, 0, 0, 0, 0, 0, (byte) 0xd0, 0x3f};
  private static final byte[] STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES =
      new byte[] {0x0a, (byte) 0xd7, (byte) 0xa3, 0x70, 0x3d, 0x0a, (byte) 0xb7, 0x3f};
  private static final int STAKE_STATE_V2_CREDITS_OBSERVED_OFFSET = 188;
  private static final int STAKE_STATE_V2_FLAG_OFFSET = 196;
  private static final int STAKE_STATE_V2_KNOWN_FLAGS_MASK = 0b0000_0001;
  private static final String BASE58_ALPHABET =
      "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  private static final BigInteger BASE_58 = BigInteger.valueOf(58L);
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final BigInteger WARMUP_COOLDOWN_RATE_BPS =
      BigInteger.valueOf(TOWER_WARMUP_COOLDOWN_RATE_BPS);
  private static final BigInteger BASIS_POINTS_PER_UNIT = BigInteger.valueOf(10_000L);
  private static final byte[] SOLANA_BPF_ELF_MAGIC = new byte[] {0x7f, 0x45, 0x4c, 0x46};
  private static final int[] BASE58_INDEX = new int[128];

  /** Solana StakeHistory sysvar entry bound into SCCP stake-history evidence. */
  public static final class StakeHistoryEntry {
    public final String epoch;
    public final String effective;
    public final String activating;
    public final String deactivating;

    public StakeHistoryEntry(
        final String epoch,
        final String effective,
        final String activating,
        final String deactivating) {
      this.epoch = Objects.requireNonNull(epoch, "epoch");
      this.effective = Objects.requireNonNull(effective, "effective");
      this.activating = Objects.requireNonNull(activating, "activating");
      this.deactivating = Objects.requireNonNull(deactivating, "deactivating");
    }
  }

  /** Parsed fields from a raw Solana {@code VoteStateVersions} account buffer. */
  public record ParsedVoteStateV1OrV3AccountData(
      byte[] nodePubkey,
      byte[] authorizedVoter,
      byte[] authorizedWithdrawer,
      byte[] inflationRewardsCollector,
      byte[] blockRevenueCollector,
      String inflationRewardsCommissionBps,
      String blockRevenueCommissionBps,
      String pendingDelegatorRewards,
      byte[] blsPubkeyCompressed,
      String rootSlot,
      String[] towerVoteSlots) {
    public ParsedVoteStateV1OrV3AccountData {
      nodePubkey = Arrays.copyOf(Objects.requireNonNull(nodePubkey, "nodePubkey"), nodePubkey.length);
      authorizedVoter =
          Arrays.copyOf(Objects.requireNonNull(authorizedVoter, "authorizedVoter"), authorizedVoter.length);
      authorizedWithdrawer =
          Arrays.copyOf(
              Objects.requireNonNull(authorizedWithdrawer, "authorizedWithdrawer"),
              authorizedWithdrawer.length);
      inflationRewardsCollector =
          Arrays.copyOf(
              Objects.requireNonNull(inflationRewardsCollector, "inflationRewardsCollector"),
              inflationRewardsCollector.length);
      blockRevenueCollector =
          Arrays.copyOf(
              Objects.requireNonNull(blockRevenueCollector, "blockRevenueCollector"),
              blockRevenueCollector.length);
      inflationRewardsCommissionBps =
          Objects.requireNonNull(inflationRewardsCommissionBps, "inflationRewardsCommissionBps");
      blockRevenueCommissionBps =
          Objects.requireNonNull(blockRevenueCommissionBps, "blockRevenueCommissionBps");
      pendingDelegatorRewards = Objects.requireNonNull(pendingDelegatorRewards, "pendingDelegatorRewards");
      blsPubkeyCompressed =
          Arrays.copyOf(
              Objects.requireNonNull(blsPubkeyCompressed, "blsPubkeyCompressed"),
              blsPubkeyCompressed.length);
      rootSlot = Objects.requireNonNull(rootSlot, "rootSlot");
      towerVoteSlots =
          Arrays.copyOf(Objects.requireNonNull(towerVoteSlots, "towerVoteSlots"), towerVoteSlots.length);
    }

    @Override
    public byte[] nodePubkey() {
      return Arrays.copyOf(nodePubkey, nodePubkey.length);
    }

    @Override
    public byte[] authorizedVoter() {
      return Arrays.copyOf(authorizedVoter, authorizedVoter.length);
    }

    @Override
    public byte[] authorizedWithdrawer() {
      return Arrays.copyOf(authorizedWithdrawer, authorizedWithdrawer.length);
    }

    @Override
    public byte[] inflationRewardsCollector() {
      return Arrays.copyOf(inflationRewardsCollector, inflationRewardsCollector.length);
    }

    @Override
    public byte[] blockRevenueCollector() {
      return Arrays.copyOf(blockRevenueCollector, blockRevenueCollector.length);
    }

    @Override
    public byte[] blsPubkeyCompressed() {
      return Arrays.copyOf(blsPubkeyCompressed, blsPubkeyCompressed.length);
    }

    @Override
    public String[] towerVoteSlots() {
      return Arrays.copyOf(towerVoteSlots, towerVoteSlots.length);
    }
  }

  /** Parsed fields from a raw Solana {@code StakeStateV2::Stake} account buffer. */
  public record ParsedStakeStateV2StakeAccountData(
      byte[] staker,
      byte[] withdrawer,
      byte[] voterPubkey,
      String delegatedStake,
      String activationEpoch,
      String deactivationEpoch,
      byte[] warmupCooldownRateBytes,
      String creditsObserved,
      String stakeFlags) {
    public ParsedStakeStateV2StakeAccountData {
      staker = Arrays.copyOf(Objects.requireNonNull(staker, "staker"), staker.length);
      withdrawer = Arrays.copyOf(Objects.requireNonNull(withdrawer, "withdrawer"), withdrawer.length);
      voterPubkey = Arrays.copyOf(Objects.requireNonNull(voterPubkey, "voterPubkey"), voterPubkey.length);
      delegatedStake = Objects.requireNonNull(delegatedStake, "delegatedStake");
      activationEpoch = Objects.requireNonNull(activationEpoch, "activationEpoch");
      deactivationEpoch = Objects.requireNonNull(deactivationEpoch, "deactivationEpoch");
      warmupCooldownRateBytes =
          Arrays.copyOf(
              Objects.requireNonNull(warmupCooldownRateBytes, "warmupCooldownRateBytes"),
              warmupCooldownRateBytes.length);
      creditsObserved = Objects.requireNonNull(creditsObserved, "creditsObserved");
      stakeFlags = Objects.requireNonNull(stakeFlags, "stakeFlags");
    }

    @Override
    public byte[] staker() {
      return Arrays.copyOf(staker, staker.length);
    }

    @Override
    public byte[] withdrawer() {
      return Arrays.copyOf(withdrawer, withdrawer.length);
    }

    @Override
    public byte[] voterPubkey() {
      return Arrays.copyOf(voterPubkey, voterPubkey.length);
    }

    @Override
    public byte[] warmupCooldownRateBytes() {
      return Arrays.copyOf(warmupCooldownRateBytes, warmupCooldownRateBytes.length);
    }
  }

  /** Solana account inclusion root and per-leaf Merkle branches. */
  public static final class AccountInclusionWitness {
    public final String root;
    public final List<List<String>> branches;

    public AccountInclusionWitness(final String root, final List<List<String>> branches) {
      this.root = Objects.requireNonNull(root, "root");
      final List<List<String>> copied = new ArrayList<>();
      for (final List<String> branch : Objects.requireNonNull(branches, "branches")) {
        copied.add(Collections.unmodifiableList(new ArrayList<>(branch)));
      }
      this.branches = Collections.unmodifiableList(copied);
    }
  }

  /** Opened Solana accounts used to build the exact account-inclusion witness. */
  public record OpenedAccountInclusionWitnessInput(
      String finalizedSlot,
      AccountOpeningInput[] validatorVoteAccountOpenings,
      byte[][] validatorVoteAccountRawData,
      AccountOpeningInput[] validatorStakeAccountOpenings,
      byte[][] validatorStakeAccountRawData,
      AccountOpeningInput stakeHistorySysvarOpening,
      byte[] stakeHistorySysvarRawData,
      String expectedAccountInclusionRoot) {
    public OpenedAccountInclusionWitnessInput {
      finalizedSlot = Objects.requireNonNull(finalizedSlot, "finalizedSlot");
      validatorVoteAccountOpenings =
          validatorVoteAccountOpenings == null
              ? new AccountOpeningInput[0]
              : Arrays.copyOf(validatorVoteAccountOpenings, validatorVoteAccountOpenings.length);
      validatorVoteAccountRawData = copyBranch(validatorVoteAccountRawData);
      validatorStakeAccountOpenings =
          validatorStakeAccountOpenings == null
              ? new AccountOpeningInput[0]
              : Arrays.copyOf(validatorStakeAccountOpenings, validatorStakeAccountOpenings.length);
      validatorStakeAccountRawData = copyBranch(validatorStakeAccountRawData);
      stakeHistorySysvarOpening = Objects.requireNonNull(stakeHistorySysvarOpening, "stakeHistorySysvarOpening");
      stakeHistorySysvarRawData =
          Arrays.copyOf(
              Objects.requireNonNull(stakeHistorySysvarRawData, "stakeHistorySysvarRawData"),
              stakeHistorySysvarRawData.length);
    }

    @Override
    public AccountOpeningInput[] validatorVoteAccountOpenings() {
      return Arrays.copyOf(validatorVoteAccountOpenings, validatorVoteAccountOpenings.length);
    }

    @Override
    public byte[][] validatorVoteAccountRawData() {
      return copyBranch(validatorVoteAccountRawData);
    }

    @Override
    public AccountOpeningInput[] validatorStakeAccountOpenings() {
      return Arrays.copyOf(validatorStakeAccountOpenings, validatorStakeAccountOpenings.length);
    }

    @Override
    public byte[][] validatorStakeAccountRawData() {
      return copyBranch(validatorStakeAccountRawData);
    }

    @Override
    public byte[] stakeHistorySysvarRawData() {
      return Arrays.copyOf(stakeHistorySysvarRawData, stakeHistorySysvarRawData.length);
    }
  }

  /** Exact opened-account inclusion root and branches accepted by the Solana verifier. */
  public static final class OpenedAccountInclusionWitness {
    public final String root;
    public final List<List<String>> branches;
    public final List<List<String>> validatorVoteAccountBranches;
    public final List<List<String>> validatorStakeAccountBranches;
    public final List<String> stakeHistorySysvarBranch;

    public OpenedAccountInclusionWitness(
        final String root,
        final List<List<String>> branches,
        final List<List<String>> validatorVoteAccountBranches,
        final List<List<String>> validatorStakeAccountBranches,
        final List<String> stakeHistorySysvarBranch) {
      this.root = Objects.requireNonNull(root, "root");
      this.branches = copyBranches(branches);
      this.validatorVoteAccountBranches = copyBranches(validatorVoteAccountBranches);
      this.validatorStakeAccountBranches = copyBranches(validatorStakeAccountBranches);
      this.stakeHistorySysvarBranch =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(stakeHistorySysvarBranch)));
    }

    private static List<List<String>> copyBranches(final List<List<String>> branches) {
      final List<List<String>> copied = new ArrayList<>();
      for (final List<String> branch : Objects.requireNonNull(branches, "branches")) {
        copied.add(Collections.unmodifiableList(new ArrayList<>(branch)));
      }
      return Collections.unmodifiableList(copied);
    }
  }

  static {
    Arrays.fill(BASE58_INDEX, -1);
    for (int i = 0; i < BASE58_ALPHABET.length(); i++) {
      BASE58_INDEX[BASE58_ALPHABET.charAt(i)] = i;
    }
  }

  private final WitnessProvider witnessProvider;
  private final ProofEngine proofEngine;

  public SolanaSccpProver() {
    this(null, null);
  }

  public SolanaSccpProver(final WitnessProvider witnessProvider, final ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public ProofRequest buildRequest(final WitnessInput input) {
    final WitnessInput resolved =
        witnessProvider == null
            ? input
            : witnessProvider.resolveWitness(witnessProviderInputSnapshot(input));
    return buildProofRequest(resolved);
  }

  public ProofResult prove(final WitnessInput input) {
    final ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("Solana SCCP local prover is not linked");
    }
    requireProductionProofRequest(request);
    return wrapProofResult(proofEngine.prove(callbackRequestSnapshot(request)), request);
  }

  private static WitnessInput witnessProviderInputSnapshot(final WitnessInput input) {
    return new WitnessInput(
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
        input.destinationBindingHash(),
        input.inclusionBranch(),
        input.sourceAdapterDeploymentHash(),
        input.sourceAdapterDeploymentReceiptHash());
  }

  public static byte[] canonicalRouteCanaryEvidenceBytes(final RouteCanaryEvidenceInput input) {
    Objects.requireNonNull(input, "input");
    final byte[] routeAllowlistHash =
        nonZeroHex32Bytes(input.routeAllowlistHash(), "routeAllowlistHash");
    final byte[] destinationBindingHash =
        nonZeroHex32Bytes(input.destinationBindingHash(), "destinationBindingHash");
    final String canonicalSolanaDestinationBindingHash =
        SourceSccpProofs.destinationBindingHash(DOMAIN_SOLANA);
    final String expectedDestinationBindingHash =
        normalizeNonZeroHex32(
            input.expectedDestinationBindingHash() == null
                ? canonicalSolanaDestinationBindingHash
                : input.expectedDestinationBindingHash(),
            "expectedDestinationBindingHash");
    if (!expectedDestinationBindingHash.equals(canonicalSolanaDestinationBindingHash)) {
      throw new IllegalArgumentException(
          "expectedDestinationBindingHash must match canonical Solana destination binding");
    }
    if (!("0x" + hexLower(destinationBindingHash)).equals(canonicalSolanaDestinationBindingHash)) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match canonical Solana destination binding");
    }
    final byte[] sourceVerifierMaterialHash =
        nonZeroHex32Bytes(input.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash");
    final byte[] sourceAdapterEngineDeploymentHash =
        nonZeroHex32Bytes(
            input.sourceAdapterEngineDeploymentHash(), "sourceAdapterEngineDeploymentHash");
    final RouteCanaryProgramDataEvidence evidence = normalizeRouteCanaryProgramDataEvidence(input);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, DOMAIN_SORA);
    writeU32Le(out, DOMAIN_SOLANA);
    write(out, routeAllowlistHash);
    write(out, destinationBindingHash);
    write(out, sourceVerifierMaterialHash);
    write(out, sourceAdapterEngineDeploymentHash);
    write(out, evidence.verifierProgram());
    write(out, hex32Bytes(evidence.verifierCodeHash(), "verifierCodeHash"));
    writeVec(out, evidence.rpcCommitment().getBytes(StandardCharsets.UTF_8));
    writeVec(out, evidence.programOwner().getBytes(StandardCharsets.UTF_8));
    writeVec(out, evidence.programdataOwner().getBytes(StandardCharsets.UTF_8));
    out.write(1);
    writeVec(out, evidence.programAccountData());
    write(out, evidence.programdataAddress());
    writeU64Le(out, evidence.programdataSlot());
    writeU64Le(out, evidence.expectedProgramdataSlot());
    writeU64Le(out, evidence.programAccountContextSlot());
    writeU64Le(out, evidence.programdataAccountContextSlot());
    writeVec(out, evidence.programdataMetadata());
    writeVec(out, evidence.programdataExecutable());
    return out.toByteArray();
  }

  public static String routeCanaryEvidenceHash(final RouteCanaryEvidenceInput input) {
    return "0x"
        + hexLower(
            hashBytes(
                SOLANA_ROUTE_CANARY_LIVE_PROGRAM_PREFIX_V1,
                canonicalRouteCanaryEvidenceBytes(input)));
  }

  public static Witness normalizeWitness(final WitnessInput input) {
    Objects.requireNonNull(input, "input");
    if (input.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("targetDomain must be SORA");
    }
    final BigInteger finalizedSlot = normalizeU64(input.finalizedSlot(), "finalizedSlot");
    final BigInteger parentSlot = normalizeU64(input.parentSlot(), "parentSlot");
    if (!parentSlot.add(BigInteger.ONE).equals(finalizedSlot)) {
      throw new IllegalArgumentException("parentSlot must be the direct parent of finalizedSlot");
    }
    final BigInteger bankSignatureCount =
        normalizeU64(input.bankSignatureCount(), "bankSignatureCount");
    if (BigInteger.ZERO.equals(bankSignatureCount)) {
      throw new IllegalArgumentException("bankSignatureCount must be nonzero");
    }
    final String parentBankHash = normalizeNonZeroHex32(input.parentBankHash(), "parentBankHash");
    final String bankHash = normalizeNonZeroHex32(input.bankHash(), "bankHash");
    final byte[] blockhashBytes = solanaHash32Bytes(input.blockhash(), "blockhash");
    final String transactionStatusRoot =
        normalizeNonZeroHex32(input.transactionStatusRoot(), "transactionStatusRoot");
    final String sourceEventDigest =
        normalizeNonZeroHex32(input.sourceEventDigest(), "sourceEventDigest");
    final String transactionSignature =
        normalizeSolanaBase58Fixed(
            input.transactionSignature(), "transactionSignature", TRANSACTION_SIGNATURE_BYTES);
    final String emitterProgramId =
        normalizeSolanaBase58Fixed(input.emitterProgramId(), "emitterProgramId", PROGRAM_ID_BYTES);
    final byte[][] inclusionBranch = normalizeInclusionBranch(input.inclusionBranch());
    if (inclusionBranch.length != 0) {
      final String derivedTransactionStatusRoot =
          transactionStatusRootFromBranch(
              sourceEventDigest, transactionSignature, emitterProgramId, inclusionBranch);
      if (!transactionStatusRoot.equals(derivedTransactionStatusRoot)) {
        throw new IllegalArgumentException("transactionStatusRoot must match inclusionBranch");
      }
    }
    final String messageProofHash =
        normalizeMessageProofHash(
            input.messageProofHash(),
            sourceEventDigest,
            transactionStatusRoot,
            transactionSignature,
            emitterProgramId,
            inclusionBranch);
    final String accountInclusionRoot =
        normalizeNonZeroHex32(input.accountInclusionRoot(), "accountInclusionRoot");
    final String accountsLtHashChecksum =
        normalizeNonZeroHex32(input.accountsLtHashChecksum(), "accountsLtHashChecksum");
    final byte[] accountsLtHashChecksumBytes =
        hex32Bytes(accountsLtHashChecksum, "accountsLtHashChecksum");
    final byte[] bankHashHardForkData = input.bankHashHardForkData();
    if (bankHashHardForkData.length > MAX_BANK_HARD_FORK_HASH_DATA_BYTES) {
      throw new IllegalArgumentException("bankHashHardForkData is too large");
    }
    final byte[] accountsLtHash = input.accountsLtHash();
    if (accountsLtHash != null) {
      if (accountsLtHash.length != ACCOUNTS_LT_HASH_BYTES) {
        throw new IllegalArgumentException("accountsLtHash must be 2048 bytes");
      }
      if (!Arrays.equals(Blake3.hash(accountsLtHash), accountsLtHashChecksumBytes)) {
        throw new IllegalArgumentException("accountsLtHashChecksum must match accountsLtHash");
      }
      final byte[] expectedBankHash =
          agaveBankHashBytes(
              hex32Bytes(parentBankHash, "parentBankHash"),
              bankSignatureCount,
              blockhashBytes,
              accountsLtHash,
              bankHashHardForkData);
      if (!Arrays.equals(hex32Bytes(bankHash, "bankHash"), expectedBankHash)) {
        throw new IllegalArgumentException("bankHash must match Agave bank hash inputs");
      }
    }
    final SourceAdapterDeploymentBinding deploymentBinding =
        normalizeSourceAdapterDeploymentBinding(
            DOMAIN_SOLANA,
            input.targetDomain(),
            input.sourceAdapterDeploymentHash(),
            input.sourceAdapterDeploymentReceiptHash());
    final String sourceStateVerifierId =
        normalizeNonEmpty(input.sourceStateVerifierId(), "sourceStateVerifierId");
    final String sourceStateVerifierHash =
        normalizeHex32(input.sourceStateVerifierHash(), "sourceStateVerifierHash");
    if (!ZERO_HASH_V1.equals(sourceStateVerifierHash)
        && !MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1.equals(sourceStateVerifierId)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierId must match Solana AccountsDB verifier profile");
    }
    final String accountsLtHashProofPublicInputsHash =
        accountsLtHashProofPublicInputsHash(
            DOMAIN_SOLANA,
            finalizedSlot.toString(),
            parentSlot.toString(),
            bankSignatureCount.toString(),
            parentBankHash,
            bankHash,
            "0x" + hexLower(blockhashBytes),
            bankHashHardForkData,
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum,
            input.accountsLtHash());
    if (input.accountsLtHashProofPublicInputsHash() != null
        && !normalizeHex32(
                input.accountsLtHashProofPublicInputsHash(),
                "accountsLtHashProofPublicInputsHash")
            .equals(accountsLtHashProofPublicInputsHash)) {
      throw new IllegalArgumentException(
          "accountsLtHashProofPublicInputsHash must match bank-state inputs");
    }
    return new Witness(
        1,
        DOMAIN_SOLANA,
        input.targetDomain(),
        normalizeNonEmpty(input.mainnetGenesisHash(), "mainnetGenesisHash"),
        finalizedSlot.toString(),
        parentSlot.toString(),
        bankSignatureCount.toString(),
        parentBankHash,
        "0x" + hexLower(blockhashBytes),
        bankHash,
        transactionStatusRoot,
        messageProofHash,
        accountInclusionRoot,
        accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash,
        bankHashHardForkData,
        accountsLtHash,
        transactionSignature,
        emitterProgramId,
        normalizeHex32(input.messageId(), "messageId"),
        normalizeHex32(input.payloadHash(), "payloadHash"),
        normalizeHex32(input.commitmentRoot(), "commitmentRoot"),
        sourceEventDigest,
        sourceStateVerifierId,
        sourceStateVerifierHash,
        deploymentBinding.sourceAdapterDeploymentHash(),
        deploymentBinding.sourceAdapterDeploymentReceiptHash(),
        inclusionBranch);
  }

  public static byte[] canonicalWitnessBytes(final WitnessInput input) {
    return canonicalWitnessBytes(normalizeWitness(input));
  }

  public static byte[] canonicalWitnessBytes(final Witness witness) {
    Objects.requireNonNull(witness, "witness");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(witness.version());
    writeU32Le(out, witness.sourceDomain());
    writeU32Le(out, witness.targetDomain());
    writeString(out, witness.mainnetGenesisHash(), "mainnetGenesisHash");
    writeU64Le(out, normalizeU64(witness.finalizedSlot(), "finalizedSlot"));
    writeU64Le(out, normalizeU64(witness.parentSlot(), "parentSlot"));
    writeU64Le(out, normalizeU64(witness.bankSignatureCount(), "bankSignatureCount"));
    write(out, solanaHash32Bytes(witness.blockhash(), "blockhash"));
    writeString(out, witness.transactionSignature(), "transactionSignature");
    writeString(out, witness.emitterProgramId(), "emitterProgramId");
    write(out, hex32Bytes(witness.parentBankHash(), "parentBankHash"));
    write(out, hex32Bytes(witness.bankHash(), "bankHash"));
    write(out, hex32Bytes(witness.transactionStatusRoot(), "transactionStatusRoot"));
    write(out, hex32Bytes(witness.messageProofHash(), "messageProofHash"));
    write(out, hex32Bytes(witness.accountInclusionRoot(), "accountInclusionRoot"));
    write(out, hex32Bytes(witness.accountsLtHashChecksum(), "accountsLtHashChecksum"));
    write(
        out,
        hex32Bytes(
            witness.accountsLtHashProofPublicInputsHash(),
            "accountsLtHashProofPublicInputsHash"));
    writeVec(out, witness.bankHashHardForkData());
    writeVec(out, witness.accountsLtHash() == null ? new byte[0] : witness.accountsLtHash());
    write(out, hex32Bytes(witness.messageId(), "messageId"));
    write(out, hex32Bytes(witness.payloadHash(), "payloadHash"));
    write(out, hex32Bytes(witness.commitmentRoot(), "commitmentRoot"));
    write(out, hex32Bytes(witness.sourceEventDigest(), "sourceEventDigest"));
    writeString(out, witness.sourceStateVerifierId(), "sourceStateVerifierId");
    write(out, hex32Bytes(witness.sourceStateVerifierHash(), "sourceStateVerifierHash"));
    write(out, hex32Bytes(witness.sourceAdapterDeploymentHash(), "sourceAdapterDeploymentHash"));
    write(
        out,
        hex32Bytes(
            witness.sourceAdapterDeploymentReceiptHash(),
            "sourceAdapterDeploymentReceiptHash"));
    writeU32Le(out, witness.inclusionBranch().length);
    final byte[][] inclusionBranch = witness.inclusionBranch();
    for (int i = 0; i < inclusionBranch.length; i++) {
      final byte[] sibling = inclusionBranch[i];
      if (sibling.length != 32) {
        throw new IllegalArgumentException("inclusionBranch[" + i + "] must be 32 bytes");
      }
      write(out, sibling);
    }
    return out.toByteArray();
  }

  public static byte[] canonicalMessageProofBytes(
      final String sourceEventDigest,
      final String transactionStatusRoot,
      final String transactionSignature,
      final String emitterProgramId,
      final byte[][] inclusionBranch) {
    final byte[][] normalized = normalizeInclusionBranch(inclusionBranch);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    write(out, nonZeroHex32Bytes(transactionStatusRoot, "transactionStatusRoot"));
    writeVec(
        out,
        decodeSolanaBase58Fixed(
            transactionSignature, "transactionSignature", TRANSACTION_SIGNATURE_BYTES));
    writeVec(out, decodeSolanaBase58Fixed(emitterProgramId, "emitterProgramId", PROGRAM_ID_BYTES));
    if (normalized.length == 0) {
      throw new IllegalArgumentException("inclusionBranch must not be empty");
    }
    writeU32Le(out, normalized.length);
    for (final byte[] sibling : normalized) {
      write(out, sibling);
    }
    return out.toByteArray();
  }

  public static String messageProofHash(
      final String sourceEventDigest,
      final String transactionStatusRoot,
      final String transactionSignature,
      final String emitterProgramId,
      final byte[][] inclusionBranch) {
    return hashHex(
        "sccp:solana:message-proof:v1",
        canonicalMessageProofBytes(
            sourceEventDigest,
            transactionStatusRoot,
            transactionSignature,
            emitterProgramId,
            inclusionBranch));
  }

  public static byte[] canonicalTransactionStatusLeafBytes(
      final String sourceEventDigest,
      final String transactionSignature,
      final String emitterProgramId) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    writeVec(
        out,
        decodeSolanaBase58Fixed(
            transactionSignature, "transactionSignature", TRANSACTION_SIGNATURE_BYTES));
    writeVec(out, decodeSolanaBase58Fixed(emitterProgramId, "emitterProgramId", PROGRAM_ID_BYTES));
    return out.toByteArray();
  }

  public static String transactionStatusLeafHash(
      final String sourceEventDigest,
      final String transactionSignature,
      final String emitterProgramId) {
    return hashHex(
        "sccp:solana:transaction-status-leaf:v1",
        canonicalTransactionStatusLeafBytes(
            sourceEventDigest, transactionSignature, emitterProgramId));
  }

  public static String transactionStatusRootFromBranch(
      final String sourceEventDigest,
      final String transactionSignature,
      final String emitterProgramId,
      final byte[][] inclusionBranch) {
    final byte[][] normalized = normalizeInclusionBranch(inclusionBranch);
    if (normalized.length == 0) {
      throw new IllegalArgumentException("inclusionBranch must not be empty");
    }
    byte[] current = hex32Bytes(
        transactionStatusLeafHash(sourceEventDigest, transactionSignature, emitterProgramId),
        "transactionStatusLeafHash");
    for (final byte[] sibling : normalized) {
      current = sourceMerkleNodeHash(current, sibling);
    }
    return "0x" + hexLower(current);
  }

  public static String mainnetEpochForSlot(final String slot) {
    return normalizeU64(slot, "slot")
        .divide(BigInteger.valueOf(MAINNET_SLOTS_PER_EPOCH))
        .toString();
  }

  public static byte[] canonicalEpochStakeRootBytes(
      final String epoch, final byte[][] validatorPublicKeys, final String[] validatorStakes) {
    final byte[] rosterBytes = canonicalVoteRosterBytes(validatorPublicKeys, validatorStakes);
    final byte[] rosterHash = hashBytes("sccp:solana:vote-roster:v1", rosterBytes);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, normalizeU64(epoch, "epoch"));
    write(out, rosterHash);
    write(out, rosterBytes);
    return out.toByteArray();
  }

  public static String epochStakeRoot(
      final String epoch, final byte[][] validatorPublicKeys, final String[] validatorStakes) {
    return hashHex(
        "sccp:solana:epoch-stake-root:v1",
        canonicalEpochStakeRootBytes(epoch, validatorPublicKeys, validatorStakes));
  }

  public static byte[] canonicalStakeActivationBytes(
      final String epoch,
      final byte[][] validatorPublicKeys,
      final String[] validatorStakes,
      final String[] validatorActivationEpochs,
      final String[] validatorDeactivationEpochs) {
    Objects.requireNonNull(validatorPublicKeys, "validatorPublicKeys");
    Objects.requireNonNull(validatorStakes, "validatorStakes");
    Objects.requireNonNull(validatorActivationEpochs, "validatorActivationEpochs");
    Objects.requireNonNull(validatorDeactivationEpochs, "validatorDeactivationEpochs");
    if (validatorActivationEpochs.length != validatorPublicKeys.length) {
      throw new IllegalArgumentException("validatorActivationEpochs must match validatorPublicKeys");
    }
    if (validatorDeactivationEpochs.length != validatorPublicKeys.length) {
      throw new IllegalArgumentException("validatorDeactivationEpochs must match validatorPublicKeys");
    }
    final BigInteger resolvedEpoch = normalizeU64(epoch, "epoch");
    final byte[] rosterBytes = canonicalVoteRosterBytes(validatorPublicKeys, validatorStakes);
    final byte[] rosterHash = hashBytes("sccp:solana:vote-roster:v1", rosterBytes);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, resolvedEpoch);
    write(out, rosterHash);
    writeU32Le(out, validatorPublicKeys.length);
    for (int i = 0; i < validatorPublicKeys.length; i++) {
      final BigInteger activationEpoch =
          normalizeU64(validatorActivationEpochs[i], "validatorActivationEpochs[" + i + "]");
      final BigInteger deactivationEpoch =
          normalizeU64(validatorDeactivationEpochs[i], "validatorDeactivationEpochs[" + i + "]");
      if (activationEpoch.compareTo(resolvedEpoch) >= 0) {
        throw new IllegalArgumentException("validatorActivationEpochs[" + i + "] must be active at epoch");
      }
      if (deactivationEpoch.compareTo(activationEpoch) <= 0) {
        throw new IllegalArgumentException(
            "validatorDeactivationEpochs[" + i + "] must be greater than activation epoch");
      }
      writeVec(out, validatorPublicKeys[i]);
      writeU64Le(out, normalizeU64(validatorStakes[i], "validatorStakes[" + i + "]"));
      writeU64Le(out, activationEpoch);
      writeU64Le(out, deactivationEpoch);
    }
    return out.toByteArray();
  }

  public static String stakeActivationHash(
      final String epoch,
      final byte[][] validatorPublicKeys,
      final String[] validatorStakes,
      final String[] validatorActivationEpochs,
      final String[] validatorDeactivationEpochs) {
    return hashHex(
        "sccp:solana:stake-activation:v1",
        canonicalStakeActivationBytes(
            epoch,
            validatorPublicKeys,
            validatorStakes,
            validatorActivationEpochs,
            validatorDeactivationEpochs));
  }

  public static byte[] canonicalAccountOpeningBytes(
      final byte[] address,
      final byte[] owner,
      final String lamports,
      final String rentEpoch,
      final boolean executable,
      final String dataHash) {
    Objects.requireNonNull(address, "address");
    Objects.requireNonNull(owner, "owner");
    if (address.length != 32 || !anyNonZero(address)) {
      throw new IllegalArgumentException("address must be a non-zero 32-byte Solana account id");
    }
    if (owner.length != 32 || !anyNonZero(owner)) {
      throw new IllegalArgumentException("owner must be a non-zero 32-byte Solana program id");
    }
    final BigInteger normalizedLamports = normalizeU64(lamports, "lamports");
    if (normalizedLamports.compareTo(BigInteger.ZERO) <= 0) {
      throw new IllegalArgumentException("lamports must be greater than zero");
    }
    final BigInteger normalizedRentEpoch = normalizeU64(rentEpoch, "rentEpoch");
    final byte[] dataHashBytes = hex32Bytes(dataHash, "dataHash");
    if (!anyNonZero(dataHashBytes)) {
      throw new IllegalArgumentException("dataHash must not be zero");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeVec(out, address);
    writeVec(out, owner);
    writeU64Le(out, normalizedLamports);
    writeU64Le(out, normalizedRentEpoch);
    out.write(executable ? 1 : 0);
    write(out, dataHashBytes);
    return out.toByteArray();
  }

  public static String accountOpeningHash(
      final byte[] address,
      final byte[] owner,
      final String lamports,
      final String rentEpoch,
      final boolean executable,
      final String dataHash) {
    return hashHex(
        "sccp:solana:account-opening:v1",
        canonicalAccountOpeningBytes(address, owner, lamports, rentEpoch, executable, dataHash));
  }

  public static String accountRawDataHash(final byte[] rawData) {
    Objects.requireNonNull(rawData, "rawData");
    if (rawData.length == 0 || rawData.length > MAX_ACCOUNT_RAW_DATA_BYTES) {
      throw new IllegalArgumentException("rawData must be between 1 and 65536 bytes");
    }
    return hashHex("sccp:solana:account-raw-data:v1", rawData);
  }

  public static String accountsLtHashChecksum(final byte[] accountsLtHash) {
    Objects.requireNonNull(accountsLtHash, "accountsLtHash");
    if (accountsLtHash.length != ACCOUNTS_LT_HASH_BYTES) {
      throw new IllegalArgumentException("accountsLtHash must be 2048 bytes");
    }
    return "0x" + hexLower(Blake3.hash(accountsLtHash));
  }

  public static byte[] accountLtHash(
      final AccountOpeningInput opening, final byte[] rawData) {
    Objects.requireNonNull(opening, "opening");
    Objects.requireNonNull(rawData, "rawData");
    final byte[] address = opening.address();
    final byte[] owner = opening.owner();
    if (address.length != 32) {
      throw new IllegalArgumentException("address must be 32 bytes");
    }
    if (owner.length != 32) {
      throw new IllegalArgumentException("owner must be 32 bytes");
    }
    if (rawData.length > MAX_ACCOUNT_RAW_DATA_BYTES) {
      throw new IllegalArgumentException("rawData must be at most 65536 bytes");
    }
    final BigInteger lamports = normalizeU64(opening.lamports(), "lamports");
    if (lamports.equals(BigInteger.ZERO)) {
      return new byte[ACCOUNTS_LT_HASH_BYTES];
    }
    final ByteArrayOutputStream preimage = new ByteArrayOutputStream();
    writeU64Le(preimage, lamports);
    write(preimage, rawData);
    preimage.write(opening.executable() ? 1 : 0);
    write(preimage, owner);
    write(preimage, address);
    return Blake3.derive(preimage.toByteArray(), ACCOUNTS_LT_HASH_BYTES);
  }

  public static byte[] accountsLtHashFromOpenings(
      final AccountOpeningInput[] openings, final byte[][] rawDataValues) {
    Objects.requireNonNull(openings, "openings");
    Objects.requireNonNull(rawDataValues, "rawDataValues");
    if (openings.length != rawDataValues.length) {
      throw new IllegalArgumentException("openings and rawDataValues must have matching lengths");
    }
    final byte[] out = new byte[ACCOUNTS_LT_HASH_BYTES];
    for (int i = 0; i < openings.length; i++) {
      addAccountsLtHashContribution(out, accountLtHash(openings[i], rawDataValues[i]));
    }
    return out;
  }

  public static byte[] openedAccountsLtHashResidual(
      final OpenedAccountsLtHashContributionsInput input) {
    return normalizeOpenedAccountsLtHashContributions(input).residualAccountsLtHash();
  }

  public static String openedAccountsLtHashResidualChecksum(
      final OpenedAccountsLtHashContributionsInput input) {
    return "0x"
        + hexLower(normalizeOpenedAccountsLtHashContributions(input).residualAccountsLtHashChecksum());
  }

  public static byte[] canonicalOpenedAccountsLtHashContributionsBytes(
      final OpenedAccountsLtHashContributionsInput input) {
    final NormalizedOpenedAccountsLtHashContributions normalized =
        normalizeOpenedAccountsLtHashContributions(input);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalized.sourceDomain());
    writeU64Le(out, normalized.finalizedSlot());
    write(out, normalized.accountInclusionRoot());
    write(out, normalized.accountsLtHashChecksum());
    write(out, normalized.openedAccountsLtHashChecksum());
    write(out, normalized.residualAccountsLtHashChecksum());
    writeVec(out, normalized.openedAccountsLtHash());
    writeVec(out, normalized.residualAccountsLtHash());
    writeU32Le(out, normalized.rows().size());
    for (final OpenedLtHashContributionRow row : normalized.rows()) {
      out.write(row.role());
      write(out, row.address());
      write(out, row.accountHash());
      write(out, row.rawDataHash());
      writeVec(out, row.accountLtHash());
    }
    return out.toByteArray();
  }

  public static String openedAccountsLtHashContributionsHash(
      final OpenedAccountsLtHashContributionsInput input) {
    return hashHex(
        ACCOUNTS_LT_HASH_OPENED_CONTRIBUTIONS_PREFIX_V1,
        canonicalOpenedAccountsLtHashContributionsBytes(input));
  }

  public static byte[] canonicalAccountsLtHashCommitmentBytes(
      final WitnessInput witnessInput, final OpenedAccountsLtHashContributionsInput openedInput) {
    final NormalizedAccountsLtHashProofRequest normalized =
        normalizeAccountsLtHashProofRequest(witnessInput, openedInput);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalized.witness().sourceDomain());
    writeU64Le(out, normalizeU64(normalized.witness().finalizedSlot(), "finalizedSlot"));
    write(out, hex32Bytes(normalized.witness().accountsLtHashChecksum(), "accountsLtHashChecksum"));
    write(
        out,
        hex32Bytes(
            normalized.openedContributionsHash(), "openedAccountsLtHashContributionsHash"));
    write(out, hex32Bytes(normalized.residualChecksum(), "openedAccountsLtHashResidualChecksum"));
    writeVec(out, normalized.accountsLtHash());
    return out.toByteArray();
  }

  public static byte[] canonicalAccountsLtHashVerificationContextBytes(
      final WitnessInput witnessInput, final OpenedAccountsLtHashContributionsInput openedInput) {
    final NormalizedAccountsLtHashProofRequest normalized =
        normalizeAccountsLtHashProofRequest(witnessInput, openedInput);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeString(out, ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1, "circuitId");
    writeString(out, ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1, "parameterSet");
    writeString(out, normalized.witness().sourceStateVerifierId(), "sourceStateVerifierId");
    write(out, hex32Bytes(normalized.witness().sourceStateVerifierHash(), "sourceStateVerifierHash"));
    write(
        out,
        hex32Bytes(
            normalized.witness().accountsLtHashProofPublicInputsHash(),
            "accountsLtHashProofPublicInputsHash"));
    write(
        out,
        hex32Bytes(
            normalized.openedContributionsHash(), "openedAccountsLtHashContributionsHash"));
    write(out, hex32Bytes(normalized.residualChecksum(), "openedAccountsLtHashResidualChecksum"));
    return out.toByteArray();
  }

  public static List<List<String>> accountsLtHashPublicInputColumns(
      final WitnessInput witnessInput, final OpenedAccountsLtHashContributionsInput openedInput) {
    final NormalizedAccountsLtHashProofRequest normalized =
        normalizeAccountsLtHashProofRequest(witnessInput, openedInput);
    final Witness witness = normalized.witness();
    final List<List<String>> columns = new ArrayList<>();
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU32Le(witness.sourceDomain()))));
    columns.add(Collections.singletonList(solanaMainnetGenesisHashPublicInput()));
    columns.add(
        Collections.singletonList(
            "0x" + hexLower(sccpWordU64Le(normalizeU64(witness.finalizedSlot(), "finalizedSlot")))));
    columns.add(
        Collections.singletonList(
            "0x" + hexLower(sccpWordU64Le(normalizeU64(witness.parentSlot(), "parentSlot")))));
    columns.add(
        Collections.singletonList(
            "0x"
                + hexLower(
                    sccpWordU64Le(
                        normalizeU64(witness.bankSignatureCount(), "bankSignatureCount")))));
    columns.add(Collections.singletonList(witness.parentBankHash()));
    columns.add(Collections.singletonList(witness.bankHash()));
    columns.add(
        Collections.singletonList("0x" + hexLower(solanaHash32Bytes(witness.blockhash(), "blockhash"))));
    columns.add(Collections.singletonList(witness.transactionStatusRoot()));
    columns.add(Collections.singletonList(witness.accountInclusionRoot()));
    columns.add(Collections.singletonList(witness.accountsLtHashChecksum()));
    columns.add(Collections.singletonList(witness.accountsLtHashProofPublicInputsHash()));
    columns.add(Collections.singletonList(normalized.openedContributionsHash()));
    columns.add(Collections.singletonList(normalized.residualChecksum()));
    return Collections.unmodifiableList(columns);
  }

  public static byte[] accountsLtHashOpenVerifySchemaDescriptor(
      final WitnessInput witnessInput, final OpenedAccountsLtHashContributionsInput openedInput) {
    final NormalizedAccountsLtHashProofRequest normalized =
        normalizeAccountsLtHashProofRequest(witnessInput, openedInput);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeString(out, ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1, "circuitId");
    writeString(out, ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1, "parameterSet");
    writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash");
    writeU32Le(out, normalized.witness().sourceDomain());
    writeString(out, "source_state_verifier_id", "schemaField");
    writeString(out, normalized.witness().sourceStateVerifierId(), "sourceStateVerifierId");
    writeString(out, "source_state_verifier_hash", "schemaField");
    write(out, hex32Bytes(normalized.witness().sourceStateVerifierHash(), "sourceStateVerifierHash"));
    final String[] requiredInputs = {
      "source_domain",
      "mainnet_genesis_hash",
      "finalized_slot",
      "parent_slot",
      "bank_signature_count",
      "parent_bank_hash",
      "bank_hash",
      "blockhash",
      "transaction_status_root",
      "account_inclusion_root",
      "accounts_lt_hash_checksum",
      "accounts_lt_hash_proof_public_inputs_hash",
      "opened_accounts_lt_hash_contributions_hash",
      "opened_accounts_lt_hash_residual_checksum"
    };
    for (final String requiredInput : requiredInputs) {
      writeString(out, requiredInput, "requiredInput");
    }
    return out.toByteArray();
  }

  public static AccountsLtHashProofRequest buildAccountsLtHashProofRequest(
      final WitnessInput witnessInput, final OpenedAccountsLtHashContributionsInput openedInput) {
    final NormalizedAccountsLtHashProofRequest normalized =
        normalizeAccountsLtHashProofRequest(witnessInput, openedInput);
    final Witness witness = normalized.witness();
    final byte[] statementBytes =
        canonicalAccountsLtHashProofPublicInputsBytes(
            witness.sourceDomain(),
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
            witness.accountsLtHash());
    final byte[] accountCommitmentBytes =
        canonicalAccountsLtHashCommitmentBytes(witnessInput, openedInput);
    final byte[] verificationContextBytes =
        canonicalAccountsLtHashVerificationContextBytes(witnessInput, openedInput);
    final byte[] dsidHash =
        hashBytes(
            ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX_V1,
            hex32Bytes(
                witness.accountsLtHashProofPublicInputsHash(),
                "accountsLtHashProofPublicInputsHash"));
    final List<AccountsLtHashFastpqTransition> transitions = new ArrayList<>();
    transitions.add(
        new AccountsLtHashFastpqTransition(
            ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY_V1, "meta_set", new byte[0], statementBytes));
    transitions.add(
        new AccountsLtHashFastpqTransition(
            ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY_V1,
            "meta_set",
            new byte[0],
            accountCommitmentBytes));
    transitions.add(
        new AccountsLtHashFastpqTransition(
            ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY_V1,
            "meta_set",
            new byte[0],
            hex32Bytes(normalized.openedContributionsHash(), "openedAccountsLtHashContributionsHash")));
    transitions.add(
        new AccountsLtHashFastpqTransition(
            ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY_V1,
            "meta_set",
            new byte[0],
            hex32Bytes(normalized.residualChecksum(), "openedAccountsLtHashResidualChecksum")));
    transitions.add(
        new AccountsLtHashFastpqTransition(
            ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY_V1,
            "meta_set",
            new byte[0],
            verificationContextBytes));
    return new AccountsLtHashProofRequest(
        1,
        "stark-fri-v1",
        ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1,
        ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1,
        witness.sourceDomain(),
        witness.finalizedSlot(),
        witness.parentSlot(),
        witness.sourceStateVerifierId(),
        witness.sourceStateVerifierHash(),
        witness.accountsLtHashProofPublicInputsHash(),
        normalized.openedContributionsHash(),
        normalized.residualChecksum(),
        statementBytes,
        accountCommitmentBytes,
        verificationContextBytes,
        accountsLtHashOpenVerifySchemaDescriptor(witnessInput, openedInput),
        accountsLtHashPublicInputColumns(witnessInput, openedInput),
        new AccountsLtHashFastpqPublicInputs(
            "0x" + hexLower(Arrays.copyOfRange(dsidHash, 0, 16)),
            witness.finalizedSlot(),
            witness.parentBankHash(),
            witness.bankHash(),
            witness.accountInclusionRoot(),
            witness.accountsLtHashProofPublicInputsHash()),
        transitions);
  }

  public static byte[] canonicalAccountInclusionLeafBytes(
      final String finalizedSlot,
      final byte[] address,
      final byte[] owner,
      final String lamports,
      final String rentEpoch,
      final boolean executable,
      final String dataHash,
      final String rawDataHash) {
    Objects.requireNonNull(address, "address");
    if (address.length != 32 || !anyNonZero(address)) {
      throw new IllegalArgumentException("address must be a non-zero 32-byte Solana account id");
    }
    final BigInteger slot = normalizeU64(finalizedSlot, "finalizedSlot");
    final byte[] rawDataHashBytes = hex32Bytes(rawDataHash, "rawDataHash");
    if (!anyNonZero(rawDataHashBytes)) {
      throw new IllegalArgumentException("rawDataHash must not be zero");
    }
    final byte[] openingHash =
        hex32Bytes(
            accountOpeningHash(address, owner, lamports, rentEpoch, executable, dataHash),
            "openingHash");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, slot);
    writeVec(out, address);
    write(out, openingHash);
    write(out, rawDataHashBytes);
    return out.toByteArray();
  }

  public static String accountInclusionLeafHash(
      final String finalizedSlot,
      final byte[] address,
      final byte[] owner,
      final String lamports,
      final String rentEpoch,
      final boolean executable,
      final String dataHash,
      final byte[] rawData) {
    return hashHex(
        "sccp:solana:account-inclusion-leaf:v1",
        canonicalAccountInclusionLeafBytes(
            finalizedSlot,
            address,
            owner,
            lamports,
            rentEpoch,
            executable,
            dataHash,
            accountRawDataHash(rawData)));
  }

  public static byte[] canonicalAccountInclusionNodeBytes(
      final String left, final String right) {
    final byte[] leftBytes = hex32Bytes(left, "left");
    final byte[] rightBytes = hex32Bytes(right, "right");
    if (!anyNonZero(leftBytes)) {
      throw new IllegalArgumentException("left must not be zero");
    }
    if (!anyNonZero(rightBytes)) {
      throw new IllegalArgumentException("right must not be zero");
    }
    final byte[] first;
    final byte[] second;
    if (compareLexicographically(leftBytes, rightBytes) <= 0) {
      first = leftBytes;
      second = rightBytes;
    } else {
      first = rightBytes;
      second = leftBytes;
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, first);
    write(out, second);
    return out.toByteArray();
  }

  public static String accountInclusionNodeHash(final String left, final String right) {
    return hashHex(
        "sccp:solana:account-inclusion-node:v1",
        canonicalAccountInclusionNodeBytes(left, right));
  }

  public static String accountInclusionRootFromBranch(
      final String leaf, final List<String> siblings) {
    if (siblings.size() > MAX_SOURCE_MERKLE_BRANCH_NODES) {
      throw new IllegalArgumentException(
          "siblings must contain at most " + MAX_SOURCE_MERKLE_BRANCH_NODES + " entries");
    }
    String current = normalizeNonZeroHex32(leaf, "leaf");
    for (int i = 0; i < siblings.size(); i++) {
      current = accountInclusionNodeHash(current, normalizeHex32(siblings.get(i), "siblings[" + i + "]"));
    }
    return current;
  }

  public static AccountInclusionWitness accountInclusionRootAndBranches(
      final List<String> leaves) {
    if (leaves.isEmpty()) {
      throw new IllegalArgumentException("leaves must be non-empty");
    }
    List<AccountInclusionLevelNode> level = new ArrayList<>();
    for (int i = 0; i < leaves.size(); i++) {
      final byte[] hash = hex32Bytes(leaves.get(i), "leaves[" + i + "]");
      if (!anyNonZero(hash)) {
        throw new IllegalArgumentException("leaves[" + i + "] must not be zero");
      }
      level.add(new AccountInclusionLevelNode(hash, Collections.singletonList(i)));
    }
    Collections.sort(level, (left, right) -> compareLexicographically(left.hash, right.hash));
    for (int i = 1; i < level.size(); i++) {
      if (Arrays.equals(level.get(i - 1).hash, level.get(i).hash)) {
        throw new IllegalArgumentException("leaves must be unique");
      }
    }
    final List<List<String>> branches = new ArrayList<>();
    for (int i = 0; i < leaves.size(); i++) {
      branches.add(new ArrayList<>());
    }
    while (level.size() > 1) {
      final List<AccountInclusionLevelNode> next = new ArrayList<>();
      for (int i = 0; i < level.size(); i += 2) {
        if (i + 1 >= level.size()) {
          next.add(level.get(i));
          continue;
        }
        final AccountInclusionLevelNode left = level.get(i);
        final AccountInclusionLevelNode right = level.get(i + 1);
        final String leftHex = "0x" + hexLower(left.hash);
        final String rightHex = "0x" + hexLower(right.hash);
        for (final int index : left.indexes) {
          branches.get(index).add(rightHex);
        }
        for (final int index : right.indexes) {
          branches.get(index).add(leftHex);
        }
        final List<Integer> indexes = new ArrayList<>(left.indexes);
        indexes.addAll(right.indexes);
        next.add(
            new AccountInclusionLevelNode(
                hex32Bytes(accountInclusionNodeHash(leftHex, rightHex), "parent"),
                indexes));
      }
      level = next;
    }
    return new AccountInclusionWitness("0x" + hexLower(level.get(0).hash), branches);
  }

  private static void requireUniqueOpenedAccountAddresses(
      final AccountOpeningInput[] voteOpenings,
      final AccountOpeningInput[] stakeOpenings,
      final AccountOpeningInput stakeHistoryOpening) {
    final Set<String> seenAddresses = new HashSet<>();
    for (final AccountOpeningInput opening : voteOpenings) {
      requireUniqueOpenedAccountAddress(seenAddresses, opening);
    }
    for (final AccountOpeningInput opening : stakeOpenings) {
      requireUniqueOpenedAccountAddress(seenAddresses, opening);
    }
    requireUniqueOpenedAccountAddress(seenAddresses, stakeHistoryOpening);
  }

  private static void requireUniqueOpenedAccountAddress(
      final Set<String> seenAddresses, final AccountOpeningInput opening) {
    if (opening.address().length != 32) {
      throw new IllegalArgumentException("address must be 32 bytes");
    }
    if (!seenAddresses.add(hexLower(opening.address()))) {
      throw new IllegalArgumentException("opened account addresses must be unique");
    }
  }

  public static OpenedAccountInclusionWitness openedAccountInclusionWitness(
      final OpenedAccountInclusionWitnessInput input) {
    Objects.requireNonNull(input, "input");
    if (input.validatorVoteAccountOpenings().length != input.validatorVoteAccountRawData().length) {
      throw new IllegalArgumentException(
          "validatorVoteAccountOpenings and validatorVoteAccountRawData must have matching lengths");
    }
    if (input.validatorVoteAccountOpenings().length > MAX_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorVoteAccountOpenings must contain at most " + MAX_VALIDATORS + " entries");
    }
    if (input.validatorStakeAccountOpenings().length != input.validatorStakeAccountRawData().length) {
      throw new IllegalArgumentException(
          "validatorStakeAccountOpenings and validatorStakeAccountRawData must have matching lengths");
    }
    if (input.validatorStakeAccountOpenings().length > MAX_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorStakeAccountOpenings must contain at most " + MAX_VALIDATORS + " entries");
    }
    requireUniqueOpenedAccountAddresses(
        input.validatorVoteAccountOpenings(),
        input.validatorStakeAccountOpenings(),
        input.stakeHistorySysvarOpening());
    final List<String> voteLeaves = new ArrayList<>();
    for (int i = 0; i < input.validatorVoteAccountOpenings().length; i++) {
      voteLeaves.add(
          accountInclusionLeafHash(
              input.finalizedSlot(),
              input.validatorVoteAccountOpenings()[i].address(),
              input.validatorVoteAccountOpenings()[i].owner(),
              input.validatorVoteAccountOpenings()[i].lamports(),
              input.validatorVoteAccountOpenings()[i].rentEpoch(),
              input.validatorVoteAccountOpenings()[i].executable(),
              input.validatorVoteAccountOpenings()[i].dataHash(),
              input.validatorVoteAccountRawData()[i]));
    }
    final List<String> stakeLeaves = new ArrayList<>();
    for (int i = 0; i < input.validatorStakeAccountOpenings().length; i++) {
      stakeLeaves.add(
          accountInclusionLeafHash(
              input.finalizedSlot(),
              input.validatorStakeAccountOpenings()[i].address(),
              input.validatorStakeAccountOpenings()[i].owner(),
              input.validatorStakeAccountOpenings()[i].lamports(),
              input.validatorStakeAccountOpenings()[i].rentEpoch(),
              input.validatorStakeAccountOpenings()[i].executable(),
              input.validatorStakeAccountOpenings()[i].dataHash(),
              input.validatorStakeAccountRawData()[i]));
    }
    final String stakeHistoryLeaf =
        accountInclusionLeafHash(
            input.finalizedSlot(),
            input.stakeHistorySysvarOpening().address(),
            input.stakeHistorySysvarOpening().owner(),
            input.stakeHistorySysvarOpening().lamports(),
            input.stakeHistorySysvarOpening().rentEpoch(),
            input.stakeHistorySysvarOpening().executable(),
            input.stakeHistorySysvarOpening().dataHash(),
            input.stakeHistorySysvarRawData());
    final List<String> leaves = new ArrayList<>(voteLeaves);
    leaves.addAll(stakeLeaves);
    leaves.add(stakeHistoryLeaf);
    final AccountInclusionWitness witness = accountInclusionRootAndBranches(leaves);
    if (input.expectedAccountInclusionRoot() != null
        && !normalizeNonZeroHex32(input.expectedAccountInclusionRoot(), "accountInclusionRoot")
            .equals(witness.root)) {
      throw new IllegalArgumentException("accountInclusionRoot must match opened account inclusion witness");
    }
    final List<List<String>> voteBranches =
        new ArrayList<>(witness.branches.subList(0, voteLeaves.size()));
    final List<List<String>> stakeBranches =
        new ArrayList<>(
            witness.branches.subList(voteLeaves.size(), voteLeaves.size() + stakeLeaves.size()));
    return new OpenedAccountInclusionWitness(
        witness.root,
        witness.branches,
        voteBranches,
        stakeBranches,
        witness.branches.get(witness.branches.size() - 1));
  }

  public static OpenedAccountInclusionWitness openedAccountInclusionWitness(
      final OpenedAccountsLtHashContributionsInput input) {
    return openedAccountInclusionWitness(
        new OpenedAccountInclusionWitnessInput(
            input.finalizedSlot(),
            input.validatorVoteAccountOpenings(),
            input.validatorVoteAccountRawData(),
            input.validatorStakeAccountOpenings(),
            input.validatorStakeAccountRawData(),
            input.stakeHistorySysvarOpening(),
            input.stakeHistorySysvarRawData(),
            input.accountInclusionRoot()));
  }

  public static byte[] canonicalVoteAccountDataBytes(
      final byte[] nodePubkey,
      final byte[] authorizedVoter,
      final byte[] authorizedWithdrawer,
      final byte[] inflationRewardsCollector,
      final byte[] blockRevenueCollector,
      final String inflationRewardsCommissionBps,
      final String blockRevenueCommissionBps,
      final String pendingDelegatorRewards,
      final byte[] blsPubkeyCompressed,
      final String rootSlot,
      final String[] towerVoteSlots) {
    requireNonZeroPublicKey(nodePubkey, "nodePubkey");
    requireNonZeroPublicKey(authorizedVoter, "authorizedVoter");
    requireNonZeroPublicKey(authorizedWithdrawer, "authorizedWithdrawer");
    requireNonZeroPublicKey(inflationRewardsCollector, "inflationRewardsCollector");
    requireNonZeroPublicKey(blockRevenueCollector, "blockRevenueCollector");
    final BigInteger normalizedInflationRewardsCommissionBps =
        normalizeU64(inflationRewardsCommissionBps, "inflationRewardsCommissionBps");
    final BigInteger normalizedBlockRevenueCommissionBps =
        normalizeU64(blockRevenueCommissionBps, "blockRevenueCommissionBps");
    final BigInteger normalizedPendingDelegatorRewards =
        normalizeU64(pendingDelegatorRewards, "pendingDelegatorRewards");
    if (normalizedInflationRewardsCommissionBps.compareTo(BASIS_POINTS_PER_UNIT) > 0) {
      throw new IllegalArgumentException("inflationRewardsCommissionBps must be at most 10000");
    }
    if (normalizedBlockRevenueCommissionBps.compareTo(BASIS_POINTS_PER_UNIT) > 0) {
      throw new IllegalArgumentException("blockRevenueCommissionBps must be at most 10000");
    }
    Objects.requireNonNull(blsPubkeyCompressed, "blsPubkeyCompressed");
    if (blsPubkeyCompressed.length != 0 && blsPubkeyCompressed.length != BLS_PUBLIC_KEY_COMPRESSED_LEN) {
      throw new IllegalArgumentException("blsPubkeyCompressed must be empty or 48 bytes");
    }
    if (blsPubkeyCompressed.length == BLS_PUBLIC_KEY_COMPRESSED_LEN
        && !anyNonZero(blsPubkeyCompressed)) {
      throw new IllegalArgumentException("blsPubkeyCompressed must be empty or non-zero 48 bytes");
    }
    final BigInteger normalizedRootSlot = normalizeU64(rootSlot, "rootSlot");
    Objects.requireNonNull(towerVoteSlots, "towerVoteSlots");
    if (towerVoteSlots.length != TOWER_VOTE_STACK_DEPTH) {
      throw new IllegalArgumentException("towerVoteSlots must contain 31 active post-root slots");
    }
    final BigInteger[] slots = new BigInteger[towerVoteSlots.length];
    BigInteger previousSlot = normalizedRootSlot;
    for (int i = 0; i < towerVoteSlots.length; i++) {
      slots[i] = normalizeU64(towerVoteSlots[i], "towerVoteSlots[" + i + "]");
      if (slots[i].compareTo(previousSlot) <= 0) {
        throw new IllegalArgumentException(
            "towerVoteSlots[" + i + "] must be greater than the previous slot");
      }
      previousSlot = slots[i];
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeVec(out, nodePubkey);
    writeVec(out, authorizedVoter);
    writeVec(out, authorizedWithdrawer);
    writeVec(out, inflationRewardsCollector);
    writeVec(out, blockRevenueCollector);
    writeU16Le(out, normalizedInflationRewardsCommissionBps.intValue());
    writeU16Le(out, normalizedBlockRevenueCommissionBps.intValue());
    writeU64Le(out, normalizedPendingDelegatorRewards);
    writeVec(out, blsPubkeyCompressed);
    writeU64Le(out, normalizedRootSlot);
    writeU32Le(out, slots.length);
    for (final BigInteger slot : slots) {
      writeU64Le(out, slot);
    }
    return out.toByteArray();
  }

  public static String voteAccountDataHash(
      final byte[] nodePubkey,
      final byte[] authorizedVoter,
      final byte[] authorizedWithdrawer,
      final byte[] inflationRewardsCollector,
      final byte[] blockRevenueCollector,
      final String inflationRewardsCommissionBps,
      final String blockRevenueCommissionBps,
      final String pendingDelegatorRewards,
      final byte[] blsPubkeyCompressed,
      final String rootSlot,
      final String[] towerVoteSlots) {
    return hashHex(
        "sccp:solana:vote-account-data:v1",
        canonicalVoteAccountDataBytes(
            nodePubkey,
            authorizedVoter,
            authorizedWithdrawer,
            inflationRewardsCollector,
            blockRevenueCollector,
            inflationRewardsCommissionBps,
            blockRevenueCommissionBps,
            pendingDelegatorRewards,
            blsPubkeyCompressed,
            rootSlot,
            towerVoteSlots));
  }

  public static ParsedVoteStateV1OrV3AccountData voteAccountDataFromRawVoteState(
      final byte[] rawData, final String epoch, final byte[] voteAccountAddress) {
    Objects.requireNonNull(rawData, "rawData");
    if (rawData.length != VOTE_STATE_ACCOUNT_DATA_LEN) {
      throw new IllegalArgumentException("rawData must be a 3762-byte Solana VoteState account");
    }
    requireNonZeroPublicKey(voteAccountAddress, "voteAccountAddress");
    final BigInteger signedEpoch = normalizeU64(epoch, "epoch");
    final int[] cursor = new int[] {0};
    final int variant = readU32LeAt(rawData, cursor, "voteStateVariant");
    final boolean hasLatency;
    if (variant == VOTE_STATE_V1_14_11_DISCRIMINANT) {
      hasLatency = false;
    } else if (variant == VOTE_STATE_V3_DISCRIMINANT) {
      hasLatency = true;
    } else if (variant == VOTE_STATE_V4_DISCRIMINANT) {
      hasLatency = true;
    } else {
      throw new IllegalArgumentException("rawData must contain VoteStateVersions::V1_14_11, ::V3, or ::V4");
    }

    final byte[] nodePubkey = readPubkeyAt(rawData, cursor, "nodePubkey");
    final byte[] authorizedWithdrawer = readPubkeyAt(rawData, cursor, "authorizedWithdrawer");
    final byte[] inflationRewardsCollector;
    final byte[] blockRevenueCollector;
    final String inflationRewardsCommissionBps;
    final String blockRevenueCommissionBps;
    final String pendingDelegatorRewards;
    final byte[] blsPubkeyCompressed;
    if (variant == VOTE_STATE_V4_DISCRIMINANT) {
      inflationRewardsCollector = readPubkeyAt(rawData, cursor, "inflationRewardsCollector");
      blockRevenueCollector = readPubkeyAt(rawData, cursor, "blockRevenueCollector");
      final int inflationRewardsCommissionBpsValue =
          readU16LeAt(rawData, cursor, "inflationRewardsCommissionBps");
      final int blockRevenueCommissionBpsValue =
          readU16LeAt(rawData, cursor, "blockRevenueCommissionBps");
      if (inflationRewardsCommissionBpsValue > BASIS_POINTS_PER_UNIT.intValue()) {
        throw new IllegalArgumentException("inflationRewardsCommissionBps must be at most 10000");
      }
      if (blockRevenueCommissionBpsValue > BASIS_POINTS_PER_UNIT.intValue()) {
        throw new IllegalArgumentException("blockRevenueCommissionBps must be at most 10000");
      }
      inflationRewardsCommissionBps = Integer.toString(inflationRewardsCommissionBpsValue);
      blockRevenueCommissionBps = Integer.toString(blockRevenueCommissionBpsValue);
      pendingDelegatorRewards = readU64LeAt(rawData, cursor, "pendingDelegatorRewards").toString();
      final int blsVariant = readU8At(rawData, cursor, "blsPubkeyCompressed");
      if (blsVariant == 0) {
        blsPubkeyCompressed = new byte[0];
      } else if (blsVariant == 1) {
        if (cursor[0] + BLS_PUBLIC_KEY_COMPRESSED_LEN > rawData.length) {
          throw new IllegalArgumentException("blsPubkeyCompressed is too short");
        }
        blsPubkeyCompressed = Arrays.copyOfRange(rawData, cursor[0], cursor[0] + BLS_PUBLIC_KEY_COMPRESSED_LEN);
        cursor[0] += BLS_PUBLIC_KEY_COMPRESSED_LEN;
      } else {
        throw new IllegalArgumentException("blsPubkeyCompressed option discriminator must be 0 or 1");
      }
    } else {
      final int commission = readU8At(rawData, cursor, "commission");
      inflationRewardsCollector = Arrays.copyOf(voteAccountAddress, voteAccountAddress.length);
      blockRevenueCollector = Arrays.copyOf(nodePubkey, nodePubkey.length);
      inflationRewardsCommissionBps = Integer.toString(commission * 100);
      blockRevenueCommissionBps = BASIS_POINTS_PER_UNIT.toString();
      pendingDelegatorRewards = "0";
      blsPubkeyCompressed = new byte[0];
    }
    final BigInteger voteCount = readU64LeAt(rawData, cursor, "towerVoteSlots");
    if (!voteCount.equals(BigInteger.valueOf(TOWER_VOTE_STACK_DEPTH))) {
      throw new IllegalArgumentException("towerVoteSlots must contain 31 active post-root slots");
    }
    final int depth = (int) TOWER_VOTE_STACK_DEPTH;
    final String[] towerVoteSlots = new String[depth];
    final BigInteger[] towerVoteSlotValues = new BigInteger[depth];
    for (int i = 0; i < depth; i++) {
      if (hasLatency) {
        readU8At(rawData, cursor, "towerVoteSlots[" + i + "].latency");
      }
      final BigInteger slot = readU64LeAt(rawData, cursor, "towerVoteSlots[" + i + "].slot");
      final int confirmationCount =
          readU32LeAt(rawData, cursor, "towerVoteSlots[" + i + "].confirmationCount");
      if (confirmationCount != depth - i) {
        throw new IllegalArgumentException(
            "towerVoteSlots[" + i + "] has an invalid Tower confirmation count");
      }
      towerVoteSlots[i] = slot.toString();
      towerVoteSlotValues[i] = slot;
    }

    if (readU8At(rawData, cursor, "rootSlot") != 1) {
      throw new IllegalArgumentException("rawData must contain a rooted vote state");
    }
    final BigInteger rootSlot = readU64LeAt(rawData, cursor, "rootSlot");
    BigInteger previousTowerSlot = rootSlot;
    for (int i = 0; i < towerVoteSlotValues.length; i++) {
      if (towerVoteSlotValues[i].compareTo(previousTowerSlot) <= 0) {
        throw new IllegalArgumentException(
            "towerVoteSlots[" + i + "] must be greater than the previous slot");
      }
      previousTowerSlot = towerVoteSlotValues[i];
    }
    final BigInteger authorizedVoterCount = readU64LeAt(rawData, cursor, "authorizedVoters");
    final int authorizedVoterLimit =
        variant == VOTE_STATE_V4_DISCRIMINANT
            ? VOTE_STATE_V4_AUTHORIZED_VOTERS
            : VOTE_STATE_PRIOR_VOTERS;
    if (authorizedVoterCount.compareTo(BigInteger.ZERO) <= 0
        || authorizedVoterCount.compareTo(BigInteger.valueOf(authorizedVoterLimit)) > 0) {
      throw new IllegalArgumentException(
          variant == VOTE_STATE_V4_DISCRIMINANT
              ? "authorizedVoters must contain 1..4 entries for VoteStateV4"
              : "authorizedVoters must contain 1..32 entries");
    }
    BigInteger previousAuthorizedEpoch = null;
    byte[] authorizedVoter = null;
    for (int i = 0; i < authorizedVoterCount.intValue(); i++) {
      final BigInteger authorizedEpoch = readU64LeAt(rawData, cursor, "authorizedVoters[" + i + "].epoch");
      if (previousAuthorizedEpoch != null
          && previousAuthorizedEpoch.compareTo(authorizedEpoch) >= 0) {
        throw new IllegalArgumentException(
            "authorizedVoters must be sorted by strictly increasing epoch");
      }
      final byte[] voter = readPubkeyAt(rawData, cursor, "authorizedVoters[" + i + "].authorizedVoter");
      if (!anyNonZero(voter)) {
        throw new IllegalArgumentException(
            "authorizedVoters[" + i + "].authorizedVoter must be non-zero");
      }
      if (authorizedEpoch.compareTo(signedEpoch) <= 0) {
        authorizedVoter = voter;
      }
      previousAuthorizedEpoch = authorizedEpoch;
    }
    if (authorizedVoter == null) {
      throw new IllegalArgumentException("authorizedVoters must include an entry at or before epoch");
    }
    if (variant != VOTE_STATE_V4_DISCRIMINANT) {
      for (int i = 0; i < VOTE_STATE_PRIOR_VOTERS; i++) {
        final byte[] priorVoter = readPubkeyAt(rawData, cursor, "priorVoters[" + i + "].pubkey");
        final BigInteger fromEpoch = readU64LeAt(rawData, cursor, "priorVoters[" + i + "].fromEpoch");
        final BigInteger untilEpoch = readU64LeAt(rawData, cursor, "priorVoters[" + i + "].untilEpoch");
        if (!anyNonZero(priorVoter)) {
          if (fromEpoch.compareTo(BigInteger.ZERO) != 0
              || untilEpoch.compareTo(BigInteger.ZERO) != 0) {
            throw new IllegalArgumentException(
                "priorVoters[" + i + "] zero pubkey must have zero epoch bounds");
          }
        } else if (fromEpoch.compareTo(untilEpoch) >= 0) {
          throw new IllegalArgumentException(
              "priorVoters[" + i + "] must have increasing epoch bounds");
        }
      }
      final BigInteger priorVotersIndex = readU64LeAt(rawData, cursor, "priorVoters.index");
      final int priorVotersIsEmpty = readU8At(rawData, cursor, "priorVoters.isEmpty");
      if (priorVotersIndex.compareTo(BigInteger.valueOf(VOTE_STATE_PRIOR_VOTERS)) >= 0
          || (priorVotersIsEmpty != 0 && priorVotersIsEmpty != 1)) {
        throw new IllegalArgumentException("priorVoters must have a valid cursor and boolean empty flag");
      }
    }
    final BigInteger epochCreditCount = readU64LeAt(rawData, cursor, "epochCredits");
    if (epochCreditCount.compareTo(BigInteger.valueOf(VOTE_STATE_MAX_EPOCH_CREDITS)) > 0) {
      throw new IllegalArgumentException("epochCredits exceeds Solana history bound");
    }
    BigInteger previousEpochCreditEpoch = null;
    BigInteger previousEpochCreditTotal = null;
    for (int i = 0; i < epochCreditCount.intValue(); i++) {
      final BigInteger creditEpoch = readU64LeAt(rawData, cursor, "epochCredits[" + i + "].epoch");
      final BigInteger credits = readU64LeAt(rawData, cursor, "epochCredits[" + i + "].credits");
      final BigInteger previousCredits =
          readU64LeAt(rawData, cursor, "epochCredits[" + i + "].previousCredits");
      if (creditEpoch.compareTo(signedEpoch) > 0
          || (previousEpochCreditEpoch != null && previousEpochCreditEpoch.compareTo(creditEpoch) >= 0)
          || previousCredits.compareTo(credits) > 0
          || (previousEpochCreditTotal != null && previousEpochCreditTotal.compareTo(previousCredits) > 0)) {
        throw new IllegalArgumentException("epochCredits must be sorted and monotonic");
      }
      previousEpochCreditEpoch = creditEpoch;
      previousEpochCreditTotal = credits;
    }
    final BigInteger lastTimestampSlot = readU64LeAt(rawData, cursor, "lastTimestamp.slot");
    final BigInteger lastTimestamp = readU64LeAt(rawData, cursor, "lastTimestamp.timestamp");
    final BigInteger lastTowerVoteSlot = towerVoteSlotValues[towerVoteSlotValues.length - 1];
    if ((lastTimestampSlot.equals(BigInteger.ZERO) && !lastTimestamp.equals(BigInteger.ZERO))
        || (!lastTimestampSlot.equals(BigInteger.ZERO)
            && (lastTimestampSlot.compareTo(lastTowerVoteSlot) > 0
                || lastTimestamp.compareTo(BigInteger.valueOf(Long.MAX_VALUE)) > 0))) {
      throw new IllegalArgumentException(
          "lastTimestamp must be default or within the Tower vote stack");
    }
    for (int i = cursor[0]; i < rawData.length; i++) {
      if (rawData[i] != 0) {
        throw new IllegalArgumentException("rawData padding must be zero");
      }
    }
    final ParsedVoteStateV1OrV3AccountData parsed =
        new ParsedVoteStateV1OrV3AccountData(
            nodePubkey,
            authorizedVoter,
            authorizedWithdrawer,
            inflationRewardsCollector,
            blockRevenueCollector,
            inflationRewardsCommissionBps,
            blockRevenueCommissionBps,
            pendingDelegatorRewards,
            blsPubkeyCompressed,
            rootSlot.toString(),
            towerVoteSlots);
    canonicalVoteAccountDataBytes(
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
        parsed.towerVoteSlots());
    return parsed;
  }

  public static String voteAccountDataHashFromRawVoteState(
      final byte[] rawData, final String epoch, final byte[] voteAccountAddress) {
    final ParsedVoteStateV1OrV3AccountData parsed =
        voteAccountDataFromRawVoteState(rawData, epoch, voteAccountAddress);
    return voteAccountDataHash(
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
        parsed.towerVoteSlots());
  }

  public static ParsedVoteStateV1OrV3AccountData voteAccountDataFromRawVoteStateV1OrV3(
      final byte[] rawData, final String epoch, final byte[] voteAccountAddress) {
    return voteAccountDataFromRawVoteState(rawData, epoch, voteAccountAddress);
  }

  public static String voteAccountDataHashFromRawVoteStateV1OrV3(
      final byte[] rawData, final String epoch, final byte[] voteAccountAddress) {
    return voteAccountDataHashFromRawVoteState(rawData, epoch, voteAccountAddress);
  }

  public static byte[] canonicalStakeAccountDataBytes(
      final byte[] staker,
      final byte[] withdrawer,
      final byte[] voterPubkey,
      final String delegatedStake,
      final String activationEpoch,
      final String deactivationEpoch,
      final String creditsObserved) {
    return canonicalStakeAccountDataBytes(
        staker,
        withdrawer,
        voterPubkey,
        delegatedStake,
        activationEpoch,
        deactivationEpoch,
        creditsObserved,
        "0");
  }

  public static byte[] canonicalStakeAccountDataBytes(
      final byte[] staker,
      final byte[] withdrawer,
      final byte[] voterPubkey,
      final String delegatedStake,
      final String activationEpoch,
      final String deactivationEpoch,
      final String creditsObserved,
      final String stakeFlags) {
    return canonicalStakeAccountDataBytes(
        staker,
        withdrawer,
        voterPubkey,
        delegatedStake,
        activationEpoch,
        deactivationEpoch,
        creditsObserved,
        stakeFlags,
        Arrays.copyOf(
            STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES,
            STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES.length));
  }

  public static byte[] canonicalStakeAccountDataBytes(
      final byte[] staker,
      final byte[] withdrawer,
      final byte[] voterPubkey,
      final String delegatedStake,
      final String activationEpoch,
      final String deactivationEpoch,
      final String creditsObserved,
      final String stakeFlags,
      final byte[] warmupCooldownRateBytes) {
    requireNonZeroPublicKey(staker, "staker");
    requireNonZeroPublicKey(withdrawer, "withdrawer");
    requireNonZeroPublicKey(voterPubkey, "voterPubkey");
    Objects.requireNonNull(warmupCooldownRateBytes, "warmupCooldownRateBytes");
    final BigInteger normalizedDelegatedStake = normalizeU64(delegatedStake, "delegatedStake");
    if (normalizedDelegatedStake.compareTo(BigInteger.ZERO) <= 0) {
      throw new IllegalArgumentException("delegatedStake must be greater than zero");
    }
    final BigInteger normalizedActivationEpoch = normalizeU64(activationEpoch, "activationEpoch");
    final BigInteger normalizedDeactivationEpoch =
        normalizeU64(deactivationEpoch, "deactivationEpoch");
    if (normalizedDeactivationEpoch.compareTo(normalizedActivationEpoch) <= 0) {
      throw new IllegalArgumentException("deactivationEpoch must be greater than activationEpoch");
    }
    if (warmupCooldownRateBytes.length != STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_BYTES) {
      throw new IllegalArgumentException("warmupCooldownRateBytes must be 8 bytes");
    }
    if (!isSupportedStakeWarmupCooldownRateBytes(warmupCooldownRateBytes)) {
      throw new IllegalArgumentException(
          "warmupCooldownRateBytes must be Solana 0.25 or 0.09 f64 bytes");
    }
    final BigInteger normalizedCreditsObserved =
        normalizeU64(creditsObserved == null ? "0" : creditsObserved, "creditsObserved");
    final BigInteger normalizedStakeFlags =
        normalizeU64(stakeFlags == null ? "0" : stakeFlags, "stakeFlags");
    if (normalizedStakeFlags.compareTo(BigInteger.valueOf(255L)) > 0
        || (normalizedStakeFlags.intValue() & ~STAKE_STATE_V2_KNOWN_FLAGS_MASK) != 0) {
      throw new IllegalArgumentException("stakeFlags contains reserved StakeFlags bits");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeVec(out, staker);
    writeVec(out, withdrawer);
    writeVec(out, voterPubkey);
    writeU64Le(out, normalizedDelegatedStake);
    writeU64Le(out, normalizedActivationEpoch);
    writeU64Le(out, normalizedDeactivationEpoch);
    writeVec(out, warmupCooldownRateBytes);
    writeU64Le(out, normalizedCreditsObserved);
    out.write(normalizedStakeFlags.intValue());
    return out.toByteArray();
  }

  public static String stakeAccountDataHash(
      final byte[] staker,
      final byte[] withdrawer,
      final byte[] voterPubkey,
      final String delegatedStake,
      final String activationEpoch,
      final String deactivationEpoch,
      final String creditsObserved) {
    return stakeAccountDataHash(
        staker,
        withdrawer,
        voterPubkey,
        delegatedStake,
        activationEpoch,
        deactivationEpoch,
        creditsObserved,
        "0");
  }

  public static String stakeAccountDataHash(
      final byte[] staker,
      final byte[] withdrawer,
      final byte[] voterPubkey,
      final String delegatedStake,
      final String activationEpoch,
      final String deactivationEpoch,
      final String creditsObserved,
      final String stakeFlags) {
    return stakeAccountDataHash(
        staker,
        withdrawer,
        voterPubkey,
        delegatedStake,
        activationEpoch,
        deactivationEpoch,
        creditsObserved,
        stakeFlags,
        Arrays.copyOf(
            STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES,
            STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES.length));
  }

  public static String stakeAccountDataHash(
      final byte[] staker,
      final byte[] withdrawer,
      final byte[] voterPubkey,
      final String delegatedStake,
      final String activationEpoch,
      final String deactivationEpoch,
      final String creditsObserved,
      final String stakeFlags,
      final byte[] warmupCooldownRateBytes) {
    return hashHex(
        "sccp:solana:stake-account-data:v1",
        canonicalStakeAccountDataBytes(
            staker,
            withdrawer,
            voterPubkey,
            delegatedStake,
            activationEpoch,
            deactivationEpoch,
            creditsObserved,
            stakeFlags,
            warmupCooldownRateBytes));
  }

  public static ParsedStakeStateV2StakeAccountData stakeAccountDataFromRawStakeStateV2(
      final byte[] rawData) {
    Objects.requireNonNull(rawData, "rawData");
    if (rawData.length != STAKE_STATE_V2_STAKE_ACCOUNT_DATA_LEN) {
      throw new IllegalArgumentException("rawData must be a 200-byte Solana StakeStateV2 account");
    }
    if (readU32Le(rawData, 0) != STAKE_STATE_V2_STAKE_DISCRIMINANT) {
      throw new IllegalArgumentException("rawData must contain StakeStateV2::Stake");
    }
    for (int i = STAKE_STATE_V2_FLAG_OFFSET + 1; i < rawData.length; i++) {
      if (rawData[i] != 0) {
        throw new IllegalArgumentException("rawData must not contain non-zero stake account padding");
      }
    }
    final int stakeFlags = rawData[STAKE_STATE_V2_FLAG_OFFSET] & 0xff;
    if ((stakeFlags & ~STAKE_STATE_V2_KNOWN_FLAGS_MASK) != 0) {
      throw new IllegalArgumentException("rawData contains reserved StakeFlags bits");
    }
    final ParsedStakeStateV2StakeAccountData parsed =
        new ParsedStakeStateV2StakeAccountData(
            Arrays.copyOfRange(rawData, STAKE_STATE_V2_STAKER_OFFSET, STAKE_STATE_V2_STAKER_OFFSET + 32),
            Arrays.copyOfRange(rawData, STAKE_STATE_V2_WITHDRAWER_OFFSET, STAKE_STATE_V2_WITHDRAWER_OFFSET + 32),
            Arrays.copyOfRange(
                rawData, STAKE_STATE_V2_VOTER_PUBKEY_OFFSET, STAKE_STATE_V2_VOTER_PUBKEY_OFFSET + 32),
            readU64Le(rawData, STAKE_STATE_V2_DELEGATED_STAKE_OFFSET).toString(),
            readU64Le(rawData, STAKE_STATE_V2_ACTIVATION_EPOCH_OFFSET).toString(),
            readU64Le(rawData, STAKE_STATE_V2_DEACTIVATION_EPOCH_OFFSET).toString(),
            Arrays.copyOfRange(
                rawData,
                STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_OFFSET,
                STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_OFFSET + STAKE_STATE_V2_WARMUP_COOLDOWN_RATE_BYTES),
            readU64Le(rawData, STAKE_STATE_V2_CREDITS_OBSERVED_OFFSET).toString(),
            Integer.toString(stakeFlags));
    canonicalStakeAccountDataBytes(
        parsed.staker(),
        parsed.withdrawer(),
        parsed.voterPubkey(),
        parsed.delegatedStake(),
        parsed.activationEpoch(),
        parsed.deactivationEpoch(),
        parsed.creditsObserved(),
        parsed.stakeFlags(),
        parsed.warmupCooldownRateBytes());
    return parsed;
  }

  public static String stakeAccountDataHashFromRawStakeStateV2(final byte[] rawData) {
    final ParsedStakeStateV2StakeAccountData parsed = stakeAccountDataFromRawStakeStateV2(rawData);
    return stakeAccountDataHash(
        parsed.staker(),
        parsed.withdrawer(),
        parsed.voterPubkey(),
        parsed.delegatedStake(),
        parsed.activationEpoch(),
        parsed.deactivationEpoch(),
        parsed.creditsObserved(),
        parsed.stakeFlags(),
        parsed.warmupCooldownRateBytes());
  }

  public static byte[] canonicalStakeAccountStateBytes(
      final String epoch,
      final byte[][] validatorPublicKeys,
      final String[] validatorStakes,
      final String[] validatorActivationEpochs,
      final String[] validatorDeactivationEpochs,
      final byte[][] validatorVoteAccountAddresses,
      final byte[][] validatorStakeAccountAddresses,
      final byte[][] validatorVoteAccountHashes,
      final byte[][] validatorStakeAccountHashes) {
    final BigInteger resolvedEpoch = normalizeU64(epoch, "epoch");
    final byte[] activationBytes =
        canonicalStakeActivationBytes(
            epoch,
            validatorPublicKeys,
            validatorStakes,
            validatorActivationEpochs,
            validatorDeactivationEpochs);
    final byte[] stakeActivationHash = hashBytes("sccp:solana:stake-activation:v1", activationBytes);
    final byte[][] voteAccounts =
        normalizeFixed32Array(
            validatorVoteAccountAddresses,
            validatorPublicKeys.length,
            "validatorVoteAccountAddresses",
            true);
    final byte[][] stakeAccounts =
        normalizeFixed32Array(
            validatorStakeAccountAddresses,
            validatorPublicKeys.length,
            "validatorStakeAccountAddresses",
            true);
    final byte[][] voteAccountHashes =
        normalizeFixed32Array(
            validatorVoteAccountHashes,
            validatorPublicKeys.length,
            "validatorVoteAccountHashes",
            false);
    final byte[][] stakeAccountHashes =
        normalizeFixed32Array(
            validatorStakeAccountHashes,
            validatorPublicKeys.length,
            "validatorStakeAccountHashes",
            false);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, resolvedEpoch);
    write(out, stakeActivationHash);
    writeU32Le(out, validatorPublicKeys.length);
    final Set<String> stakeAccountKeys = new HashSet<>();
    for (final byte[] stakeAccount : stakeAccounts) {
      stakeAccountKeys.add(hexLower(stakeAccount));
    }
    for (int i = 0; i < validatorPublicKeys.length; i++) {
      if (Arrays.equals(voteAccounts[i], stakeAccounts[i])) {
        throw new IllegalArgumentException("validatorStakeAccountAddresses[" + i + "] must differ from vote account");
      }
      if (stakeAccountKeys.contains(hexLower(voteAccounts[i]))) {
        throw new IllegalArgumentException(
            "validatorVoteAccountAddresses[" + i + "] must not overlap stake accounts");
      }
      writeVec(out, validatorPublicKeys[i]);
      writeU64Le(out, normalizeU64(validatorStakes[i], "validatorStakes[" + i + "]"));
      writeU64Le(out, normalizeU64(validatorActivationEpochs[i], "validatorActivationEpochs[" + i + "]"));
      writeU64Le(out, normalizeU64(validatorDeactivationEpochs[i], "validatorDeactivationEpochs[" + i + "]"));
      writeVec(out, voteAccounts[i]);
      writeVec(out, stakeAccounts[i]);
      write(out, voteAccountHashes[i]);
      write(out, stakeAccountHashes[i]);
    }
    return out.toByteArray();
  }

  public static String stakeAccountStateHash(
      final String epoch,
      final byte[][] validatorPublicKeys,
      final String[] validatorStakes,
      final String[] validatorActivationEpochs,
      final String[] validatorDeactivationEpochs,
      final byte[][] validatorVoteAccountAddresses,
      final byte[][] validatorStakeAccountAddresses,
      final byte[][] validatorVoteAccountHashes,
      final byte[][] validatorStakeAccountHashes) {
    return hashHex(
        "sccp:solana:stake-account-state:v1",
        canonicalStakeAccountStateBytes(
            epoch,
            validatorPublicKeys,
            validatorStakes,
            validatorActivationEpochs,
            validatorDeactivationEpochs,
            validatorVoteAccountAddresses,
            validatorStakeAccountAddresses,
            validatorVoteAccountHashes,
            validatorStakeAccountHashes));
  }

  private static final class NormalizedStakeHistoryEntry {
    final BigInteger epoch;
    final BigInteger effective;
    final BigInteger activating;
    final BigInteger deactivating;

    NormalizedStakeHistoryEntry(
        final BigInteger epoch,
        final BigInteger effective,
        final BigInteger activating,
        final BigInteger deactivating) {
      this.epoch = epoch;
      this.effective = effective;
      this.activating = activating;
      this.deactivating = deactivating;
    }
  }

  private static final class StakeActivationStatus {
    final BigInteger effective;
    final BigInteger activating;
    final BigInteger deactivating;

    StakeActivationStatus(
        final BigInteger effective, final BigInteger activating, final BigInteger deactivating) {
      this.effective = effective;
      this.activating = activating;
      this.deactivating = deactivating;
    }
  }

  private static NormalizedStakeHistoryEntry stakeHistoryEntryForEpoch(
      final NormalizedStakeHistoryEntry[] stakeHistoryEntries, final BigInteger epoch) {
    for (final NormalizedStakeHistoryEntry entry : stakeHistoryEntries) {
      if (entry.epoch.equals(epoch)) {
        return entry;
      }
    }
    return null;
  }

  private static BigInteger stakeChangeAllowance(
      final BigInteger accountPortion,
      final BigInteger clusterPortion,
      final BigInteger clusterEffective) {
    if (accountPortion.equals(BigInteger.ZERO)
        || clusterPortion.equals(BigInteger.ZERO)
        || clusterEffective.equals(BigInteger.ZERO)) {
      return BigInteger.ZERO;
    }
    final BigInteger numerator =
        accountPortion.multiply(clusterEffective).multiply(WARMUP_COOLDOWN_RATE_BPS);
    final BigInteger denominator = clusterPortion.multiply(BASIS_POINTS_PER_UNIT);
    final BigInteger delta = numerator.divide(denominator);
    return delta.compareTo(accountPortion) < 0 ? delta : accountPortion;
  }

  private static BigInteger[] stakeAndActivatingV2(
      final BigInteger targetEpoch,
      final BigInteger delegatedStake,
      final BigInteger activationEpoch,
      final BigInteger deactivationEpoch,
      final NormalizedStakeHistoryEntry[] stakeHistoryEntries) {
    if (activationEpoch.equals(MAX_U64)) {
      return new BigInteger[] {delegatedStake, BigInteger.ZERO};
    }
    if (activationEpoch.equals(deactivationEpoch)) {
      return new BigInteger[] {BigInteger.ZERO, BigInteger.ZERO};
    }
    if (targetEpoch.equals(activationEpoch)) {
      return new BigInteger[] {BigInteger.ZERO, delegatedStake};
    }
    if (targetEpoch.compareTo(activationEpoch) < 0) {
      return new BigInteger[] {BigInteger.ZERO, BigInteger.ZERO};
    }
    NormalizedStakeHistoryEntry previousClusterStake =
        stakeHistoryEntryForEpoch(stakeHistoryEntries, activationEpoch);
    if (previousClusterStake == null) {
      return new BigInteger[] {delegatedStake, BigInteger.ZERO};
    }

    BigInteger previousEpoch = activationEpoch;
    BigInteger activatedStakeAmount = BigInteger.ZERO;
    while (true) {
      final BigInteger currentEpoch = previousEpoch.add(BigInteger.ONE);
      if (previousClusterStake.activating.equals(BigInteger.ZERO)) {
        break;
      }
      final BigInteger remainingActivatingStake = delegatedStake.subtract(activatedStakeAmount);
      final BigInteger newlyEffectiveStake =
          stakeChangeAllowance(
                  remainingActivatingStake,
                  previousClusterStake.activating,
                  previousClusterStake.effective)
              .max(BigInteger.ONE);
      activatedStakeAmount = activatedStakeAmount.add(newlyEffectiveStake).min(delegatedStake);
      if (activatedStakeAmount.compareTo(delegatedStake) >= 0) {
        activatedStakeAmount = delegatedStake;
        break;
      }
      if (currentEpoch.compareTo(targetEpoch) >= 0
          || currentEpoch.compareTo(deactivationEpoch) >= 0) {
        break;
      }
      final NormalizedStakeHistoryEntry currentClusterStake =
          stakeHistoryEntryForEpoch(stakeHistoryEntries, currentEpoch);
      if (currentClusterStake == null) {
        break;
      }
      previousEpoch = currentEpoch;
      previousClusterStake = currentClusterStake;
    }

    return new BigInteger[] {activatedStakeAmount, delegatedStake.subtract(activatedStakeAmount)};
  }

  private static StakeActivationStatus delegationStakeStatusV2(
      final BigInteger targetEpoch,
      final BigInteger delegatedStake,
      final BigInteger activationEpoch,
      final BigInteger deactivationEpoch,
      final NormalizedStakeHistoryEntry[] stakeHistoryEntries) {
    final BigInteger[] stakeAndActivating =
        stakeAndActivatingV2(
            targetEpoch, delegatedStake, activationEpoch, deactivationEpoch, stakeHistoryEntries);
    final BigInteger effectiveStake = stakeAndActivating[0];
    final BigInteger activatingStake = stakeAndActivating[1];
    if (targetEpoch.compareTo(deactivationEpoch) < 0) {
      return new StakeActivationStatus(effectiveStake, activatingStake, BigInteger.ZERO);
    }
    if (targetEpoch.equals(deactivationEpoch)) {
      return new StakeActivationStatus(effectiveStake, BigInteger.ZERO, effectiveStake);
    }
    NormalizedStakeHistoryEntry previousClusterStake =
        stakeHistoryEntryForEpoch(stakeHistoryEntries, deactivationEpoch);
    if (previousClusterStake == null) {
      return new StakeActivationStatus(BigInteger.ZERO, BigInteger.ZERO, BigInteger.ZERO);
    }

    BigInteger previousEpoch = deactivationEpoch;
    BigInteger remainingDeactivatingStake = effectiveStake;
    while (true) {
      final BigInteger currentEpoch = previousEpoch.add(BigInteger.ONE);
      if (previousClusterStake.deactivating.equals(BigInteger.ZERO)) {
        break;
      }
      final BigInteger newlyDeactivatedStake =
          stakeChangeAllowance(
                  remainingDeactivatingStake,
                  previousClusterStake.deactivating,
                  previousClusterStake.effective)
              .max(BigInteger.ONE);
      remainingDeactivatingStake =
          remainingDeactivatingStake.subtract(newlyDeactivatedStake).max(BigInteger.ZERO);
      if (remainingDeactivatingStake.equals(BigInteger.ZERO)) {
        break;
      }
      if (currentEpoch.compareTo(targetEpoch) >= 0) {
        break;
      }
      final NormalizedStakeHistoryEntry currentClusterStake =
          stakeHistoryEntryForEpoch(stakeHistoryEntries, currentEpoch);
      if (currentClusterStake == null) {
        break;
      }
      previousEpoch = currentEpoch;
      previousClusterStake = currentClusterStake;
    }

    return new StakeActivationStatus(
        remainingDeactivatingStake, BigInteger.ZERO, remainingDeactivatingStake);
  }

  public static byte[] canonicalStakeHistorySysvarDataBytes(
      final StakeHistoryEntry[] stakeHistoryEntries) {
    Objects.requireNonNull(stakeHistoryEntries, "stakeHistoryEntries");
    if (stakeHistoryEntries.length == 0 || stakeHistoryEntries.length > 512) {
      throw new IllegalArgumentException("stakeHistoryEntries must be non-empty and at most 512 entries");
    }
    BigInteger previousEpoch = null;
    final NormalizedStakeHistoryEntry[] normalizedEntries =
        new NormalizedStakeHistoryEntry[stakeHistoryEntries.length];
    for (int i = 0; i < stakeHistoryEntries.length; i++) {
      final StakeHistoryEntry entry = Objects.requireNonNull(stakeHistoryEntries[i], "stakeHistoryEntries[" + i + "]");
      final NormalizedStakeHistoryEntry normalized =
          new NormalizedStakeHistoryEntry(
              normalizeU64(entry.epoch, "stakeHistoryEntries[" + i + "].epoch"),
              normalizeU64(entry.effective, "stakeHistoryEntries[" + i + "].effective"),
              normalizeU64(entry.activating, "stakeHistoryEntries[" + i + "].activating"),
              normalizeU64(entry.deactivating, "stakeHistoryEntries[" + i + "].deactivating"));
      if (previousEpoch != null && previousEpoch.compareTo(normalized.epoch) >= 0) {
        throw new IllegalArgumentException("stakeHistoryEntries must be sorted by strictly increasing epoch");
      }
      previousEpoch = normalized.epoch;
      normalizedEntries[i] = normalized;
    }

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU64Le(out, BigInteger.valueOf(normalizedEntries.length));
    for (int i = normalizedEntries.length - 1; i >= 0; i--) {
      final NormalizedStakeHistoryEntry entry = normalizedEntries[i];
      writeU64Le(out, entry.epoch);
      writeU64Le(out, entry.effective);
      writeU64Le(out, entry.activating);
      writeU64Le(out, entry.deactivating);
    }
    return out.toByteArray();
  }

  public static String stakeHistorySysvarDataHash(
      final StakeHistoryEntry[] stakeHistoryEntries) {
    return hashHex(
        "sccp:solana:stake-history-sysvar-data:v1",
        canonicalStakeHistorySysvarDataBytes(stakeHistoryEntries));
  }

  public static String stakeHistorySysvarDataHashFromRawData(final byte[] rawData) {
    Objects.requireNonNull(rawData, "rawData");
    if (rawData.length < 8 || (rawData.length - 8) % 32 != 0) {
      throw new IllegalArgumentException("rawData must be Solana StakeHistory sysvar bincode Vec bytes");
    }
    final BigInteger entryCount = readU64Le(rawData, 0);
    if (entryCount.compareTo(BigInteger.ZERO) <= 0
        || entryCount.compareTo(BigInteger.valueOf(512)) > 0
        || rawData.length != 8 + entryCount.intValue() * 32) {
      throw new IllegalArgumentException("rawData must contain 1..512 StakeHistory sysvar entries");
    }
    int offset = 8;
    BigInteger previousEpoch = null;
    for (int i = 0; i < entryCount.intValue(); i++) {
      final BigInteger epoch = readU64Le(rawData, offset);
      offset += 32;
      if (previousEpoch != null && previousEpoch.compareTo(epoch) <= 0) {
        throw new IllegalArgumentException("rawData StakeHistory entries must be newest-first");
      }
      previousEpoch = epoch;
    }
    return hashHex("sccp:solana:stake-history-sysvar-data:v1", rawData);
  }

  public static byte[] canonicalStakeHistoryBytes(
      final String epoch,
      final byte[][] validatorPublicKeys,
      final String[] validatorEffectiveStakes,
      final String[] validatorDelegatedStakes,
      final String[] validatorActivationEpochs,
      final String[] validatorDeactivationEpochs,
      final byte[][] validatorVoteAccountAddresses,
      final byte[][] validatorStakeAccountAddresses,
      final byte[][] validatorVoteAccountHashes,
      final byte[][] validatorStakeAccountHashes,
      final StakeHistoryEntry[] stakeHistoryEntries) {
    Objects.requireNonNull(validatorEffectiveStakes, "validatorEffectiveStakes");
    Objects.requireNonNull(validatorDelegatedStakes, "validatorDelegatedStakes");
    Objects.requireNonNull(stakeHistoryEntries, "stakeHistoryEntries");
    if (validatorEffectiveStakes.length != validatorPublicKeys.length) {
      throw new IllegalArgumentException("validatorEffectiveStakes must match validatorPublicKeys");
    }
    if (validatorDelegatedStakes.length != validatorPublicKeys.length) {
      throw new IllegalArgumentException("validatorDelegatedStakes must match validatorPublicKeys");
    }
    if (validatorActivationEpochs.length != validatorPublicKeys.length) {
      throw new IllegalArgumentException("validatorActivationEpochs must match validatorPublicKeys");
    }
    if (validatorDeactivationEpochs.length != validatorPublicKeys.length) {
      throw new IllegalArgumentException("validatorDeactivationEpochs must match validatorPublicKeys");
    }
    if (stakeHistoryEntries.length == 0 || stakeHistoryEntries.length > 512) {
      throw new IllegalArgumentException("stakeHistoryEntries must be non-empty and at most 512 entries");
    }
    final BigInteger resolvedEpoch = normalizeU64(epoch, "epoch");
    final BigInteger[] effectiveStakes = new BigInteger[validatorPublicKeys.length];
    final BigInteger[] delegatedStakes = new BigInteger[validatorPublicKeys.length];
    final BigInteger[] activationEpochs = new BigInteger[validatorPublicKeys.length];
    final BigInteger[] deactivationEpochs = new BigInteger[validatorPublicKeys.length];
    for (int i = 0; i < validatorPublicKeys.length; i++) {
      effectiveStakes[i] = normalizeU64(validatorEffectiveStakes[i], "validatorEffectiveStakes[" + i + "]");
      delegatedStakes[i] = normalizeU64(validatorDelegatedStakes[i], "validatorDelegatedStakes[" + i + "]");
      activationEpochs[i] = normalizeU64(validatorActivationEpochs[i], "validatorActivationEpochs[" + i + "]");
      deactivationEpochs[i] =
          normalizeU64(validatorDeactivationEpochs[i], "validatorDeactivationEpochs[" + i + "]");
    }
    BigInteger previousEpoch = null;
    NormalizedStakeHistoryEntry signedEpochEntry = null;
    final NormalizedStakeHistoryEntry[] normalizedEntries =
        new NormalizedStakeHistoryEntry[stakeHistoryEntries.length];
    for (int i = 0; i < stakeHistoryEntries.length; i++) {
      final StakeHistoryEntry entry = Objects.requireNonNull(stakeHistoryEntries[i], "stakeHistoryEntries[" + i + "]");
      final NormalizedStakeHistoryEntry normalized =
          new NormalizedStakeHistoryEntry(
              normalizeU64(entry.epoch, "stakeHistoryEntries[" + i + "].epoch"),
              normalizeU64(entry.effective, "stakeHistoryEntries[" + i + "].effective"),
              normalizeU64(entry.activating, "stakeHistoryEntries[" + i + "].activating"),
              normalizeU64(entry.deactivating, "stakeHistoryEntries[" + i + "].deactivating"));
      normalizedEntries[i] = normalized;
      if (normalized.epoch.compareTo(resolvedEpoch) > 0) {
        throw new IllegalArgumentException("stakeHistoryEntries[" + i + "].epoch must not exceed epoch");
      }
      if (previousEpoch != null && previousEpoch.compareTo(normalized.epoch) >= 0) {
        throw new IllegalArgumentException("stakeHistoryEntries must be sorted by strictly increasing epoch");
      }
      previousEpoch = normalized.epoch;
      if (normalized.epoch.equals(resolvedEpoch)) {
        signedEpochEntry = normalized;
      }
    }
    if (signedEpochEntry == null) {
      throw new IllegalArgumentException("stakeHistoryEntries must include epoch");
    }
    BigInteger totalEffectiveStake = BigInteger.ZERO;
    BigInteger totalDelegatedStake = BigInteger.ZERO;
    BigInteger totalActivatingStake = BigInteger.ZERO;
    BigInteger totalDeactivatingStake = BigInteger.ZERO;
    for (int i = 0; i < validatorPublicKeys.length; i++) {
      if (delegatedStakes[i].compareTo(BigInteger.ZERO) <= 0) {
        throw new IllegalArgumentException("validatorDelegatedStakes[" + i + "] must be greater than zero");
      }
      if (deactivationEpochs[i].compareTo(activationEpochs[i]) <= 0) {
        throw new IllegalArgumentException(
            "validatorDeactivationEpochs[" + i + "] must be greater than activation epoch");
      }
      final StakeActivationStatus status =
          delegationStakeStatusV2(
              resolvedEpoch,
              delegatedStakes[i],
              activationEpochs[i],
              deactivationEpochs[i],
              normalizedEntries);
      if (status.effective.compareTo(BigInteger.ZERO) <= 0
          || !status.effective.equals(effectiveStakes[i])) {
        throw new IllegalArgumentException(
            "validatorEffectiveStakes[" + i + "] must equal replayed StakeHistory effective stake");
      }
      totalEffectiveStake = totalEffectiveStake.add(status.effective);
      totalDelegatedStake = totalDelegatedStake.add(delegatedStakes[i]);
      totalActivatingStake = totalActivatingStake.add(status.activating);
      totalDeactivatingStake = totalDeactivatingStake.add(status.deactivating);
    }
    if (totalEffectiveStake.equals(BigInteger.ZERO)
        || totalDelegatedStake.compareTo(totalEffectiveStake) < 0) {
      throw new IllegalArgumentException(
          "replayed StakeHistory effective stake must be non-zero and not exceed delegated stake");
    }
    if (!signedEpochEntry.effective.equals(totalEffectiveStake)) {
      throw new IllegalArgumentException(
          "signed epoch StakeHistory effective stake must equal replayed validator effective stake");
    }
    if (signedEpochEntry.activating.compareTo(totalActivatingStake) < 0) {
      throw new IllegalArgumentException(
          "signed epoch StakeHistory activating stake must cover replayed validators");
    }
    if (signedEpochEntry.deactivating.compareTo(totalDeactivatingStake) < 0) {
      throw new IllegalArgumentException(
          "signed epoch StakeHistory deactivating stake must cover replayed validators");
    }
    final byte[] stakeAccountStateHash =
        hashBytes(
            "sccp:solana:stake-account-state:v1",
            canonicalStakeAccountStateBytes(
                epoch,
                validatorPublicKeys,
                validatorDelegatedStakes,
                validatorActivationEpochs,
                validatorDeactivationEpochs,
                validatorVoteAccountAddresses,
                validatorStakeAccountAddresses,
                validatorVoteAccountHashes,
                validatorStakeAccountHashes));

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, resolvedEpoch);
    write(out, stakeAccountStateHash);
    writeU32Le(out, validatorPublicKeys.length);
    for (int i = 0; i < validatorPublicKeys.length; i++) {
      writeVec(out, validatorPublicKeys[i]);
      writeU64Le(out, effectiveStakes[i]);
      writeU64Le(out, delegatedStakes[i]);
      writeU64Le(out, activationEpochs[i]);
      writeU64Le(out, deactivationEpochs[i]);
    }
    writeU32Le(out, normalizedEntries.length);
    for (final NormalizedStakeHistoryEntry entry : normalizedEntries) {
      writeU64Le(out, entry.epoch);
      writeU64Le(out, entry.effective);
      writeU64Le(out, entry.activating);
      writeU64Le(out, entry.deactivating);
    }
    return out.toByteArray();
  }

  public static String stakeHistoryHash(
      final String epoch,
      final byte[][] validatorPublicKeys,
      final String[] validatorEffectiveStakes,
      final String[] validatorDelegatedStakes,
      final String[] validatorActivationEpochs,
      final String[] validatorDeactivationEpochs,
      final byte[][] validatorVoteAccountAddresses,
      final byte[][] validatorStakeAccountAddresses,
      final byte[][] validatorVoteAccountHashes,
      final byte[][] validatorStakeAccountHashes,
      final StakeHistoryEntry[] stakeHistoryEntries) {
    return hashHex(
        "sccp:solana:stake-history:v1",
        canonicalStakeHistoryBytes(
            epoch,
            validatorPublicKeys,
            validatorEffectiveStakes,
            validatorDelegatedStakes,
            validatorActivationEpochs,
            validatorDeactivationEpochs,
            validatorVoteAccountAddresses,
            validatorStakeAccountAddresses,
            validatorVoteAccountHashes,
            validatorStakeAccountHashes,
            stakeHistoryEntries));
  }

  public static byte[] canonicalTowerLockoutBytes(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String parentBankHash) {
    return canonicalTowerLockoutBytes(finalizedSlot, rootedSlot, parentSlot, parentBankHash, null);
  }

  public static byte[] canonicalTowerLockoutBytes(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String epoch) {
    final BigInteger finalized = normalizeU64(finalizedSlot, "finalizedSlot");
    final BigInteger expectedEpoch = normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch");
    final BigInteger resolvedEpoch =
        epoch == null ? expectedEpoch : normalizeU64(epoch, "epoch");
    if (!resolvedEpoch.equals(expectedEpoch)) {
      throw new IllegalArgumentException("epoch must match Solana mainnet finalizedSlot");
    }
    final BigInteger rooted = normalizeU64(rootedSlot, "rootedSlot");
    final BigInteger parent = normalizeU64(parentSlot, "parentSlot");
    if (rooted.compareTo(parent) > 0) {
      throw new IllegalArgumentException("rootedSlot must be less than or equal to parentSlot");
    }
    if (!parent.add(BigInteger.ONE).equals(finalized)) {
      throw new IllegalArgumentException("parentSlot must be the direct parent of finalizedSlot");
    }
    if (finalized.subtract(rooted).compareTo(BigInteger.valueOf(TOWER_VOTE_STACK_DEPTH)) < 0) {
      throw new IllegalArgumentException("rootedSlot must satisfy the Solana Tower lockout depth");
    }
    final byte[] parentBankHashBytes = hex32Bytes(parentBankHash, "parentBankHash");
    if (!anyNonZero(parentBankHashBytes)) {
      throw new IllegalArgumentException("parentBankHash must not be zero");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, resolvedEpoch);
    writeU64Le(out, BigInteger.valueOf(TOWER_LOCKOUT_CONFIRMATION_DEPTH));
    writeU64Le(out, finalized);
    writeU64Le(out, rooted);
    writeU64Le(out, parent);
    write(out, parentBankHashBytes);
    return out.toByteArray();
  }

  public static String towerLockoutHash(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String parentBankHash) {
    return towerLockoutHash(finalizedSlot, rootedSlot, parentSlot, parentBankHash, null);
  }

  public static String towerLockoutHash(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String epoch) {
    return hashHex(
        "sccp:solana:tower-lockout:v1",
        canonicalTowerLockoutBytes(finalizedSlot, rootedSlot, parentSlot, parentBankHash, epoch));
  }

  public static byte[] canonicalTowerReplayBytes(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String[] towerVoteSlots) {
    return canonicalTowerReplayBytes(
        finalizedSlot, rootedSlot, parentSlot, ZERO_HASH_V1, towerVoteSlots, null);
  }

  public static byte[] canonicalTowerReplayBytes(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String bankForkHash,
      final String[] towerVoteSlots) {
    return canonicalTowerReplayBytes(
        finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots, null);
  }

  public static byte[] canonicalTowerReplayBytes(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String bankForkHash,
      final String[] towerVoteSlots,
      final String epoch) {
    final BigInteger finalized = normalizeU64(finalizedSlot, "finalizedSlot");
    final BigInteger expectedEpoch = normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch");
    final BigInteger resolvedEpoch =
        epoch == null ? expectedEpoch : normalizeU64(epoch, "epoch");
    if (!resolvedEpoch.equals(expectedEpoch)) {
      throw new IllegalArgumentException("epoch must match Solana mainnet finalizedSlot");
    }
    final BigInteger rooted = normalizeU64(rootedSlot, "rootedSlot");
    final BigInteger parent = normalizeU64(parentSlot, "parentSlot");
    if (!parent.add(BigInteger.ONE).equals(finalized)) {
      throw new IllegalArgumentException("parentSlot must be the direct parent of finalizedSlot");
    }
    if (rooted.compareTo(finalized) >= 0) {
      throw new IllegalArgumentException("rootedSlot must be less than finalizedSlot");
    }
    if (finalized.subtract(rooted).compareTo(BigInteger.valueOf(TOWER_VOTE_STACK_DEPTH)) < 0) {
      throw new IllegalArgumentException("rootedSlot must satisfy the Solana Tower lockout depth");
    }
    final byte[] bankForkHashBytes = hex32Bytes(bankForkHash, "bankForkHash");
    if (!anyNonZero(bankForkHashBytes)) {
      throw new IllegalArgumentException("bankForkHash must not be zero");
    }

    final int depth = (int) TOWER_VOTE_STACK_DEPTH;
    Objects.requireNonNull(towerVoteSlots, "towerVoteSlots");
    if (towerVoteSlots.length != depth) {
      throw new IllegalArgumentException("towerVoteSlots must contain 31 active post-root slots");
    }
    final BigInteger[] votes = new BigInteger[towerVoteSlots.length];
    for (int i = 0; i < towerVoteSlots.length; i++) {
      votes[i] = normalizeU64(towerVoteSlots[i], "towerVoteSlots[" + i + "]");
    }
    if (votes[0].compareTo(rooted) <= 0) {
      throw new IllegalArgumentException("towerVoteSlots[0] must be greater than rootedSlot");
    }
    if (!votes[depth - 1].equals(finalized)) {
      throw new IllegalArgumentException("last towerVoteSlots entry must equal finalizedSlot");
    }
    if (!votes[depth - 2].equals(parent)) {
      throw new IllegalArgumentException("penultimate towerVoteSlots entry must equal parentSlot");
    }
    for (int i = 1; i < votes.length; i++) {
      if (votes[i - 1].compareTo(votes[i]) >= 0) {
        throw new IllegalArgumentException("towerVoteSlots must be strictly increasing");
      }
    }
    for (int i = 0; i < votes.length; i++) {
      if (votes[i].compareTo(finalized) > 0) {
        throw new IllegalArgumentException("towerVoteSlots[" + i + "] must not exceed finalizedSlot");
      }
      final int confirmationCount = depth - i;
      final BigInteger lockout = BigInteger.ONE.shiftLeft(confirmationCount);
      if (votes[i].add(lockout).compareTo(finalized) <= 0) {
        throw new IllegalArgumentException(
            "towerVoteSlots[" + i + "] does not satisfy its Tower lockout");
      }
    }

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, resolvedEpoch);
    writeU64Le(out, BigInteger.valueOf(TOWER_LOCKOUT_CONFIRMATION_DEPTH));
    writeU64Le(out, finalized);
    writeU64Le(out, rooted);
    writeU64Le(out, parent);
    write(out, bankForkHashBytes);
    writeU32Le(out, votes.length);
    for (int i = 0; i < votes.length; i++) {
      writeU64Le(out, votes[i]);
      writeU64Le(out, BigInteger.valueOf(depth - i));
    }
    return out.toByteArray();
  }

  public static String towerReplayHash(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String[] towerVoteSlots) {
    return towerReplayHash(finalizedSlot, rootedSlot, parentSlot, ZERO_HASH_V1, towerVoteSlots, null);
  }

  public static String towerReplayHash(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String bankForkHash,
      final String[] towerVoteSlots) {
    return towerReplayHash(finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots, null);
  }

  public static String towerReplayHash(
      final String finalizedSlot,
      final String rootedSlot,
      final String parentSlot,
      final String bankForkHash,
      final String[] towerVoteSlots,
      final String epoch) {
    return hashHex(
        "sccp:solana:tower-replay:v1",
        canonicalTowerReplayBytes(
            finalizedSlot, rootedSlot, parentSlot, bankForkHash, towerVoteSlots, epoch));
  }

  public static byte[] canonicalBankForkBytes(
      final String finalizedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot) {
    throw new IllegalArgumentException("accountInclusionRoot is required");
  }

  public static byte[] canonicalBankForkBytes(
      final String finalizedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot) {
    throw new IllegalArgumentException("accountsLtHashChecksum is required");
  }

  public static byte[] canonicalBankForkBytes(
      final String finalizedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum) {
    throw new IllegalArgumentException("bankSignatureCount is required");
  }

  public static byte[] canonicalBankForkBytes(
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum) {
    return canonicalBankForkBytes(
        finalizedSlot,
        parentSlot,
        bankSignatureCount,
        parentBankHash,
        bankHash,
        blockhash,
        null,
        new byte[0],
        transactionStatusRoot,
        accountInclusionRoot,
        accountsLtHashChecksum,
        null);
  }

  public static byte[] canonicalBankForkBytes(
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum,
      final String epoch) {
    return canonicalBankForkBytes(
        finalizedSlot,
        parentSlot,
        bankSignatureCount,
        parentBankHash,
        bankHash,
        blockhash,
        null,
        new byte[0],
        transactionStatusRoot,
        accountInclusionRoot,
        accountsLtHashChecksum,
        epoch);
  }

  public static byte[] canonicalBankForkBytes(
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final byte[] accountsLtHash,
      final byte[] bankHashHardForkData,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum,
      final String epoch) {
    final BigInteger finalized = normalizeU64(finalizedSlot, "finalizedSlot");
    final BigInteger expectedEpoch = normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch");
    final BigInteger resolvedEpoch =
        epoch == null ? expectedEpoch : normalizeU64(epoch, "epoch");
    if (!resolvedEpoch.equals(expectedEpoch)) {
      throw new IllegalArgumentException("epoch must match Solana mainnet finalizedSlot");
    }
    final BigInteger parent = normalizeU64(parentSlot, "parentSlot");
    if (!parent.add(BigInteger.ONE).equals(finalized)) {
      throw new IllegalArgumentException("parentSlot must be the direct parent of finalizedSlot");
    }
    final BigInteger signatureCount = normalizeU64(bankSignatureCount, "bankSignatureCount");
    if (BigInteger.ZERO.equals(signatureCount)) {
      throw new IllegalArgumentException("bankSignatureCount must be nonzero");
    }
    final byte[] parentBankHashBytes = hex32Bytes(parentBankHash, "parentBankHash");
    if (!anyNonZero(parentBankHashBytes)) {
      throw new IllegalArgumentException("parentBankHash must not be zero");
    }
    final byte[] bankHashBytes = hex32Bytes(bankHash, "bankHash");
    if (!anyNonZero(bankHashBytes)) {
      throw new IllegalArgumentException("bankHash must not be zero");
    }
    if (Arrays.equals(parentBankHashBytes, bankHashBytes)) {
      throw new IllegalArgumentException("parentBankHash must differ from bankHash");
    }
    final byte[] blockhashBytes = solanaHash32Bytes(blockhash, "blockhash");
    if (!anyNonZero(blockhashBytes)) {
      throw new IllegalArgumentException("blockhash must not be zero");
    }
    final byte[] transactionStatusRootBytes =
        hex32Bytes(transactionStatusRoot, "transactionStatusRoot");
    if (!anyNonZero(transactionStatusRootBytes)) {
      throw new IllegalArgumentException("transactionStatusRoot must not be zero");
    }
    final byte[] accountInclusionRootBytes = hex32Bytes(accountInclusionRoot, "accountInclusionRoot");
    if (!anyNonZero(accountInclusionRootBytes)) {
      throw new IllegalArgumentException("accountInclusionRoot must not be zero");
    }
    final byte[] accountsLtHashChecksumBytes =
        hex32Bytes(accountsLtHashChecksum, "accountsLtHashChecksum");
    if (!anyNonZero(accountsLtHashChecksumBytes)) {
      throw new IllegalArgumentException("accountsLtHashChecksum must not be zero");
    }
    final byte[] hardForkData = bankHashHardForkData == null ? new byte[0] : bankHashHardForkData;
    if (hardForkData.length > MAX_BANK_HARD_FORK_HASH_DATA_BYTES) {
      throw new IllegalArgumentException("bankHashHardForkData is too large");
    }
    if (accountsLtHash != null) {
      final byte[] expectedBankHash =
          agaveBankHashBytes(
              parentBankHashBytes, signatureCount, blockhashBytes, accountsLtHash, hardForkData);
      if (!Arrays.equals(bankHashBytes, expectedBankHash)) {
        throw new IllegalArgumentException("bankHash must match Agave bank hash inputs");
      }
      if (!Arrays.equals(Blake3.hash(accountsLtHash), accountsLtHashChecksumBytes)) {
        throw new IllegalArgumentException("accountsLtHashChecksum must match accountsLtHash");
      }
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU64Le(out, resolvedEpoch);
    writeU64Le(out, finalized);
    writeU64Le(out, parent);
    writeU64Le(out, signatureCount);
    write(out, parentBankHashBytes);
    write(out, bankHashBytes);
    write(out, blockhashBytes);
    write(out, transactionStatusRootBytes);
    write(out, accountInclusionRootBytes);
    write(out, accountsLtHashChecksumBytes);
    writeVec(out, hardForkData);
    return out.toByteArray();
  }

  public static String bankForkHash(
      final String finalizedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot) {
    throw new IllegalArgumentException("accountInclusionRoot is required");
  }

  public static String bankForkHash(
      final String finalizedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot) {
    throw new IllegalArgumentException("accountsLtHashChecksum is required");
  }

  public static String bankForkHash(
      final String finalizedSlot,
      final String parentSlot,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum) {
    throw new IllegalArgumentException("bankSignatureCount is required");
  }

  public static String bankForkHash(
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum) {
    return bankForkHash(
        finalizedSlot,
        parentSlot,
        bankSignatureCount,
        parentBankHash,
        bankHash,
        blockhash,
        transactionStatusRoot,
        accountInclusionRoot,
        accountsLtHashChecksum,
        null);
  }

  public static String bankForkHash(
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum,
      final String epoch) {
    return hashHex(
        "sccp:solana:bank-fork:v1",
        canonicalBankForkBytes(
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            bankHash,
            blockhash,
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum,
            epoch));
  }

  public static String bankForkHash(
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final byte[] accountsLtHash,
      final byte[] bankHashHardForkData,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum,
      final String epoch) {
    return hashHex(
        "sccp:solana:bank-fork:v1",
        canonicalBankForkBytes(
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            bankHash,
            blockhash,
            accountsLtHash,
            bankHashHardForkData,
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum,
            epoch));
  }

  public static byte[] canonicalAccountsLtHashProofPublicInputsBytes(
      final int sourceDomain,
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final byte[] bankHashHardForkData,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum) {
    return canonicalAccountsLtHashProofPublicInputsBytes(
        sourceDomain,
        finalizedSlot,
        parentSlot,
        bankSignatureCount,
        parentBankHash,
        bankHash,
        blockhash,
        bankHashHardForkData,
        transactionStatusRoot,
        accountInclusionRoot,
        accountsLtHashChecksum,
        null);
  }

  public static byte[] canonicalAccountsLtHashProofPublicInputsBytes(
      final int sourceDomain,
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final byte[] bankHashHardForkData,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum,
      final byte[] accountsLtHash) {
    if (sourceDomain != DOMAIN_SOLANA) {
      throw new IllegalArgumentException("sourceDomain must be Solana");
    }
    final BigInteger finalized = normalizeU64(finalizedSlot, "finalizedSlot");
    final BigInteger epoch = normalizeU64(mainnetEpochForSlot(finalized.toString()), "epoch");
    final BigInteger parent = normalizeU64(parentSlot, "parentSlot");
    final BigInteger signatureCount = normalizeU64(bankSignatureCount, "bankSignatureCount");
    final byte[] blockhashBytes = solanaHash32Bytes(blockhash, "blockhash");
    final byte[] hardForkData = bankHashHardForkData == null ? new byte[0] : bankHashHardForkData;
    final byte[] checkedAccountsLtHash =
        accountsLtHash == null ? null : Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    final byte[] bankForkHashBytes =
        hex32Bytes(
            bankForkHash(
                finalized.toString(),
                parent.toString(),
                signatureCount.toString(),
                parentBankHash,
                bankHash,
                "0x" + hexLower(blockhashBytes),
                checkedAccountsLtHash,
                hardForkData,
                transactionStatusRoot,
                accountInclusionRoot,
                accountsLtHashChecksum,
                null),
            "bankForkHash");

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, sourceDomain);
    writeString(out, RECURSIVE_PROOF_BACKEND_V1, "backend");
    writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash");
    writeU64Le(out, epoch);
    writeU64Le(out, finalized);
    writeU64Le(out, parent);
    writeU64Le(out, signatureCount);
    write(out, hex32Bytes(parentBankHash, "parentBankHash"));
    write(out, hex32Bytes(bankHash, "bankHash"));
    write(out, blockhashBytes);
    write(out, hex32Bytes(transactionStatusRoot, "transactionStatusRoot"));
    write(out, hex32Bytes(accountInclusionRoot, "accountInclusionRoot"));
    write(out, hex32Bytes(accountsLtHashChecksum, "accountsLtHashChecksum"));
    writeVec(out, hardForkData);
    write(out, bankForkHashBytes);
    return out.toByteArray();
  }

  public static String accountsLtHashProofPublicInputsHash(
      final int sourceDomain,
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final byte[] bankHashHardForkData,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum) {
    return accountsLtHashProofPublicInputsHash(
        sourceDomain,
        finalizedSlot,
        parentSlot,
        bankSignatureCount,
        parentBankHash,
        bankHash,
        blockhash,
        bankHashHardForkData,
        transactionStatusRoot,
        accountInclusionRoot,
        accountsLtHashChecksum,
        null);
  }

  public static String accountsLtHashProofPublicInputsHash(
      final int sourceDomain,
      final String finalizedSlot,
      final String parentSlot,
      final String bankSignatureCount,
      final String parentBankHash,
      final String bankHash,
      final String blockhash,
      final byte[] bankHashHardForkData,
      final String transactionStatusRoot,
      final String accountInclusionRoot,
      final String accountsLtHashChecksum,
      final byte[] accountsLtHash) {
    return hashHex(
        ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX_V1,
        canonicalAccountsLtHashProofPublicInputsBytes(
            sourceDomain,
            finalizedSlot,
            parentSlot,
            bankSignatureCount,
            parentBankHash,
            bankHash,
            blockhash,
            bankHashHardForkData,
            transactionStatusRoot,
            accountInclusionRoot,
            accountsLtHashChecksum,
            accountsLtHash));
  }

  public static byte[] canonicalSourceStateVerificationProofBytes(
      final SourceStateVerificationProof proof) {
    Objects.requireNonNull(proof, "proof");
    requireSourceStateProofLabel(proof.proofFamily(), "proofFamily");
    requireSourceStateProofLabel(proof.circuitId(), "circuitId");
    if (proof.version() != 1) {
      throw new IllegalArgumentException("sourceStateProof.version must be 1");
    }
    if (!"stark-fri-v1".equals(proof.proofFamily())) {
      throw new IllegalArgumentException("sourceStateProof.proofFamily must be stark-fri-v1");
    }
    if (!ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1.equals(proof.circuitId())
        && !TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1.equals(proof.circuitId())
        && !FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(proof.circuitId())
        && !BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(proof.circuitId())) {
      throw new IllegalArgumentException(
          "sourceStateProof.circuitId must be a Solana source-state verification circuit");
    }
    final byte[] proofBytes = proof.proofBytes();
    if (proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    if (proofBytes.length > SOURCE_STATE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must be at most " + SOURCE_STATE_MAX_PROOF_BYTES + " bytes");
    }
    if (!anyNonZero(proofBytes)) {
      throw new IllegalArgumentException("proofBytes must not be all zero");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(proof.version());
    writeString(out, proof.proofFamily(), "proofFamily");
    writeString(out, proof.circuitId(), "circuitId");
    writeVec(out, proofBytes);
    return out.toByteArray();
  }

  public static String accountsLtHashProofHash(final SourceStateVerificationProof proof) {
    Objects.requireNonNull(proof, "proof");
    if (!ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1.equals(proof.circuitId())) {
      throw new IllegalArgumentException(
          "accountsLtHashProof.circuitId must be the Solana AccountsLtHash circuit");
    }
    return hashHex(
        "sccp:solana:accounts-lt-proof:v1",
        canonicalSourceStateVerificationProofBytes(proof));
  }

  public static SourceStateVerificationProof wrapSourceStateVerificationProof(
      final byte[] proofBytes, final AccountsLtHashProofRequest request) {
    Objects.requireNonNull(request, "request");
    requireSourceStateProofRequestForWrapping(request);
    return wrapSourceStateVerificationProof(
        proofBytes,
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.sourceDomain());
  }

  public static SourceStateVerificationProof wrapSourceStateVerificationProof(
      final byte[] proofBytes, final FullLightClientAuditProofRequest request) {
    Objects.requireNonNull(request, "request");
    requireSourceStateProofRequestForWrapping(request);
    return wrapSourceStateVerificationProof(
        proofBytes,
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.sourceDomain());
  }

  private static void requireSourceStateProofRequestForWrapping(
      final AccountsLtHashProofRequest request) {
    if (request.version() != 1) {
      throw new IllegalArgumentException("Solana source-state proof request.version must be 1");
    }
    if (!"stark-fri-v1".equals(request.proofFamily())) {
      throw new IllegalArgumentException(
          "Solana source-state proof request.proofFamily must be stark-fri-v1");
    }
    if (!ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1.equals(request.circuitId())) {
      throw new IllegalArgumentException(
          "request.circuitId must be the Solana AccountsLtHash OpenVerify circuit");
    }
    if (!ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET_V1.equals(request.parameterSet())) {
      throw new IllegalArgumentException("request.parameterSet must be fastpq-lane-balanced");
    }
    if (request.sourceDomain() != DOMAIN_SOLANA) {
      throw new IllegalArgumentException(
          "Solana source-state proof request.sourceDomain must be Solana");
    }
    final BigInteger finalizedSlot = normalizeU64(request.finalizedSlot(), "request.finalizedSlot");
    final BigInteger parentSlot = normalizeU64(request.parentSlot(), "request.parentSlot");
    if (!parentSlot.add(BigInteger.ONE).equals(finalizedSlot)) {
      throw new IllegalArgumentException(
          "request.parentSlot must be the direct parent of finalizedSlot");
    }
    if (!MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1.equals(request.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierId must match Solana AccountsDB verifier profile");
    }
    final String sourceStateVerifierHash =
        normalizeNonZeroHex32(
            request.sourceStateVerifierHash(), "request.sourceStateVerifierHash");
    if (TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierHash must not be the Solana template verifier hash");
    }
    final String accountsLtHashProofPublicInputsHash =
        normalizeNonZeroHex32(
            request.accountsLtHashProofPublicInputsHash(),
            "request.accountsLtHashProofPublicInputsHash");
    normalizeNonZeroHex32(
        request.openedAccountsLtHashContributionsHash(),
        "request.openedAccountsLtHashContributionsHash");
    normalizeNonZeroHex32(
        request.openedAccountsLtHashResidualChecksum(),
        "request.openedAccountsLtHashResidualChecksum");
    requireSolanaOpenVerifyRequestPayloadForWrapping(
        request.statementBytes(),
        request.accountCommitmentBytes(),
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        request.publicInputColumns(),
        new String[] {
          request.fastpqPublicInputs().dsid(),
          request.fastpqPublicInputs().slot(),
          request.fastpqPublicInputs().oldRoot(),
          request.fastpqPublicInputs().newRoot(),
          request.fastpqPublicInputs().permRoot(),
          request.fastpqPublicInputs().txSetHash()
        },
        request.fastpqTransitions().stream()
            .map(transition -> new SourceStateTransitionCheck(
                transition.key(), transition.operation(), transition.oldValue(), transition.newValue()))
            .collect(Collectors.toList()),
        Arrays.asList(
            new SourceStateTransitionCheck(
                ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY_V1,
                "meta_set",
                new byte[0],
                request.statementBytes()),
            new SourceStateTransitionCheck(
                ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY_V1,
                "meta_set",
                new byte[0],
                request.accountCommitmentBytes()),
            new SourceStateTransitionCheck(
                ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY_V1,
                "meta_set",
                new byte[0],
                hex32Bytes(
                    request.openedAccountsLtHashContributionsHash(),
                    "request.openedAccountsLtHashContributionsHash")),
            new SourceStateTransitionCheck(
                ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY_V1,
                "meta_set",
                new byte[0],
                hex32Bytes(
                    request.openedAccountsLtHashResidualChecksum(),
                    "request.openedAccountsLtHashResidualChecksum")),
            new SourceStateTransitionCheck(
                ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY_V1,
                "meta_set",
                new byte[0],
                request.verificationContextBytes())));
    if (!accountsLtHashProofPublicInputsHash.equals(
        hashHex(ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX_V1, request.statementBytes()))) {
      throw new IllegalArgumentException(
          "request.accountsLtHashProofPublicInputsHash must match request.statementBytes");
    }
    final String expectedDsid =
        "0x"
            + hexLower(
                Arrays.copyOfRange(
                    hashBytes(
                        ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX_V1,
                        hex32Bytes(
                            accountsLtHashProofPublicInputsHash,
                            "request.accountsLtHashProofPublicInputsHash")),
                    0,
                    16));
    if (!normalizeHexBytes(request.fastpqPublicInputs().dsid(), "request.fastpqPublicInputs.dsid", 16)
        .equals(expectedDsid)) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.dsid must match request.statementBytes");
    }
    if (!normalizeNonZeroHex32(
            request.fastpqPublicInputs().txSetHash(), "request.fastpqPublicInputs.txSetHash")
        .equals(accountsLtHashProofPublicInputsHash)) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.txSetHash must match request.statementBytes");
    }
    requireSolanaSourceStatePublicInputBindingForWrapping(request);
  }

  private static void requireSourceStateProofRequestForWrapping(
      final FullLightClientAuditProofRequest request) {
    if (request.version() != 1) {
      throw new IllegalArgumentException("Solana source-state proof request.version must be 1");
    }
    if (!"stark-fri-v1".equals(request.proofFamily())) {
      throw new IllegalArgumentException(
          "Solana source-state proof request.proofFamily must be stark-fri-v1");
    }
    if (!FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1.equals(request.parameterSet())) {
      throw new IllegalArgumentException("request.parameterSet must be fastpq-lane-balanced");
    }
    if (request.sourceDomain() != DOMAIN_SOLANA) {
      throw new IllegalArgumentException(
          "Solana source-state proof request.sourceDomain must be Solana");
    }
    final FullLightClientAuditRoleProfile profile = auditRoleProfileForRequest(request.role());
    if (request.roleCode() != profile.code()) {
      throw new IllegalArgumentException("request.roleCode must match request.role");
    }
    if (!profile.circuitId().equals(request.circuitId())) {
      throw new IllegalArgumentException("request.circuitId must match request.role");
    }
    if (!profile.verifierId().equals(request.verifierId())) {
      throw new IllegalArgumentException("request.verifierId must match request.role");
    }
    normalizeU64(request.finalizedSlot(), "request.finalizedSlot");
    final String verifierHash = normalizeNonZeroHex32(request.verifierHash(), "request.verifierHash");
    if (!MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1.equals(request.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierId must match Solana AccountsDB verifier profile");
    }
    final String sourceStateVerifierHash =
        normalizeNonZeroHex32(
            request.sourceStateVerifierHash(), "request.sourceStateVerifierHash");
    if (TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierHash must not be the Solana template verifier hash");
    }
    String auditStatementHash = "";
    final ArrayList<String> roleSeparatedRequestHashes = new ArrayList<>();
    roleSeparatedRequestHashes.add(sourceStateVerifierHash);
    for (final String[] field : new String[][] {
      {"request.sourceVerifierMaterialHash", request.sourceVerifierMaterialHash()},
      {"request.sourceAdapterDeploymentHash", request.sourceAdapterDeploymentHash()},
      {"request.fullLightClientGateHash", request.fullLightClientGateHash()},
      {"request.finalityContextHash", request.finalityContextHash()},
      {"request.voteMessageHash", request.voteMessageHash()},
      {"request.accountsLtHashProofHash", request.accountsLtHashProofHash()},
      {"request.auditStatementHash", request.auditStatementHash()}
    }) {
      final String normalizedHash = normalizeNonZeroHex32(field[1], field[0]);
      roleSeparatedRequestHashes.add(normalizedHash);
      if ("request.auditStatementHash".equals(field[0])) {
        auditStatementHash = normalizedHash;
      }
    }
    if (roleSeparatedRequestHashes.contains(verifierHash)) {
      throw new IllegalArgumentException(
          "request.verifierHash must be role-separated from Solana full-light audit request hashes");
    }
    requireSolanaOpenVerifyRequestPayloadForWrapping(
        request.statementBytes(),
        null,
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        request.publicInputColumns(),
        new String[] {
          request.fastpqPublicInputs().dsid(),
          request.fastpqPublicInputs().slot(),
          request.fastpqPublicInputs().oldRoot(),
          request.fastpqPublicInputs().newRoot(),
          request.fastpqPublicInputs().permRoot(),
          request.fastpqPublicInputs().txSetHash()
        },
        request.fastpqTransitions().stream()
            .map(transition -> new SourceStateTransitionCheck(
                transition.key(), transition.operation(), transition.oldValue(), transition.newValue()))
            .collect(Collectors.toList()),
        Arrays.asList(
            new SourceStateTransitionCheck(
                "0x"
                    + hexLower(
                        fullLightClientAuditFastpqKey(
                            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1, profile)),
                "meta_set",
                new byte[0],
                request.statementBytes()),
            new SourceStateTransitionCheck(
                "0x"
                    + hexLower(
                        fullLightClientAuditFastpqKey(
                            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1, profile)),
                "meta_set",
                new byte[0],
                request.verificationContextBytes()),
            new SourceStateTransitionCheck(
                "0x"
                    + hexLower(
                        fullLightClientAuditFastpqKey(
                            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1, profile)),
                "meta_set",
                new byte[0],
                hex32Bytes(request.fullLightClientGateHash(), "request.fullLightClientGateHash"))));
    if (!auditStatementHash.equals(
        hashHex(FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1, request.statementBytes()))) {
      throw new IllegalArgumentException(
          "request.auditStatementHash must match request.statementBytes");
    }
    final byte[] auditStatementHashBytes =
        hex32Bytes(auditStatementHash, "request.auditStatementHash");
    final byte[] dsidPreimage = new byte[1 + auditStatementHashBytes.length];
    dsidPreimage[0] = (byte) profile.code();
    System.arraycopy(auditStatementHashBytes, 0, dsidPreimage, 1, auditStatementHashBytes.length);
    final String expectedDsid =
        "0x"
            + hexLower(
                Arrays.copyOfRange(
                    hashBytes(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1, dsidPreimage),
                    0,
                    16));
    if (!normalizeHexBytes(request.fastpqPublicInputs().dsid(), "request.fastpqPublicInputs.dsid", 16)
        .equals(expectedDsid)) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.dsid must match request.statementBytes");
    }
    if (!normalizeNonZeroHex32(
            request.fastpqPublicInputs().txSetHash(), "request.fastpqPublicInputs.txSetHash")
        .equals(auditStatementHash)) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.txSetHash must match request.statementBytes");
    }
    requireSolanaSourceStatePublicInputBindingForWrapping(request);
  }

  private static FullLightClientAuditRoleProfile auditRoleProfileForRequest(final String role) {
    switch (normalizeNonEmpty(role, "request.role")) {
      case "towerReplay":
      case "tower_replay":
        return auditRoleProfile(FullLightClientAuditRole.TOWER_REPLAY);
      case "fullAccountsdbLattice":
      case "full_accountsdb_lattice":
        return auditRoleProfile(FullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE);
      case "bankForkChoice":
      case "bank_fork_choice":
        return auditRoleProfile(FullLightClientAuditRole.BANK_FORK_CHOICE);
      default:
        throw new IllegalArgumentException(
            "request.role must be tower_replay, full_accountsdb_lattice, or bank_fork_choice");
    }
  }

  private record SourceStateTransitionCheck(
      String key, String operation, byte[] oldValue, byte[] newValue) {}

  private static void requireSolanaSourceStatePublicInputBindingForWrapping(
      final AccountsLtHashProofRequest request) {
    final List<List<String>> publicInputColumns = request.publicInputColumns();
    if (publicInputColumns == null || publicInputColumns.isEmpty()) {
      throw new IllegalArgumentException("request.publicInputColumns is required");
    }
    final String sourceDomainColumn = "0x" + hexLower(sccpWordU32Le(DOMAIN_SOLANA));
    final String mainnetGenesisColumn = solanaMainnetGenesisHashPublicInput();
    if (!ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1.equals(request.circuitId())) {
      throw new IllegalArgumentException(
          "request.circuitId must be the Solana AccountsLtHash OpenVerify circuit");
    }
    requirePublicInputColumn(publicInputColumns, 0, sourceDomainColumn, "source_domain");
    requirePublicInputColumn(publicInputColumns, 1, mainnetGenesisColumn, "mainnet_genesis_hash");
    requirePublicInputColumn(
        publicInputColumns,
        2,
        "0x" + hexLower(sccpWordU64Le(normalizeU64(
            request.finalizedSlot(), "request.finalizedSlot"))),
        "finalized_slot");
    requirePublicInputColumn(
        publicInputColumns,
        3,
        "0x" + hexLower(sccpWordU64Le(normalizeU64(
            request.parentSlot(), "request.parentSlot"))),
        "parent_slot");
    requirePublicInputColumn(
        publicInputColumns,
        11,
        normalizeNonZeroHex32(
            request.accountsLtHashProofPublicInputsHash(),
            "request.accountsLtHashProofPublicInputsHash"),
        "accounts_lt_hash_proof_public_inputs_hash");
    requirePublicInputColumn(
        publicInputColumns,
        12,
        normalizeNonZeroHex32(
            request.openedAccountsLtHashContributionsHash(),
            "request.openedAccountsLtHashContributionsHash"),
        "opened_accounts_lt_hash_contributions_hash");
    requirePublicInputColumn(
        publicInputColumns,
        13,
        normalizeNonZeroHex32(
            request.openedAccountsLtHashResidualChecksum(),
            "request.openedAccountsLtHashResidualChecksum"),
        "opened_accounts_lt_hash_residual_checksum");
  }

  private static void requireSolanaSourceStatePublicInputBindingForWrapping(
      final FullLightClientAuditProofRequest request) {
    final List<List<String>> publicInputColumns = request.publicInputColumns();
    if (publicInputColumns == null || publicInputColumns.isEmpty()) {
      throw new IllegalArgumentException("request.publicInputColumns is required");
    }
    final String sourceDomainColumn = "0x" + hexLower(sccpWordU32Le(DOMAIN_SOLANA));
    final String mainnetGenesisColumn = solanaMainnetGenesisHashPublicInput();
    final FullLightClientAuditRoleProfile profile = auditRoleProfileForRequest(request.role());
    requirePublicInputColumn(
        publicInputColumns, 0, "0x" + hexLower(sccpWordU8(profile.code())), "role");
    requirePublicInputColumn(publicInputColumns, 1, sourceDomainColumn, "source_domain");
    requirePublicInputColumn(publicInputColumns, 2, mainnetGenesisColumn, "mainnet_genesis_hash");
    requirePublicInputColumn(
        publicInputColumns,
        3,
        "0x" + hexLower(sccpWordU64Le(normalizeU64(
            request.finalizedSlot(), "request.finalizedSlot"))),
        "finalized_slot");
    requirePublicInputColumn(
        publicInputColumns,
        4,
        normalizeNonZeroHex32(request.finalityContextHash(), "request.finalityContextHash"),
        "finality_context_hash");
    requirePublicInputColumn(
        publicInputColumns,
        5,
        normalizeNonZeroHex32(request.auditStatementHash(), "request.auditStatementHash"),
        "audit_statement_hash");
    requirePublicInputColumn(
        publicInputColumns,
        6,
        normalizeNonZeroHex32(
            request.sourceVerifierMaterialHash(), "request.sourceVerifierMaterialHash"),
        "source_verifier_material_hash");
    requirePublicInputColumn(
        publicInputColumns,
        7,
        normalizeNonZeroHex32(
            request.sourceAdapterDeploymentHash(), "request.sourceAdapterDeploymentHash"),
        "source_adapter_deployment_hash");
    requirePublicInputColumn(
        publicInputColumns,
        8,
        normalizeNonZeroHex32(request.fullLightClientGateHash(), "request.fullLightClientGateHash"),
        "full_light_client_gate_hash");
    requirePublicInputColumn(
        publicInputColumns,
        9,
        normalizeNonZeroHex32(request.verifierHash(), "request.verifierHash"),
        "verifier_hash");
    requirePublicInputColumn(
        publicInputColumns,
        13,
        normalizeNonZeroHex32(request.voteMessageHash(), "request.voteMessageHash"),
        "vote_message_hash");
    requirePublicInputColumn(
        publicInputColumns,
        14,
        normalizeNonZeroHex32(request.accountsLtHashProofHash(), "request.accountsLtHashProofHash"),
        "accounts_lt_hash_proof_hash");
  }

  private static void requirePublicInputColumn(
      final List<List<String>> publicInputColumns,
      final int index,
      final String expected,
      final String fieldName) {
    if (publicInputColumns == null || index >= publicInputColumns.size()) {
      throw new IllegalArgumentException("request.publicInputColumns must bind " + fieldName);
    }
    final List<String> column = publicInputColumns.get(index);
    if (column == null || column.size() != 1) {
      throw new IllegalArgumentException("request.publicInputColumns must bind " + fieldName);
    }
    final String actual =
        normalizeNonEmpty(column.get(0), "request.publicInputColumns[" + index + "][0]");
    if (!expected.equals(actual)) {
      throw new IllegalArgumentException("request.publicInputColumns must bind " + fieldName);
    }
  }

  private static void requireSolanaOpenVerifyRequestPayloadForWrapping(
      final byte[] statementBytes,
      final byte[] accountCommitmentBytes,
      final byte[] verificationContextBytes,
      final byte[] schemaDescriptor,
      final List<List<String>> publicInputColumns,
      final String[] fastpqFields,
      final List<SourceStateTransitionCheck> transitions,
      final List<SourceStateTransitionCheck> expectedTransitions) {
    if (statementBytes.length == 0) {
      throw new IllegalArgumentException("request.statementBytes must not be empty");
    }
    if (accountCommitmentBytes != null && accountCommitmentBytes.length == 0) {
      throw new IllegalArgumentException("request.accountCommitmentBytes must not be empty");
    }
    if (verificationContextBytes.length == 0) {
      throw new IllegalArgumentException("request.verificationContextBytes must not be empty");
    }
    if (schemaDescriptor.length == 0) {
      throw new IllegalArgumentException("request.schemaDescriptor must not be empty");
    }
    if (publicInputColumns == null || publicInputColumns.isEmpty()) {
      throw new IllegalArgumentException("request.publicInputColumns is required");
    }
    for (int index = 0; index < publicInputColumns.size(); index++) {
      final List<String> column = publicInputColumns.get(index);
      if (column == null || column.isEmpty()) {
        throw new IllegalArgumentException(
            "request.publicInputColumns[" + index + "] must not be empty");
      }
      for (int valueIndex = 0; valueIndex < column.size(); valueIndex++) {
        normalizeNonEmpty(
            column.get(valueIndex),
            "request.publicInputColumns[" + index + "][" + valueIndex + "]");
      }
    }
    for (int index = 0; index < fastpqFields.length; index++) {
      normalizeNonEmpty(fastpqFields[index], "request.fastpqPublicInputs[" + index + "]");
    }
    if (transitions == null || transitions.isEmpty()) {
      throw new IllegalArgumentException("request.fastpqTransitions is required");
    }
    for (int index = 0; index < transitions.size(); index++) {
      final SourceStateTransitionCheck transition = transitions.get(index);
      normalizeNonEmpty(transition.key(), "request.fastpqTransitions[" + index + "].key");
      normalizeNonEmpty(
          transition.operation(), "request.fastpqTransitions[" + index + "].operation");
      if (transition.newValue() == null || transition.newValue().length == 0) {
        throw new IllegalArgumentException(
            "request.fastpqTransitions[" + index + "].newValue must not be empty");
      }
    }
    final List<SourceStateTransitionCheck> actual = new ArrayList<>(transitions);
    final List<SourceStateTransitionCheck> expected = new ArrayList<>(expectedTransitions);
    actual.sort((left, right) -> left.key().compareTo(right.key()));
    expected.sort((left, right) -> left.key().compareTo(right.key()));
    if (actual.size() != expected.size()) {
      throw new IllegalArgumentException(
          "request.fastpqTransitions must match the canonical Solana source-state request");
    }
    for (int index = 0; index < actual.size(); index++) {
      final SourceStateTransitionCheck left = actual.get(index);
      final SourceStateTransitionCheck right = expected.get(index);
      if (!Objects.equals(left.key(), right.key())
          || !Objects.equals(left.operation(), right.operation())
          || !Arrays.equals(left.oldValue(), right.oldValue())
          || !Arrays.equals(left.newValue(), right.newValue())) {
        throw new IllegalArgumentException(
            "request.fastpqTransitions must match the canonical Solana source-state request");
      }
    }
  }

  private static SourceStateVerificationProof wrapSourceStateVerificationProof(
      final byte[] proofBytes,
      final int version,
      final String proofFamily,
      final String circuitId,
      final int sourceDomain) {
    final byte[] normalizedProofBytes =
        Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
    if (version != 1) {
      throw new IllegalArgumentException("sourceStateProof.version must be 1");
    }
    if (!"stark-fri-v1".equals(proofFamily)) {
      throw new IllegalArgumentException("sourceStateProof.proofFamily must be stark-fri-v1");
    }
    requireSourceStateProofLabel(proofFamily, "sourceStateProof.proofFamily");
    requireSourceStateProofLabel(circuitId, "sourceStateProof.circuitId");
    if (sourceDomain != DOMAIN_SOLANA) {
      throw new IllegalArgumentException("sourceStateProof.sourceDomain must be Solana");
    }
    if (!ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId)
        && !TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId)
        && !FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId)
        && !BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId)) {
      throw new IllegalArgumentException(
          "sourceStateProof.circuitId must be a Solana source-state verification circuit");
    }
    if (normalizedProofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    if (normalizedProofBytes.length > SOURCE_STATE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must be at most " + SOURCE_STATE_MAX_PROOF_BYTES + " bytes");
    }
    if (!anyNonZero(normalizedProofBytes)) {
      throw new IllegalArgumentException("proofBytes must not be all zero");
    }
    return new SourceStateVerificationProof(version, proofFamily, circuitId, normalizedProofBytes);
  }

  public static byte[] canonicalFullLightClientAuditFinalityContextBytes(
      final FullLightClientAuditProofInput input) {
    return canonicalFinalityContextBytes(
        normalizeFullLightClientAuditInput(input, FullLightClientAuditRole.TOWER_REPLAY).context());
  }

  public static String fullLightClientAuditFinalityContextHash(
      final FullLightClientAuditProofInput input) {
    return hashHex(
        "sccp:solana:finality-context:v1",
        canonicalFullLightClientAuditFinalityContextBytes(input));
  }

  public static byte[] canonicalFullLightClientAuditVoteMessageBytes(
      final FullLightClientAuditProofInput input) {
    final NormalizedFullLightClientAuditInput value =
        normalizeFullLightClientAuditInput(input, FullLightClientAuditRole.TOWER_REPLAY);
    return canonicalVoteMessageBytes(value.witness(), value.finalityContextHash());
  }

  public static String fullLightClientAuditVoteMessageHash(
      final FullLightClientAuditProofInput input) {
    return hashHex(
        "sccp:solana:finalized-vote:v1",
        canonicalFullLightClientAuditVoteMessageBytes(input));
  }

  public static byte[] canonicalFullLightClientAuditStatementBytes(
      final FullLightClientAuditProofInput input, final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput value = normalizeFullLightClientAuditInput(input, role);
    final FullLightClientAuditRoleProfile profile = auditRoleProfile(role);
    final Witness witness = value.witness();
    final NormalizedFullLightClientAuditContext context = value.context();
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(profile.code());
    writeString(out, profile.circuitId(), "circuitId");
    writeString(out, RECURSIVE_PROOF_BACKEND_V1, "backend");
    writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash");
    writeU32Le(out, DOMAIN_SOLANA);
    writeU64Le(out, context.epoch());
    writeU64Le(out, normalizeU64(witness.finalizedSlot(), "finalizedSlot"));
    writeU64Le(out, context.rootedSlot());
    writeU64Le(out, context.parentSlot());
    write(out, hex32Bytes(value.finalityContextHash(), "finalityContextHash"));
    write(out, hex32Bytes(value.voteMessageHash(), "voteMessageHash"));
    write(out, hex32Bytes(value.accountsLtHashProofHash(), "accountsLtHashProofHash"));
    switch (role) {
      case TOWER_REPLAY:
        write(out, hex32Bytes(context.towerLockoutHash(), "towerLockoutHash"));
        write(out, hex32Bytes(context.towerReplayHash(), "towerReplayHash"));
        write(out, hex32Bytes(context.bankForkHash(), "bankForkHash"));
        write(out, hex32Bytes(context.epochStakeRoot(), "epochStakeRoot"));
        write(out, hex32Bytes(context.stakeActivationHash(), "stakeActivationHash"));
        write(out, hex32Bytes(context.stakeAccountStateHash(), "stakeAccountStateHash"));
        write(out, hex32Bytes(context.stakeHistoryHash(), "stakeHistoryHash"));
        write(out, hex32Bytes(context.stakeHistorySysvarAccountHash(), "stakeHistorySysvarAccountHash"));
        write(out, hex32Bytes(context.accountInclusionRoot(), "accountInclusionRoot"));
        writeU32Le(out, context.towerVoteSlots().size());
        for (final BigInteger slot : context.towerVoteSlots()) {
          writeU64Le(out, slot);
        }
        break;
      case FULL_ACCOUNTSDB_LATTICE:
        write(out, hex32Bytes(context.accountInclusionRoot(), "accountInclusionRoot"));
        write(out, hex32Bytes(context.accountsLtHashChecksum(), "accountsLtHashChecksum"));
        write(
            out,
            hex32Bytes(
                context.accountsLtHashProofPublicInputsHash(),
                "accountsLtHashProofPublicInputsHash"));
        write(
            out,
            hex32Bytes(
                value.openedAccountsLtHashContributionsHash(),
                "openedAccountsLtHashContributionsHash"));
        write(
            out,
            hex32Bytes(
                value.openedAccountsLtHashResidualChecksum(),
                "openedAccountsLtHashResidualChecksum"));
        write(out, hex32Bytes(value.accountsLtHashProofHash(), "accountsLtHashProofHash"));
        break;
      case BANK_FORK_CHOICE:
        write(out, hex32Bytes(context.parentBankHash(), "parentBankHash"));
        write(out, hex32Bytes(witness.bankHash(), "bankHash"));
        write(out, hex32Bytes(witness.blockhash(), "blockhash"));
        write(out, hex32Bytes(witness.transactionStatusRoot(), "transactionStatusRoot"));
        write(out, hex32Bytes(context.accountInclusionRoot(), "accountInclusionRoot"));
        write(out, hex32Bytes(context.accountsLtHashChecksum(), "accountsLtHashChecksum"));
        writeU64Le(out, context.bankSignatureCount());
        writeVec(out, context.bankHashHardForkData());
        write(out, hex32Bytes(context.bankForkHash(), "bankForkHash"));
        write(out, hex32Bytes(context.towerReplayHash(), "towerReplayHash"));
        break;
      default:
        throw new IllegalArgumentException("unsupported Solana full light-client audit role");
    }
    return out.toByteArray();
  }

  public static String fullLightClientAuditStatementHash(
      final FullLightClientAuditProofInput input, final FullLightClientAuditRole role) {
    return hashHex(
        FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1,
        canonicalFullLightClientAuditStatementBytes(input, role));
  }

  public static List<List<String>> fullLightClientAuditPublicInputColumns(
      final FullLightClientAuditProofInput input, final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput value = normalizeFullLightClientAuditInput(input, role);
    final FullLightClientAuditRoleProfile profile = auditRoleProfile(role);
    final List<List<String>> columns = new ArrayList<>();
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU8(profile.code()))));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU32Le(DOMAIN_SOLANA))));
    columns.add(Collections.singletonList(solanaMainnetGenesisHashPublicInput()));
    columns.add(
        Collections.singletonList(
            "0x"
                + hexLower(
                    sccpWordU64Le(
                        normalizeU64(value.witness().finalizedSlot(), "finalizedSlot")))));
    columns.add(Collections.singletonList(value.finalityContextHash()));
    columns.add(Collections.singletonList(fullLightClientAuditStatementHash(input, role)));
    columns.add(Collections.singletonList(value.sourceVerifierMaterialHash()));
    columns.add(Collections.singletonList(value.sourceAdapterDeploymentHash()));
    columns.add(Collections.singletonList(value.fullLightClientGateHash()));
    columns.add(Collections.singletonList(value.verifierHash()));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(value.context().epoch()))));
    columns.add(
        Collections.singletonList("0x" + hexLower(sccpWordU64Le(value.context().rootedSlot()))));
    columns.add(
        Collections.singletonList("0x" + hexLower(sccpWordU64Le(value.context().parentSlot()))));
    columns.add(Collections.singletonList(value.voteMessageHash()));
    columns.add(Collections.singletonList(value.accountsLtHashProofHash()));
    for (final String column : auditRoleColumns(value, role)) {
      columns.add(Collections.singletonList(column));
    }
    return copyStringColumns(columns);
  }

  public static byte[] fullLightClientAuditOpenVerifySchemaDescriptor(
      final FullLightClientAuditProofInput input, final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput value = normalizeFullLightClientAuditInput(input, role);
    final FullLightClientAuditRoleProfile profile = auditRoleProfile(role);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(profile.code());
    writeString(out, profile.circuitId(), "circuitId");
    writeString(out, FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1, "parameterSet");
    writeString(out, MAINNET_GENESIS_HASH, "mainnetGenesisHash");
    writeU32Le(out, DOMAIN_SOLANA);
    writeString(out, "verifier_id", "schemaField");
    writeString(out, profile.verifierId(), "verifierId");
    writeString(out, "verifier_hash", "schemaField");
    write(out, hex32Bytes(value.verifierHash(), "verifierHash"));
    writeString(out, "source_verifier_material_hash", "schemaField");
    write(out, hex32Bytes(value.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash"));
    writeString(out, "source_adapter_deployment_hash", "schemaField");
    write(out, hex32Bytes(value.sourceAdapterDeploymentHash(), "sourceAdapterDeploymentHash"));
    writeString(out, "full_light_client_gate_hash", "schemaField");
    write(out, hex32Bytes(value.fullLightClientGateHash(), "fullLightClientGateHash"));
    final List<String> required = new ArrayList<>();
    Collections.addAll(
        required,
        "role",
        "source_domain",
        "mainnet_genesis_hash",
        "finalized_slot",
        "finality_context_hash",
        "audit_statement_hash",
        "source_verifier_material_hash",
        "source_adapter_deployment_hash",
        "full_light_client_gate_hash",
        "verifier_hash",
        "epoch",
        "rooted_slot",
        "parent_slot",
        "vote_message_hash",
        "accounts_lt_hash_proof_hash");
    required.addAll(profile.requiredInputNames());
    for (final String name : required) {
      writeString(out, name, "requiredInput");
    }
    return out.toByteArray();
  }

  public static FullLightClientAuditProofRequest buildFullLightClientAuditProofRequest(
      final FullLightClientAuditProofInput input, final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput value = normalizeFullLightClientAuditInput(input, role);
    final FullLightClientAuditRoleProfile profile = auditRoleProfile(role);
    final byte[] statementBytes = canonicalFullLightClientAuditStatementBytes(input, role);
    final String auditStatementHash = fullLightClientAuditStatementHash(input, role);
    requireFullLightClientAuditRoleRequestHashSeparation(value, auditStatementHash);
    final byte[] verificationContextBytes =
        canonicalFullLightClientAuditContextBytes(value, auditStatementHash);
    final List<FullLightClientAuditFastpqTransition> transitions = new ArrayList<>();
    transitions.add(
        new FullLightClientAuditFastpqTransition(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1, profile)),
            "meta_set",
            new byte[0],
            statementBytes));
    transitions.add(
        new FullLightClientAuditFastpqTransition(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1, profile)),
            "meta_set",
            new byte[0],
            verificationContextBytes));
    transitions.add(
        new FullLightClientAuditFastpqTransition(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1, profile)),
            "meta_set",
            new byte[0],
            hex32Bytes(value.fullLightClientGateHash(), "fullLightClientGateHash")));
    Collections.sort(transitions, (left, right) -> left.key().compareTo(right.key()));
    return new FullLightClientAuditProofRequest(
        1,
        "stark-fri-v1",
        profile.circuitId(),
        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1,
        profile.name(),
        profile.code(),
        DOMAIN_SOLANA,
        value.witness().finalizedSlot(),
        profile.verifierId(),
        value.verifierHash(),
        value.witness().sourceStateVerifierId(),
        value.witness().sourceStateVerifierHash(),
        value.sourceVerifierMaterialHash(),
        value.sourceAdapterDeploymentHash(),
        value.fullLightClientGateHash(),
        value.finalityContextHash(),
        value.voteMessageHash(),
        value.accountsLtHashProofHash(),
        auditStatementHash,
        statementBytes,
        verificationContextBytes,
        fullLightClientAuditOpenVerifySchemaDescriptor(input, role),
        fullLightClientAuditPublicInputColumns(input, role),
        fullLightClientAuditFastpqPublicInputs(value, auditStatementHash),
        transitions);
  }

  private static void requireFullLightClientAuditRoleRequestHashSeparation(
      final NormalizedFullLightClientAuditInput value, final String auditStatementHash) {
    final List<String> requestHashes =
        Arrays.asList(
            value.witness().sourceStateVerifierHash(),
            value.sourceVerifierMaterialHash(),
            value.sourceAdapterDeploymentHash(),
            value.fullLightClientGateHash(),
            value.finalityContextHash(),
            value.voteMessageHash(),
            value.accountsLtHashProofHash(),
            auditStatementHash);
    if (requestHashes.contains(value.verifierHash())) {
      throw new IllegalArgumentException(
          "verifierHash must be role-separated from Solana full-light audit request hashes");
    }
  }

  public static FullLightClientAuditProofRequest buildTowerReplayProofRequest(
      final FullLightClientAuditProofInput input) {
    return buildFullLightClientAuditProofRequest(input, FullLightClientAuditRole.TOWER_REPLAY);
  }

  public static FullLightClientAuditProofRequest buildFullAccountsdbLatticeProofRequest(
      final FullLightClientAuditProofInput input) {
    return buildFullLightClientAuditProofRequest(
        input, FullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE);
  }

  public static FullLightClientAuditProofRequest buildBankForkChoiceProofRequest(
      final FullLightClientAuditProofInput input) {
    return buildFullLightClientAuditProofRequest(input, FullLightClientAuditRole.BANK_FORK_CHOICE);
  }

  public static FullLightClientAuditProofRequests buildFullLightClientAuditProofRequests(
      final FullLightClientAuditProofInput input) {
    return new FullLightClientAuditProofRequests(
        buildTowerReplayProofRequest(input),
        buildFullAccountsdbLatticeProofRequest(input),
        buildBankForkChoiceProofRequest(input));
  }

  public static String agaveBankHash(
      final String parentBankHash,
      final String bankSignatureCount,
      final String blockhash,
      final byte[] accountsLtHash) {
    return agaveBankHash(parentBankHash, bankSignatureCount, blockhash, accountsLtHash, new byte[0]);
  }

  public static String agaveBankHash(
      final String parentBankHash,
      final String bankSignatureCount,
      final String blockhash,
      final byte[] accountsLtHash,
      final byte[] bankHashHardForkData) {
    final byte[] parentBankHashBytes = hex32Bytes(parentBankHash, "parentBankHash");
    if (!anyNonZero(parentBankHashBytes)) {
      throw new IllegalArgumentException("parentBankHash must not be zero");
    }
    final BigInteger signatureCount = normalizeU64(bankSignatureCount, "bankSignatureCount");
    if (BigInteger.ZERO.equals(signatureCount)) {
      throw new IllegalArgumentException("bankSignatureCount must be nonzero");
    }
    final byte[] blockhashBytes = solanaHash32Bytes(blockhash, "blockhash");
    if (!anyNonZero(blockhashBytes)) {
      throw new IllegalArgumentException("blockhash must not be zero");
    }
    return "0x"
        + hexLower(
            agaveBankHashBytes(
                parentBankHashBytes,
                signatureCount,
                blockhashBytes,
                accountsLtHash,
                bankHashHardForkData == null ? new byte[0] : bankHashHardForkData));
  }

  public static ProofRequest buildProofRequest(final WitnessInput input) {
    final Witness witness = normalizeWitness(input);
    final String witnessHash = hashHex("sccp:solana:witness:v1", canonicalWitnessBytes(witness));
    final ProofContext proofContext =
        normalizeProofContext(input.statementHash(), input.destinationBindingHash());
    final String proofContextHash = proofContextHash(proofContext);
    final SourceAdapterDeploymentBinding deploymentBinding =
        normalizeSourceAdapterDeploymentBinding(
            witness.sourceDomain(),
            witness.targetDomain(),
            witness.sourceAdapterDeploymentHash(),
            witness.sourceAdapterDeploymentReceiptHash());
    if (ZERO_HASH_V1.equals(deploymentBinding.sourceAdapterDeploymentHash())) {
      throw new IllegalArgumentException(
          "Solana SCCP proof request requires non-zero source adapter deployment binding");
    }
    final String deploymentBindingHash = sourceAdapterDeploymentBindingHash(deploymentBinding);
    return new ProofRequest(
        1,
        RECURSIVE_PROOF_BACKEND_V1,
        DOMAIN_SOLANA,
        witness.targetDomain(),
        witness.mainnetGenesisHash(),
        witnessHash,
        proofContextHash,
        deploymentBindingHash,
        witness.sourceStateVerifierId(),
        witness.sourceStateVerifierHash(),
        new PublicInputs(
            witness.messageId(),
            witness.payloadHash(),
            witness.commitmentRoot(),
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
            witness.sourceEventDigest(),
            witness.sourceStateVerifierId(),
            witness.sourceStateVerifierHash(),
            proofContext.statementHash(),
            proofContext.destinationBindingHash(),
            deploymentBinding.sourceAdapterDeploymentHash(),
            deploymentBinding.sourceAdapterDeploymentReceiptHash(),
            deploymentBindingHash),
        witness,
        proofContext,
        deploymentBinding);
  }

  public static SourceAdapterDeploymentBinding normalizeSourceAdapterDeploymentBinding(
      final int sourceDomain,
      final int targetDomain,
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash) {
    final String deploymentHash =
        normalizeHex32(sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash");
    final String receiptHash =
        normalizeHex32(
            sourceAdapterDeploymentReceiptHash, "sourceAdapterDeploymentReceiptHash");
    final boolean deploymentIsZero = ZERO_HASH_V1.equals(deploymentHash);
    final boolean receiptIsZero = ZERO_HASH_V1.equals(receiptHash);
    if (deploymentIsZero != receiptIsZero) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentHash and sourceAdapterDeploymentReceiptHash "
              + "must both be zero or both be non-zero");
    }
    if (!deploymentIsZero && deploymentHash.equals(receiptHash)) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentHash must differ from sourceAdapterDeploymentReceiptHash");
    }
    return new SourceAdapterDeploymentBinding(
        1, sourceDomain, targetDomain, deploymentHash, receiptHash);
  }

  public static SourceAdapterDeploymentBinding normalizeSourceAdapterDeploymentBinding(
      final String sourceAdapterDeploymentHash,
      final String sourceAdapterDeploymentReceiptHash) {
    return normalizeSourceAdapterDeploymentBinding(
        DOMAIN_SOLANA, DOMAIN_SORA, sourceAdapterDeploymentHash, sourceAdapterDeploymentReceiptHash);
  }

  public static byte[] canonicalSourceAdapterDeploymentBindingBytes(
      final SourceAdapterDeploymentBinding binding) {
    Objects.requireNonNull(binding, "binding");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(binding.version());
    writeU32Le(out, binding.sourceDomain());
    writeU32Le(out, binding.targetDomain());
    write(out, hex32Bytes(binding.sourceAdapterDeploymentHash(), "sourceAdapterDeploymentHash"));
    write(
        out,
        hex32Bytes(
            binding.sourceAdapterDeploymentReceiptHash(),
            "sourceAdapterDeploymentReceiptHash"));
    return out.toByteArray();
  }

  public static String sourceAdapterDeploymentBindingHash(
      final SourceAdapterDeploymentBinding binding) {
    return hashHex(
        "sccp:source-adapter-deployment-binding:v1",
        canonicalSourceAdapterDeploymentBindingBytes(binding));
  }

  public static ProofContext normalizeProofContext(
      final String statementHash, final String destinationBindingHash) {
    return new ProofContext(
        1,
        normalizeNonZeroHex32(statementHash, "statementHash"),
        normalizeNonZeroHex32(destinationBindingHash, "destinationBindingHash"));
  }

  public static byte[] canonicalProofContextBytes(final ProofContext context) {
    Objects.requireNonNull(context, "context");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(context.version());
    write(out, hex32Bytes(context.statementHash(), "statementHash"));
    write(out, hex32Bytes(context.destinationBindingHash(), "destinationBindingHash"));
    return out.toByteArray();
  }

  public static String proofContextHash(final ProofContext context) {
    return hashHex("sccp:solana:proof-context:v1", canonicalProofContextBytes(context));
  }

  public static byte[] canonicalSubmissionPublicInputsBytes(final SubmissionPublicInputs input) {
    Objects.requireNonNull(input, "input");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(input.version());
    write(out, hex32Bytes(input.messageId(), "messageId"));
    write(out, hex32Bytes(input.payloadHash(), "payloadHash"));
    writeU32Le(out, input.targetDomain());
    write(out, hex32Bytes(input.commitmentRoot(), "commitmentRoot"));
    writeU64Le(out, normalizeU64(input.finalityHeight(), "finalityHeight"));
    write(out, hex32Bytes(input.finalityBlockHash(), "finalityBlockHash"));
    return out.toByteArray();
  }

  public static Submission buildSubmission(final SubmissionInput input) {
    Objects.requireNonNull(input, "input");
    final byte[] bundleBytes = requireNativeRecursivePayloadBytes(input.bundleBytes(), "bundleBytes");
    if (input.publicInputs().version() != 1) {
      throw new IllegalArgumentException("publicInputs.version must be 1");
    }
    if (input.publicInputs().targetDomain() != DOMAIN_SOLANA) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be Solana");
    }
    if (input.proofResult() == null) {
      throw new IllegalArgumentException("proofResult must be a wrapped Solana SCCP proof result");
    }
    final ProofResult checkedProofResult =
        requireWrappedProofResultForSubmission(input.proofResult(), input.publicInputs());
    final byte[] proofBytes = checkedProofResult.proofBytes();
    if (!Arrays.equals(input.proofBytes(), proofBytes)) {
      throw new IllegalArgumentException("proofBytes must match proofResult.proofBytes");
    }
    final byte[] publicInputsBytes = canonicalSubmissionPublicInputsBytes(input.publicInputs());
    final ProofContext proofContext = checkedProofResult.proofContext();
    final String proofContextStatementHash =
        normalizeHex32(proofContext.statementHash(), "proofResult.proofContext.statementHash");
    final String proofContextDestinationBindingHash =
        normalizeHex32(
            proofContext.destinationBindingHash(),
            "proofResult.proofContext.destinationBindingHash");
    if (!normalizeHex32(input.statementHash(), "statementHash")
        .equals(proofContextStatementHash)) {
      throw new IllegalArgumentException("statementHash must match proofResult.proofContext");
    }
    if (!normalizeHex32(input.destinationBindingHash(), "destinationBindingHash")
        .equals(proofContextDestinationBindingHash)) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match proofResult.proofContext");
    }
    if (!proofContextDestinationBindingHash.equals(
        SourceSccpProofs.destinationBindingHash(DOMAIN_SOLANA))) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match canonical Solana destination binding");
    }
    final String expectedProofContextHash = proofContextHash(proofContext);
    if (input.proofContextHash() != null
        && !normalizeHex32(input.proofContextHash(), "proofContextHash")
            .equals(expectedProofContextHash)) {
      throw new IllegalArgumentException(
          "proofContextHash must match statementHash and destinationBindingHash");
    }
    final byte[] statementHashBytes = hex32Bytes(proofContextStatementHash, "statementHash");
    final byte[] destinationBindingHashBytes =
        hex32Bytes(proofContextDestinationBindingHash, "destinationBindingHash");
    final byte[] proofContextHashBytes =
        hex32Bytes(expectedProofContextHash, "proofContextHash");
    final List<SubmissionArgumentBytes> argumentBytes = new ArrayList<>();
    argumentBytes.add(new SubmissionArgumentBytes("proof_bytes", proofBytes));
    argumentBytes.add(new SubmissionArgumentBytes("public_inputs", publicInputsBytes));
    argumentBytes.add(new SubmissionArgumentBytes("bundle_bytes", bundleBytes));
    argumentBytes.add(new SubmissionArgumentBytes("statement_hash", statementHashBytes));
    argumentBytes.add(
        new SubmissionArgumentBytes("destination_binding_hash", destinationBindingHashBytes));
    argumentBytes.add(new SubmissionArgumentBytes("proof_context_hash", proofContextHashBytes));
    final byte[] instructionData = encodeInstructionData(argumentBytes);
    final List<SubmissionArgument> arguments = new ArrayList<>(argumentBytes.size());
    for (final SubmissionArgumentBytes argument : argumentBytes) {
      arguments.add(new SubmissionArgument(argument.key, "raw_bytes", "0x" + hexLower(argument.bytes)));
    }
    return new Submission(
        1,
        BORSH_INSTRUCTION_V1,
        "program_instruction",
        SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
        proofBytes,
        input.publicInputs(),
        publicInputsBytes,
        bundleBytes,
        proofContextStatementHash,
        proofContextDestinationBindingHash,
        expectedProofContextHash,
        arguments,
        instructionData,
        "0x" + hexLower(instructionData),
        instructionData,
        "0x" + hexLower(instructionData));
  }

  private static byte[] requireNativeRecursivePayloadBytes(
      final byte[] bytes, final String label) {
    Objects.requireNonNull(bytes, label);
    if (bytes.length == 0) {
      throw new IllegalArgumentException(label + " must not be empty");
    }
    if (bytes.length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          label + " must be at most " + NATIVE_RECURSIVE_MAX_PROOF_BYTES + " bytes");
    }
    if (!anyNonZero(bytes)) {
      throw new IllegalArgumentException(label + " must not be all zero");
    }
    return Arrays.copyOf(bytes, bytes.length);
  }

  public static ProofResult wrapProofResult(final byte[] proofBytes, final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (!RECURSIVE_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException(
          "Solana SCCP proof request backend must be sccp-solana-recursive-mainnet-v1");
    }
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    if (proofBytes.length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must be at most " + NATIVE_RECURSIVE_MAX_PROOF_BYTES + " bytes");
    }
    if (!anyNonZero(proofBytes)) {
      throw new IllegalArgumentException("proofBytes must not be all zero");
    }
    requireCanonicalProofRequest(request);
    requireProductionProofRequest(request);
    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    write(payload, hex32Bytes(request.witnessHash(), "witnessHash"));
    write(payload, hex32Bytes(request.proofContextHash(), "proofContextHash"));
    write(
        payload,
        hex32Bytes(
            request.sourceAdapterDeploymentBindingHash(),
            "sourceAdapterDeploymentBindingHash"));
    write(payload, proofBytes);
    return new ProofResult(
        1,
        request.backend(),
        Arrays.copyOf(proofBytes, proofBytes.length),
        Base64.getEncoder().encodeToString(proofBytes),
        request.publicInputs(),
        request.witnessHash(),
        request.proofContextHash(),
        request.sourceAdapterDeploymentBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.proofContext(),
        request.sourceAdapterDeploymentBinding(),
        hashHex("sccp:solana:proof-envelope:v1", payload.toByteArray()));
  }

  private static void requireProofResultSourcePublicInputShape(final PublicInputs publicInputs) {
    final BigInteger finalizedSlot =
        normalizeU64(publicInputs.finalizedSlot(), "proofResult.publicInputs.finalizedSlot");
    final BigInteger parentSlot =
        normalizeU64(publicInputs.parentSlot(), "proofResult.publicInputs.parentSlot");
    if (!parentSlot.add(BigInteger.ONE).equals(finalizedSlot)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.parentSlot must be the direct parent of finalizedSlot");
    }
    final BigInteger bankSignatureCount =
        normalizeU64(
            publicInputs.bankSignatureCount(), "proofResult.publicInputs.bankSignatureCount");
    if (BigInteger.ZERO.equals(bankSignatureCount)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.bankSignatureCount must be nonzero");
    }
    normalizeNonZeroHex32(publicInputs.parentBankHash(), "proofResult.publicInputs.parentBankHash");
    normalizeNonZeroHex32(publicInputs.blockhash(), "proofResult.publicInputs.blockhash");
    normalizeNonZeroHex32(publicInputs.bankHash(), "proofResult.publicInputs.bankHash");
    normalizeNonZeroHex32(
        publicInputs.transactionStatusRoot(), "proofResult.publicInputs.transactionStatusRoot");
    normalizeNonZeroHex32(
        publicInputs.messageProofHash(), "proofResult.publicInputs.messageProofHash");
    normalizeNonZeroHex32(
        publicInputs.accountInclusionRoot(), "proofResult.publicInputs.accountInclusionRoot");
    normalizeNonZeroHex32(
        publicInputs.accountsLtHashChecksum(),
        "proofResult.publicInputs.accountsLtHashChecksum");
    normalizeNonZeroHex32(
        publicInputs.accountsLtHashProofPublicInputsHash(),
        "proofResult.publicInputs.accountsLtHashProofPublicInputsHash");
    normalizeNonZeroHex32(
        publicInputs.sourceEventDigest(), "proofResult.publicInputs.sourceEventDigest");
  }

  private static ProofResult requireWrappedProofResultForSubmission(
      final ProofResult proofResult, final SubmissionPublicInputs submissionPublicInputs) {
    Objects.requireNonNull(proofResult, "proofResult");
    Objects.requireNonNull(submissionPublicInputs, "publicInputs");
    if (proofResult.version() != 1) {
      throw new IllegalArgumentException("proofResult.version must be 1");
    }
    if (!RECURSIVE_PROOF_BACKEND_V1.equals(proofResult.backend())) {
      throw new IllegalArgumentException(
          "proofResult.backend must be sccp-solana-recursive-mainnet-v1");
    }
    final ProofContext proofContext =
        Objects.requireNonNull(proofResult.proofContext(), "proofResult.proofContext");
    if (proofContext.version() != 1) {
      throw new IllegalArgumentException("proofResult.proofContext.version must be 1");
    }
    final String expectedProofContextHash = proofContextHash(proofContext);
    if (!normalizeHex32(proofResult.proofContextHash(), "proofResult.proofContextHash")
        .equals(expectedProofContextHash)) {
      throw new IllegalArgumentException(
          "proofResult.proofContextHash must match statementHash and destinationBindingHash");
    }
    final byte[] proofBytes = proofResult.proofBytes();
    if (proofBytes.length == 0) {
      throw new IllegalArgumentException("proofResult.proofBytes must not be empty");
    }
    if (proofBytes.length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofResult.proofBytes must be at most "
              + NATIVE_RECURSIVE_MAX_PROOF_BYTES
              + " bytes");
    }
    if (!anyNonZero(proofBytes)) {
      throw new IllegalArgumentException("proofResult.proofBytes must not be all zero");
    }
    if (!Base64.getEncoder().encodeToString(proofBytes).equals(proofResult.proofBase64())) {
      throw new IllegalArgumentException(
          "proofResult.proofBase64 must match proofResult.proofBytes");
    }
    final String envelopeHash =
        normalizeHex32(proofResult.envelopeHash(), "proofResult.envelopeHash");
    if (ZERO_HASH_V1.equals(envelopeHash)) {
      throw new IllegalArgumentException("proofResult.envelopeHash must be non-zero");
    }
    final String sourceAdapterDeploymentBindingHash =
        normalizeHex32(
            proofResult.sourceAdapterDeploymentBindingHash(),
            "proofResult.sourceAdapterDeploymentBindingHash");
    if (ZERO_HASH_V1.equals(sourceAdapterDeploymentBindingHash)) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBindingHash must be non-zero");
    }
    final SourceAdapterDeploymentBinding deploymentBinding =
        Objects.requireNonNull(
            proofResult.sourceAdapterDeploymentBinding(),
            "proofResult.sourceAdapterDeploymentBinding");
    if (deploymentBinding.version() != 1) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBinding.version must be 1");
    }
    if (deploymentBinding.sourceDomain() != DOMAIN_SOLANA
        || deploymentBinding.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBinding must be Solana -> SORA");
    }
    final String deploymentHash =
        normalizeHex32(
            deploymentBinding.sourceAdapterDeploymentHash(),
            "proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash");
    final String deploymentReceiptHash =
        normalizeHex32(
            deploymentBinding.sourceAdapterDeploymentReceiptHash(),
            "proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash");
    if (ZERO_HASH_V1.equals(deploymentHash) || ZERO_HASH_V1.equals(deploymentReceiptHash)) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBinding deployment hashes must be non-zero");
    }
    final String expectedSourceAdapterDeploymentBindingHash =
        sourceAdapterDeploymentBindingHash(deploymentBinding);
    if (!sourceAdapterDeploymentBindingHash.equals(expectedSourceAdapterDeploymentBindingHash)) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding");
    }
    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    final String witnessHash =
        normalizeNonZeroHex32(proofResult.witnessHash(), "proofResult.witnessHash");
    write(payload, hex32Bytes(witnessHash, "proofResult.witnessHash"));
    write(payload, hex32Bytes(expectedProofContextHash, "proofResult.proofContextHash"));
    write(
        payload,
        hex32Bytes(
            sourceAdapterDeploymentBindingHash,
            "proofResult.sourceAdapterDeploymentBindingHash"));
    write(payload, proofBytes);
    if (!envelopeHash.equals(hashHex("sccp:solana:proof-envelope:v1", payload.toByteArray()))) {
      throw new IllegalArgumentException(
          "proofResult.envelopeHash must match wrapped proof bytes");
    }
    if (!MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1.equals(proofResult.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "proofResult.sourceStateVerifierId must match Solana AccountsDB verifier profile");
    }
    final String sourceStateVerifierHash =
        normalizeHex32(
            proofResult.sourceStateVerifierHash(), "proofResult.sourceStateVerifierHash");
    if (ZERO_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException("proofResult.sourceStateVerifierHash must be non-zero");
    }
    if (TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "proofResult.sourceStateVerifierHash must not be the Solana template verifier hash");
    }
    final PublicInputs resultPublicInputs =
        Objects.requireNonNull(proofResult.publicInputs(), "proofResult.publicInputs");
    if (!Objects.equals(resultPublicInputs.sourceStateVerifierId(), proofResult.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.sourceStateVerifierId must match proofResult.sourceStateVerifierId");
    }
    if (!normalizeHex32(
            resultPublicInputs.sourceStateVerifierHash(),
            "proofResult.publicInputs.sourceStateVerifierHash")
        .equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.sourceStateVerifierHash must match proofResult.sourceStateVerifierHash");
    }
    requireProofResultSourcePublicInputShape(resultPublicInputs);
    final String proofContextStatementHash =
        normalizeHex32(proofContext.statementHash(), "proofResult.proofContext.statementHash");
    final String proofContextDestinationBindingHash =
        normalizeHex32(
            proofContext.destinationBindingHash(),
            "proofResult.proofContext.destinationBindingHash");
    if (!normalizeHex32(
            resultPublicInputs.statementHash(), "proofResult.publicInputs.statementHash")
        .equals(proofContextStatementHash)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.statementHash must match proofContext");
    }
    if (!normalizeHex32(
            resultPublicInputs.destinationBindingHash(),
            "proofResult.publicInputs.destinationBindingHash")
        .equals(proofContextDestinationBindingHash)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.destinationBindingHash must match proofContext");
    }
    if (!normalizeHex32(
            resultPublicInputs.sourceAdapterDeploymentHash(),
            "proofResult.publicInputs.sourceAdapterDeploymentHash")
        .equals(deploymentHash)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.sourceAdapterDeploymentHash must match sourceAdapterDeploymentBinding");
    }
    if (!normalizeHex32(
            resultPublicInputs.sourceAdapterDeploymentReceiptHash(),
            "proofResult.publicInputs.sourceAdapterDeploymentReceiptHash")
        .equals(deploymentReceiptHash)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.sourceAdapterDeploymentReceiptHash must match sourceAdapterDeploymentBinding");
    }
    if (!normalizeHex32(
            resultPublicInputs.sourceAdapterDeploymentBindingHash(),
            "proofResult.publicInputs.sourceAdapterDeploymentBindingHash")
        .equals(expectedSourceAdapterDeploymentBindingHash)) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding");
    }
    if (!normalizeHex32(resultPublicInputs.messageId(), "proofResult.publicInputs.messageId")
        .equals(normalizeHex32(submissionPublicInputs.messageId(), "publicInputs.messageId"))) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.messageId must match publicInputs.messageId");
    }
    if (!normalizeHex32(resultPublicInputs.payloadHash(), "proofResult.publicInputs.payloadHash")
        .equals(normalizeHex32(submissionPublicInputs.payloadHash(), "publicInputs.payloadHash"))) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.payloadHash must match publicInputs.payloadHash");
    }
    if (!normalizeHex32(
            resultPublicInputs.commitmentRoot(), "proofResult.publicInputs.commitmentRoot")
        .equals(
            normalizeHex32(
                submissionPublicInputs.commitmentRoot(), "publicInputs.commitmentRoot"))) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.commitmentRoot must match publicInputs.commitmentRoot");
    }
    if (!normalizeU64(resultPublicInputs.finalizedSlot(), "proofResult.publicInputs.finalizedSlot")
        .equals(
            normalizeU64(
                submissionPublicInputs.finalityHeight(), "publicInputs.finalityHeight"))) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.finalizedSlot must match publicInputs.finalityHeight");
    }
    if (!normalizeHex32(resultPublicInputs.bankHash(), "proofResult.publicInputs.bankHash")
        .equals(
            normalizeHex32(
                submissionPublicInputs.finalityBlockHash(), "publicInputs.finalityBlockHash"))) {
      throw new IllegalArgumentException(
          "proofResult.publicInputs.bankHash must match publicInputs.finalityBlockHash");
    }
    return proofResult;
  }

  private static void requireCanonicalProofRequest(final ProofRequest request) {
    final Witness witness = request.witness();
    final ProofRequest expected =
        buildProofRequest(
            new WitnessInput(
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
                request.proofContext().statementHash(),
                request.proofContext().destinationBindingHash(),
                witness.inclusionBranch(),
                witness.sourceAdapterDeploymentHash(),
                witness.sourceAdapterDeploymentReceiptHash()));
    if (request.version() != expected.version()
        || !Objects.equals(request.backend(), expected.backend())
        || request.sourceDomain() != expected.sourceDomain()
        || request.targetDomain() != expected.targetDomain()
        || !Objects.equals(request.mainnetGenesisHash(), expected.mainnetGenesisHash())
        || !Objects.equals(request.witnessHash(), expected.witnessHash())
        || !Objects.equals(request.proofContextHash(), expected.proofContextHash())
        || !Objects.equals(
            request.sourceAdapterDeploymentBindingHash(),
            expected.sourceAdapterDeploymentBindingHash())
        || !Objects.equals(request.sourceStateVerifierId(), expected.sourceStateVerifierId())
        || !Objects.equals(request.sourceStateVerifierHash(), expected.sourceStateVerifierHash())
        || !Objects.equals(request.publicInputs(), expected.publicInputs())
        || !Arrays.equals(
            canonicalWitnessBytes(request.witness()), canonicalWitnessBytes(expected.witness()))
        || !Objects.equals(request.proofContext(), expected.proofContext())
        || !Objects.equals(
            request.sourceAdapterDeploymentBinding(), expected.sourceAdapterDeploymentBinding())) {
      throw new IllegalArgumentException("proof request must be canonical");
    }
  }

  private static void requireProductionProofRequest(final ProofRequest request) {
    if (request.sourceDomain() != DOMAIN_SOLANA || request.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("Solana SCCP production proofs must target SORA");
    }
    if (!MAINNET_GENESIS_HASH.equals(request.mainnetGenesisHash())
        || !MAINNET_GENESIS_HASH.equals(request.witness().mainnetGenesisHash())) {
      throw new IllegalArgumentException("mainnetGenesisHash must match Solana mainnet-beta");
    }
    if (!MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1.equals(request.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "sourceStateVerifierId must match Solana AccountsDB verifier profile");
    }
    final String sourceStateVerifierHash =
        normalizeHex32(request.sourceStateVerifierHash(), "sourceStateVerifierHash");
    if (ZERO_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be zero for Solana production proofs");
    }
    if (TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be the Solana template verifier hash");
    }
    if (request.witness().inclusionBranch().length == 0) {
      throw new IllegalArgumentException(
          "inclusionBranch must not be empty for Solana production proofs");
    }
    final byte[] accountsLtHash = request.witness().accountsLtHash();
    if (accountsLtHash == null) {
      throw new IllegalArgumentException(
          "accountsLtHash must be present for Solana production proofs");
    }
    if (accountsLtHash.length != ACCOUNTS_LT_HASH_BYTES) {
      throw new IllegalArgumentException("accountsLtHash must be 2048 bytes");
    }
    if (!anyNonZero(accountsLtHash)) {
      throw new IllegalArgumentException("accountsLtHash must not be zero");
    }
    if (ZERO_HASH_V1.equals(
        normalizeHex32(
            request.sourceAdapterDeploymentBinding().sourceAdapterDeploymentHash(),
            "sourceAdapterDeploymentHash"))) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentHash must not be zero for Solana production proofs");
    }
    if (ZERO_HASH_V1.equals(
        normalizeHex32(
            request.sourceAdapterDeploymentBinding().sourceAdapterDeploymentReceiptHash(),
            "sourceAdapterDeploymentReceiptHash"))) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentReceiptHash must not be zero for Solana production proofs");
    }
  }

  private record FullLightClientAuditRoleProfile(
      String name, int code, String circuitId, String verifierId, List<String> requiredInputNames) {}

  private record NormalizedFullLightClientAuditContext(
      int version,
      BigInteger epoch,
      BigInteger rootedSlot,
      BigInteger parentSlot,
      List<BigInteger> towerVoteSlots,
      String parentBankHash,
      BigInteger bankSignatureCount,
      byte[] bankHashHardForkData,
      String epochStakeRoot,
      String stakeActivationHash,
      String stakeAccountStateHash,
      String stakeHistoryHash,
      String stakeHistorySysvarAccountHash,
      String accountInclusionRoot,
      String accountsLtHashChecksum,
      String accountsLtHashProofPublicInputsHash,
      String towerLockoutHash,
      String towerReplayHash,
      String bankForkHash) {
    private NormalizedFullLightClientAuditContext {
      towerVoteSlots = Collections.unmodifiableList(new ArrayList<>(towerVoteSlots));
      bankHashHardForkData = Arrays.copyOf(bankHashHardForkData, bankHashHardForkData.length);
    }

    @Override
    public byte[] bankHashHardForkData() {
      return Arrays.copyOf(bankHashHardForkData, bankHashHardForkData.length);
    }
  }

  private record NormalizedFullLightClientAuditInput(
      FullLightClientAuditRole role,
      Witness witness,
      NormalizedFullLightClientAuditContext context,
      String sourceVerifierMaterialHash,
      String sourceAdapterDeploymentHash,
      String fullLightClientGateHash,
      String verifierHash,
      String finalityContextHash,
      String voteMessageHash,
      String accountsLtHashProofHash,
      String openedAccountsLtHashContributionsHash,
      String openedAccountsLtHashResidualChecksum) {}

  private static FullLightClientAuditRoleProfile auditRoleProfile(final FullLightClientAuditRole role) {
    Objects.requireNonNull(role, "role");
    switch (role) {
      case TOWER_REPLAY:
        return new FullLightClientAuditRoleProfile(
            "tower_replay",
            1,
            TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1,
            MAINNET_TOWER_REPLAY_VERIFIER_ID_V1,
            Arrays.asList(
                "tower_lockout_hash",
                "tower_replay_hash",
                "bank_fork_hash",
                "epoch_stake_root",
                "stake_activation_hash",
                "stake_account_state_hash",
                "stake_history_hash",
                "stake_history_sysvar_account_hash",
                "account_inclusion_root"));
      case FULL_ACCOUNTSDB_LATTICE:
        return new FullLightClientAuditRoleProfile(
            "full_accountsdb_lattice",
            2,
            FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1,
            MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1,
            Arrays.asList(
                "account_inclusion_root",
                "accounts_lt_hash_checksum",
                "accounts_lt_hash_proof_public_inputs_hash",
                "opened_accounts_lt_hash_contributions_hash",
                "opened_accounts_lt_hash_residual_checksum",
                "accounts_lt_hash_proof_hash"));
      case BANK_FORK_CHOICE:
        return new FullLightClientAuditRoleProfile(
            "bank_fork_choice",
            3,
            BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1,
            MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1,
            Arrays.asList(
                "parent_bank_hash",
                "bank_hash",
                "blockhash",
                "transaction_status_root",
                "account_inclusion_root",
                "accounts_lt_hash_checksum",
                "bank_signature_count",
                "bank_hash_hard_fork_data_hash",
                "bank_fork_hash",
                "tower_replay_hash"));
      default:
        throw new IllegalArgumentException("unsupported Solana full light-client audit role");
    }
  }

  private static String roleVerifierHash(
      final FullLightClientAuditProofInput input, final FullLightClientAuditRole role) {
    switch (role) {
      case TOWER_REPLAY:
        return normalizeNonZeroHex32(
            input.solanaTowerReplayVerifierHash(), "solanaTowerReplayVerifierHash");
      case FULL_ACCOUNTSDB_LATTICE:
        return normalizeNonZeroHex32(
            input.solanaFullAccountsdbLatticeVerifierHash(),
            "solanaFullAccountsdbLatticeVerifierHash");
      case BANK_FORK_CHOICE:
        return normalizeNonZeroHex32(
            input.solanaBankForkChoiceVerifierHash(), "solanaBankForkChoiceVerifierHash");
      default:
        throw new IllegalArgumentException("unsupported Solana full light-client audit role");
    }
  }

  private static void requireFullLightClientAuditRoleSeparation(
      final FullLightClientAuditProofInput input, final Witness witness) {
    final List<String> auditHashes =
        Arrays.asList(
            roleVerifierHash(input, FullLightClientAuditRole.TOWER_REPLAY),
            roleVerifierHash(input, FullLightClientAuditRole.FULL_ACCOUNTSDB_LATTICE),
            roleVerifierHash(input, FullLightClientAuditRole.BANK_FORK_CHOICE));
    final Set<String> seen = new HashSet<>();
    for (final String auditHash : auditHashes) {
      if (!seen.add(auditHash)) {
        throw new IllegalArgumentException(
            "Solana full-light-client audit verifier hashes must be role-separated");
      }
      if (TEMPLATE_SOURCE_MATERIAL_HASHES_V1.contains(auditHash)) {
        throw new IllegalArgumentException(
            "Solana full-light-client audit verifier hashes must not reuse built-in template material");
      }
    }
    final List<String> existingHashes =
        Arrays.asList(
            normalizeNonZeroHex32(input.sourceTrustAnchorHash(), "sourceTrustAnchorHash"),
            normalizeNonZeroHex32(input.consensusVerifierHash(), "consensusVerifierHash"),
            normalizeNonZeroHex32(
                input.messageInclusionVerifierHash(), "messageInclusionVerifierHash"),
            normalizeNonZeroHex32(input.finalityPolicyHash(), "finalityPolicyHash"),
            witness.sourceStateVerifierHash(),
            normalizeNonZeroHex32(
                input.adapterVerifierVkHash() == null
                    ? SourceSccpProofs.sourceAdapterVerifierVkHash(DOMAIN_SOLANA)
                    : input.adapterVerifierVkHash(),
                "adapterVerifierVkHash"),
            witness.sourceAdapterDeploymentReceiptHash());
    for (final String auditHash : auditHashes) {
      for (final String existingHash : existingHashes) {
        if (!ZERO_HASH_V1.equals(existingHash) && auditHash.equals(existingHash)) {
          throw new IllegalArgumentException(
              "Solana full-light-client audit verifier hashes must not reuse existing source-adapter material");
        }
      }
    }
  }

  private static String fullLightClientGateHashFromBoundHashes(
      final String sourceVerifierMaterialHash,
      final String sourceAdapterDeploymentHash,
      final String towerReplayVerifierHash,
      final String fullAccountsdbLatticeVerifierHash,
      final String bankForkChoiceVerifierHash) {
    final String[] verifierIds = {
      MAINNET_TOWER_REPLAY_VERIFIER_ID_V1,
      MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1,
      MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1
    };
    final byte[][] verifierHashes = {
      hex32Bytes(
          normalizeNonZeroHex32(towerReplayVerifierHash, "solanaTowerReplayVerifierHash"),
          "solanaTowerReplayVerifierHash"),
      hex32Bytes(
          normalizeNonZeroHex32(
              fullAccountsdbLatticeVerifierHash, "solanaFullAccountsdbLatticeVerifierHash"),
          "solanaFullAccountsdbLatticeVerifierHash"),
      hex32Bytes(
          normalizeNonZeroHex32(bankForkChoiceVerifierHash, "solanaBankForkChoiceVerifierHash"),
          "solanaBankForkChoiceVerifierHash")
    };
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, DOMAIN_SOLANA);
    writeU32Le(out, DOMAIN_SORA);
    writeVec(out, SOLANA_SOURCE_CHAIN_KEY_V1.getBytes(StandardCharsets.UTF_8));
    out.write(SOLANA_SOURCE_PROOF_PLAN_CODE_V1);
    out.write(SOLANA_FINALITY_MODEL_CODE_V1);
    writeVec(out, MAINNET_GENESIS_HASH.getBytes(StandardCharsets.UTF_8));
    write(
        out,
        hex32Bytes(
            normalizeNonZeroHex32(sourceVerifierMaterialHash, "sourceVerifierMaterialHash"),
            "sourceVerifierMaterialHash"));
    write(
        out,
        hex32Bytes(
            normalizeNonZeroHex32(sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"),
            "sourceAdapterDeploymentHash"));
    for (int i = 0; i < verifierIds.length; i++) {
      writeVec(out, verifierIds[i].getBytes(StandardCharsets.UTF_8));
      write(out, verifierHashes[i]);
    }
    return hashHex("sccp:solana:full-light-client-gate:v1", out.toByteArray());
  }

  private static NormalizedFullLightClientAuditInput normalizeFullLightClientAuditInput(
      final FullLightClientAuditProofInput input, final FullLightClientAuditRole role) {
    Objects.requireNonNull(input, "input");
    final Witness witness =
        normalizeWitness(
            witnessInputWithOpenedAccountsLtHash(input.witnessInput(), input.openedAccounts()));
    if (witness.sourceDomain() != DOMAIN_SOLANA || witness.targetDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("Solana audit requests require a Solana -> SORA witness");
    }
    if (!MAINNET_GENESIS_HASH.equals(witness.mainnetGenesisHash())) {
      throw new IllegalArgumentException("mainnetGenesisHash must match Solana mainnet-beta");
    }
    if (ZERO_HASH_V1.equals(witness.sourceStateVerifierHash())) {
      throw new IllegalArgumentException("sourceStateVerifierHash must not be zero");
    }
    if (TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1.equals(witness.sourceStateVerifierHash())) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be the Solana template verifier hash");
    }
    requireFullLightClientAuditRoleSeparation(input, witness);
    final String sourceAdapterDeploymentReceiptHash =
        normalizeNonZeroHex32(
            input.sourceAdapterDeploymentReceiptHash(), "sourceAdapterDeploymentReceiptHash");
    if (!sourceAdapterDeploymentReceiptHash.equals(witness.sourceAdapterDeploymentReceiptHash())) {
      throw new IllegalArgumentException("sourceAdapterDeploymentReceiptHash must match witness");
    }
    final String sourceVerifierMaterialHash =
        SourceSccpProofs.sourceVerifierMaterialHash(
            DOMAIN_SOLANA,
            input.sourceTrustAnchorHash(),
            input.consensusVerifierHash(),
            input.messageInclusionVerifierHash(),
            input.finalityPolicyHash(),
            witness.sourceStateVerifierHash(),
            null,
            null,
            null,
            null,
            null);
    if (input.sourceVerifierMaterialHash() != null
        && !normalizeHex32(input.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash")
            .equals(sourceVerifierMaterialHash)) {
      throw new IllegalArgumentException("sourceVerifierMaterialHash must match sourceVerifierMaterial");
    }
    final String sourceAdapterDeploymentHash =
        SourceSccpProofs.sourceAdapterEngineDeploymentHash(
            DOMAIN_SOLANA,
            input.sourceTrustAnchorHash(),
            input.consensusVerifierHash(),
            input.messageInclusionVerifierHash(),
            input.finalityPolicyHash(),
            sourceAdapterDeploymentReceiptHash,
            DOMAIN_SORA,
            input.adapterVerifierVkHash(),
            witness.sourceStateVerifierHash(),
            null,
            null,
            null,
            null,
            null,
            input.solanaTowerReplayVerifierHash(),
            input.solanaFullAccountsdbLatticeVerifierHash(),
            input.solanaBankForkChoiceVerifierHash());
    if (input.sourceAdapterDeploymentHash() != null
        && !normalizeHex32(input.sourceAdapterDeploymentHash(), "sourceAdapterDeploymentHash")
            .equals(sourceAdapterDeploymentHash)) {
      throw new IllegalArgumentException("sourceAdapterDeploymentHash must match sourceAdapterDeployment");
    }
    final String fullLightClientGateHash =
        SourceSccpProofs.solanaFullLightClientGateHash(
            DOMAIN_SOLANA,
            input.sourceTrustAnchorHash(),
            input.consensusVerifierHash(),
            input.messageInclusionVerifierHash(),
            input.finalityPolicyHash(),
            sourceAdapterDeploymentReceiptHash,
            input.solanaTowerReplayVerifierHash(),
            input.solanaFullAccountsdbLatticeVerifierHash(),
            input.solanaBankForkChoiceVerifierHash(),
            DOMAIN_SORA,
            input.adapterVerifierVkHash(),
            witness.sourceStateVerifierHash(),
            null,
            null,
            null,
            null,
            null);
    if (input.fullLightClientGateHash() != null
        && !normalizeHex32(input.fullLightClientGateHash(), "fullLightClientGateHash")
            .equals(fullLightClientGateHash)) {
      throw new IllegalArgumentException(
          "fullLightClientGateHash must match the bound Solana full-light-client audit verifier hashes");
    }
    if (!sourceAdapterDeploymentHash.equals(witness.sourceAdapterDeploymentHash())) {
      throw new IllegalArgumentException("sourceAdapterDeploymentHash must match witness");
    }
    final NormalizedFullLightClientAuditContext context =
        normalizeFullLightClientAuditContext(input, witness);
    final String finalityContextHash =
        hashHex("sccp:solana:finality-context:v1", canonicalFinalityContextBytes(context));
    if (input.finalityContextHash() != null
        && !normalizeHex32(input.finalityContextHash(), "finalityContextHash")
            .equals(finalityContextHash)) {
      throw new IllegalArgumentException("finalityContextHash must match finality context fields");
    }
    final String voteMessageHash =
        hashHex("sccp:solana:finalized-vote:v1", canonicalVoteMessageBytes(witness, finalityContextHash));
    if (input.voteMessageHash() != null
        && !normalizeHex32(input.voteMessageHash(), "voteMessageHash").equals(voteMessageHash)) {
      throw new IllegalArgumentException(
          "voteMessageHash must match finality context and message proof");
    }
    final String accountsLtHashProofHash = accountsLtHashProofHash(input.accountsLtHashProof());
    if (input.accountsLtHashProofHash() != null
        && !normalizeHex32(input.accountsLtHashProofHash(), "accountsLtHashProofHash")
            .equals(accountsLtHashProofHash)) {
      throw new IllegalArgumentException("accountsLtHashProofHash must match accountsLtHashProof");
    }
    final String openedContributionsHash =
        openedAccountsLtHashContributionsHash(input.openedAccounts());
    final String residualChecksum = openedAccountsLtHashResidualChecksum(input.openedAccounts());
    if (input.openedAccountsLtHashContributionsHash() != null
        && !normalizeHex32(
                input.openedAccountsLtHashContributionsHash(),
                "openedAccountsLtHashContributionsHash")
            .equals(openedContributionsHash)) {
      throw new IllegalArgumentException(
          "openedAccountsLtHashContributionsHash must match opened AccountsLtHash inputs");
    }
    if (input.openedAccountsLtHashResidualChecksum() != null
        && !normalizeHex32(
                input.openedAccountsLtHashResidualChecksum(),
                "openedAccountsLtHashResidualChecksum")
            .equals(residualChecksum)) {
      throw new IllegalArgumentException(
          "openedAccountsLtHashResidualChecksum must match opened AccountsLtHash inputs");
    }
    return new NormalizedFullLightClientAuditInput(
        role,
        witness,
        context,
        sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash,
        fullLightClientGateHash,
        roleVerifierHash(input, role),
        finalityContextHash,
        voteMessageHash,
        accountsLtHashProofHash,
        openedContributionsHash,
        residualChecksum);
  }

  private static NormalizedFullLightClientAuditContext normalizeFullLightClientAuditContext(
      final FullLightClientAuditProofInput input, final Witness witness) {
    final BigInteger epoch =
        input.epoch() == null
            ? normalizeU64(mainnetEpochForSlot(witness.finalizedSlot()), "epoch")
            : normalizeU64(input.epoch(), "epoch");
    if (!epoch.equals(normalizeU64(mainnetEpochForSlot(witness.finalizedSlot()), "epoch"))) {
      throw new IllegalArgumentException("epoch must match Solana mainnet finalizedSlot");
    }
    final BigInteger rooted = normalizeU64(input.rootedSlot(), "rootedSlot");
    final BigInteger parent = normalizeU64(witness.parentSlot(), "parentSlot");
    final List<BigInteger> towerVoteSlots = new ArrayList<>();
    for (int i = 0; i < input.towerVoteSlots().length; i++) {
      towerVoteSlots.add(normalizeU64(input.towerVoteSlots()[i], "towerVoteSlots[" + i + "]"));
    }
    final String bankForkHash =
        bankForkHash(
            witness.finalizedSlot(),
            witness.parentSlot(),
            witness.bankSignatureCount(),
            witness.parentBankHash(),
            witness.bankHash(),
            witness.blockhash(),
            witness.accountsLtHash(),
            witness.bankHashHardForkData(),
            witness.transactionStatusRoot(),
            witness.accountInclusionRoot(),
            witness.accountsLtHashChecksum(),
            epoch.toString());
    final String towerLockoutHash =
        towerLockoutHash(
            witness.finalizedSlot(),
            rooted.toString(),
            witness.parentSlot(),
            witness.parentBankHash(),
            epoch.toString());
    final String[] towerVoteSlotStrings = new String[towerVoteSlots.size()];
    for (int i = 0; i < towerVoteSlots.size(); i++) {
      towerVoteSlotStrings[i] = towerVoteSlots.get(i).toString();
    }
    final String towerReplayHash =
        towerReplayHash(
            witness.finalizedSlot(),
            rooted.toString(),
            witness.parentSlot(),
            bankForkHash,
            towerVoteSlotStrings,
            epoch.toString());
    if (input.towerLockoutHash() != null
        && !normalizeHex32(input.towerLockoutHash(), "towerLockoutHash").equals(towerLockoutHash)) {
      throw new IllegalArgumentException("towerLockoutHash must match finality context fields");
    }
    if (input.towerReplayHash() != null
        && !normalizeHex32(input.towerReplayHash(), "towerReplayHash").equals(towerReplayHash)) {
      throw new IllegalArgumentException("towerReplayHash must match finality context fields");
    }
    if (input.bankForkHash() != null
        && !normalizeHex32(input.bankForkHash(), "bankForkHash").equals(bankForkHash)) {
      throw new IllegalArgumentException("bankForkHash must match finality context fields");
    }
    return new NormalizedFullLightClientAuditContext(
        1,
        epoch,
        rooted,
        parent,
        towerVoteSlots,
        witness.parentBankHash(),
        normalizeU64(witness.bankSignatureCount(), "bankSignatureCount"),
        witness.bankHashHardForkData(),
        normalizeNonZeroHex32(input.epochStakeRoot(), "epochStakeRoot"),
        normalizeNonZeroHex32(input.stakeActivationHash(), "stakeActivationHash"),
        normalizeNonZeroHex32(input.stakeAccountStateHash(), "stakeAccountStateHash"),
        normalizeNonZeroHex32(input.stakeHistoryHash(), "stakeHistoryHash"),
        normalizeNonZeroHex32(
            input.stakeHistorySysvarAccountHash(), "stakeHistorySysvarAccountHash"),
        witness.accountInclusionRoot(),
        witness.accountsLtHashChecksum(),
        witness.accountsLtHashProofPublicInputsHash(),
        towerLockoutHash,
        towerReplayHash,
        bankForkHash);
  }

  private static byte[] canonicalFinalityContextBytes(
      final NormalizedFullLightClientAuditContext context) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(context.version());
    writeU64Le(out, context.epoch());
    writeU64Le(out, context.rootedSlot());
    writeU64Le(out, context.parentSlot());
    writeU32Le(out, context.towerVoteSlots().size());
    for (final BigInteger slot : context.towerVoteSlots()) {
      writeU64Le(out, slot);
    }
    write(out, hex32Bytes(context.parentBankHash(), "parentBankHash"));
    writeU64Le(out, context.bankSignatureCount());
    writeVec(out, context.bankHashHardForkData());
    write(out, hex32Bytes(context.epochStakeRoot(), "epochStakeRoot"));
    write(out, hex32Bytes(context.stakeActivationHash(), "stakeActivationHash"));
    write(out, hex32Bytes(context.stakeAccountStateHash(), "stakeAccountStateHash"));
    write(out, hex32Bytes(context.stakeHistoryHash(), "stakeHistoryHash"));
    write(
        out,
        hex32Bytes(context.stakeHistorySysvarAccountHash(), "stakeHistorySysvarAccountHash"));
    write(out, hex32Bytes(context.accountInclusionRoot(), "accountInclusionRoot"));
    write(out, hex32Bytes(context.accountsLtHashChecksum(), "accountsLtHashChecksum"));
    write(
        out,
        hex32Bytes(
            context.accountsLtHashProofPublicInputsHash(),
            "accountsLtHashProofPublicInputsHash"));
    write(out, hex32Bytes(context.towerLockoutHash(), "towerLockoutHash"));
    write(out, hex32Bytes(context.towerReplayHash(), "towerReplayHash"));
    write(out, hex32Bytes(context.bankForkHash(), "bankForkHash"));
    return out.toByteArray();
  }

  private static byte[] canonicalVoteMessageBytes(
      final Witness witness, final String finalityContextHash) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, DOMAIN_SOLANA);
    writeU64Le(out, normalizeU64(witness.finalizedSlot(), "finalizedSlot"));
    write(out, hex32Bytes(witness.blockhash(), "blockhash"));
    write(out, hex32Bytes(witness.bankHash(), "bankHash"));
    write(out, hex32Bytes(witness.transactionStatusRoot(), "transactionStatusRoot"));
    write(out, hex32Bytes(witness.messageProofHash(), "messageProofHash"));
    write(out, hex32Bytes(finalityContextHash, "finalityContextHash"));
    return out.toByteArray();
  }

  private static List<String> auditRoleColumns(
      final NormalizedFullLightClientAuditInput value, final FullLightClientAuditRole role) {
    switch (role) {
      case TOWER_REPLAY:
        return Arrays.asList(
            value.context().towerLockoutHash(),
            value.context().towerReplayHash(),
            value.context().bankForkHash(),
            value.context().epochStakeRoot(),
            value.context().stakeActivationHash(),
            value.context().stakeAccountStateHash(),
            value.context().stakeHistoryHash(),
            value.context().stakeHistorySysvarAccountHash(),
            value.context().accountInclusionRoot());
      case FULL_ACCOUNTSDB_LATTICE:
        return Arrays.asList(
            value.context().accountInclusionRoot(),
            value.context().accountsLtHashChecksum(),
            value.context().accountsLtHashProofPublicInputsHash(),
            value.openedAccountsLtHashContributionsHash(),
            value.openedAccountsLtHashResidualChecksum(),
            value.accountsLtHashProofHash());
      case BANK_FORK_CHOICE:
        return Arrays.asList(
            value.context().parentBankHash(),
            value.witness().bankHash(),
            value.witness().blockhash(),
            value.witness().transactionStatusRoot(),
            value.context().accountInclusionRoot(),
            value.context().accountsLtHashChecksum(),
            "0x" + hexLower(sccpWordU64Le(value.context().bankSignatureCount())),
            hashHex(BANK_HASH_HARD_FORK_DATA_PREFIX_V1, value.context().bankHashHardForkData()),
            value.context().bankForkHash(),
            value.context().towerReplayHash());
      default:
        throw new IllegalArgumentException("unsupported Solana full light-client audit role");
    }
  }

  private static FullLightClientAuditFastpqPublicInputs fullLightClientAuditFastpqPublicInputs(
      final NormalizedFullLightClientAuditInput value, final String statementHash) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(auditRoleProfile(value.role()).code());
    write(out, hex32Bytes(statementHash, "auditStatementHash"));
    final byte[] dsidHash =
        hashBytes(FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1, out.toByteArray());
    final String oldRoot;
    final String newRoot;
    final String permRoot;
    switch (value.role()) {
      case TOWER_REPLAY:
        oldRoot = value.context().towerLockoutHash();
        newRoot = value.context().towerReplayHash();
        permRoot = value.context().bankForkHash();
        break;
      case FULL_ACCOUNTSDB_LATTICE:
        oldRoot = value.context().accountInclusionRoot();
        newRoot = value.context().accountsLtHashChecksum();
        permRoot = value.openedAccountsLtHashContributionsHash();
        break;
      case BANK_FORK_CHOICE:
        oldRoot = value.context().parentBankHash();
        newRoot = value.witness().bankHash();
        permRoot = value.context().bankForkHash();
        break;
      default:
        throw new IllegalArgumentException("unsupported Solana full light-client audit role");
    }
    return new FullLightClientAuditFastpqPublicInputs(
        "0x" + hexLower(Arrays.copyOfRange(dsidHash, 0, 16)),
        value.witness().finalizedSlot(),
        oldRoot,
        newRoot,
        permRoot,
        statementHash);
  }

  private static byte[] canonicalFullLightClientAuditContextBytes(
      final NormalizedFullLightClientAuditInput value, final String statementHash) {
    final FullLightClientAuditRoleProfile profile = auditRoleProfile(value.role());
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(profile.code());
    writeString(out, profile.circuitId(), "circuitId");
    writeString(out, FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1, "parameterSet");
    writeString(out, profile.verifierId(), "verifierId");
    write(out, hex32Bytes(value.verifierHash(), "verifierHash"));
    write(out, hex32Bytes(value.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash"));
    write(out, hex32Bytes(value.sourceAdapterDeploymentHash(), "sourceAdapterDeploymentHash"));
    write(out, hex32Bytes(value.fullLightClientGateHash(), "fullLightClientGateHash"));
    write(out, hex32Bytes(value.finalityContextHash(), "finalityContextHash"));
    write(out, hex32Bytes(statementHash, "auditStatementHash"));
    return out.toByteArray();
  }

  private static byte[] fullLightClientAuditFastpqKey(
      final String prefix, final FullLightClientAuditRoleProfile profile) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] circuitBytes = profile.circuitId().getBytes(StandardCharsets.UTF_8);
    final byte[] out = new byte[prefixBytes.length + 1 + circuitBytes.length];
    System.arraycopy(prefixBytes, 0, out, 0, prefixBytes.length);
    out[prefixBytes.length] = 0;
    System.arraycopy(circuitBytes, 0, out, prefixBytes.length + 1, circuitBytes.length);
    return out;
  }

  private static byte[] sccpWordU8(final int value) {
    if (value < 0 || value > 0xff) {
      throw new IllegalArgumentException("u8 value out of range");
    }
    final byte[] out = new byte[32];
    out[0] = (byte) value;
    return out;
  }

  private record OpenedLtHashContributionRow(
      int role,
      byte[] address,
      byte[] accountHash,
      byte[] rawDataHash,
      byte[] accountLtHash) {
    private OpenedLtHashContributionRow {
      address = Arrays.copyOf(address, address.length);
      accountHash = Arrays.copyOf(accountHash, accountHash.length);
      rawDataHash = Arrays.copyOf(rawDataHash, rawDataHash.length);
      accountLtHash = Arrays.copyOf(accountLtHash, accountLtHash.length);
    }

    @Override
    public byte[] address() {
      return Arrays.copyOf(address, address.length);
    }

    @Override
    public byte[] accountHash() {
      return Arrays.copyOf(accountHash, accountHash.length);
    }

    @Override
    public byte[] rawDataHash() {
      return Arrays.copyOf(rawDataHash, rawDataHash.length);
    }

    @Override
    public byte[] accountLtHash() {
      return Arrays.copyOf(accountLtHash, accountLtHash.length);
    }
  }

  private record NormalizedOpenedAccountsLtHashContributions(
      int sourceDomain,
      BigInteger finalizedSlot,
      byte[] accountInclusionRoot,
      byte[] accountsLtHashChecksum,
      List<OpenedLtHashContributionRow> rows,
      byte[] openedAccountsLtHash,
      byte[] openedAccountsLtHashChecksum,
      byte[] residualAccountsLtHash,
      byte[] residualAccountsLtHashChecksum) {
    private NormalizedOpenedAccountsLtHashContributions {
      accountInclusionRoot = Arrays.copyOf(accountInclusionRoot, accountInclusionRoot.length);
      accountsLtHashChecksum = Arrays.copyOf(accountsLtHashChecksum, accountsLtHashChecksum.length);
      rows = Collections.unmodifiableList(new ArrayList<>(rows));
      openedAccountsLtHash = Arrays.copyOf(openedAccountsLtHash, openedAccountsLtHash.length);
      openedAccountsLtHashChecksum =
          Arrays.copyOf(openedAccountsLtHashChecksum, openedAccountsLtHashChecksum.length);
      residualAccountsLtHash = Arrays.copyOf(residualAccountsLtHash, residualAccountsLtHash.length);
      residualAccountsLtHashChecksum =
          Arrays.copyOf(residualAccountsLtHashChecksum, residualAccountsLtHashChecksum.length);
    }

    @Override
    public byte[] accountInclusionRoot() {
      return Arrays.copyOf(accountInclusionRoot, accountInclusionRoot.length);
    }

    @Override
    public byte[] accountsLtHashChecksum() {
      return Arrays.copyOf(accountsLtHashChecksum, accountsLtHashChecksum.length);
    }

    @Override
    public byte[] openedAccountsLtHash() {
      return Arrays.copyOf(openedAccountsLtHash, openedAccountsLtHash.length);
    }

    @Override
    public byte[] openedAccountsLtHashChecksum() {
      return Arrays.copyOf(openedAccountsLtHashChecksum, openedAccountsLtHashChecksum.length);
    }

    @Override
    public byte[] residualAccountsLtHash() {
      return Arrays.copyOf(residualAccountsLtHash, residualAccountsLtHash.length);
    }

    @Override
    public byte[] residualAccountsLtHashChecksum() {
      return Arrays.copyOf(residualAccountsLtHashChecksum, residualAccountsLtHashChecksum.length);
    }
  }

  private static NormalizedOpenedAccountsLtHashContributions normalizeOpenedAccountsLtHashContributions(
      final OpenedAccountsLtHashContributionsInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_SOLANA) {
      throw new IllegalArgumentException("sourceDomain must be Solana");
    }
    final BigInteger finalizedSlot = normalizeU64(input.finalizedSlot(), "finalizedSlot");
    final byte[] accountInclusionRoot =
        hex32Bytes(
            normalizeNonZeroHex32(input.accountInclusionRoot(), "accountInclusionRoot"),
            "accountInclusionRoot");
    final byte[] accountsLtHashChecksum =
        hex32Bytes(
            normalizeNonZeroHex32(input.accountsLtHashChecksum(), "accountsLtHashChecksum"),
            "accountsLtHashChecksum");
    final byte[] accountsLtHash = input.accountsLtHash();
    if (accountsLtHash.length != ACCOUNTS_LT_HASH_BYTES) {
      throw new IllegalArgumentException("accountsLtHash must be 2048 bytes");
    }
    if (!anyNonZero(accountsLtHash)) {
      throw new IllegalArgumentException("accountsLtHash must not be zero");
    }
    if (!Arrays.equals(Blake3.hash(accountsLtHash), accountsLtHashChecksum)) {
      throw new IllegalArgumentException("accountsLtHashChecksum must match accountsLtHash");
    }
    final List<OpenedLtHashContributionRow> rows = openedLtHashContributionRows(input);
    final byte[] openedAccountsLtHash = new byte[ACCOUNTS_LT_HASH_BYTES];
    for (final OpenedLtHashContributionRow row : rows) {
      addAccountsLtHashContribution(openedAccountsLtHash, row.accountLtHash());
    }
    final byte[] openedAccountsLtHashChecksum = Blake3.hash(openedAccountsLtHash);
    final byte[] residualAccountsLtHash = Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    subtractAccountsLtHashContribution(residualAccountsLtHash, openedAccountsLtHash);
    if (!anyNonZero(residualAccountsLtHash)) {
      throw new IllegalArgumentException("openedAccountsLtHashResidual must not be zero");
    }
    final byte[] residualAccountsLtHashChecksum = Blake3.hash(residualAccountsLtHash);
    return new NormalizedOpenedAccountsLtHashContributions(
        input.sourceDomain(),
        finalizedSlot,
        accountInclusionRoot,
        accountsLtHashChecksum,
        rows,
        openedAccountsLtHash,
        openedAccountsLtHashChecksum,
        residualAccountsLtHash,
        residualAccountsLtHashChecksum);
  }

  private record NormalizedAccountsLtHashProofRequest(
      Witness witness,
      NormalizedOpenedAccountsLtHashContributions opened,
      byte[] accountsLtHash,
      String openedContributionsHash,
      String residualChecksum) {
    private NormalizedAccountsLtHashProofRequest {
      accountsLtHash = Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    }

    @Override
    public byte[] accountsLtHash() {
      return Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    }
  }

  private static NormalizedAccountsLtHashProofRequest normalizeAccountsLtHashProofRequest(
      final WitnessInput witnessInput, final OpenedAccountsLtHashContributionsInput openedInput) {
    Objects.requireNonNull(witnessInput, "witnessInput");
    Objects.requireNonNull(openedInput, "openedInput");
    if (!MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1.equals(witnessInput.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "sourceStateVerifierId must match Solana AccountsDB verifier profile");
    }
    final String sourceStateVerifierHash =
        normalizeHex32(witnessInput.sourceStateVerifierHash(), "sourceStateVerifierHash");
    if (ZERO_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException("sourceStateVerifierHash must not be zero");
    }
    if (TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1.equals(sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be the Solana template verifier hash");
    }
    final WitnessInput withAccountsLtHash =
        witnessInputWithOpenedAccountsLtHash(witnessInput, openedInput);
    final Witness witness = normalizeWitness(withAccountsLtHash);
    final NormalizedOpenedAccountsLtHashContributions opened =
        normalizeOpenedAccountsLtHashContributions(openedInput);
    if (!normalizeU64(witness.finalizedSlot(), "finalizedSlot").equals(opened.finalizedSlot())) {
      throw new IllegalArgumentException("opened finalizedSlot must match witness");
    }
    if (!Arrays.equals(
        hex32Bytes(witness.accountInclusionRoot(), "accountInclusionRoot"),
        opened.accountInclusionRoot())) {
      throw new IllegalArgumentException("opened accountInclusionRoot must match witness");
    }
    if (!Arrays.equals(
        hex32Bytes(witness.accountsLtHashChecksum(), "accountsLtHashChecksum"),
        opened.accountsLtHashChecksum())) {
      throw new IllegalArgumentException("opened accountsLtHashChecksum must match witness");
    }
    return new NormalizedAccountsLtHashProofRequest(
        witness,
        opened,
        openedInput.accountsLtHash(),
        openedAccountsLtHashContributionsHash(openedInput),
        openedAccountsLtHashResidualChecksum(openedInput));
  }

  private static WitnessInput witnessInputWithOpenedAccountsLtHash(
      final WitnessInput witnessInput, final OpenedAccountsLtHashContributionsInput openedInput) {
    final byte[] suppliedAccountsLtHash = witnessInput.accountsLtHash();
    final byte[] openedAccountsLtHash = openedInput.accountsLtHash();
    if (suppliedAccountsLtHash != null
        && !Arrays.equals(suppliedAccountsLtHash, openedAccountsLtHash)) {
      throw new IllegalArgumentException("witness accountsLtHash must match opened accountsLtHash");
    }
    return new WitnessInput(
        witnessInput.targetDomain(),
        witnessInput.mainnetGenesisHash(),
        witnessInput.finalizedSlot(),
        witnessInput.parentSlot(),
        witnessInput.bankSignatureCount(),
        witnessInput.parentBankHash(),
        witnessInput.blockhash(),
        witnessInput.bankHash(),
        witnessInput.transactionStatusRoot(),
        witnessInput.messageProofHash(),
        witnessInput.accountInclusionRoot(),
        witnessInput.accountsLtHashChecksum(),
        witnessInput.accountsLtHashProofPublicInputsHash(),
        witnessInput.bankHashHardForkData(),
        openedAccountsLtHash,
        witnessInput.transactionSignature(),
        witnessInput.emitterProgramId(),
        witnessInput.messageId(),
        witnessInput.payloadHash(),
        witnessInput.commitmentRoot(),
        witnessInput.sourceEventDigest(),
        witnessInput.sourceStateVerifierId(),
        witnessInput.sourceStateVerifierHash(),
        witnessInput.statementHash(),
        witnessInput.destinationBindingHash(),
        witnessInput.inclusionBranch(),
        witnessInput.sourceAdapterDeploymentHash(),
        witnessInput.sourceAdapterDeploymentReceiptHash());
  }

  private static List<OpenedLtHashContributionRow> openedLtHashContributionRows(
      final OpenedAccountsLtHashContributionsInput input) {
    final AccountOpeningInput[] voteOpenings = input.validatorVoteAccountOpenings();
    final byte[][] voteRawData = input.validatorVoteAccountRawData();
    final byte[][] voteLtHashes = input.validatorVoteAccountLtHashes();
    final AccountOpeningInput[] stakeOpenings = input.validatorStakeAccountOpenings();
    final byte[][] stakeRawData = input.validatorStakeAccountRawData();
    final byte[][] stakeLtHashes = input.validatorStakeAccountLtHashes();
    final boolean deriveVoteLtHashes = voteLtHashes.length == 0;
    final boolean deriveStakeLtHashes = stakeLtHashes.length == 0;
    if (voteOpenings.length > MAX_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorVoteAccountOpenings must contain at most " + MAX_VALIDATORS + " entries");
    }
    if (stakeOpenings.length > MAX_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorStakeAccountOpenings must contain at most " + MAX_VALIDATORS + " entries");
    }
    if (voteOpenings.length != voteRawData.length
        || (!deriveVoteLtHashes && voteOpenings.length != voteLtHashes.length)) {
      throw new IllegalArgumentException("validator vote account arrays must have matching lengths");
    }
    if (stakeOpenings.length != stakeRawData.length
        || (!deriveStakeLtHashes && stakeOpenings.length != stakeLtHashes.length)) {
      throw new IllegalArgumentException("validator stake account arrays must have matching lengths");
    }
    final List<OpenedLtHashContributionRow> rows = new ArrayList<>();
    final List<String> seenAddresses = new ArrayList<>();
    for (int i = 0; i < voteOpenings.length; i++) {
      final OpenedLtHashContributionRow row =
          openedLtHashContributionRow(
              OPENED_LT_HASH_ROLE_VOTE,
              voteOpenings[i],
              voteRawData[i],
              deriveVoteLtHashes ? null : voteLtHashes[i],
              "validatorVoteAccountLtHashes[" + i + "]",
              false);
      addUniqueOpenedLtHashContributionRow(rows, seenAddresses, row);
    }
    for (int i = 0; i < stakeOpenings.length; i++) {
      final OpenedLtHashContributionRow row =
          openedLtHashContributionRow(
              OPENED_LT_HASH_ROLE_STAKE,
              stakeOpenings[i],
              stakeRawData[i],
              deriveStakeLtHashes ? null : stakeLtHashes[i],
              "validatorStakeAccountLtHashes[" + i + "]",
              false);
      addUniqueOpenedLtHashContributionRow(rows, seenAddresses, row);
    }
    final byte[] stakeHistoryAccountLtHash = input.stakeHistorySysvarAccountLtHash();
    addUniqueOpenedLtHashContributionRow(
        rows,
        seenAddresses,
        openedLtHashContributionRow(
            OPENED_LT_HASH_ROLE_STAKE_HISTORY_SYSVAR,
            input.stakeHistorySysvarOpening(),
            input.stakeHistorySysvarRawData(),
            stakeHistoryAccountLtHash,
            "stakeHistorySysvarAccountLtHash",
            true));
    rows.sort(
        (left, right) -> {
          final int roleComparison = Integer.compare(left.role(), right.role());
          return roleComparison != 0
              ? roleComparison
              : compareLexicographically(left.address(), right.address());
        });
    return rows;
  }

  private static void addUniqueOpenedLtHashContributionRow(
      final List<OpenedLtHashContributionRow> rows,
      final List<String> seenAddresses,
      final OpenedLtHashContributionRow row) {
    final String addressKey = hexLower(row.address());
    if (seenAddresses.contains(addressKey)) {
      throw new IllegalArgumentException("opened account addresses must be unique");
    }
    seenAddresses.add(addressKey);
    rows.add(row);
  }

  private static OpenedLtHashContributionRow openedLtHashContributionRow(
      final int role,
      final AccountOpeningInput opening,
      final byte[] rawData,
      final byte[] suppliedAccountLtHash,
      final String field,
      final boolean allowEmptyDerive) {
    if (opening.address().length != 32) {
      throw new IllegalArgumentException("address must be 32 bytes");
    }
    final byte[] expectedAccountLtHash = accountLtHash(opening, rawData);
    final byte[] accountLtHash;
    if (suppliedAccountLtHash != null && !(allowEmptyDerive && suppliedAccountLtHash.length == 0)) {
      if (suppliedAccountLtHash.length != ACCOUNTS_LT_HASH_BYTES) {
        throw new IllegalArgumentException(field + " must be 2048 bytes");
      }
      if (!Arrays.equals(suppliedAccountLtHash, expectedAccountLtHash)) {
        throw new IllegalArgumentException(field + " must match the opening and rawData");
      }
      accountLtHash = Arrays.copyOf(suppliedAccountLtHash, suppliedAccountLtHash.length);
    } else {
      accountLtHash = expectedAccountLtHash;
    }
    return new OpenedLtHashContributionRow(
        role,
        opening.address(),
        hex32Bytes(accountOpeningHash(opening), "accountHash"),
        hex32Bytes(accountRawDataHash(rawData), "rawDataHash"),
        accountLtHash);
  }

  private static String accountOpeningHash(final AccountOpeningInput opening) {
    return accountOpeningHash(
        opening.address(),
        opening.owner(),
        opening.lamports(),
        opening.rentEpoch(),
        opening.executable(),
        opening.dataHash());
  }

  private static void addAccountsLtHashContribution(final byte[] target, final byte[] contribution) {
    if (target.length != ACCOUNTS_LT_HASH_BYTES || contribution.length != ACCOUNTS_LT_HASH_BYTES) {
      throw new IllegalArgumentException("accountsLtHash contributions must be 2048 bytes");
    }
    for (int index = 0; index < LT_HASH_ELEMENTS; index++) {
      final int offset = index * 2;
      final int mixed =
          (((target[offset] & 0xff) | ((target[offset + 1] & 0xff) << 8))
                  + ((contribution[offset] & 0xff) | ((contribution[offset + 1] & 0xff) << 8)))
              & 0xffff;
      target[offset] = (byte) mixed;
      target[offset + 1] = (byte) (mixed >>> 8);
    }
  }

  private static void subtractAccountsLtHashContribution(final byte[] target, final byte[] contribution) {
    if (target.length != ACCOUNTS_LT_HASH_BYTES || contribution.length != ACCOUNTS_LT_HASH_BYTES) {
      throw new IllegalArgumentException("accountsLtHash contributions must be 2048 bytes");
    }
    for (int index = 0; index < LT_HASH_ELEMENTS; index++) {
      final int offset = index * 2;
      final int mixed =
          (((target[offset] & 0xff) | ((target[offset + 1] & 0xff) << 8))
                  - ((contribution[offset] & 0xff) | ((contribution[offset + 1] & 0xff) << 8)))
              & 0xffff;
      target[offset] = (byte) mixed;
      target[offset + 1] = (byte) (mixed >>> 8);
    }
  }

  private static byte[] sccpWordU32Le(final int value) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32Le(out, value);
    final byte[] word = new byte[32];
    System.arraycopy(out.toByteArray(), 0, word, 0, 4);
    return word;
  }

  private static byte[] sccpWordU64Le(final BigInteger value) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU64Le(out, value);
    final byte[] word = new byte[32];
    System.arraycopy(out.toByteArray(), 0, word, 0, 8);
    return word;
  }

  private static String hashHex(final String prefix, final byte[] payload) {
    return "0x" + hexLower(hashBytes(prefix, payload));
  }

  private static String solanaMainnetGenesisHashPublicInput() {
    return hashHex(
        MAINNET_GENESIS_HASH_PREFIX_V1, MAINNET_GENESIS_HASH.getBytes(StandardCharsets.UTF_8));
  }

  private static byte[] hashBytes(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return Blake2b.digest256(preimage);
  }

  private static byte[] sourceMerkleNodeHash(final byte[] left, final byte[] right) {
    final byte[] payload = new byte[left.length + right.length];
    System.arraycopy(left, 0, payload, 0, left.length);
    System.arraycopy(right, 0, payload, left.length, right.length);
    return hashBytes("sccp:source:node:v1", payload);
  }

  private static byte[] sha256Hashv(final byte[]... parts) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      for (final byte[] part : parts) {
        digest.update(part);
      }
      return digest.digest();
    } catch (final NoSuchAlgorithmException error) {
      throw new IllegalStateException("SHA-256 is unavailable", error);
    }
  }

  private static byte[] agaveBankHashBytes(
      final byte[] parentBankHash,
      final BigInteger bankSignatureCount,
      final byte[] blockhash,
      final byte[] accountsLtHash,
      final byte[] bankHashHardForkData) {
    if (BigInteger.ZERO.equals(bankSignatureCount)) {
      throw new IllegalArgumentException("bankSignatureCount must be nonzero");
    }
    if (accountsLtHash == null || accountsLtHash.length != ACCOUNTS_LT_HASH_BYTES) {
      throw new IllegalArgumentException("accountsLtHash must be 2048 bytes");
    }
    if (!anyNonZero(accountsLtHash)) {
      throw new IllegalArgumentException("accountsLtHash must not be zero");
    }
    if (bankHashHardForkData.length > MAX_BANK_HARD_FORK_HASH_DATA_BYTES) {
      throw new IllegalArgumentException("bankHashHardForkData is too large");
    }
    final ByteArrayOutputStream signatureCountBytes = new ByteArrayOutputStream();
    writeU64Le(signatureCountBytes, bankSignatureCount);
    byte[] bankHash = sha256Hashv(parentBankHash, signatureCountBytes.toByteArray(), blockhash);
    bankHash = sha256Hashv(bankHash, accountsLtHash);
    if (bankHashHardForkData.length != 0) {
      bankHash = sha256Hashv(bankHash, bankHashHardForkData);
    }
    return bankHash;
  }

  private static int compareLexicographically(final byte[] left, final byte[] right) {
    if (left.length != right.length) {
      return left.length - right.length;
    }
    for (int i = 0; i < left.length; i++) {
      final int diff = (left[i] & 0xFF) - (right[i] & 0xFF);
      if (diff != 0) {
        return diff;
      }
    }
    return 0;
  }

  private record RouteCanaryProgramDataEvidence(
      byte[] verifierProgram,
      String verifierCodeHash,
      String rpcCommitment,
      String programOwner,
      String programdataOwner,
      byte[] programAccountData,
      byte[] programdataAddress,
      BigInteger programdataSlot,
      BigInteger expectedProgramdataSlot,
      BigInteger programAccountContextSlot,
      BigInteger programdataAccountContextSlot,
      byte[] programdataMetadata,
      byte[] programdataExecutable) {}

  private static BigInteger canonicalPositiveU64(final String value, final String field) {
    final String text = normalizeNonEmpty(value, field);
    if (!text.equals(value) || !text.matches("[0-9]+")) {
      throw new IllegalArgumentException(field + " must be canonical decimal");
    }
    final BigInteger numeric = new BigInteger(text);
    if (numeric.equals(BigInteger.ZERO)) {
      throw new IllegalArgumentException(field + " must be positive");
    }
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    if (!text.equals(numeric.toString())) {
      throw new IllegalArgumentException(field + " must be canonical decimal");
    }
    return numeric;
  }

  private static byte[] strictBase64Bytes(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical base64");
    }
    final byte[] decoded = Base64.getDecoder().decode(value);
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical base64");
    }
    return decoded;
  }

  private static byte[] solanaUpgradeableProgramAccountData(final byte[] programdataAddress) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32Le(out, SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG);
    write(out, programdataAddress);
    return out.toByteArray();
  }

  private static byte[] solanaImmutableProgramdataMetadata(final BigInteger programdataSlot) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32Le(out, SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG);
    writeU64Le(out, programdataSlot);
    out.write(0);
    write(out, new byte[32]);
    return out.toByteArray();
  }

  private static String solanaVerifierProgramCodeHash(final byte[] programBytes) {
    if (programBytes.length == 0
        || !anyNonZero(programBytes)
        || programBytes.length < SOLANA_BPF_ELF_MAGIC.length) {
      throw new IllegalArgumentException(
          "solanaProgramdataExecutable must be non-empty BPF ELF bytes");
    }
    for (int i = 0; i < SOLANA_BPF_ELF_MAGIC.length; i++) {
      if (programBytes[i] != SOLANA_BPF_ELF_MAGIC[i]) {
        throw new IllegalArgumentException(
            "solanaProgramdataExecutable must be non-empty BPF ELF bytes");
      }
    }
    return "0x" + hexLower(Blake2b.digest256(programBytes));
  }

  private static RouteCanaryProgramDataEvidence normalizeRouteCanaryProgramDataEvidence(
      final RouteCanaryEvidenceInput input) {
    final byte[] verifierProgram =
        decodeSolanaBase58Fixed(input.verifierIdentity(), "verifierIdentity", PROGRAM_ID_BYTES);
    final byte[] programdataAddress =
        decodeSolanaBase58Fixed(
            input.solanaProgramdataAddress(), "solanaProgramdataAddress", PROGRAM_ID_BYTES);
    if (Arrays.equals(verifierProgram, programdataAddress)) {
      throw new IllegalArgumentException("solanaProgramdataAddress must differ from verifierIdentity");
    }
    final BigInteger programdataSlot =
        canonicalPositiveU64(input.solanaProgramdataSlot(), "solanaProgramdataSlot");
    final BigInteger expectedProgramdataSlot =
        canonicalPositiveU64(
            input.solanaExpectedProgramdataSlot(), "solanaExpectedProgramdataSlot");
    if (!programdataSlot.equals(expectedProgramdataSlot)) {
      throw new IllegalArgumentException(
          "solanaExpectedProgramdataSlot must match solanaProgramdataSlot");
    }
    final BigInteger programContextSlot =
        canonicalPositiveU64(
            input.solanaProgramAccountContextSlot(), "solanaProgramAccountContextSlot");
    final BigInteger programdataContextSlot =
        canonicalPositiveU64(
            input.solanaProgramdataAccountContextSlot(), "solanaProgramdataAccountContextSlot");
    if (programContextSlot.compareTo(programdataSlot) < 0
        || programdataContextSlot.compareTo(programdataSlot) < 0) {
      throw new IllegalArgumentException(
          "Solana ProgramData context slots must be at or after programdataSlot");
    }
    final String rpcCommitment = normalizeNonEmpty(input.solanaRpcCommitment(), "solanaRpcCommitment");
    if (!"finalized".equals(rpcCommitment)) {
      throw new IllegalArgumentException("solanaRpcCommitment must be finalized");
    }
    final String programOwner = normalizeNonEmpty(input.solanaProgramOwner(), "solanaProgramOwner");
    final String programdataOwner =
        normalizeNonEmpty(input.solanaProgramdataOwner(), "solanaProgramdataOwner");
    if (!UPGRADEABLE_LOADER_ID.equals(programOwner)) {
      throw new IllegalArgumentException("solanaProgramOwner must be the BPF upgradeable loader");
    }
    if (!UPGRADEABLE_LOADER_ID.equals(programdataOwner)) {
      throw new IllegalArgumentException("solanaProgramdataOwner must be the BPF upgradeable loader");
    }
    if (!input.solanaProgramImmutable()) {
      throw new IllegalArgumentException("solanaProgramImmutable must be true");
    }
    final byte[] programAccountData =
        strictBase64Bytes(
            input.solanaProgramAccountDataBase64(), "solanaProgramAccountDataBase64");
    if (!Arrays.equals(programAccountData, solanaUpgradeableProgramAccountData(programdataAddress))) {
      throw new IllegalArgumentException(
          "solanaProgramAccountDataBase64 must bind solanaProgramdataAddress");
    }
    final byte[] programdataMetadata =
        strictBase64Bytes(
            input.solanaProgramdataMetadataBase64(), "solanaProgramdataMetadataBase64");
    if (programdataMetadata.length != SOLANA_PROGRAMDATA_METADATA_LEN
        || !Arrays.equals(
            programdataMetadata, solanaImmutableProgramdataMetadata(programdataSlot))) {
      throw new IllegalArgumentException(
          "solanaProgramdataMetadataBase64 must bind immutable ProgramData metadata");
    }
    final String metadataHash = "0x" + hexLower(Blake2b.digest256(programdataMetadata));
    if (!normalizeNonZeroHex32(
            input.solanaProgramdataMetadataBlake2b256(),
            "solanaProgramdataMetadataBlake2b256")
        .equals(metadataHash)) {
      throw new IllegalArgumentException(
          "solanaProgramdataMetadataBlake2b256 must match metadata bytes");
    }
    final byte[] programdataExecutable =
        strictBase64Bytes(
            input.solanaProgramdataExecutableBase64(), "solanaProgramdataExecutableBase64");
    final String executableHash = solanaVerifierProgramCodeHash(programdataExecutable);
    if (!normalizeNonZeroHex32(
            input.solanaProgramdataExecutableBlake2b256(),
            "solanaProgramdataExecutableBlake2b256")
        .equals(executableHash)) {
      throw new IllegalArgumentException(
          "solanaProgramdataExecutableBlake2b256 must match executable bytes");
    }
    if (!normalizeNonZeroHex32(input.verifierCodeHash(), "verifierCodeHash")
        .equals(executableHash)) {
      throw new IllegalArgumentException("verifierCodeHash must match ProgramData executable hash");
    }
    return new RouteCanaryProgramDataEvidence(
        verifierProgram,
        executableHash,
        rpcCommitment,
        programOwner,
        programdataOwner,
        programAccountData,
        programdataAddress,
        programdataSlot,
        expectedProgramdataSlot,
        programContextSlot,
        programdataContextSlot,
        programdataMetadata,
        programdataExecutable);
  }

  private static String normalizeNonEmpty(final String value, final String field) {
    final String trimmed = Objects.requireNonNull(value, field).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must be non-empty");
    }
    return trimmed;
  }

  private static void requireSourceStateProofLabel(final String value, final String field) {
    if (Objects.requireNonNull(value, field).getBytes(StandardCharsets.UTF_8).length
        > SOURCE_STATE_MAX_PROOF_LABEL_BYTES) {
      throw new IllegalArgumentException(
          field + " must be at most " + SOURCE_STATE_MAX_PROOF_LABEL_BYTES + " bytes");
    }
  }

  private static byte[] decodeSolanaBase58(final String value, final String field) {
    final String text = normalizeNonEmpty(value, field);
    BigInteger numeric = BigInteger.ZERO;
    for (int i = 0; i < text.length(); i++) {
      final char symbol = text.charAt(i);
      if (symbol >= BASE58_INDEX.length || BASE58_INDEX[symbol] < 0) {
        throw new IllegalArgumentException(field + " must be canonical base58");
      }
      numeric = numeric.multiply(BASE_58).add(BigInteger.valueOf(BASE58_INDEX[symbol]));
    }
    byte[] payload = numeric.signum() == 0 ? new byte[0] : numeric.toByteArray();
    while (payload.length > 1 && payload[0] == 0) {
      payload = Arrays.copyOfRange(payload, 1, payload.length);
    }
    int leadingZeros = 0;
    while (leadingZeros < text.length() && text.charAt(leadingZeros) == '1') {
      leadingZeros++;
    }
    final byte[] out = new byte[leadingZeros + payload.length];
    System.arraycopy(payload, 0, out, leadingZeros, payload.length);
    return out;
  }

  private static byte[] decodeSolanaBase58Fixed(
      final String value, final String field, final int byteLength) {
    final byte[] raw = decodeSolanaBase58(value, field);
    if (raw.length != byteLength) {
      throw new IllegalArgumentException(field + " must decode to " + byteLength + " bytes");
    }
    boolean nonzero = false;
    for (final byte b : raw) {
      nonzero |= b != 0;
    }
    if (!nonzero) {
      throw new IllegalArgumentException(field + " must not decode to zero");
    }
    return raw;
  }

  private static String normalizeSolanaBase58Fixed(
      final String value, final String field, final int byteLength) {
    final String text = normalizeNonEmpty(value, field);
    decodeSolanaBase58Fixed(text, field, byteLength);
    return text;
  }

  private static String normalizeHex32(final String value, final String field) {
    return "0x" + hexLower(hex32Bytes(value, field));
  }

  private static String normalizeHexBytes(
      final String value, final String field, final int byteLength) {
    return "0x" + hexLower(hexBytes(value, field, byteLength));
  }

  private static String normalizeNonZeroHex32(final String value, final String field) {
    final byte[] bytes = hex32Bytes(value, field);
    if (!anyNonZero(bytes)) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return "0x" + hexLower(bytes);
  }

  private static byte[] nonZeroHex32Bytes(final String value, final String field) {
    final byte[] bytes = hex32Bytes(value, field);
    if (!anyNonZero(bytes)) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return bytes;
  }

  private static byte[] solanaHash32Bytes(final String value, final String field) {
    final String text = normalizeNonEmpty(value, field);
    final boolean maybeHex =
        text.regionMatches(true, 0, "0x", 0, 2)
            || text.matches("[0-9a-fA-F]{64}");
    final byte[] raw = maybeHex ? hex32Bytes(text, field) : decodeSolanaBase58Fixed(text, field, 32);
    if (!anyNonZero(raw)) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return raw;
  }

  private static String normalizeMessageProofHash(
      final String value,
      final String sourceEventDigest,
      final String transactionStatusRoot,
      final String transactionSignature,
      final String emitterProgramId,
      final byte[][] inclusionBranch) {
    if (inclusionBranch.length == 0) {
      return normalizeHex32(value, "messageProofHash");
    }
    final String derived =
        messageProofHash(
            sourceEventDigest,
            transactionStatusRoot,
            transactionSignature,
            emitterProgramId,
            inclusionBranch);
    if (value == null || value.trim().isEmpty()) {
      return derived;
    }
    final String provided = normalizeHex32(value, "messageProofHash");
    if (!provided.equals(derived)) {
      throw new IllegalArgumentException("messageProofHash must match inclusionBranch");
    }
    return provided;
  }

  private static byte[][] normalizeInclusionBranch(final byte[][] branch) {
    final byte[][] copied = copyBranch(branch);
    if (copied.length > MAX_SOURCE_MERKLE_BRANCH_NODES) {
      throw new IllegalArgumentException(
          "inclusionBranch must contain at most "
              + MAX_SOURCE_MERKLE_BRANCH_NODES
              + " entries");
    }
    for (int i = 0; i < copied.length; i++) {
      if (copied[i].length != 32) {
        throw new IllegalArgumentException("inclusionBranch[" + i + "] must be 32 bytes");
      }
    }
    return copied;
  }

  private static byte[][] copyBranch(final byte[][] branch) {
    if (branch == null) {
      return new byte[0][];
    }
    final byte[][] copied = new byte[branch.length][];
    for (int i = 0; i < branch.length; i++) {
      final byte[] sibling = Objects.requireNonNull(branch[i], "inclusionBranch[" + i + "]");
      copied[i] = Arrays.copyOf(sibling, sibling.length);
    }
    return copied;
  }

  private static List<List<String>> copyStringColumns(final List<List<String>> columns) {
    if (columns == null) {
      return Collections.emptyList();
    }
    final List<List<String>> copied = new ArrayList<>();
    for (final List<String> column : columns) {
      copied.add(Collections.unmodifiableList(new ArrayList<>(column)));
    }
    return Collections.unmodifiableList(copied);
  }

  private static final class AccountInclusionLevelNode {
    final byte[] hash;
    final List<Integer> indexes;

    AccountInclusionLevelNode(final byte[] hash, final List<Integer> indexes) {
      this.hash = hash;
      this.indexes = indexes;
    }
  }

  private static byte[] hex32Bytes(final String value, final String field) {
    return hexBytes(value, field, 32);
  }

  private static byte[] hexBytes(final String value, final String field, final int byteLength) {
    Objects.requireNonNull(value, field);
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical hex");
    }
    String body = value;
    if (body.regionMatches(true, 0, "0x", 0, 2)) {
      body = body.substring(2);
    }
    for (int i = 0; i < body.length(); i++) {
      if (Character.isWhitespace(body.charAt(i))) {
        throw new IllegalArgumentException(field + " must be canonical hex");
      }
    }
    if (body.length() != byteLength * 2) {
      throw new IllegalArgumentException(field + " must be " + byteLength + " bytes");
    }
    final byte[] out = new byte[byteLength];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(body.charAt(i * 2), 16);
      final int lo = Character.digit(body.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " must be canonical hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static BigInteger normalizeU64(final String value, final String field) {
    final String trimmed = Objects.requireNonNull(value, field).trim();
    if (!trimmed.matches("[0-9]+")) {
      throw new IllegalArgumentException(field + " must be an unsigned integer");
    }
    final BigInteger numeric = new BigInteger(trimmed);
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    return numeric;
  }

  private static void writeString(
      final ByteArrayOutputStream out, final String value, final String field) {
    final byte[] bytes = normalizeNonEmpty(value, field).getBytes(StandardCharsets.UTF_8);
    writeU32Le(out, bytes.length);
    write(out, bytes);
  }

  private static byte[] encodeInstructionData(final List<SubmissionArgumentBytes> argumentBytes) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeVec(out, SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.getBytes(StandardCharsets.UTF_8));
    for (final SubmissionArgumentBytes argument : argumentBytes) {
      writeVec(out, argument.bytes);
    }
    return out.toByteArray();
  }

  private static byte[] canonicalVoteRosterBytes(
      final byte[][] validatorPublicKeys, final String[] validatorStakes) {
    Objects.requireNonNull(validatorPublicKeys, "validatorPublicKeys");
    Objects.requireNonNull(validatorStakes, "validatorStakes");
    if (validatorPublicKeys.length == 0 || validatorPublicKeys.length != validatorStakes.length) {
      throw new IllegalArgumentException("validatorStakes must match validatorPublicKeys");
    }
    if (validatorPublicKeys.length > MAX_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorPublicKeys must contain 1.." + MAX_VALIDATORS + " entries");
    }
    final List<String> seen = new ArrayList<>();
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, validatorPublicKeys.length);
    for (int i = 0; i < validatorPublicKeys.length; i++) {
      final byte[] publicKey = Objects.requireNonNull(validatorPublicKeys[i], "validatorPublicKeys[" + i + "]");
      if (publicKey.length != 32) {
        throw new IllegalArgumentException("validatorPublicKeys[" + i + "] must be 32 bytes");
      }
      if (!anyNonZero(publicKey)) {
        throw new IllegalArgumentException("validatorPublicKeys[" + i + "] must not be zero");
      }
      final String publicKeyHex = hexLower(publicKey);
      if (seen.contains(publicKeyHex)) {
        throw new IllegalArgumentException("validatorPublicKeys must not contain duplicates");
      }
      seen.add(publicKeyHex);
      final BigInteger stake = normalizeU64(validatorStakes[i], "validatorStakes[" + i + "]");
      if (stake.equals(BigInteger.ZERO)) {
        throw new IllegalArgumentException("validatorStakes[" + i + "] must be greater than zero");
      }
      writeVec(out, publicKey);
      writeU64Le(out, stake);
    }
    return out.toByteArray();
  }

  private static byte[][] normalizeFixed32Array(
      final byte[][] values, final int expectedLength, final String label, final boolean unique) {
    Objects.requireNonNull(values, label);
    if (values.length != expectedLength) {
      throw new IllegalArgumentException(label + " must match validatorPublicKeys");
    }
    final List<String> seen = new ArrayList<>();
    final byte[][] out = new byte[values.length][];
    for (int i = 0; i < values.length; i++) {
      final byte[] value = Objects.requireNonNull(values[i], label + "[" + i + "]");
      if (value.length != 32) {
        throw new IllegalArgumentException(label + "[" + i + "] must be 32 bytes");
      }
      if (!anyNonZero(value)) {
        throw new IllegalArgumentException(label + "[" + i + "] must not be zero");
      }
      if (unique) {
        final String encoded = hexLower(value);
        if (seen.contains(encoded)) {
          throw new IllegalArgumentException(label + " must not contain duplicates");
        }
        seen.add(encoded);
      }
      out[i] = value;
    }
    return out;
  }

  private static void writeVec(final ByteArrayOutputStream out, final byte[] bytes) {
    writeU32Le(out, bytes.length);
    write(out, bytes);
  }

  private static void writeU32Le(final ByteArrayOutputStream out, final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u32 value must not be negative");
    }
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeU16Le(final ByteArrayOutputStream out, final int value) {
    if (value < 0 || value > 0xffff) {
      throw new IllegalArgumentException("u16 value out of range");
    }
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
  }

  private static void writeU64Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int i = 0; i < 8; i++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static int readU32Le(final byte[] bytes, final int offset) {
    if (offset < 0 || offset + 4 > bytes.length) {
      throw new IllegalArgumentException("rawData is too short");
    }
    return (bytes[offset] & 0xff)
        | ((bytes[offset + 1] & 0xff) << 8)
        | ((bytes[offset + 2] & 0xff) << 16)
        | ((bytes[offset + 3] & 0xff) << 24);
  }

  private static int readU8At(final byte[] bytes, final int[] cursor, final String field) {
    if (cursor[0] + 1 > bytes.length) {
      throw new IllegalArgumentException(field + " is too short");
    }
    return bytes[cursor[0]++] & 0xff;
  }

  private static int readU32LeAt(final byte[] bytes, final int[] cursor, final String field) {
    if (cursor[0] + 4 > bytes.length) {
      throw new IllegalArgumentException(field + " is too short");
    }
    final int value = readU32Le(bytes, cursor[0]);
    cursor[0] += 4;
    return value;
  }

  private static int readU16LeAt(final byte[] bytes, final int[] cursor, final String field) {
    if (cursor[0] + 2 > bytes.length) {
      throw new IllegalArgumentException(field + " is too short");
    }
    final int value = (bytes[cursor[0]] & 0xff) | ((bytes[cursor[0] + 1] & 0xff) << 8);
    cursor[0] += 2;
    return value;
  }

  private static BigInteger readU64Le(final byte[] bytes, final int offset) {
    if (offset < 0 || offset + 8 > bytes.length) {
      throw new IllegalArgumentException("rawData is too short");
    }
    BigInteger value = BigInteger.ZERO;
    for (int i = 7; i >= 0; i--) {
      value = value.shiftLeft(8).add(BigInteger.valueOf(bytes[offset + i] & 0xffL));
    }
    return value;
  }

  private static BigInteger readU64LeAt(final byte[] bytes, final int[] cursor, final String field) {
    if (cursor[0] + 8 > bytes.length) {
      throw new IllegalArgumentException(field + " is too short");
    }
    final BigInteger value = readU64Le(bytes, cursor[0]);
    cursor[0] += 8;
    return value;
  }

  private static byte[] readPubkeyAt(final byte[] bytes, final int[] cursor, final String field) {
    if (cursor[0] + 32 > bytes.length) {
      throw new IllegalArgumentException(field + " is too short");
    }
    final byte[] value = Arrays.copyOfRange(bytes, cursor[0], cursor[0] + 32);
    cursor[0] += 32;
    return value;
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private static void requireNonZeroPublicKey(final byte[] value, final String label) {
    Objects.requireNonNull(value, label);
    if (value.length != 32 || !anyNonZero(value)) {
      throw new IllegalArgumentException(
          label + " must be a non-zero 32-byte Solana public key");
    }
  }

  private static boolean isSupportedStakeWarmupCooldownRateBytes(final byte[] bytes) {
    return Arrays.equals(bytes, STAKE_STATE_V2_LEGACY_WARMUP_COOLDOWN_RATE_BYTES)
        || Arrays.equals(bytes, STAKE_STATE_V2_CURRENT_WARMUP_COOLDOWN_RATE_BYTES);
  }

  private static boolean anyNonZero(final byte[] bytes) {
    for (final byte b : bytes) {
      if (b != 0) {
        return true;
      }
    }
    return false;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  private static final class SubmissionArgumentBytes {
    private final String key;
    private final byte[] bytes;

    private SubmissionArgumentBytes(final String key, final byte[] bytes) {
      this.key = key;
      this.bytes = Arrays.copyOf(bytes, bytes.length);
    }
  }

  /** Optional witness resolver backed by app-controlled Solana RPC calls. */
  public interface WitnessProvider {
    WitnessInput resolveWitness(WitnessInput input);
  }

  /** Local proof engine linked by the application bundle. */
  public interface ProofEngine {
    byte[] prove(ProofRequest request);
  }

  /** Local proof engine for nested Solana AccountsLtHash source-state requests. */
  public interface AccountsLtHashProofEngine {
    byte[] prove(AccountsLtHashProofRequest request);
  }

  /** Local proof engine for Solana full-light source-state audit role requests. */
  public interface FullLightClientAuditProofEngine {
    byte[] prove(FullLightClientAuditProofRequest request);
  }

  /** Role-separated Solana full-light audit proof capsules. */
  public record FullLightClientAuditProofs(
      SourceStateVerificationProof towerReplay,
      SourceStateVerificationProof fullAccountsdbLattice,
      SourceStateVerificationProof bankForkChoice) {}

  /** Source-state proof wrapper for Android Solana proof engines. */
  public static final class SourceStateProver {
    private final AccountsLtHashProofEngine accountsLtHashProofEngine;
    private final FullLightClientAuditProofEngine fullLightClientAuditProofEngine;

    public SourceStateProver() {
      this(null, null);
    }

    public SourceStateProver(
        final AccountsLtHashProofEngine accountsLtHashProofEngine,
        final FullLightClientAuditProofEngine fullLightClientAuditProofEngine) {
      this.accountsLtHashProofEngine = accountsLtHashProofEngine;
      this.fullLightClientAuditProofEngine = fullLightClientAuditProofEngine;
    }

    public SourceStateVerificationProof proveAccountsLtHash(
        final WitnessInput witnessInput,
        final OpenedAccountsLtHashContributionsInput openedAccounts) {
      return proveAccountsLtHash(buildAccountsLtHashProofRequest(witnessInput, openedAccounts));
    }

    public SourceStateVerificationProof proveAccountsLtHash(final AccountsLtHashProofRequest request) {
      requireSourceStateProofRequestForWrapping(request);
      if (accountsLtHashProofEngine == null) {
        throw new IllegalStateException("Solana SCCP source-state prover is not linked");
      }
      return wrapSourceStateVerificationProof(
          accountsLtHashProofEngine.prove(callbackRequestSnapshot(request)), request);
    }

    public FullLightClientAuditProofs proveFullLightClientAudit(
        final FullLightClientAuditProofInput input) {
      final FullLightClientAuditProofRequests requests = buildFullLightClientAuditProofRequests(input);
      return new FullLightClientAuditProofs(
          proveFullLightClientAudit(requests.towerReplay()),
          proveFullLightClientAudit(requests.fullAccountsdbLattice()),
          proveFullLightClientAudit(requests.bankForkChoice()));
    }

    public SourceStateVerificationProof proveFullLightClientAudit(
        final FullLightClientAuditProofRequest request) {
      requireSourceStateProofRequestForWrapping(request);
      if (fullLightClientAuditProofEngine == null) {
        throw new IllegalStateException("Solana SCCP source-state prover is not linked");
      }
      return wrapSourceStateVerificationProof(
          fullLightClientAuditProofEngine.prove(callbackRequestSnapshot(request)), request);
    }
  }

  static ProofRequest callbackRequestSnapshot(final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    return new ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.mainnetGenesisHash(),
        request.witnessHash(),
        request.proofContextHash(),
        request.sourceAdapterDeploymentBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.publicInputs(),
        callbackWitnessSnapshot(request.witness()),
        request.proofContext(),
        request.sourceAdapterDeploymentBinding());
  }

  private static Witness callbackWitnessSnapshot(final Witness witness) {
    Objects.requireNonNull(witness, "witness");
    return new Witness(
        witness.version(),
        witness.sourceDomain(),
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
        witness.sourceAdapterDeploymentHash(),
        witness.sourceAdapterDeploymentReceiptHash(),
        witness.inclusionBranch());
  }

  private static AccountsLtHashProofRequest callbackRequestSnapshot(
      final AccountsLtHashProofRequest request) {
    Objects.requireNonNull(request, "request");
    return new AccountsLtHashProofRequest(
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
        copyAccountsLtHashTransitions(request.fastpqTransitions()));
  }

  private static List<AccountsLtHashFastpqTransition> copyAccountsLtHashTransitions(
      final List<AccountsLtHashFastpqTransition> transitions) {
    return transitions.stream()
        .map(
            transition ->
                new AccountsLtHashFastpqTransition(
                    transition.key(),
                    transition.operation(),
                    transition.oldValue(),
                    transition.newValue()))
        .collect(Collectors.toList());
  }

  private static FullLightClientAuditProofRequest callbackRequestSnapshot(
      final FullLightClientAuditProofRequest request) {
    Objects.requireNonNull(request, "request");
    return new FullLightClientAuditProofRequest(
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.parameterSet(),
        request.role(),
        request.roleCode(),
        request.sourceDomain(),
        request.finalizedSlot(),
        request.verifierId(),
        request.verifierHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.sourceVerifierMaterialHash(),
        request.sourceAdapterDeploymentHash(),
        request.fullLightClientGateHash(),
        request.finalityContextHash(),
        request.voteMessageHash(),
        request.accountsLtHashProofHash(),
        request.auditStatementHash(),
        request.statementBytes(),
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        request.publicInputColumns(),
        request.fastpqPublicInputs(),
        copyFullLightClientAuditTransitions(request.fastpqTransitions()));
  }

  private static List<FullLightClientAuditFastpqTransition> copyFullLightClientAuditTransitions(
      final List<FullLightClientAuditFastpqTransition> transitions) {
    return transitions.stream()
        .map(
            transition ->
                new FullLightClientAuditFastpqTransition(
                    transition.key(),
                    transition.operation(),
                    transition.oldValue(),
                    transition.newValue()))
        .collect(Collectors.toList());
  }

  /** Solana account opening metadata used by mobile proof-generation helpers. */
  public record AccountOpeningInput(
      byte[] address,
      byte[] owner,
      String lamports,
      String rentEpoch,
      boolean executable,
      String dataHash) {
    public AccountOpeningInput {
      address = Arrays.copyOf(Objects.requireNonNull(address, "address"), address.length);
      owner = Arrays.copyOf(Objects.requireNonNull(owner, "owner"), owner.length);
      lamports = Objects.requireNonNull(lamports, "lamports");
      rentEpoch = Objects.requireNonNull(rentEpoch, "rentEpoch");
      dataHash = Objects.requireNonNull(dataHash, "dataHash");
    }

    @Override
    public byte[] address() {
      return Arrays.copyOf(address, address.length);
    }

    @Override
    public byte[] owner() {
      return Arrays.copyOf(owner, owner.length);
    }
  }

  /** Opened Solana AccountsLtHash rows supplied by a native/mobile source-state prover. */
  public record OpenedAccountsLtHashContributionsInput(
      int sourceDomain,
      String finalizedSlot,
      String accountInclusionRoot,
      String accountsLtHashChecksum,
      byte[] accountsLtHash,
      AccountOpeningInput[] validatorVoteAccountOpenings,
      byte[][] validatorVoteAccountRawData,
      byte[][] validatorVoteAccountLtHashes,
      AccountOpeningInput[] validatorStakeAccountOpenings,
      byte[][] validatorStakeAccountRawData,
      byte[][] validatorStakeAccountLtHashes,
      AccountOpeningInput stakeHistorySysvarOpening,
      byte[] stakeHistorySysvarRawData,
      byte[] stakeHistorySysvarAccountLtHash) {
    public OpenedAccountsLtHashContributionsInput {
      finalizedSlot = Objects.requireNonNull(finalizedSlot, "finalizedSlot");
      accountInclusionRoot = Objects.requireNonNull(accountInclusionRoot, "accountInclusionRoot");
      accountsLtHashChecksum = Objects.requireNonNull(accountsLtHashChecksum, "accountsLtHashChecksum");
      accountsLtHash = Arrays.copyOf(Objects.requireNonNull(accountsLtHash, "accountsLtHash"), accountsLtHash.length);
      validatorVoteAccountOpenings =
          validatorVoteAccountOpenings == null
              ? new AccountOpeningInput[0]
              : Arrays.copyOf(validatorVoteAccountOpenings, validatorVoteAccountOpenings.length);
      validatorVoteAccountRawData = copyBranch(validatorVoteAccountRawData);
      validatorVoteAccountLtHashes = copyBranch(validatorVoteAccountLtHashes);
      validatorStakeAccountOpenings =
          validatorStakeAccountOpenings == null
              ? new AccountOpeningInput[0]
              : Arrays.copyOf(validatorStakeAccountOpenings, validatorStakeAccountOpenings.length);
      validatorStakeAccountRawData = copyBranch(validatorStakeAccountRawData);
      validatorStakeAccountLtHashes = copyBranch(validatorStakeAccountLtHashes);
      stakeHistorySysvarOpening = Objects.requireNonNull(stakeHistorySysvarOpening, "stakeHistorySysvarOpening");
      stakeHistorySysvarRawData =
          Arrays.copyOf(
              Objects.requireNonNull(stakeHistorySysvarRawData, "stakeHistorySysvarRawData"),
              stakeHistorySysvarRawData.length);
      stakeHistorySysvarAccountLtHash =
          Arrays.copyOf(
              Objects.requireNonNull(stakeHistorySysvarAccountLtHash, "stakeHistorySysvarAccountLtHash"),
              stakeHistorySysvarAccountLtHash.length);
    }

    @Override
    public byte[] accountsLtHash() {
      return Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    }

    @Override
    public AccountOpeningInput[] validatorVoteAccountOpenings() {
      return Arrays.copyOf(validatorVoteAccountOpenings, validatorVoteAccountOpenings.length);
    }

    @Override
    public byte[][] validatorVoteAccountRawData() {
      return copyBranch(validatorVoteAccountRawData);
    }

    @Override
    public byte[][] validatorVoteAccountLtHashes() {
      return copyBranch(validatorVoteAccountLtHashes);
    }

    @Override
    public AccountOpeningInput[] validatorStakeAccountOpenings() {
      return Arrays.copyOf(validatorStakeAccountOpenings, validatorStakeAccountOpenings.length);
    }

    @Override
    public byte[][] validatorStakeAccountRawData() {
      return copyBranch(validatorStakeAccountRawData);
    }

    @Override
    public byte[][] validatorStakeAccountLtHashes() {
      return copyBranch(validatorStakeAccountLtHashes);
    }

    @Override
    public byte[] stakeHistorySysvarRawData() {
      return Arrays.copyOf(stakeHistorySysvarRawData, stakeHistorySysvarRawData.length);
    }

    @Override
    public byte[] stakeHistorySysvarAccountLtHash() {
      return Arrays.copyOf(stakeHistorySysvarAccountLtHash, stakeHistorySysvarAccountLtHash.length);
    }
  }

  /** FastPQ public inputs bound to a Solana AccountsLtHash source-state proof request. */
  public record AccountsLtHashFastpqPublicInputs(
      String dsid, String slot, String oldRoot, String newRoot, String permRoot, String txSetHash) {}

  /** One FastPQ transition supplied to a Solana AccountsLtHash source-state prover. */
  public record AccountsLtHashFastpqTransition(
      String key, String operation, byte[] oldValue, byte[] newValue) {
    public AccountsLtHashFastpqTransition {
      oldValue = oldValue == null ? new byte[0] : Arrays.copyOf(oldValue, oldValue.length);
      newValue = newValue == null ? new byte[0] : Arrays.copyOf(newValue, newValue.length);
    }

    @Override
    public byte[] oldValue() {
      return Arrays.copyOf(oldValue, oldValue.length);
    }

    @Override
    public byte[] newValue() {
      return Arrays.copyOf(newValue, newValue.length);
    }
  }

  /** Source-state proof request for the nested Solana AccountsLtHash proof. */
  public record AccountsLtHashProofRequest(
      int version,
      String proofFamily,
      String circuitId,
      String parameterSet,
      int sourceDomain,
      String finalizedSlot,
      String parentSlot,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String accountsLtHashProofPublicInputsHash,
      String openedAccountsLtHashContributionsHash,
      String openedAccountsLtHashResidualChecksum,
      byte[] statementBytes,
      byte[] accountCommitmentBytes,
      byte[] verificationContextBytes,
      byte[] schemaDescriptor,
      List<List<String>> publicInputColumns,
      AccountsLtHashFastpqPublicInputs fastpqPublicInputs,
      List<AccountsLtHashFastpqTransition> fastpqTransitions) {
    public AccountsLtHashProofRequest {
      statementBytes = statementBytes == null ? new byte[0] : Arrays.copyOf(statementBytes, statementBytes.length);
      accountCommitmentBytes =
          accountCommitmentBytes == null
              ? new byte[0]
              : Arrays.copyOf(accountCommitmentBytes, accountCommitmentBytes.length);
      verificationContextBytes =
          verificationContextBytes == null
              ? new byte[0]
              : Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
      schemaDescriptor =
          schemaDescriptor == null ? new byte[0] : Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
      publicInputColumns = copyStringColumns(publicInputColumns);
      fastpqTransitions =
          fastpqTransitions == null
              ? Collections.emptyList()
              : Collections.unmodifiableList(new ArrayList<>(fastpqTransitions));
    }

    @Override
    public byte[] statementBytes() {
      return Arrays.copyOf(statementBytes, statementBytes.length);
    }

    @Override
    public byte[] accountCommitmentBytes() {
      return Arrays.copyOf(accountCommitmentBytes, accountCommitmentBytes.length);
    }

    @Override
    public byte[] verificationContextBytes() {
      return Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
    }

    @Override
    public byte[] schemaDescriptor() {
      return Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
    }
  }

  /** Source-state verification proof capsule generated by a user-side prover. */
  public record SourceStateVerificationProof(
      int version, String proofFamily, String circuitId, byte[] proofBytes) {
    public SourceStateVerificationProof {
      proofFamily = Objects.requireNonNull(proofFamily, "proofFamily");
      circuitId = Objects.requireNonNull(circuitId, "circuitId");
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
    }

    public SourceStateVerificationProof(final String circuitId, final byte[] proofBytes) {
      this(1, "stark-fri-v1", circuitId, proofBytes);
    }

    public String proofBase64() {
      return Base64.getEncoder().encodeToString(proofBytes);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }
  }

  /** Solana full light-client audit role proven by a user-side prover. */
  public enum FullLightClientAuditRole {
    /** Verifies rooted Tower lockouts and replay over the voted bank fork. */
    TOWER_REPLAY,
    /** Verifies the full AccountsDB lattice around the nested AccountsLtHash proof. */
    FULL_ACCOUNTSDB_LATTICE,
    /** Verifies bank hash and fork-choice state for the finalized vote. */
    BANK_FORK_CHOICE
  }

  /** Input required to build Solana full light-client audit proof requests on Android clients. */
  public record FullLightClientAuditProofInput(
      WitnessInput witnessInput,
      OpenedAccountsLtHashContributionsInput openedAccounts,
      SourceStateVerificationProof accountsLtHashProof,
      String rootedSlot,
      String[] towerVoteSlots,
      String epoch,
      String epochStakeRoot,
      String stakeActivationHash,
      String stakeAccountStateHash,
      String stakeHistoryHash,
      String stakeHistorySysvarAccountHash,
      String sourceTrustAnchorHash,
      String consensusVerifierHash,
      String messageInclusionVerifierHash,
      String finalityPolicyHash,
      String sourceAdapterDeploymentReceiptHash,
      String solanaTowerReplayVerifierHash,
      String solanaFullAccountsdbLatticeVerifierHash,
      String solanaBankForkChoiceVerifierHash,
      String adapterVerifierVkHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterDeploymentHash,
      String fullLightClientGateHash,
      String finalityContextHash,
      String voteMessageHash,
      String accountsLtHashProofHash,
      String openedAccountsLtHashContributionsHash,
      String openedAccountsLtHashResidualChecksum,
      String towerLockoutHash,
      String towerReplayHash,
      String bankForkHash) {
    public FullLightClientAuditProofInput {
      witnessInput = Objects.requireNonNull(witnessInput, "witnessInput");
      openedAccounts = Objects.requireNonNull(openedAccounts, "openedAccounts");
      accountsLtHashProof = Objects.requireNonNull(accountsLtHashProof, "accountsLtHashProof");
      towerVoteSlots = towerVoteSlots == null ? new String[0] : Arrays.copyOf(towerVoteSlots, towerVoteSlots.length);
    }

    @Override
    public String[] towerVoteSlots() {
      return Arrays.copyOf(towerVoteSlots, towerVoteSlots.length);
    }
  }

  /** FastPQ public inputs bound to a Solana full light-client audit role proof request. */
  public record FullLightClientAuditFastpqPublicInputs(
      String dsid, String slot, String oldRoot, String newRoot, String permRoot, String txSetHash) {}

  /** One FastPQ transition supplied to a Solana full light-client audit role prover. */
  public record FullLightClientAuditFastpqTransition(
      String key, String operation, byte[] oldValue, byte[] newValue) {
    public FullLightClientAuditFastpqTransition {
      oldValue = oldValue == null ? new byte[0] : Arrays.copyOf(oldValue, oldValue.length);
      newValue = newValue == null ? new byte[0] : Arrays.copyOf(newValue, newValue.length);
    }

    @Override
    public byte[] oldValue() {
      return Arrays.copyOf(oldValue, oldValue.length);
    }

    @Override
    public byte[] newValue() {
      return Arrays.copyOf(newValue, newValue.length);
    }
  }

  /** OpenVerify request for one Solana full light-client audit role proof. */
  public record FullLightClientAuditProofRequest(
      int version,
      String proofFamily,
      String circuitId,
      String parameterSet,
      String role,
      int roleCode,
      int sourceDomain,
      String finalizedSlot,
      String verifierId,
      String verifierHash,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterDeploymentHash,
      String fullLightClientGateHash,
      String finalityContextHash,
      String voteMessageHash,
      String accountsLtHashProofHash,
      String auditStatementHash,
      byte[] statementBytes,
      byte[] verificationContextBytes,
      byte[] schemaDescriptor,
      List<List<String>> publicInputColumns,
      FullLightClientAuditFastpqPublicInputs fastpqPublicInputs,
      List<FullLightClientAuditFastpqTransition> fastpqTransitions) {
    public FullLightClientAuditProofRequest {
      statementBytes = statementBytes == null ? new byte[0] : Arrays.copyOf(statementBytes, statementBytes.length);
      verificationContextBytes =
          verificationContextBytes == null
              ? new byte[0]
              : Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
      schemaDescriptor =
          schemaDescriptor == null ? new byte[0] : Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
      publicInputColumns = copyStringColumns(publicInputColumns);
      fastpqTransitions =
          fastpqTransitions == null
              ? Collections.emptyList()
              : Collections.unmodifiableList(new ArrayList<>(fastpqTransitions));
    }

    @Override
    public byte[] statementBytes() {
      return Arrays.copyOf(statementBytes, statementBytes.length);
    }

    @Override
    public byte[] verificationContextBytes() {
      return Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
    }

    @Override
    public byte[] schemaDescriptor() {
      return Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
    }
  }

  /** Role-separated Solana full light-client audit proof requests. */
  public record FullLightClientAuditProofRequests(
      FullLightClientAuditProofRequest towerReplay,
      FullLightClientAuditProofRequest fullAccountsdbLattice,
      FullLightClientAuditProofRequest bankForkChoice) {}

  /** Solana destination ProgramData evidence collected by UI code before route canary submission. */
  public record RouteCanaryEvidenceInput(
      String routeAllowlistHash,
      String destinationBindingHash,
      String expectedDestinationBindingHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterEngineDeploymentHash,
      String verifierIdentity,
      String verifierCodeHash,
      String solanaRpcCommitment,
      String solanaProgramOwner,
      String solanaProgramdataOwner,
      boolean solanaProgramImmutable,
      String solanaProgramAccountDataBase64,
      String solanaProgramdataAddress,
      String solanaProgramdataSlot,
      String solanaExpectedProgramdataSlot,
      String solanaProgramAccountContextSlot,
      String solanaProgramdataAccountContextSlot,
      String solanaProgramdataMetadataBlake2b256,
      String solanaProgramdataMetadataBase64,
      String solanaProgramdataExecutableBlake2b256,
      String solanaProgramdataExecutableBase64) {
    public RouteCanaryEvidenceInput {
      Objects.requireNonNull(routeAllowlistHash, "routeAllowlistHash");
      Objects.requireNonNull(destinationBindingHash, "destinationBindingHash");
      Objects.requireNonNull(sourceVerifierMaterialHash, "sourceVerifierMaterialHash");
      Objects.requireNonNull(sourceAdapterEngineDeploymentHash, "sourceAdapterEngineDeploymentHash");
      Objects.requireNonNull(verifierIdentity, "verifierIdentity");
      Objects.requireNonNull(verifierCodeHash, "verifierCodeHash");
      Objects.requireNonNull(solanaRpcCommitment, "solanaRpcCommitment");
      Objects.requireNonNull(solanaProgramOwner, "solanaProgramOwner");
      Objects.requireNonNull(solanaProgramdataOwner, "solanaProgramdataOwner");
      Objects.requireNonNull(solanaProgramAccountDataBase64, "solanaProgramAccountDataBase64");
      Objects.requireNonNull(solanaProgramdataAddress, "solanaProgramdataAddress");
      Objects.requireNonNull(solanaProgramdataSlot, "solanaProgramdataSlot");
      Objects.requireNonNull(solanaExpectedProgramdataSlot, "solanaExpectedProgramdataSlot");
      Objects.requireNonNull(solanaProgramAccountContextSlot, "solanaProgramAccountContextSlot");
      Objects.requireNonNull(
          solanaProgramdataAccountContextSlot, "solanaProgramdataAccountContextSlot");
      Objects.requireNonNull(
          solanaProgramdataMetadataBlake2b256, "solanaProgramdataMetadataBlake2b256");
      Objects.requireNonNull(solanaProgramdataMetadataBase64, "solanaProgramdataMetadataBase64");
      Objects.requireNonNull(
          solanaProgramdataExecutableBlake2b256, "solanaProgramdataExecutableBlake2b256");
      Objects.requireNonNull(
          solanaProgramdataExecutableBase64, "solanaProgramdataExecutableBase64");
    }
  }

  /** Raw Solana SCCP witness data collected by portal or mobile UI code. */
  public record WitnessInput(
      int targetDomain,
      String mainnetGenesisHash,
      String finalizedSlot,
      String parentSlot,
      String bankSignatureCount,
      String parentBankHash,
      String blockhash,
      String bankHash,
      String transactionStatusRoot,
      String messageProofHash,
      String accountInclusionRoot,
      String accountsLtHashChecksum,
      String accountsLtHashProofPublicInputsHash,
      byte[] bankHashHardForkData,
      byte[] accountsLtHash,
      String transactionSignature,
      String emitterProgramId,
      String messageId,
      String payloadHash,
      String commitmentRoot,
      String sourceEventDigest,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String statementHash,
      String destinationBindingHash,
      byte[][] inclusionBranch,
      String sourceAdapterDeploymentHash,
      String sourceAdapterDeploymentReceiptHash) {
    public WitnessInput(
        final int targetDomain,
        final String mainnetGenesisHash,
        final String finalizedSlot,
        final String parentSlot,
        final String bankSignatureCount,
        final String parentBankHash,
        final String blockhash,
        final String bankHash,
        final String transactionStatusRoot,
        final String messageProofHash,
        final String accountInclusionRoot,
        final String accountsLtHashChecksum,
        final String accountsLtHashProofPublicInputsHash,
        final byte[] bankHashHardForkData,
        final byte[] accountsLtHash,
        final String transactionSignature,
        final String emitterProgramId,
        final String messageId,
        final String payloadHash,
        final String commitmentRoot,
        final String sourceEventDigest,
        final String statementHash,
        final String destinationBindingHash,
        final byte[][] inclusionBranch,
        final String sourceAdapterDeploymentHash,
        final String sourceAdapterDeploymentReceiptHash) {
      this(
          targetDomain,
          mainnetGenesisHash,
          finalizedSlot,
          parentSlot,
          bankSignatureCount,
          parentBankHash,
          blockhash,
          bankHash,
          transactionStatusRoot,
          messageProofHash,
          accountInclusionRoot,
          accountsLtHashChecksum,
          accountsLtHashProofPublicInputsHash,
          bankHashHardForkData,
          accountsLtHash,
          transactionSignature,
          emitterProgramId,
          messageId,
          payloadHash,
          commitmentRoot,
          sourceEventDigest,
          MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1,
          ZERO_HASH_V1,
          statementHash,
          destinationBindingHash,
          inclusionBranch,
          sourceAdapterDeploymentHash,
          sourceAdapterDeploymentReceiptHash);
    }

    public WitnessInput {
      bankHashHardForkData =
          bankHashHardForkData == null
              ? new byte[0]
              : Arrays.copyOf(bankHashHardForkData, bankHashHardForkData.length);
      accountsLtHash =
          accountsLtHash == null ? null : Arrays.copyOf(accountsLtHash, accountsLtHash.length);
      inclusionBranch = copyBranch(inclusionBranch);
      sourceStateVerifierId =
          sourceStateVerifierId == null ? MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1 : sourceStateVerifierId;
      sourceStateVerifierHash =
          sourceStateVerifierHash == null ? ZERO_HASH_V1 : sourceStateVerifierHash;
      sourceAdapterDeploymentHash =
          sourceAdapterDeploymentHash == null ? ZERO_HASH_V1 : sourceAdapterDeploymentHash;
      sourceAdapterDeploymentReceiptHash =
          sourceAdapterDeploymentReceiptHash == null
              ? ZERO_HASH_V1
              : sourceAdapterDeploymentReceiptHash;
    }

    @Override
    public byte[] bankHashHardForkData() {
      return Arrays.copyOf(bankHashHardForkData, bankHashHardForkData.length);
    }

    @Override
    public byte[] accountsLtHash() {
      return accountsLtHash == null ? null : Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    }

    @Override
    public byte[][] inclusionBranch() {
      return copyBranch(inclusionBranch);
    }
  }

  /** Canonical Solana SCCP witness passed into local proof generation. */
  public record Witness(
      int version,
      int sourceDomain,
      int targetDomain,
      String mainnetGenesisHash,
      String finalizedSlot,
      String parentSlot,
      String bankSignatureCount,
      String parentBankHash,
      String blockhash,
      String bankHash,
      String transactionStatusRoot,
      String messageProofHash,
      String accountInclusionRoot,
      String accountsLtHashChecksum,
      String accountsLtHashProofPublicInputsHash,
      byte[] bankHashHardForkData,
      byte[] accountsLtHash,
      String transactionSignature,
      String emitterProgramId,
      String messageId,
      String payloadHash,
      String commitmentRoot,
      String sourceEventDigest,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String sourceAdapterDeploymentHash,
      String sourceAdapterDeploymentReceiptHash,
      byte[][] inclusionBranch) {
    public Witness {
      bankHashHardForkData =
          bankHashHardForkData == null
              ? new byte[0]
              : Arrays.copyOf(bankHashHardForkData, bankHashHardForkData.length);
      accountsLtHash =
          accountsLtHash == null ? null : Arrays.copyOf(accountsLtHash, accountsLtHash.length);
      inclusionBranch = copyBranch(inclusionBranch);
    }

    @Override
    public byte[] bankHashHardForkData() {
      return Arrays.copyOf(bankHashHardForkData, bankHashHardForkData.length);
    }

    @Override
    public byte[] accountsLtHash() {
      return accountsLtHash == null ? null : Arrays.copyOf(accountsLtHash, accountsLtHash.length);
    }

    @Override
    public byte[][] inclusionBranch() {
      return copyBranch(inclusionBranch);
    }
  }

  /** Public inputs exposed by the Solana SCCP proof request. */
  public record PublicInputs(
      String messageId,
      String payloadHash,
      String commitmentRoot,
      String finalizedSlot,
      String parentSlot,
      String bankSignatureCount,
      String parentBankHash,
      String blockhash,
      String bankHash,
      String transactionStatusRoot,
      String messageProofHash,
      String accountInclusionRoot,
      String accountsLtHashChecksum,
      String accountsLtHashProofPublicInputsHash,
      String sourceEventDigest,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String statementHash,
      String destinationBindingHash,
      String sourceAdapterDeploymentHash,
      String sourceAdapterDeploymentReceiptHash,
      String sourceAdapterDeploymentBindingHash) {}

  /** Statement and verifier deployment context proved by the local Solana SCCP prover. */
  public record ProofContext(int version, String statementHash, String destinationBindingHash) {}

  /** Source-adapter deployment binding carried by local Solana SCCP proof requests. */
  public record SourceAdapterDeploymentBinding(
      int version,
      int sourceDomain,
      int targetDomain,
      String sourceAdapterDeploymentHash,
      String sourceAdapterDeploymentReceiptHash) {}

  /** Request passed to a linked local Solana SCCP prover. */
  public record ProofRequest(
      int version,
      String backend,
      int sourceDomain,
      int targetDomain,
      String mainnetGenesisHash,
      String witnessHash,
      String proofContextHash,
      String sourceAdapterDeploymentBindingHash,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      PublicInputs publicInputs,
      Witness witness,
      ProofContext proofContext,
      SourceAdapterDeploymentBinding sourceAdapterDeploymentBinding) {}

  /** Proof envelope returned after local Solana SCCP proof generation. */
  public record ProofResult(
      int version,
      String backend,
      byte[] proofBytes,
      String proofBase64,
      PublicInputs publicInputs,
      String witnessHash,
      String proofContextHash,
      String sourceAdapterDeploymentBindingHash,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      ProofContext proofContext,
      SourceAdapterDeploymentBinding sourceAdapterDeploymentBinding,
      String envelopeHash) {
    public ProofResult {
      proofBytes = Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }
  }

  /** Transparent SCCP public inputs serialized into Solana verifier instruction data. */
  public record SubmissionPublicInputs(
      int version,
      String messageId,
      String payloadHash,
      int targetDomain,
      String commitmentRoot,
      String finalityHeight,
      String finalityBlockHash) {
    public SubmissionPublicInputs(
        final String messageId,
        final String payloadHash,
        final int targetDomain,
        final String commitmentRoot,
        final String finalityHeight,
        final String finalityBlockHash) {
      this(1, messageId, payloadHash, targetDomain, commitmentRoot, finalityHeight, finalityBlockHash);
    }
  }

  /** Inputs for a Solana SCCP verifier program instruction. */
  public record SubmissionInput(
      SubmissionPublicInputs publicInputs,
      byte[] proofBytes,
      byte[] bundleBytes,
      String statementHash,
      String destinationBindingHash,
      String proofContextHash,
      ProofResult proofResult) {
    public SubmissionInput {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      bundleBytes = Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
    }

    public SubmissionInput(
        final SubmissionPublicInputs publicInputs,
        final byte[] proofBytes,
        final byte[] bundleBytes,
        final String statementHash,
        final String destinationBindingHash) {
      this(publicInputs, proofBytes, bundleBytes, statementHash, destinationBindingHash, null, null);
    }

    public SubmissionInput(
        final SubmissionPublicInputs publicInputs,
        final byte[] proofBytes,
        final byte[] bundleBytes,
        final String statementHash,
        final String destinationBindingHash,
        final String proofContextHash) {
      this(publicInputs, proofBytes, bundleBytes, statementHash, destinationBindingHash, proofContextHash, null);
    }

    public SubmissionInput(
        final SubmissionPublicInputs publicInputs,
        final ProofResult proofResult,
        final byte[] bundleBytes) {
      this(
          publicInputs,
          requireWrappedProofResultForSubmission(proofResult, publicInputs).proofBytes(),
          bundleBytes,
          proofResult.proofContext().statementHash(),
          proofResult.proofContext().destinationBindingHash(),
          proofResult.proofContextHash(),
          proofResult);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }
  }

  /** One Solana SCCP submission argument in Rust template order. */
  public record SubmissionArgument(String key, String encoding, String bytesHex) {}

  /** Prebuilt Solana SCCP verifier instruction data for wallet or RPC submission. */
  public record Submission(
      int version,
      String envelopeEncoding,
      String submissionKind,
      String verifierEntrypoint,
      byte[] proofBytes,
      SubmissionPublicInputs publicInputs,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      String statementHash,
      String destinationBindingHash,
      String proofContextHash,
      List<SubmissionArgument> arguments,
      byte[] instructionData,
      String instructionDataHex,
      byte[] envelopeBytes,
      String envelopeHex) {
    public Submission {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      publicInputsBytes =
          Arrays.copyOf(Objects.requireNonNull(publicInputsBytes, "publicInputsBytes"), publicInputsBytes.length);
      bundleBytes = Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      arguments = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(arguments, "arguments")));
      instructionData =
          Arrays.copyOf(Objects.requireNonNull(instructionData, "instructionData"), instructionData.length);
      envelopeBytes = Arrays.copyOf(Objects.requireNonNull(envelopeBytes, "envelopeBytes"), envelopeBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] publicInputsBytes() {
      return Arrays.copyOf(publicInputsBytes, publicInputsBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] instructionData() {
      return Arrays.copyOf(instructionData, instructionData.length);
    }

    @Override
    public byte[] envelopeBytes() {
      return Arrays.copyOf(envelopeBytes, envelopeBytes.length);
    }
  }
}
