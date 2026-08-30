// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.security.GeneralSecurityException;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.security.interfaces.ECPublicKey;
import java.security.spec.ECGenParameterSpec;
import java.util.Arrays;
import java.util.List;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.junit.Test;

/** Exact argument and signature-flow tests for the physical Android KeyMint profile. */
public final class KagemushaAndroidKeyMintTests {

  @Test
  public void highLevelRegistrationDerivesAndUsesTheExactPreKeyChallenge() throws Exception {
    final FakeBackend backend = new FakeBackend();
    final KagemushaAndroidKeyMint keyMint = new KagemushaAndroidKeyMint(backend);
    final byte[] accountSeed = new byte[32];
    Arrays.fill(accountSeed, (byte) 0x22);
    final byte[] accountPublicKey =
        new Ed25519PrivateKeyParameters(accountSeed, 0).generatePublicKey().getEncoded();
    final String accountId =
        AccountAddress.fromAccount(accountPublicKey, "ed25519").toI105(0x02f1);
    final byte[] signingCertificateSha256 = canonicalHash(0x11);
    final byte[] recentBlockHash = canonicalHash(0x13);
    final KagemushaDevicePublicKeyV2 deviceAuthority =
        new KagemushaDevicePublicKeyV2(
            uncompressed((ECPublicKey) backend.keyPair.getPublic()));
    final KagemushaAndroidKeyMint.RegistrationParameters parameters =
        new KagemushaAndroidKeyMint.RegistrationParameters(
            "physical-device-1",
            accountId,
            null,
            "org.hyperledger.iroha.pk3",
            signingCertificateSha256,
            deviceAuthority,
            42,
            recentBlockHash,
            2_000_000_000_000L);

    final KagemushaAndroidKeyMint.GeneratedRegistration generated =
        keyMint.generateRegistration(
            "kagemusha-registration-1",
            parameters,
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED);
    final DeviceAttestationRegistration registration = generated.registration();
    assertArrayEquals(parameters.attestationChallenge(), backend.request.challenge());
    assertTrue(backend.request.requiresDevicePropertiesProjection());
    assertEquals("org.hyperledger.iroha.pk3", backend.request.expectedPackageName());
    assertArrayEquals(
        signingCertificateSha256,
        backend.request.expectedSigningCertificateSha256());
    assertArrayEquals(parameters.attestationChallenge(), registration.challengeHash());
    assertArrayEquals(
        generated.material().assertionPublicKeySec1(), registration.assertionPublicKey());
    assertArrayEquals(generated.material().attestationReport(), registration.attestationReport());
    assertEquals(1, registration.assertionUsageCountLimit().intValue());
    assertTrue(registration.oneUse());
    assertEquals(generated.material().keyId(), registration.keyId());
    assertEquals(androidProperties(false), registration.androidAttestedDeviceProperties());
  }

  @Test
  public void generatesExactSingleUseP256ProfileAndSignsPreparationBytes() throws Exception {
    final FakeBackend backend = new FakeBackend();
    final KagemushaAndroidKeyMint keyMint = new KagemushaAndroidKeyMint(backend);
    final byte[] challenge = canonicalHash(0x21);

    final KagemushaAndroidKeyMint.RegistrationMaterial material =
        keyMint.generateRegistrationMaterial(
            "kagemusha-operation-1",
            challenge,
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED);

    assertEquals(1, backend.generateCalls);
    assertEquals("kagemusha-operation-1", backend.request.alias());
    assertEquals("EC", backend.request.keyAlgorithm());
    assertEquals("secp256r1", backend.request.curveName());
    assertEquals(android.security.keystore.KeyProperties.PURPOSE_SIGN, backend.request.purposes());
    assertEquals("SHA-256", backend.request.digest());
    assertEquals(Integer.valueOf(1), backend.request.maxUsageCount());
    assertFalse(backend.request.strongBoxRequired());
    assertArrayEquals(challenge, backend.request.challenge());
    assertEquals(65, material.assertionPublicKeySec1().length);
    assertEquals(64, material.keyId().length());
    assertEquals(1, material.certificateChainDer().size());
    assertArrayEquals(new byte[] {(byte) 0x81, 0x43, 0x30, 0x01, 0x01},
        material.attestationReport());

    final byte[] signingBytes = new byte[237];
    for (int index = 0; index < signingBytes.length; index++) {
      signingBytes[index] = (byte) (index * 17 + 3);
    }
    final byte[] signatureDer = keyMint.signPreparationForAuthorization(material, signingBytes);
    assertEquals("SHA256withECDSA", backend.signatureAlgorithm);
    assertArrayEquals(signingBytes, backend.signedMessage);
    assertTrue(backend.deleted);
    assertTrue(material.isConsumed());
    assertEquals(64, KagemushaP256Codec.rawLowSFromStrictDer(signatureDer).length);

    final Signature verifier = Signature.getInstance("SHA256withECDSA");
    verifier.initVerify(backend.keyPair.getPublic());
    verifier.update(signingBytes);
    assertTrue(verifier.verify(signatureDer));
    assertThrows(
        IllegalStateException.class,
        () -> keyMint.signPreparationForAuthorization(material, signingBytes));
  }

  @Test
  public void failsClosedBeforeGenerationWithoutApi31HardwareSingleUse() throws Exception {
    final FakeBackend oldApi = new FakeBackend();
    oldApi.apiLevel = 30;
    final KagemushaAndroidKeyMint oldApiKeyMint = new KagemushaAndroidKeyMint(oldApi);
    assertThrows(
        GeneralSecurityException.class,
        () -> oldApiKeyMint.generateRegistrationMaterial(
            "old-api",
            canonicalHash(0x31),
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED));
    assertEquals(0, oldApi.generateCalls);

    final FakeBackend softwareUsageLimit = new FakeBackend();
    softwareUsageLimit.hardwareSingleUse = false;
    final KagemushaAndroidKeyMint softwareKeyMint =
        new KagemushaAndroidKeyMint(softwareUsageLimit);
    assertThrows(
        GeneralSecurityException.class,
        () -> softwareKeyMint.generateRegistrationMaterial(
            "software-limit",
            canonicalHash(0x41),
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED));
    assertEquals(0, softwareUsageLimit.generateCalls);
  }

  @Test
  public void managedPre12StrongBoxUsesDistinctReceiptFirstProfileWithoutTag405()
      throws Exception {
    final FakeBackend backend = new FakeBackend();
    backend.apiLevel = 30;
    backend.strongBox = true;
    backend.managedDeviceProperties = true;
    backend.remainingUsageCount = null;
    final KagemushaAndroidKeyMint keyMint = new KagemushaAndroidKeyMint(backend);
    final byte[] accountSeed = new byte[32];
    Arrays.fill(accountSeed, (byte) 0x32);
    final byte[] accountPublicKey =
        new Ed25519PrivateKeyParameters(accountSeed, 0).generatePublicKey().getEncoded();
    final String accountId =
        AccountAddress.fromAccount(accountPublicKey, "ed25519").toI105(0x02f1);
    final KagemushaAndroidKeyMint.RegistrationParameters parameters =
        new KagemushaAndroidKeyMint.RegistrationParameters(
            "managed-pre12-device",
            accountId,
            null,
            "org.hyperledger.iroha.pk3",
            canonicalHash(0x33),
            new KagemushaDevicePublicKeyV2(
                uncompressed((ECPublicKey) backend.keyPair.getPublic())),
            43,
            canonicalHash(0x35),
            2_000_000_000_000L);

    final KagemushaAndroidKeyMint.GeneratedRegistration generated =
        keyMint.generateRegistration(
            "managed-pre12-alias",
            parameters,
            KagemushaAndroidKeyMint.StrongBoxPolicy.REQUIRED);

    assertEquals(
        KagemushaAndroidKeyMint.AssertionProfile
            .MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST,
        backend.request.assertionProfile());
    assertNull(backend.request.maxUsageCount());
    assertNull(generated.registration().assertionUsageCountLimit());
    assertEquals(
        DeviceAttestationRegistration.ANDROID_KEYMINT_MANAGED_PRE12_ASSERTION_SCHEME,
        generated.registration().assertionScheme());
    assertEquals(androidProperties(true, 110_000),
        generated.registration().androidAttestedDeviceProperties());
    assertArrayEquals(backend.request.challenge(), generated.registration().challengeHash());
    assertFalse(Arrays.equals(parameters.attestationChallenge(), backend.request.challenge()));
  }

  @Test
  public void pre12StrongBoxFailsClosedWithoutManagedOwnershipOrCompleteStrongBoxEvidence()
      throws Exception {
    final FakeBackend ordinary = new FakeBackend();
    ordinary.apiLevel = 30;
    ordinary.strongBox = true;
    final KagemushaAndroidKeyMint ordinaryKeyMint = new KagemushaAndroidKeyMint(ordinary);
    assertThrows(
        GeneralSecurityException.class,
        () -> ordinaryKeyMint.generateRegistrationMaterial(
            "ordinary-pre12",
            canonicalHash(0x36),
            KagemushaAndroidKeyMint.StrongBoxPolicy.REQUIRED));
    assertEquals(0, ordinary.generateCalls);

    final FakeBackend managed = new FakeBackend();
    managed.apiLevel = 30;
    managed.strongBox = true;
    managed.managedDeviceProperties = true;
    managed.remainingUsageCount = null;
    managed.projectedProperties = androidProperties(false, 110_000);
    final KagemushaAndroidKeyMint managedKeyMint = new KagemushaAndroidKeyMint(managed);
    final byte[] accountSeed = new byte[32];
    Arrays.fill(accountSeed, (byte) 0x37);
    final String accountId =
        AccountAddress.fromAccount(
                new Ed25519PrivateKeyParameters(accountSeed, 0)
                    .generatePublicKey()
                    .getEncoded(),
                "ed25519")
            .toI105(0x02f1);
    final KagemushaAndroidKeyMint.RegistrationParameters parameters =
        new KagemushaAndroidKeyMint.RegistrationParameters(
            "managed-invalid-evidence",
            accountId,
            null,
            "org.hyperledger.iroha.pk3",
            canonicalHash(0x39),
            new KagemushaDevicePublicKeyV2(
                uncompressed((ECPublicKey) managed.keyPair.getPublic())),
            44,
            canonicalHash(0x3b),
            2_000_000_000_000L);
    assertThrows(
        IllegalArgumentException.class,
        () -> managedKeyMint.generateRegistration(
            "managed-tee-evidence",
            parameters,
            KagemushaAndroidKeyMint.StrongBoxPolicy.REQUIRED));
    assertEquals("managed-tee-evidence", managed.deletedAlias);
  }

  @Test
  public void strongBoxIsExplicitRequiredAndNeverDowngrades() throws Exception {
    final FakeBackend unavailable = new FakeBackend();
    unavailable.strongBox = false;
    final KagemushaAndroidKeyMint unavailableKeyMint = new KagemushaAndroidKeyMint(unavailable);
    assertThrows(
        GeneralSecurityException.class,
        () -> unavailableKeyMint.generateRegistrationMaterial(
            "strongbox-unavailable",
            canonicalHash(0x51),
            KagemushaAndroidKeyMint.StrongBoxPolicy.REQUIRED));
    assertEquals(0, unavailable.generateCalls);

    final FakeBackend generationFailure = new FakeBackend();
    generationFailure.strongBox = true;
    generationFailure.failGeneration = true;
    final KagemushaAndroidKeyMint requiredKeyMint = new KagemushaAndroidKeyMint(generationFailure);
    assertThrows(
        GeneralSecurityException.class,
        () -> requiredKeyMint.generateRegistrationMaterial(
            "strongbox-failure",
            canonicalHash(0x61),
            KagemushaAndroidKeyMint.StrongBoxPolicy.REQUIRED));
    assertEquals(1, generationFailure.generateCalls);
    assertTrue(generationFailure.request.strongBoxRequired());
    assertEquals("strongbox-failure", generationFailure.deletedAlias);
  }

  @Test
  public void rejectsUntruthfulGeneratedHardwareProjection() throws Exception {
    final FakeBackend software = new FakeBackend();
    software.generatedInsideHardware = false;
    final KagemushaAndroidKeyMint keyMint = new KagemushaAndroidKeyMint(software);
    assertThrows(
        GeneralSecurityException.class,
        () -> keyMint.generateRegistrationMaterial(
            "software-key",
            canonicalHash(0x71),
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED));
    assertEquals("software-key", software.deletedAlias);

    final FakeBackend wrongUsage = new FakeBackend();
    wrongUsage.remainingUsageCount = 2;
    final KagemushaAndroidKeyMint wrongUsageKeyMint = new KagemushaAndroidKeyMint(wrongUsage);
    assertThrows(
        GeneralSecurityException.class,
        () -> wrongUsageKeyMint.generateRegistrationMaterial(
            "wrong-usage",
            canonicalHash(0x73),
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED));
    assertEquals("wrong-usage", wrongUsage.deletedAlias);
  }

  @Test
  public void challengeMustBeCanonicalAndDefensivelyCopied() throws Exception {
    final FakeBackend backend = new FakeBackend();
    final KagemushaAndroidKeyMint keyMint = new KagemushaAndroidKeyMint(backend);
    assertThrows(
        IllegalArgumentException.class,
        () -> keyMint.generateRegistrationMaterial(
            "short-challenge",
            new byte[31],
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED));
    assertThrows(
        IllegalArgumentException.class,
        () -> keyMint.generateRegistrationMaterial(
            "noncanonical-hash",
            new byte[32],
            KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED));

    final byte[] challenge = canonicalHash(0x7b);
    keyMint.generateRegistrationMaterial(
        "defensive-challenge",
        challenge,
        KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED);
    challenge[0] ^= 0x7f;
    assertEquals((byte) 0x7b, backend.request.challenge()[0]);
  }

  private static byte[] canonicalHash(final int marker) {
    final byte[] hash = new byte[32];
    Arrays.fill(hash, (byte) marker);
    hash[31] |= 1;
    return hash;
  }

  private static OfflineAndroidAttestedDevicePropertiesV2 androidProperties(
      final boolean strongBox) {
    return androidProperties(strongBox, 140_000);
  }

  private static OfflineAndroidAttestedDevicePropertiesV2 androidProperties(
      final boolean strongBox, final long osVersion) {
    return new OfflineAndroidAttestedDevicePropertiesV2(
        OfflineAndroidAttestedDevicePropertiesV2.VERSION_V2,
        300,
        300,
        strongBox
            ? OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.STRONG_BOX
            : OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.TRUSTED_ENVIRONMENT,
        "google",
        "husky",
        "husky",
        "Google",
        "Pixel 8 Pro",
        osVersion,
        202_608,
        20_260_805,
        20_260_801,
        canonicalHash(0x42),
        canonicalHash(0x24));
  }

  private static byte[] uncompressed(final ECPublicKey publicKey) {
    final byte[] result = new byte[65];
    result[0] = 0x04;
    copyCoordinate(publicKey.getW().getAffineX().toByteArray(), result, 1);
    copyCoordinate(publicKey.getW().getAffineY().toByteArray(), result, 33);
    return result;
  }

  private static void copyCoordinate(
      final byte[] signed, final byte[] destination, final int destinationOffset) {
    final int sourceOffset = signed.length == 33 && signed[0] == 0 ? 1 : 0;
    final int length = signed.length - sourceOffset;
    System.arraycopy(signed, sourceOffset, destination, destinationOffset + 32 - length, length);
  }

  private static final class FakeBackend implements KagemushaAndroidKeyMint.Backend {
    private final KeyPair keyPair;
    private int apiLevel = 31;
    private boolean hardwareSingleUse = true;
    private boolean strongBox;
    private boolean managedDeviceProperties;
    private boolean failGeneration;
    private boolean generatedInsideHardware = true;
    private Integer remainingUsageCount = 1;
    private OfflineAndroidAttestedDevicePropertiesV2 projectedProperties;
    private int generateCalls;
    private KagemushaAndroidKeyMint.GenerationRequest request;
    private String signatureAlgorithm;
    private byte[] signedMessage;
    private boolean deleted;
    private String deletedAlias;

    private FakeBackend() throws GeneralSecurityException {
      final KeyPairGenerator generator = KeyPairGenerator.getInstance("EC");
      generator.initialize(new ECGenParameterSpec("secp256r1"));
      keyPair = generator.generateKeyPair();
    }

    @Override
    public int apiLevel() {
      return apiLevel;
    }

    @Override
    public boolean supportsHardwareSingleUse() {
      return hardwareSingleUse;
    }

    @Override
    public boolean supportsStrongBox() {
      return strongBox;
    }

    @Override
    public boolean supportsManagedDevicePropertiesAttestation() {
      return managedDeviceProperties;
    }

    @Override
    public KagemushaAndroidKeyMint.GeneratedKey generate(
        final KagemushaAndroidKeyMint.GenerationRequest request)
        throws GeneralSecurityException {
      generateCalls++;
      this.request = request;
      if (failGeneration) {
        try {
          delete(request.alias(), request.assertionProfile());
        } catch (final GeneralSecurityException impossible) {
          throw new AssertionError(impossible);
        }
        throw new GeneralSecurityException("injected StrongBox generation failure");
      }
      return new KagemushaAndroidKeyMint.GeneratedKey(
          uncompressed((ECPublicKey) keyPair.getPublic()),
          List.of(new byte[] {0x30, 0x01, 0x01}),
          generatedInsideHardware,
          request.strongBoxRequired(),
          remainingUsageCount,
          request.requiresDevicePropertiesProjection()
              ? projectedProperties != null
                  ? projectedProperties
                  : request.assertionProfile()
                          == KagemushaAndroidKeyMint.AssertionProfile
                              .MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST
                      ? androidProperties(true, 110_000)
                      : androidProperties(request.strongBoxRequired())
              : null);
    }

    @Override
    public byte[] sign(
        final String alias,
        final String algorithm,
        final byte[] message,
        final KagemushaAndroidKeyMint.AssertionProfile assertionProfile)
        throws GeneralSecurityException {
      signatureAlgorithm = algorithm;
      signedMessage = message.clone();
      final Signature signer = Signature.getInstance(algorithm);
      signer.initSign(keyPair.getPrivate());
      signer.update(message);
      return signer.sign();
    }

    @Override
    public void delete(
        final String alias,
        final KagemushaAndroidKeyMint.AssertionProfile assertionProfile)
        throws GeneralSecurityException {
      deleted = true;
      deletedAlias = alias;
    }
  }
}
