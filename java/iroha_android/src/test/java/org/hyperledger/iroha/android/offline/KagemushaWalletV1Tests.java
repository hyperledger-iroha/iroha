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
import org.hyperledger.iroha.sdk.offline.KagemushaAggregateStateCommitmentV1;
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
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareRecoveryV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareTerminalResultV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareProfileV1;
import org.junit.Test;

/** Java migration-surface checks for aggregate-balance Kagemusha V1 orchestration. */
public final class KagemushaWalletV1Tests {
  @Test
  public void javaProviderIsTheCanonicalKotlinHardwareContract() {
    assertTrue(
        org.hyperledger.iroha.sdk.offline.KagemushaHardwareProviderV1.class
            .isAssignableFrom(KagemushaHardwareProviderV1.class));
    assertEquals(16, KagemushaHardwareCapabilityV1.values().length);
  }

  @Test
  public void facadeExposesReceiveFoldDrainSendRedeemRecoveryAndRotation() throws Exception {
    final Set<String> methods =
        Arrays.stream(KagemushaWalletV1.class.getDeclaredMethods())
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
                "stagePayment",
                "stageMintCredit",
                "foldPendingCredit",
                "drainPendingCredits",
                "send",
                "recoverPayment",
                "recordAcknowledgement",
                "redeem",
                "recoverRedemption",
                "rotateHardwareEpoch")));
    assertEquals(
        BigInteger.class,
        KagemushaWalletV1.class.getMethod("drainPendingCredits").getReturnType());
  }

  @Test
  public void javaFacadeDrainsMoreThanSixteenCreditsFromOneStableSnapshot() {
    final FoldProvider provider = new FoldProvider(33);
    final KagemushaWalletV1 wallet = KagemushaWalletV1.open(provider);

    assertEquals(BigInteger.valueOf(33), wallet.drainPendingCredits());
    assertEquals(34, provider.foldResults.size());
    assertTrue(provider.foldResults.subList(0, 33).stream().allMatch(Boolean::booleanValue));
    assertEquals(Boolean.FALSE, provider.foldResults.get(33));
    assertEquals(1, provider.watermarkCalls);
    assertEquals(0, provider.remainingCredits);
    assertEquals(BigInteger.valueOf(33), wallet.journalRevision());
    assertEquals(BigInteger.valueOf(33), wallet.aggregateState().sequence);
  }

  private static final class FoldProvider implements KagemushaHardwareProviderV1 {
    private int remainingCredits;
    private int watermarkCalls;
    private final List<Boolean> foldResults = new ArrayList<>();
    private BigInteger revision = BigInteger.ZERO;
    private long sequence;
    private int stateTag = 0x51;

    private FoldProvider(final int pending) {
      this.remainingCredits = pending;
    }

    @Override
    public KagemushaHardwareQualificationV1 qualification() {
      return Fixture.QUALIFICATION;
    }

    @Override
    public KagemushaHardwareRecoveryV1 recover() {
      return new KagemushaHardwareRecoveryV1(
          stateBytes(), revision, BigInteger.valueOf(remainingCredits), BigInteger.ZERO);
    }

    @Override
    public byte[] bootstrapState() {
      return stateBytes();
    }

    @Override
    public byte[] createPaymentRequest(
        final byte[] recipientAccount,
        final BigInteger amount,
        final long validityWindowMillis) {
      throw unused();
    }

    @Override
    public KagemushaHardwarePaymentStageV1 stagePayment(
        final byte[] canonicalRequest, final byte[] canonicalPayment) {
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
      return revision;
    }

    @Override
    public byte[] foldPendingCredit(final BigInteger inboxSequenceInclusive) {
      assertTrue(inboxSequenceInclusive.signum() >= 0);
      final boolean folded = remainingCredits > 0;
      foldResults.add(folded);
      if (!folded) {
        return null;
      }
      remainingCredits -= 1;
      revision = revision.add(BigInteger.ONE);
      sequence += 1;
      stateTag += 1;
      return stateBytes();
    }

    @Override
    public KagemushaHardwareTerminalResultV1 commitPayment(final byte[] canonicalRequest) {
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
      throw unused();
    }

    private byte[] stateBytes() {
      return KagemushaNoritoV1.encodeAggregateStateShape(
          new KagemushaAggregateStateCommitmentV1(
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
