// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.ProofAttachment;
import org.hyperledger.iroha.android.model.instructions.ProofVerifierKeyRef;
import org.hyperledger.iroha.android.test.FixtureGeneratorRunner;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

/** Exact Rust/Java parity and adversarial coverage for the sole ABI-22 registration path. */
public final class RegisterOfflineDeviceAttestationTests {
  private static final NetworkId TEST_NETWORK_ID =
      NetworkId.parse(
          "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
  private static final FeePaymentIntent TEST_FEE_PAYMENT =
      FeePaymentIntent.authority(Collections.emptyList());

  private static final String P256_GENERATOR =
      "04"
          + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
          + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5";

  @Test
  public void nullTtlUsesCanonicalTransactionDefault() throws Exception {
    final String accountId =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x42), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final DeviceAttestationRegistration registration = registration(accountId);
    final RegisterOfflineDeviceAttestation request =
        new RegisterOfflineDeviceAttestation(
            TEST_NETWORK_ID,
            accountId,
            registration,
            1_900_000_000_000L,
            null,
            7L,
            TEST_FEE_PAYMENT,
            Collections.emptyMap());
    final TransactionPayload canonicalPayload =
        TransactionPayload.builder()
            .setNetworkId(TEST_NETWORK_ID)
            .setAuthority(accountId)
            .setCreationTimeMs(1_900_000_000_000L)
            .setExecutable(
                Executable.instructions(Collections.singletonList(request.instruction())))
            .setNonce(7L)
            .setFeePayment(TEST_FEE_PAYMENT)
            .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
            .build();

    assertEquals(Long.valueOf(100_000L), canonicalPayload.timeToLiveMs().get());
    assertEquals(canonicalPayload.timeToLiveMs(), request.transactionPayload().timeToLiveMs());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID,
                accountId,
                registration,
                registration.expiresAtMs() - 99_999L,
                null,
                7L,
                TEST_FEE_PAYMENT,
                Collections.emptyMap()));
  }

  @Test
  public void registrationAndInstructionExactlyMatchRustCurrentModel() throws Exception {
    final List<String> rust = rustFixture();
    assertEquals(5, rust.size());
    final DeviceAttestationRegistration registration = registration(rust.get(3));

    assertEquals(22, DeviceAttestationRegistration.REQUIRED_NATIVE_BRIDGE_ABI_VERSION);
    assertArrayEquals(hexToBytes(rust.get(0)), registration.noritoEncoded());
    assertArrayEquals(hexToBytes(rust.get(2)), registration.challengeHash());
    assertArrayEquals(hexToBytes(rust.get(4)), registration.canonicalRegistrationHash());
    assert !Arrays.equals(
        registration.canonicalRegistrationHash(), sha256(registration.noritoEncoded()))
        : "registration ID is canonical Iroha Hash, not raw SHA-256";

    final RegisterOfflineDeviceAttestation request = request(registration);
    final InstructionBox instruction = request.instruction();
    assertEquals(OfflineDeviceAttestationCodec.INSTRUCTION_SCHEMA, instruction.name());
    assertTrue(instruction.payload() instanceof InstructionBox.WirePayload);
    final byte[] payload = ((InstructionBox.WirePayload) instruction.payload()).payloadBytes();
    assertArrayEquals(hexToBytes(rust.get(1)), payload);
    assertEquals(
        registration,
        DeviceAttestationRegistration.decodeCanonical(
            registration.noritoEncoded(), AccountAddress.DEFAULT_I105_DISCRIMINANT));
    assertEquals(
        registration,
        RegisterOfflineDeviceAttestation.decodeInstructionPayloadCanonical(
            payload, AccountAddress.DEFAULT_I105_DISCRIMINANT));
    request.validateExactPayload(request.transactionPayload());
  }

  @Test
  public void hashAndAppIdentitySubstitutionsFailClosed() throws Exception {
    final String accountId = rustFixture().get(3);
    final DeviceAttestationRegistration canonical = registration(accountId);
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registration(
                accountId,
                "org.hyperledger.iroha.abi20.fixture",
                signingCertificate(),
                IrohaHash.prehash(bytes("wrong-challenge")),
                null,
                null,
                null));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registration(
                accountId,
                "org.hyperledger.iroha.abi20.fixture",
                signingCertificate(),
                null,
                IrohaHash.prehash(bytes("wrong-report")),
                null,
                null));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registration(
                accountId,
                "org.hyperledger.iroha.abi20.fixture",
                signingCertificate(),
                null,
                null,
                IrohaHash.prehash(bytes("wrong-evidence")),
                null));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registration(
                accountId,
                "org.hyperledger.iroha.abi20.fixture",
                signingCertificate(),
                null,
                null,
                null,
                concat(
                    bytes(DeviceAttestationRegistration.DEVICE_ATTESTATION_EVIDENCE_PREFIX),
                    IrohaHash.prehash(bytes("different-report")))));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registration(
                accountId,
                "org.hyperledger.iroha.substituted",
                signingCertificate(),
                canonical.challengeHash(),
                null,
                null,
                null));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registration(
                accountId,
                "org.hyperledger.iroha.abi20.fixture",
                sha256(bytes("substituted-signing-certificate")),
                canonical.challengeHash(),
                null,
                null,
                null));
  }

  @Test
  public void managedPre12StrongBoxProfileIsExplicitAndTruthfullyOmitsTag405()
      throws Exception {
    final String accountId =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x43), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final DeviceAttestationRegistration managed =
        registrationForProfile(
            accountId,
            androidProperties(
                110_000,
                OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.STRONG_BOX),
            DeviceAttestationRegistration.ANDROID_KEYMINT_MANAGED_PRE12_ASSERTION_SCHEME,
            null);

    assertEquals(
        DeviceAttestationRegistration.ANDROID_KEYMINT_MANAGED_PRE12_ASSERTION_SCHEME,
        managed.assertionScheme());
    assertEquals(null, managed.assertionUsageCountLimit());
    assertEquals(
        managed,
        DeviceAttestationRegistration.decodeCanonical(
            managed.noritoEncoded(), AccountAddress.DEFAULT_I105_DISCRIMINANT));
    assertFalse(Arrays.equals(managed.challengeHash(), registration(accountId).challengeHash()));

    assertThrows(
        IllegalArgumentException.class,
        () ->
            registrationForProfile(
                accountId,
                androidProperties(
                    110_000,
                    OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.STRONG_BOX),
                DeviceAttestationRegistration.ANDROID_KEYMINT_MANAGED_PRE12_ASSERTION_SCHEME,
                1));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registrationForProfile(
                accountId,
                androidProperties(
                    110_000,
                    OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.TRUSTED_ENVIRONMENT),
                DeviceAttestationRegistration.ANDROID_KEYMINT_MANAGED_PRE12_ASSERTION_SCHEME,
                null));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            registrationForProfile(
                accountId,
                androidProperties(
                    120_000,
                    OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.STRONG_BOX),
                DeviceAttestationRegistration.ANDROID_KEYMINT_MANAGED_PRE12_ASSERTION_SCHEME,
                null));
  }

  @Test
  public void unknownMalformedAndNoncanonicalBytesFailClosed() throws Exception {
    final String accountId = rustFixture().get(3);
    final byte[] canonical = registration(accountId).noritoEncoded();
    final byte[] malformed = canonical.clone();
    malformed[malformed.length - 1] ^= 1;
    assertThrows(
        IllegalArgumentException.class,
        () ->
            DeviceAttestationRegistration.decodeCanonical(
                malformed, AccountAddress.DEFAULT_I105_DISCRIMINANT));

    final byte[] payload =
        NoritoCodec.fromBytesView(
                canonical, OfflineDeviceAttestationCodec.REGISTRATION_SCHEMA)
            .asBytes();
    final byte[] unknownField =
        NoritoCodec.encode(
            payload,
            OfflineDeviceAttestationCodec.REGISTRATION_SCHEMA,
            new RawPayloadAdapter(true));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            DeviceAttestationRegistration.decodeCanonical(
                unknownField, AccountAddress.DEFAULT_I105_DISCRIMINANT));

    final byte[] alternateFlags =
        NoritoCodec.encode(
            payload,
            OfflineDeviceAttestationCodec.REGISTRATION_SCHEMA,
            new RawPayloadAdapter(false),
            0);
    assertThrows(
        IllegalArgumentException.class,
        () ->
            DeviceAttestationRegistration.decodeCanonical(
                alternateFlags, AccountAddress.DEFAULT_I105_DISCRIMINANT));
  }

  @Test
  public void transactionRejectsExtraInstructionsAndInvalidTtlNonce() throws Exception {
    final String accountId = rustFixture().get(3);
    final DeviceAttestationRegistration registration = registration(accountId);
    final RegisterOfflineDeviceAttestation request = request(registration);
    final TransactionPayload extra =
        request
            .transactionPayload()
            .toBuilder()
            .setExecutable(
                Executable.instructions(List.of(request.instruction(), request.instruction())))
            .build();
    assertThrows(IllegalArgumentException.class, () -> request.validateExactPayload(extra));
    final TransactionPayload attached =
        request
            .transactionPayload()
            .toBuilder()
            .setAttachments(
                Collections.singletonList(
                    new ProofAttachment(
                        "halo2",
                        new byte[] {0x01},
                        new ProofVerifierKeyRef("halo2", "vk1"))))
            .build();
    assertThrows(IllegalArgumentException.class, () -> request.validateExactPayload(attached));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID, accountId, registration, 1_900_000_000_000L, 0L, 1L,
                TEST_FEE_PAYMENT, Collections.emptyMap()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID, accountId, registration, 1_900_000_000_000L, 1L, 0L,
                TEST_FEE_PAYMENT, Collections.emptyMap()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID, accountId, registration, Long.MAX_VALUE - 1, 2L, 1L,
                TEST_FEE_PAYMENT, Collections.emptyMap()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID,
                accountId,
                registration,
                registration.expiresAtMs() - 1,
                2L,
                1L,
                TEST_FEE_PAYMENT,
                Collections.emptyMap()));
  }

  private static RegisterOfflineDeviceAttestation request(
      final DeviceAttestationRegistration registration) {
    return new RegisterOfflineDeviceAttestation(
        TEST_NETWORK_ID,
        registration.accountId(),
        registration,
        1_900_000_000_000L,
        60_000L,
        7L,
        TEST_FEE_PAYMENT,
        Collections.emptyMap());
  }

  private static DeviceAttestationRegistration registration(final String accountId)
      throws Exception {
    return registration(
        accountId,
        "org.hyperledger.iroha.abi22.v2.fixture",
        signingCertificate(),
        null,
        null,
        null,
        null);
  }

  private static DeviceAttestationRegistration registration(
      final String accountId,
      final String packageName,
      final byte[] signingCertificate,
      final byte[] challengeHash,
      final byte[] reportHash,
      final byte[] evidenceHash,
      final byte[] evidence)
      throws Exception {
    final byte[] assertionPublicKey = hexToBytes(P256_GENERATOR);
    return new DeviceAttestationRegistration(
        DeviceAttestationRegistration.REGISTRATION_VERSION,
        DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM,
        hexLower(sha256(assertionPublicKey)),
        "abi22-v2-android-unit-test-device",
        accountId,
        null,
        null,
        null,
        null,
        packageName,
        signingCertificate,
        androidProperties(),
        new KagemushaDevicePublicKeyV2(hexToBytes(P256_GENERATOR)),
        DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_SCHEME,
        DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
        assertionPublicKey,
        1,
        true,
        challengeHash,
        reportHash,
        bytes("abi22-v2-unit-test-not-physical-attestation-evidence"),
        evidenceHash,
        evidence,
        42,
        IrohaHash.prehash(bytes("abi22-v2-unit-test-block")),
        2_000_000_000_000L);
  }

  private static DeviceAttestationRegistration registrationForProfile(
      final String accountId,
      final OfflineAndroidAttestedDevicePropertiesV2 properties,
      final String assertionScheme,
      final Integer usageCountLimit)
      throws Exception {
    final byte[] assertionPublicKey = hexToBytes(P256_GENERATOR);
    return new DeviceAttestationRegistration(
        DeviceAttestationRegistration.REGISTRATION_VERSION,
        DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM,
        hexLower(sha256(assertionPublicKey)),
        "abi22-v2-managed-pre12-unit-test-device",
        accountId,
        null,
        null,
        null,
        null,
        "org.hyperledger.iroha.abi22.v2.fixture",
        signingCertificate(),
        properties,
        new KagemushaDevicePublicKeyV2(hexToBytes(P256_GENERATOR)),
        assertionScheme,
        DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
        assertionPublicKey,
        usageCountLimit,
        true,
        null,
        null,
        bytes("abi22-v2-managed-pre12-not-physical-attestation-evidence"),
        null,
        null,
        42,
        IrohaHash.prehash(bytes("abi22-v2-managed-pre12-block")),
        2_000_000_000_000L);
  }

  private static byte[] signingCertificate() throws Exception {
    return sha256(bytes("abi22-v2-unit-test-signing-certificate"));
  }

  private static OfflineAndroidAttestedDevicePropertiesV2 androidProperties() {
    return androidProperties(
        140_000, OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.STRONG_BOX);
  }

  private static OfflineAndroidAttestedDevicePropertiesV2 androidProperties(
      final long osVersion,
      final OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel securityLevel) {
    final byte[] verifiedBootKey = new byte[32];
    final byte[] verifiedBootHash = new byte[32];
    Arrays.fill(verifiedBootKey, (byte) 0x42);
    Arrays.fill(verifiedBootHash, (byte) 0x24);
    return new OfflineAndroidAttestedDevicePropertiesV2(
        OfflineAndroidAttestedDevicePropertiesV2.VERSION_V2,
        300,
        300,
        securityLevel,
        "google",
        "husky",
        "husky",
        "Google",
        "Pixel 8 Pro",
        osVersion,
        202_608,
        20_260_805,
        20_260_801,
        verifiedBootKey,
        verifiedBootHash);
  }

  private static List<String> rustFixture() throws Exception {
    return FixtureGeneratorRunner.run("offline-device-attestation");
  }

  private static byte[] bytes(final String value) {
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] sha256(final byte[] value) throws Exception {
    return MessageDigest.getInstance("SHA-256").digest(value);
  }

  private static byte[] filled(final int length, final byte value) {
    final byte[] out = new byte[length];
    Arrays.fill(out, value);
    return out;
  }

  private static byte[] concat(final byte[] left, final byte[] right) {
    final byte[] out = Arrays.copyOf(left, left.length + right.length);
    System.arraycopy(right, 0, out, left.length, right.length);
    return out;
  }

  private static String hexLower(final byte[] value) {
    final StringBuilder out = new StringBuilder(value.length * 2);
    for (final byte item : value) {
      out.append(String.format("%02x", item & 0xff));
    }
    return out.toString();
  }

  private static byte[] hexToBytes(final String value) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex must contain complete bytes");
    }
    final byte[] out = new byte[value.length() / 2];
    for (int index = 0; index < out.length; index++) {
      final int high = Character.digit(value.charAt(index * 2), 16);
      final int low = Character.digit(value.charAt(index * 2 + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException("invalid hex");
      }
      out[index] = (byte) ((high << 4) | low);
    }
    return out;
  }

  private static final class RawPayloadAdapter implements TypeAdapter<byte[]> {
    private final boolean appendUnknownField;

    private RawPayloadAdapter(final boolean appendUnknownField) {
      this.appendUnknownField = appendUnknownField;
    }

    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      encoder.writeBytes(value);
      if (appendUnknownField) {
        encoder.writeLength(1, true);
        encoder.writeByte(0);
      }
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      return decoder.readBytes(decoder.remaining());
    }
  }
}
