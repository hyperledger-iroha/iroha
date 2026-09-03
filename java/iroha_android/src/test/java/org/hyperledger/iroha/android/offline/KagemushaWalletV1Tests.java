// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertSame;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.lang.reflect.Method;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceIntentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAccountIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAggregateStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAmountPolicyV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAssetDefinitionIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAssetIncarnationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDevicePublicKeyV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSignatureV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareCapabilityV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareCredentialV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareMintStageV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwarePaymentStageV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwarePlatformClassV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareQualificationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareReceiveFoldBatchV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareRecoveryV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareTerminalResultV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareProfileV1;
import org.hyperledger.iroha.sdk.offline.KagemushaOpenReceiveV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestModeV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaStagedPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaWireV1;
import org.junit.Test;

/** Java mirror checks for aggregate-balance KAGEMUSHA V1 orchestration. */
public final class KagemushaWalletV1Tests {
  @Test
  public void freshWalletUsesThePersistedPostBootstrapSnapshot() {
    for (final BigInteger revision : Arrays.asList(BigInteger.ZERO, BigInteger.ONE)) {
      final FoldProvider provider = new FoldProvider(0);
      provider.aggregateMissing = true;
      provider.bootstrapRevision = revision;

      final KagemushaWalletV1 wallet = KagemushaWalletV1.open(provider);

      assertEquals(1, provider.bootstrapCalls);
      assertEquals(2, provider.recoveryCalls);
      assertEquals(revision, wallet.journalRevision());
      assertArrayEquals(
          provider.recover().aggregateState(),
          KagemushaNoritoV1.encodeAggregateStateShape(wallet.aggregateState()));
    }
  }

  @Test
  public void bootstrapRejectsMissingSubstitutedAndInconsistentPersistedState() {
    for (final String failure : Arrays.asList("missing", "substituted", "revision")) {
      final FoldProvider provider = new FoldProvider(0);
      provider.aggregateMissing = true;
      provider.bootstrapRevision = BigInteger.ONE;
      provider.omitBootstrapPersistence = failure.equals("missing");
      provider.replaceStateAfterBootstrap = failure.equals("substituted");
      if (failure.equals("revision")) provider.journalReadOverride = BigInteger.valueOf(2);

      assertThrows(IllegalArgumentException.class, () -> KagemushaWalletV1.open(provider));

      assertEquals(1, provider.bootstrapCalls);
      assertEquals(2, provider.recoveryCalls);
    }
  }

  @Test
  public void recoveryNeverReinitializesAnExistingWalletWithMissingState() {
    final FoldProvider provider = new FoldProvider(0, BigInteger.valueOf(5), BigInteger.valueOf(5));
    final KagemushaWalletV1 wallet = KagemushaWalletV1.open(provider);
    final KagemushaAggregateStateCommitmentV1 previous = wallet.aggregateState();
    provider.aggregateMissing = true;

    assertThrows(IllegalArgumentException.class, wallet::recover);

    assertEquals(0, provider.bootstrapCalls);
    assertSame(previous, wallet.aggregateState());
    assertEquals(BigInteger.valueOf(5), wallet.journalRevision());
  }

  @Test
  public void recoveryRejectsRollbackReplacementAndCrossWalletSnapshots() {
    for (final String failure : Arrays.asList("rollback", "replacement", "wallet")) {
      final FoldProvider provider = new FoldProvider(0, BigInteger.valueOf(5), BigInteger.valueOf(5));
      final KagemushaWalletV1 wallet = KagemushaWalletV1.open(provider);
      final KagemushaAggregateStateCommitmentV1 previous = wallet.aggregateState();
      if (failure.equals("rollback")) {
        provider.revision = BigInteger.valueOf(4);
      } else if (failure.equals("replacement")) {
        provider.stateTag += 1;
      } else {
        provider.recoveredLaneTag = 0x75;
        provider.revision = BigInteger.valueOf(6);
      }

      assertThrows(IllegalArgumentException.class, wallet::recover);

      assertSame(previous, wallet.aggregateState());
      assertEquals(BigInteger.valueOf(5), wallet.journalRevision());
    }
  }

  @Test
  public void javaProviderIsTheCanonicalKotlinHardwareContract() {
    assertTrue(
        org.hyperledger.iroha.sdk.offline.KagemushaHardwareProviderV1.class
            .isAssignableFrom(KagemushaHardwareProviderV1.class));
    assertEquals(
        Arrays.asList(
            "EXACT_NEXT_PREDECESSOR_CONSUMPTION",
            "ONE_USE_SUCCESSOR_AUTHORIZATION",
            "ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL",
            "SEALED_TRANSITION_RECOVERY",
            "ONE_USE_ACCEPTANCE_TICKETS",
            "DURABLE_INBOX_RESERVATION",
            "AUTHENTICATED_INBOUND_STAGING",
            "AUTHORITATIVE_REPLAY_ROOT_RECOVERY",
            "SENDER_OUTBOX_RESERVATION",
            "AUTHENTICATED_DURABLE_RETRY_OUTBOX",
            "ATOMIC_VERIFIED_CANDIDATE_COMMIT",
            "RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE",
            "TRUSTED_TIME_OR_LEASE",
            "OFFLINE_HARDWARE_EPOCH_ROTATION",
            "ROLLBACK_SAFE_COUNTER_ROLLOVER",
            "NO_SOFTWARE_FALLBACK"),
        Arrays.stream(KagemushaHardwareCapabilityV1.values())
            .map(Enum::name)
            .collect(Collectors.toList()));
  }

  @Test
  public void facadeExposesReceiveFoldDrainSendRedeemRecoveryAndRotation() throws Exception {
    final Set<String> methods =
        Arrays.stream(KagemushaWalletV1.class.getDeclaredMethods())
            .map(Method::getName)
            .collect(Collectors.toSet());
    assertEquals(
        Arrays.asList(
            "open",
            "recover",
            "journalRevision",
            "hardwareCredential",
            "aggregateState",
            "createPaymentRequest",
            "prepareAcceptanceIntent",
            "recoverAcceptanceIntent",
            "issueAcceptanceTicket",
            "recoverAcceptanceTicket",
            "stagePayment",
            "stageMintCredit",
            "foldReceiveBatch",
            "drainPendingCredits",
            "send",
            "recoverPayment",
            "recordAcknowledgement",
            "redeem",
            "recoverRedemption",
            "rotateHardwareEpoch")
            .stream()
            .collect(Collectors.toSet()),
        methods);
    assertEquals(
        BigInteger.class,
        KagemushaWalletV1.class.getMethod("drainPendingCredits").getReturnType());
  }

  @Test
  public void javaFacadeUsesCompactIntentAcrossThePeerExchange() throws Exception {
    final Class<KagemushaWalletV1> walletType = KagemushaWalletV1.class;
    final Class<KagemushaPaymentRequestV1> requestType = KagemushaPaymentRequestV1.class;
    final Class<KagemushaAcceptanceIntentV1> intentType = KagemushaAcceptanceIntentV1.class;
    final Class<KagemushaAcceptanceTicketV1> ticketType = KagemushaAcceptanceTicketV1.class;
    assertEquals(
        intentType,
        walletType
            .getMethod("prepareAcceptanceIntent", requestType, BigInteger.class)
            .getReturnType());
    assertEquals(
        intentType,
        walletType.getMethod("recoverAcceptanceIntent", requestType, byte[].class).getReturnType());
    assertEquals(
        ticketType,
        walletType.getMethod("issueAcceptanceTicket", requestType, intentType).getReturnType());
    assertEquals(
        ticketType,
        walletType
            .getMethod("recoverAcceptanceTicket", requestType, intentType, byte[].class)
            .getReturnType());
    assertEquals(
        KagemushaPaymentV1.class,
        walletType.getMethod("send", requestType, intentType, ticketType).getReturnType());
    assertEquals(
        KagemushaStagedPaymentV1.class,
        walletType
            .getMethod("stagePayment", requestType, intentType, ticketType, KagemushaPaymentV1.class)
            .getReturnType());
    assertEquals(
        KagemushaPaymentV1.class,
        walletType
            .getMethod("recoverPayment", requestType, intentType, ticketType, byte[].class)
            .getReturnType());
    assertEquals(
        void.class,
        walletType
            .getMethod(
                "recordAcknowledgement",
                requestType,
                intentType,
                ticketType,
                KagemushaPaymentV1.class,
                KagemushaAcknowledgementV1.class)
            .getReturnType());
  }

  @Test
  public void javaFacadeDrainsMoreThanSixteenCreditsFromOneStableSnapshot() {
    final FoldProvider provider = new FoldProvider(33);
    final KagemushaWalletV1 wallet = KagemushaWalletV1.open(provider);

    assertEquals(BigInteger.valueOf(33), wallet.drainPendingCredits());
    assertEquals(Arrays.asList(16, 16, 1, 0), provider.foldOccupancies);
    assertEquals(1, provider.watermarkCalls);
    assertEquals(0, provider.remainingCredits);
    assertEquals(BigInteger.valueOf(3), wallet.journalRevision());
    assertEquals(BigInteger.valueOf(3), wallet.aggregateState().sequence);
  }

  @Test
  public void javaFacadeRejectsPreparedIntentAmountSubstitution() {
    final KagemushaPaymentRequestV1 request = Fixture.paymentRequest();
    final BigInteger requestedAmount = BigInteger.valueOf(25);
    final BigInteger substitutedAmount = BigInteger.valueOf(26);
    // Both amounts fit the reusable request. Only the caller's exact amount
    // distinguishes them; rejecting the request mode would miss this regression.
    assertTrue(request.requestMode.acceptsPaymentAmount(requestedAmount));
    assertTrue(request.requestMode.acceptsPaymentAmount(substitutedAmount));
    final KagemushaWalletV1 wallet = KagemushaWalletV1.open(new FoldProvider(0));
    assertEquals(
        requestedAmount, wallet.prepareAcceptanceIntent(request, requestedAmount).exactAmount);

    final FoldProvider provider = new FoldProvider(0);
    provider.preparedAmountOverride = substitutedAmount;
    final KagemushaWalletV1 substituted = KagemushaWalletV1.open(provider);
    final IllegalArgumentException failure =
        assertThrows(
            IllegalArgumentException.class,
            () -> substituted.prepareAcceptanceIntent(request, requestedAmount));
    assertEquals("prepared intent changed the requested amount", failure.getMessage());
  }

  @Test
  public void javaFacadeRotatesExhaustedCountersBeforeFoldingPendingCredits() {
    final BigInteger maximumCounter = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
    final FoldProvider provider = new FoldProvider(17, maximumCounter, maximumCounter);
    final KagemushaWalletV1 wallet = KagemushaWalletV1.open(provider);
    final KagemushaHardwareCredentialV1 previousCredential = wallet.hardwareCredential();
    assertEquals(maximumCounter, wallet.journalRevision());
    assertEquals(maximumCounter, wallet.aggregateState().sequence);

    final KagemushaAggregateStateCommitmentV1 rotated = wallet.rotateHardwareEpoch();

    assertEquals(1, provider.rotationCalls);
    assertTrue(provider.foldOccupancies.isEmpty());
    assertEquals(0, provider.watermarkCalls);
    assertEquals(17, provider.remainingCredits);
    assertEquals(BigInteger.ZERO, rotated.sequence);
    assertEquals(BigInteger.ZERO, wallet.journalRevision());
    final KagemushaHardwareCredentialV1 rotatedCredential = wallet.hardwareCredential();
    assertEquals(
        previousCredential.hardwareEpochGeneration + 1,
        rotatedCredential.hardwareEpochGeneration);
    assertFalse(
        Arrays.equals(previousCredential.credentialId(), rotatedCredential.credentialId()));
    assertFalse(
        Arrays.equals(previousCredential.hardwareEpochId(), rotatedCredential.hardwareEpochId()));
    assertArrayEquals(rotatedCredential.hardwareEpochId(), rotated.hardwareEpochId());
    assertArrayEquals(
        provider.qualification().credential.credentialId(), rotatedCredential.credentialId());

    assertEquals(BigInteger.valueOf(17), wallet.drainPendingCredits());
    assertEquals(Arrays.asList(16, 1, 0), provider.foldOccupancies);
    assertEquals(1, provider.watermarkCalls);
    assertEquals(0, provider.remainingCredits);
    assertEquals(BigInteger.valueOf(2), wallet.journalRevision());
    assertEquals(BigInteger.valueOf(2), wallet.aggregateState().sequence);
  }

  @Test
  public void javaFacadePreservesItsSnapshotWhenTheNativeFoldIsInvalid() {
    // Cover both counters and a non-progressing commitment without claiming
    // that the host can roll back a malformed native provider's durable state.
    for (final int[] failure :
        new int[][] {{0, 1, 0}, {2, 1, 0}, {1, 0, 0}, {1, 2, 0}, {1, 1, 1}}) {
      final FoldProvider provider = new FoldProvider(1);
      provider.foldRevisionStep = failure[0];
      provider.foldSequenceStep = failure[1];
      provider.preserveStateCommitment = failure[2] != 0;
      final KagemushaWalletV1 wallet = KagemushaWalletV1.open(provider);
      final KagemushaAggregateStateCommitmentV1 previousState = wallet.aggregateState();
      final byte[] previousBytes = KagemushaNoritoV1.encodeAggregateStateShape(previousState);
      final BigInteger previousRevision = wallet.journalRevision();

      assertThrows(IllegalArgumentException.class, wallet::foldReceiveBatch);

      assertEquals(Arrays.asList(1), provider.foldOccupancies);
      assertSame(previousState, wallet.aggregateState());
      assertArrayEquals(
          previousBytes, KagemushaNoritoV1.encodeAggregateStateShape(wallet.aggregateState()));
      assertEquals(previousRevision, wallet.journalRevision());
    }
  }

  private static final class FoldProvider implements KagemushaHardwareProviderV1 {
    private int remainingCredits;
    private int watermarkCalls;
    private int rotationCalls;
    private final List<Integer> foldOccupancies = new ArrayList<>();
    private BigInteger revision;
    private BigInteger sequence;
    private KagemushaHardwareQualificationV1 activeQualification = Fixture.QUALIFICATION;
    private int stateTag = 0x51;
    private int foldRevisionStep = 1;
    private int foldSequenceStep = 1;
    private boolean preserveStateCommitment;
    private BigInteger preparedAmountOverride;
    private boolean aggregateMissing;
    private int bootstrapCalls;
    private int recoveryCalls;
    private BigInteger bootstrapRevision = BigInteger.ZERO;
    private BigInteger journalReadOverride;
    private boolean omitBootstrapPersistence;
    private boolean replaceStateAfterBootstrap;
    private int recoveredLaneTag = 0x52;

    private FoldProvider(final int pending) {
      this(pending, BigInteger.ZERO, BigInteger.ZERO);
    }

    private FoldProvider(
        final int pending, final BigInteger initialRevision, final BigInteger initialSequence) {
      this.remainingCredits = pending;
      this.revision = initialRevision;
      this.sequence = initialSequence;
    }

    @Override
    public KagemushaHardwareQualificationV1 qualification() {
      return activeQualification;
    }

    @Override
    public KagemushaHardwareRecoveryV1 recover() {
      recoveryCalls += 1;
      return new KagemushaHardwareRecoveryV1(
          aggregateMissing ? null : stateBytes(),
          revision,
          BigInteger.valueOf(remainingCredits),
          BigInteger.ZERO);
    }

    @Override
    public byte[] bootstrapState() {
      bootstrapCalls += 1;
      revision = bootstrapRevision;
      aggregateMissing = omitBootstrapPersistence;
      final byte[] result = stateBytes();
      if (replaceStateAfterBootstrap) stateTag += 1;
      return result;
    }

    @Override
    public byte[] createPaymentRequest(
        final byte[] recipientAccount,
        final KagemushaPaymentRequestModeV1 requestMode,
        final long validityWindowMillis) {
      throw unused();
    }

    @Override
    public byte[] prepareAcceptanceIntent(
        final byte[] canonicalRequest, final BigInteger exactAmount) {
      final KagemushaPaymentRequestV1 request =
          KagemushaNoritoV1.decodePaymentRequestShapeExact(canonicalRequest);
      return KagemushaNoritoV1.encodeAcceptanceIntentShape(
          new KagemushaAcceptanceIntentV1(
              1,
              KagemushaNoritoV1.paymentRequestDigest(request),
              bytes(0x54),
              preparedAmountOverride == null ? exactAmount : preparedAmountOverride,
              bytes(0x55)),
          request);
    }

    @Override
    public byte[] recoverAcceptanceIntent(final byte[] intentId) {
      return null;
    }

    @Override
    public byte[] issueAcceptanceTicket(
        final byte[] canonicalRequest, final byte[] canonicalIntent) {
      throw unused();
    }

    @Override
    public byte[] recoverAcceptanceTicket(final byte[] acceptanceTicketId) {
      return null;
    }

    @Override
    public KagemushaHardwarePaymentStageV1 stagePayment(
        final byte[] canonicalRequest,
        final byte[] canonicalIntent,
        final byte[] canonicalTicket,
        final byte[] canonicalPayment) {
      throw unused();
    }

    @Override
    public KagemushaHardwareMintStageV1 stageMintCredit(
        final byte[] canonicalAuthorization, final byte[] canonicalMintCredit) {
      throw unused();
    }

    @Override
    public BigInteger pendingCreditWatermark() {
      watermarkCalls += 1;
      return BigInteger.valueOf(100L + remainingCredits);
    }

    @Override
    public BigInteger journalRevision() {
      return journalReadOverride == null ? revision : journalReadOverride;
    }

    @Override
    public KagemushaHardwareReceiveFoldBatchV1 foldReceiveBatch(
        final BigInteger inboxSequenceInclusive) {
      assertTrue(inboxSequenceInclusive.signum() >= 0);
      final int occupancy = Math.min(KagemushaWireV1.RECEIVE_FOLD_BATCH_SIZE, remainingCredits);
      foldOccupancies.add(occupancy);
      if (occupancy == 0) {
        return null;
      }
      remainingCredits -= occupancy;
      revision = revision.add(BigInteger.valueOf(foldRevisionStep));
      sequence = sequence.add(BigInteger.valueOf(foldSequenceStep));
      if (!preserveStateCommitment) {
        stateTag += 1;
      }
      return new KagemushaHardwareReceiveFoldBatchV1(stateBytes(), occupancy);
    }

    @Override
    public KagemushaHardwareTerminalResultV1 commitPayment(
        final byte[] canonicalRequest,
        final byte[] canonicalIntent,
        final byte[] canonicalTicket) {
      throw unused();
    }

    @Override
    public byte[] recoverPayment(final byte[] creditId) {
      return null;
    }

    @Override
    public void recordAcknowledgement(
        final byte[] creditId,
        final byte[] canonicalRequest,
        final byte[] canonicalIntent,
        final byte[] canonicalTicket,
        final byte[] canonicalPayment,
        final byte[] canonicalAcknowledgement) {}

    @Override
    public KagemushaHardwareTerminalResultV1 commitRedemption(
        final BigInteger amount, final byte[] beneficiaryAccount) {
      throw unused();
    }

    @Override
    public byte[] recoverRedemption(final byte[] redemptionId) {
      return null;
    }

    @Override
    public byte[] rotateHardwareEpoch() {
      rotationCalls += 1;
      activeQualification =
          new KagemushaHardwareQualificationV1(
              activeQualification.protocolVersion,
              activeQualification.profile,
              Fixture.nextCredential(activeQualification.credential),
              activeQualification.releaseId(),
              activeQualification.capabilities());
      revision = BigInteger.ZERO;
      sequence = BigInteger.ZERO;
      stateTag += 1;
      return stateBytes();
    }

    private byte[] stateBytes() {
      return KagemushaNoritoV1.encodeAggregateStateShape(
          new KagemushaAggregateStateCommitmentV1(
              1,
              activeQualification.releaseId(),
              Fixture.NETWORK,
              Fixture.ASSET,
              Fixture.INCARNATION,
              4,
              Fixture.LIABILITY_POOL,
              bytes(recoveredLaneTag),
              activeQualification.credential.hardwareEpochId(),
              activeQualification.credential.deviceKeyReference(),
              activeQualification.profile.hardwareProfileId(),
              sequence,
              bytes(stateTag)));
    }

    private static UnsupportedOperationException unused() {
      return new UnsupportedOperationException("not used");
    }
  }

  private static final class Fixture {
    private static final NetworkId NETWORK = NetworkId.fromBytes(bytes(0x11));
    private static final KagemushaAssetDefinitionIdV1 ASSET =
        KagemushaAssetDefinitionIdV1.parse("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    private static final KagemushaAssetIncarnationV1 INCARNATION =
        new KagemushaAssetIncarnationV1(bytes(0x21));
    private static final KagemushaDevicePublicKeyV1 PUBLIC_KEY =
        new KagemushaDevicePublicKeyV1(
            hex(
                "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
                    + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"));
    private static final KagemushaDeviceSignatureV1 SIGNATURE = signature();
    private static final KagemushaHardwareProfileV1 PROFILE =
        new KagemushaHardwareProfileV1(
            1,
            1,
            bytes(0x31),
            bytes(0x32),
            KagemushaHardwarePlatformClassV1.ANDROID_OEM_SERVICE,
            bytes(0x33),
            bytes(0x34),
            bytes(0x35),
            bytes(0x36),
            bytes(0x37),
            1,
            PUBLIC_KEY,
            0xffff,
            bytes(0x38),
            0,
            100_000);
    private static final KagemushaHardwareCredentialV1 CREDENTIAL = credential();
    private static final KagemushaHardwareQualificationV1 QUALIFICATION =
        new KagemushaHardwareQualificationV1(
            1,
            PROFILE,
            CREDENTIAL,
            bytes(0x45),
            EnumSet.allOf(KagemushaHardwareCapabilityV1.class));
    private static final byte[] LIABILITY_POOL =
        KagemushaNoritoV1.liabilityPoolId(NETWORK, ASSET, INCARNATION);

    private static KagemushaPaymentRequestV1 paymentRequest() {
      // This uses the existing structural wallet fixture, not a real proof or
      // qualified hardware signature; the regression checks facade orchestration.
      return new KagemushaPaymentRequestV1(
          1,
          QUALIFICATION.releaseId(),
          NETWORK,
          ASSET,
          INCARNATION,
          4,
          LIABILITY_POOL,
          KagemushaAccountIdV1.parse(
              "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"),
          new KagemushaOpenReceiveV1(
              new KagemushaAmountPolicyV1(BigInteger.ONE, BigInteger.valueOf(100))),
          CREDENTIAL,
          bytes(0x53),
          1_000,
          2_000,
          SIGNATURE);
    }

    private static KagemushaHardwareCredentialV1 credential() {
      return new KagemushaHardwareCredentialV1(
          1,
          bytes(0x41),
          NETWORK,
          PROFILE.hardwareProfileId(),
          bytes(0x42),
          PROFILE.firmwarePolicyDigest(),
          PROFILE.policyEpoch,
          bytes(0x43),
          bytes(0x44),
          1,
          PUBLIC_KEY,
          KagemushaNoritoV1.deviceKeyReference(PUBLIC_KEY),
          10,
          90_000,
          SIGNATURE);
    }

    private static KagemushaHardwareCredentialV1 nextCredential(
        final KagemushaHardwareCredentialV1 previous) {
      return new KagemushaHardwareCredentialV1(
          previous.version,
          bytes(0x46),
          previous.networkId,
          previous.hardwareProfileId(),
          previous.suiteId(),
          previous.firmwarePolicyDigest(),
          previous.policyEpoch,
          previous.laneCommitment(),
          bytes(0x47),
          previous.hardwareEpochGeneration + 1,
          previous.devicePublicKey,
          previous.deviceKeyReference(),
          previous.issuedAtMs,
          previous.expiresAtMs,
          SIGNATURE);
    }

    private static KagemushaDeviceSignatureV1 signature() {
      final byte[] raw = new byte[64];
      raw[31] = 1;
      raw[63] = 1;
      return new KagemushaDeviceSignatureV1(raw);
    }
  }

  private static byte[] bytes(final int tag) {
    final byte[] result = new byte[32];
    Arrays.fill(result, (byte) tag);
    return result;
  }

  private static byte[] hex(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }
}
