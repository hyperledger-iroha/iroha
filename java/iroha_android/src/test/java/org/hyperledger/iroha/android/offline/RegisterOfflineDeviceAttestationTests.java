// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.File;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

/** Exact Rust/Java parity and adversarial coverage for the sole ABI-21 registration path. */
public final class RegisterOfflineDeviceAttestationTests {
  private static final FeePaymentIntent TEST_FEE_PAYMENT =
      FeePaymentIntent.authority(Collections.emptyList());

  private static final String P256_GENERATOR =
      "04"
          + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
          + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5";

  @Test
  public void registrationAndInstructionExactlyMatchRustCurrentModel() throws Exception {
    final List<String> rust = rustFixture();
    assertEquals(5, rust.size());
    final DeviceAttestationRegistration registration = registration(rust.get(3));

    assertEquals(21, DeviceAttestationRegistration.REQUIRED_NATIVE_BRIDGE_ABI_VERSION);
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
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                "00000000", accountId, registration, 1_900_000_000_000L, 0L, 1L,
                TEST_FEE_PAYMENT, Collections.emptyMap()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                "00000000", accountId, registration, 1_900_000_000_000L, 1L, 0L,
                TEST_FEE_PAYMENT, Collections.emptyMap()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                "00000000", accountId, registration, Long.MAX_VALUE - 1, 2L, 1L,
                TEST_FEE_PAYMENT, Collections.emptyMap()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new RegisterOfflineDeviceAttestation(
                "00000000",
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
        "00000000",
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
        "org.hyperledger.iroha.abi20.fixture",
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
        1,
        DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM,
        hexLower(sha256(assertionPublicKey)),
        "abi20-android-unit-test-device",
        accountId,
        null,
        null,
        null,
        null,
        packageName,
        signingCertificate,
        new KagemushaDevicePublicKeyV2(hexToBytes(P256_GENERATOR)),
        DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_SCHEME,
        DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
        assertionPublicKey,
        1,
        true,
        challengeHash,
        reportHash,
        bytes("abi20-unit-test-not-physical-attestation-evidence"),
        evidenceHash,
        evidence,
        42,
        IrohaHash.prehash(bytes("abi20-unit-test-block")),
        2_000_000_000_000L);
  }

  private static byte[] signingCertificate() throws Exception {
    return sha256(bytes("abi20-unit-test-signing-certificate"));
  }

  private static List<String> rustFixture() throws Exception {
    final File root = locateRepoRoot();
    final File target = new File(root, "target/kotlin-fixture-gen-test");
    final File binary = new File(target, "debug/kotlin-fixture-gen");
    final Path lockPath = new File(root, "target/kotlin-fixture-gen-test.lock").toPath();
    Files.createDirectories(lockPath.getParent());
    try (FileChannel channel =
            FileChannel.open(
                lockPath,
                StandardOpenOption.CREATE,
                StandardOpenOption.READ,
                StandardOpenOption.WRITE);
        java.nio.channels.FileLock ignored = channel.lock()) {
      // Always ask Cargo to refresh the generator. Finding an older binary is not
      // sufficient after a wire-ABI cutover and can compare Java against stale Rust bytes.
      final ProcessBuilder build =
          new ProcessBuilder("cargo", "build", "-p", "kotlin-fixture-gen")
              .directory(root)
              .redirectErrorStream(true);
      build.environment().put("CARGO_TARGET_DIR", target.getAbsolutePath());
      final Process process = build.start();
      final String output =
          new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
      final int exit = process.waitFor();
      if (exit != 0) {
        throw new IllegalStateException("kotlin-fixture-gen build failed: " + output);
      }
    }
    final Process process =
        new ProcessBuilder(binary.getAbsolutePath(), "offline-device-attestation")
            .directory(root)
            .redirectErrorStream(true)
            .start();
    final String output =
        new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
    final int exit = process.waitFor();
    if (exit != 0 || output.isEmpty()) {
      throw new IllegalStateException(
          "offline-device-attestation fixture failed (" + exit + "): " + output);
    }
    return Arrays.asList(output.split("\\R"));
  }

  private static File locateRepoRoot() {
    File directory = new File("").getAbsoluteFile();
    while (directory != null && !new File(directory, "Cargo.toml").isFile()) {
      directory = directory.getParentFile();
    }
    if (directory == null) {
      throw new IllegalStateException("could not locate Iroha repository root");
    }
    return directory;
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
