// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertEquals;
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
import org.hyperledger.iroha.sdk.offline.OfflineCashAggregateStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAssetDefinitionIdV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAssetIncarnationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashDevicePublicKeyV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashDeviceSignatureV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareCapabilityV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareCredentialV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareFoldBatchV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareMintStageV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwarePaymentStageV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwarePlatformClassV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareQualificationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareRecoveryV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareTerminalResultV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareProfileV1;
import org.junit.Test;

/** Java migration-surface checks for aggregate-balance Offline Cash V1 orchestration. */
public final class OfflineCashWalletV1Tests {
  @Test
  public void javaProviderIsTheCanonicalKotlinHardwareContract() {
    assertTrue(
        org.hyperledger.iroha.sdk.offline.OfflineCashHardwareProviderV1.class
            .isAssignableFrom(OfflineCashHardwareProviderV1.class));
    assertEquals(16, OfflineCashHardwareCapabilityV1.values().length);
  }

  @Test
  public void facadeExposesFixedBatchDrainSendRedeemRecoveryAndRotation() throws Exception {
    final Set<String> methods =
        Arrays.stream(OfflineCashWalletV1.class.getDeclaredMethods())
            .map(Method::getName)
            .collect(Collectors.toSet());
    assertTrue(
        methods.containsAll(
            Arrays.asList(
                "open",
                "recover",
                "journalRevision",
                "hardwareCredential",
                "aggregateState",
                "createPaymentRequest",
                "authorizeAcceptanceIntent",
                "issueAcceptanceTicket",
                "stagePayment",
                "stageMintCredit",
                "foldPendingCreditBatch",
                "drainPendingCredits",
                "send",
                "recoverPayment",
                "recordAcknowledgement",
                "redeem",
                "recoverRedemption",
                "rotateHardwareEpoch")));
    assertEquals(
        BigInteger.class,
        OfflineCashWalletV1.class.getMethod("drainPendingCredits").getReturnType());
  }

  @Test
  public void javaFacadeDrainsMoreThanSixteenCreditsFromOneStableSnapshot() {
    final BatchProvider provider = new BatchProvider(33);
    final OfflineCashWalletV1 wallet = OfflineCashWalletV1.open(provider);

    assertEquals(BigInteger.valueOf(33), wallet.drainPendingCredits());
    assertEquals(Arrays.asList(16, 16, 1, 0), provider.foldResults);
    assertEquals(Arrays.asList(16, 16, 16, 16), provider.maximumArguments);
    assertEquals(1, provider.watermarkCalls);
    assertEquals(0, provider.remainingCredits);
    assertEquals(BigInteger.valueOf(3), wallet.journalRevision());
    assertEquals(BigInteger.valueOf(3), wallet.aggregateState().sequence);
  }

  private static final class BatchProvider implements OfflineCashHardwareProviderV1 {
    private int remainingCredits;
    private int watermarkCalls;
    private final List<Integer> foldResults = new ArrayList<>();
    private final List<Integer> maximumArguments = new ArrayList<>();
    private BigInteger revision = BigInteger.ZERO;
    private long sequence;
    private int stateTag = 0x51;

    private BatchProvider(final int pending) {
      this.remainingCredits = pending;
    }

    @Override
    public OfflineCashHardwareQualificationV1 qualification() {
      return Fixture.QUALIFICATION;
    }

    @Override
    public OfflineCashHardwareRecoveryV1 recover() {
      return new OfflineCashHardwareRecoveryV1(
          stateBytes(), revision, BigInteger.valueOf(remainingCredits), BigInteger.ZERO);
    }

    @Override
    public byte[] bootstrapState() {
      return stateBytes();
    }

    @Override
    public byte[] createPaymentRequest(
        final byte[] recipientAccount,
        final byte[] requestMode,
        final long validityWindowMillis) {
      throw unused();
    }

    @Override
    public byte[] createAcceptanceIntentAuthorization(
        final byte[] canonicalRequest, final BigInteger exactAmount) {
      throw unused();
    }

    @Override
    public byte[] issueAcceptanceTicket(
        final byte[] canonicalRequest, final byte[] canonicalAuthorization) {
      throw unused();
    }

    @Override
    public OfflineCashHardwarePaymentStageV1 stagePayment(
        final byte[] canonicalRequest, final byte[] canonicalPayment) {
      throw unused();
    }

    @Override
    public OfflineCashHardwareMintStageV1 stageMintCredit(
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
      return revision;
    }

    @Override
    public OfflineCashHardwareFoldBatchV1 foldPendingCreditBatch(
        final BigInteger inboxSequenceInclusive, final int maximumCredits) {
      assertTrue(inboxSequenceInclusive.signum() >= 0);
      maximumArguments.add(maximumCredits);
      final int folded = Math.min(remainingCredits, maximumCredits);
      foldResults.add(folded);
      if (folded == 0) {
        return new OfflineCashHardwareFoldBatchV1(0, null);
      }
      remainingCredits -= folded;
      revision = revision.add(BigInteger.ONE);
      sequence += 1;
      stateTag += 1;
      return new OfflineCashHardwareFoldBatchV1(folded, stateBytes());
    }

    @Override
    public OfflineCashHardwareTerminalResultV1 commitPayment(
        final byte[] canonicalRequest,
        final byte[] canonicalAuthorization,
        final byte[] canonicalTicket) {
      throw unused();
    }

    @Override
    public byte[] recoverPayment(final byte[] creditId) {
      return null;
    }

    @Override
    public void recordAcknowledgement(
        final byte[] creditId, final byte[] canonicalAcknowledgement) {}

    @Override
    public OfflineCashHardwareTerminalResultV1 commitRedemption(
        final BigInteger amount, final byte[] beneficiaryAccount) {
      throw unused();
    }

    @Override
    public byte[] recoverRedemption(final byte[] redemptionId) {
      return null;
    }

    @Override
    public byte[] rotateHardwareEpoch() {
      throw unused();
    }

    private byte[] stateBytes() {
      return OfflineCashNoritoV1.encodeAggregateStateShape(
          new OfflineCashAggregateStateCommitmentV1(
              1,
              Fixture.QUALIFICATION.releaseId(),
              Fixture.NETWORK,
              Fixture.ASSET,
              Fixture.INCARNATION,
              4,
              Fixture.LIABILITY_POOL,
              bytes(0x52),
              Fixture.CREDENTIAL.hardwareEpochId(),
              Fixture.CREDENTIAL.deviceKeyReference(),
              Fixture.PROFILE.hardwareProfileId(),
              BigInteger.valueOf(sequence),
              bytes(stateTag)));
    }

    private static UnsupportedOperationException unused() {
      return new UnsupportedOperationException("not used");
    }
  }

  private static final class Fixture {
    private static final NetworkId NETWORK = NetworkId.fromBytes(bytes(0x11));
    private static final OfflineCashAssetDefinitionIdV1 ASSET =
        OfflineCashAssetDefinitionIdV1.parse("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    private static final OfflineCashAssetIncarnationV1 INCARNATION =
        new OfflineCashAssetIncarnationV1(bytes(0x21));
    private static final OfflineCashDevicePublicKeyV1 PUBLIC_KEY =
        new OfflineCashDevicePublicKeyV1(
            hex(
                "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
                    + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"));
    private static final OfflineCashDeviceSignatureV1 SIGNATURE = signature();
    private static final OfflineCashHardwareProfileV1 PROFILE =
        new OfflineCashHardwareProfileV1(
            1,
            1,
            bytes(0x31),
            bytes(0x32),
            OfflineCashHardwarePlatformClassV1.ANDROID_OEM_SERVICE,
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
    private static final OfflineCashHardwareCredentialV1 CREDENTIAL = credential();
    private static final OfflineCashHardwareQualificationV1 QUALIFICATION =
        new OfflineCashHardwareQualificationV1(
            1,
            PROFILE,
            CREDENTIAL,
            bytes(0x45),
            EnumSet.allOf(OfflineCashHardwareCapabilityV1.class));
    private static final byte[] LIABILITY_POOL =
        OfflineCashNoritoV1.liabilityPoolId(NETWORK, ASSET, INCARNATION);

    private static OfflineCashHardwareCredentialV1 credential() {
      return new OfflineCashHardwareCredentialV1(
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
          OfflineCashNoritoV1.deviceKeyReference(PUBLIC_KEY),
          10,
          90_000,
          SIGNATURE);
    }

    private static OfflineCashDeviceSignatureV1 signature() {
      final byte[] raw = new byte[64];
      raw[31] = 1;
      raw[63] = 1;
      return new OfflineCashDeviceSignatureV1(raw);
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
