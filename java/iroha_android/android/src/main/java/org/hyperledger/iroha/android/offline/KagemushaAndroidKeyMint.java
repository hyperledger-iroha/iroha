// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import android.annotation.TargetApi;
import android.app.admin.DevicePolicyManager;
import android.content.ComponentName;
import android.content.Context;
import android.content.pm.PackageManager;
import android.os.Build;
import android.security.AttestedKeyPair;
import android.security.KeyChain;
import android.security.KeyChainException;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyInfo;
import android.security.keystore.KeyProperties;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.math.BigInteger;
import java.security.GeneralSecurityException;
import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.security.PrivateKey;
import java.security.ProviderException;
import java.security.Signature;
import java.security.cert.Certificate;
import java.security.cert.CertificateEncodingException;
import java.security.cert.X509Certificate;
import java.security.interfaces.ECPublicKey;
import java.security.spec.ECGenParameterSpec;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerificationException;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerifier;

/**
 * Physical Android KeyMint path for one-use Kagemusha request authorization.
 *
 * <p>This class is deliberately separate from the generic transaction keystore. It is available
 * only in the Android artifact. Android 12+ uses hardware-enforced
 * {@code usageCountLimit == 1}. API 28--30 is supported only for a device/profile-owner app via
 * {@link DevicePolicyManager#generateKeyPair}, {@link DevicePolicyManager#ID_TYPE_BASE_INFO}, and
 * required StrongBox; that explicit profile never claims KeyMint tag 405 and relies on
 * receipt-first ledger consumption plus immediate alias deletion.
 *
 * <p>Every generated key has the exact profile {@code EC/secp256r1}, sign-only purpose,
 * {@code SHA-256}, and the caller-supplied canonical attestation challenge. Android 12+ also has
 * a hardware-enforced maximum usage count of one; the managed pre-12 profile explicitly has no
 * such tag. {@link #authorize} signs the complete preparation preimage with
 * {@code SHA256withECDSA}, validates the strict DER result, feeds it to the existing native
 * authorization finalizer, and removes the exhausted alias. StrongBox is requested only through
 * {@link StrongBoxPolicy#REQUIRED}; failure never falls back to TEE.
 */
public final class KagemushaAndroidKeyMint {
  public static final int MINIMUM_API_LEVEL = Build.VERSION_CODES.P;
  public static final int HARDWARE_USAGE_LIMIT_MINIMUM_API_LEVEL = Build.VERSION_CODES.S;
  public static final String KEY_ALGORITHM = KeyProperties.KEY_ALGORITHM_EC;
  public static final String CURVE_NAME = "secp256r1";
  public static final String DIGEST = KeyProperties.DIGEST_SHA256;
  public static final String SIGNATURE_ALGORITHM = "SHA256withECDSA";
  public static final int PURPOSES = KeyProperties.PURPOSE_SIGN;
  public static final int MAX_USAGE_COUNT = 1;

  private static final String ANDROID_KEYSTORE = "AndroidKeyStore";
  private static final int MAX_ALIAS_BYTES = 128;
  private static final int MAX_ATTESTATION_REPORT_BYTES = 64 * 1024;

  /** Closed StrongBox policy: either do not request it, or require it without downgrade. */
  public enum StrongBoxPolicy {
    NOT_REQUESTED,
    REQUIRED,
  }

  /** Closed assertion profiles; neither profile may be silently relabelled as the other. */
  public enum AssertionProfile {
    HARDWARE_USAGE_LIMIT(
        DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_SCHEME, MAX_USAGE_COUNT),
    MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST(
        DeviceAttestationRegistration.ANDROID_KEYMINT_MANAGED_PRE12_ASSERTION_SCHEME, null);

    private final String scheme;
    private final Integer usageCountLimit;

    AssertionProfile(final String scheme, final Integer usageCountLimit) {
      this.scheme = scheme;
      this.usageCountLimit = usageCountLimit;
    }

    public String scheme() {
      return scheme;
    }

    public Integer usageCountLimit() {
      return usageCountLimit;
    }
  }

  private final Backend backend;
  private final Object owner = new Object();

  /** Create the physical KeyMint service for the current Android application. */
  public KagemushaAndroidKeyMint(final Context context) throws GeneralSecurityException {
    this(
        context,
        new ComponentName(
            Objects.requireNonNull(context, "context").getApplicationContext(),
            KagemushaManagedDeviceAdminReceiver.class));
  }

  /**
   * Create the physical service with the exact active device-admin component used by a managed
   * API 28--30 deployment.
   *
   * <p>Android 12+ callers may use the simpler constructor. A pre-12 embedding application that
   * provisions its own {@code DeviceAdminReceiver} must pass that active component here; package
   * ownership alone is insufficient authority for {@link DevicePolicyManager#generateKeyPair}.
   */
  public KagemushaAndroidKeyMint(final Context context, final ComponentName deviceAdmin)
      throws GeneralSecurityException {
    this(
        new PlatformBackend(
            Objects.requireNonNull(context, "context").getApplicationContext(),
            Objects.requireNonNull(deviceAdmin, "deviceAdmin")));
  }

  KagemushaAndroidKeyMint(final Backend backend) {
    this.backend = Objects.requireNonNull(backend, "backend");
  }

  /**
   * Derive the exact pre-key challenge, generate the physical assertion key, and construct the
   * matching on-chain registration.
   *
   * <p>This is the preferred public flow. It prevents application code from accidentally supplying
   * a challenge derived from registration fields other than those ultimately submitted on-chain.
   */
  public GeneratedRegistration generateRegistration(
      final String alias,
      final RegistrationParameters parameters,
      final StrongBoxPolicy strongBoxPolicy)
      throws GeneralSecurityException {
    final RegistrationParameters requiredParameters =
        Objects.requireNonNull(parameters, "parameters");
    final AssertionProfile assertionProfile = selectAssertionProfile(strongBoxPolicy);
    final RegistrationMaterial material =
        generateRegistrationMaterial(
            alias,
            requiredParameters.attestationChallenge(assertionProfile),
            strongBoxPolicy,
            assertionProfile,
            requiredParameters.androidPackageName,
            requiredParameters.androidSigningCertificateSha256);
    try {
      return new GeneratedRegistration(
          requiredParameters.registration(material), material);
    } catch (final RuntimeException | Error failure) {
      try {
        delete(material);
      } catch (final GeneralSecurityException cleanupFailure) {
        failure.addSuppressed(cleanupFailure);
      }
      throw failure;
    }
  }

  /**
   * Generate one operation-scoped assertion key and return registration material.
   *
   * <p>This lower-level two-stage primitive is for applications that must persist the challenge
   * before generation. The challenge must come from
   * {@link DeviceAttestationRegistration#androidPreKeyGenerationChallengeHash}; applications that
   * do not need that split should use {@link #generateRegistration}.
   *
   * @param alias application-owned AndroidKeyStore alias; it must not already exist
   * @param attestationChallenge exact canonical 32-byte Android pre-key challenge hash
   * @param strongBoxPolicy whether StrongBox is explicitly required without fallback
   */
  public RegistrationMaterial generateRegistrationMaterial(
      final String alias,
      final byte[] attestationChallenge,
      final StrongBoxPolicy strongBoxPolicy)
      throws GeneralSecurityException {
    return generateRegistrationMaterial(
        alias,
        attestationChallenge,
        strongBoxPolicy,
        selectAssertionProfile(strongBoxPolicy),
        null,
        null);
  }

  private RegistrationMaterial generateRegistrationMaterial(
      final String alias,
      final byte[] attestationChallenge,
      final StrongBoxPolicy strongBoxPolicy,
      final AssertionProfile assertionProfile,
      final String expectedPackageName,
      final byte[] expectedSigningCertificateSha256)
      throws GeneralSecurityException {
    final String canonicalAlias = requireAlias(alias);
    final byte[] challenge = requireChallenge(attestationChallenge);
    final StrongBoxPolicy policy = Objects.requireNonNull(strongBoxPolicy, "strongBoxPolicy");
    final AssertionProfile profile =
        Objects.requireNonNull(assertionProfile, "assertionProfile");
    requirePlatformCapabilities(policy, profile);
    if (profile == AssertionProfile.MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST
        && (expectedPackageName == null || expectedSigningCertificateSha256 == null)) {
      throw new GeneralSecurityException(
          "managed pre-Android-12 registration requires the high-level app-bound flow");
    }

    final GenerationRequest request =
        new GenerationRequest(
            canonicalAlias,
            challenge,
            policy == StrongBoxPolicy.REQUIRED,
            profile,
            expectedPackageName,
            expectedSigningCertificateSha256);
    final GeneratedKey generated = backend.generate(request);
    try {
      if (!generated.insideSecureHardware()) {
        throw new GeneralSecurityException(
            "Kagemusha KeyMint assertion key is not inside secure hardware");
      }
      if (profile == AssertionProfile.HARDWARE_USAGE_LIMIT
          && !Integer.valueOf(MAX_USAGE_COUNT).equals(generated.remainingUsageCount())) {
        throw new GeneralSecurityException(
            "Kagemusha KeyMint assertion key does not expose one remaining hardware use");
      }
      if (profile == AssertionProfile.MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST
          && generated.remainingUsageCount() != null) {
        throw new GeneralSecurityException(
            "managed pre-Android-12 StrongBox assertion must not claim a hardware usage limit");
      }
      if (policy == StrongBoxPolicy.REQUIRED && !generated.strongBoxBacked()) {
        throw new GeneralSecurityException(
            "StrongBox was required but KeyMint generated a weaker key");
      }

      final byte[] publicKey = KagemushaP256Codec.requireUncompressedPublicKey(
          generated.publicKeySec1());
      final List<byte[]> certificateChain =
          requireCertificateChain(generated.certificateChainDer());
      final byte[] attestationReport = encodeCertificateArray(certificateChain);
      return new RegistrationMaterial(
          owner,
          canonicalAlias,
          publicKey,
          certificateChain,
          attestationReport,
          generated.strongBoxBacked(),
          generated.attestedDeviceProperties(),
          profile);
    } catch (final GeneralSecurityException | RuntimeException | Error failure) {
      deleteAfterRejectedGeneration(canonicalAlias, profile, failure);
      throw failure;
    }
  }

  /**
   * Consume one generated assertion key to authorize the exact native preparation.
   *
   * <p>The alias is deleted after the signing attempt. A failed or interrupted finalization cannot
   * be retried with the same material because the hardware use may already have been consumed.
   */
  public KagemushaRecursiveSpendProver.RequestAuthorization authorize(
      final KagemushaRecursiveSpendProver.RequestAuthorizationPreparation preparation,
      final RegistrationMaterial material)
      throws GeneralSecurityException {
    final KagemushaRecursiveSpendProver.RequestAuthorizationPreparation requiredPreparation =
        Objects.requireNonNull(preparation, "preparation");
    final byte[] signatureDer =
        signPreparationForAuthorization(material, requiredPreparation.signingBytes());
    return KagemushaRecursiveSpendProver.finalizeRequestAuthorization(
        requiredPreparation, signatureDer);
  }

  byte[] signPreparationForAuthorization(
      final RegistrationMaterial material, final byte[] signingBytes)
      throws GeneralSecurityException {
    final RegistrationMaterial requiredMaterial = requireOwnedMaterial(material);
    final byte[] message = copyRequired(signingBytes, "signingBytes");
    if (!requiredMaterial.consumed.compareAndSet(false, true)) {
      throw new IllegalStateException("Kagemusha KeyMint registration material is already consumed");
    }

    final byte[] signatureDer;
    try {
      signatureDer =
          backend.sign(
              requiredMaterial.alias,
              SIGNATURE_ALGORITHM,
              message,
              requiredMaterial.assertionProfile);
    } catch (final GeneralSecurityException | RuntimeException | Error failure) {
      deleteAfterRejectedGeneration(
          requiredMaterial.alias, requiredMaterial.assertionProfile, failure);
      throw failure;
    }
    backend.delete(requiredMaterial.alias, requiredMaterial.assertionProfile);
    KagemushaP256Codec.rawLowSFromStrictDer(signatureDer);
    return signatureDer.clone();
  }

  /** Delete an unused generated alias and permanently consume its material. */
  public void delete(final RegistrationMaterial material) throws GeneralSecurityException {
    final RegistrationMaterial requiredMaterial = requireOwnedMaterial(material);
    requiredMaterial.consumed.set(true);
    backend.delete(requiredMaterial.alias, requiredMaterial.assertionProfile);
  }

  private AssertionProfile selectAssertionProfile(final StrongBoxPolicy strongBoxPolicy)
      throws GeneralSecurityException {
    final StrongBoxPolicy policy = Objects.requireNonNull(strongBoxPolicy, "strongBoxPolicy");
    if (backend.apiLevel() >= HARDWARE_USAGE_LIMIT_MINIMUM_API_LEVEL) {
      return AssertionProfile.HARDWARE_USAGE_LIMIT;
    }
    if (backend.apiLevel() >= MINIMUM_API_LEVEL
        && policy == StrongBoxPolicy.REQUIRED
        && backend.supportsStrongBox()
        && backend.supportsManagedDevicePropertiesAttestation()) {
      return AssertionProfile.MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST;
    }
    throw new GeneralSecurityException(
        "pre-Android-12 eligibility requires a device/profile-owner StrongBox "
            + "ID_TYPE_BASE_INFO attestation path; ordinary applications remain drain-only");
  }

  private void requirePlatformCapabilities(
      final StrongBoxPolicy policy, final AssertionProfile assertionProfile)
      throws GeneralSecurityException {
    if (backend.apiLevel() < MINIMUM_API_LEVEL) {
      throw new GeneralSecurityException(
          "Kagemusha KeyMint assertions require Android 9 / API 28 or newer");
    }
    if (assertionProfile == AssertionProfile.HARDWARE_USAGE_LIMIT
        && (backend.apiLevel() < HARDWARE_USAGE_LIMIT_MINIMUM_API_LEVEL
            || !backend.supportsHardwareSingleUse())) {
      throw new GeneralSecurityException(
          "device lacks hardware-enforced AndroidKeyStore single-use keys");
    }
    if (assertionProfile
            == AssertionProfile.MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST
        && (backend.apiLevel() >= HARDWARE_USAGE_LIMIT_MINIMUM_API_LEVEL
            || policy != StrongBoxPolicy.REQUIRED
            || !backend.supportsManagedDevicePropertiesAttestation())) {
      throw new GeneralSecurityException(
          "managed pre-Android-12 assertions require device/profile ownership and StrongBox");
    }
    if (policy == StrongBoxPolicy.REQUIRED && !backend.supportsStrongBox()) {
      throw new GeneralSecurityException(
          "StrongBox is required by policy but unavailable on this device");
    }
  }

  private RegistrationMaterial requireOwnedMaterial(final RegistrationMaterial material) {
    final RegistrationMaterial value = Objects.requireNonNull(material, "material");
    if (value.owner != owner) {
      throw new IllegalArgumentException(
          "registration material belongs to a different Kagemusha KeyMint service");
    }
    return value;
  }

  private void deleteAfterRejectedGeneration(
      final String alias, final AssertionProfile assertionProfile, final Throwable failure)
      throws GeneralSecurityException {
    try {
      backend.delete(alias, assertionProfile);
    } catch (final GeneralSecurityException cleanupFailure) {
      if (failure != null) {
        failure.addSuppressed(cleanupFailure);
      } else {
        throw cleanupFailure;
      }
    }
  }

  private static String requireAlias(final String alias) {
    Objects.requireNonNull(alias, "alias");
    if (alias.isEmpty()
        || !alias.equals(alias.trim())
        || alias.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > MAX_ALIAS_BYTES) {
      throw new IllegalArgumentException(
          "alias must be canonical non-empty text within 128 UTF-8 bytes");
    }
    for (int index = 0; index < alias.length(); index++) {
      final char character = alias.charAt(index);
      if (character < 0x20 || character == 0x7f) {
        throw new IllegalArgumentException("alias must not contain control characters");
      }
    }
    return alias;
  }

  private static byte[] requireChallenge(final byte[] challenge) {
    final byte[] value = Objects.requireNonNull(challenge, "attestationChallenge").clone();
    DeviceAttestationRegistration.requireHash(value, "attestationChallenge");
    return value;
  }

  private static byte[] copyRequired(final byte[] value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.length == 0) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    return value.clone();
  }

  private static List<byte[]> requireCertificateChain(final List<byte[]> certificates)
      throws GeneralSecurityException {
    if (certificates == null || certificates.isEmpty()) {
      throw new GeneralSecurityException(
          "Android KeyMint did not return an attestation certificate chain");
    }
    final List<byte[]> copies = new ArrayList<>(certificates.size());
    for (final byte[] certificate : certificates) {
      if (certificate == null || certificate.length == 0) {
        throw new GeneralSecurityException(
            "Android KeyMint returned an empty attestation certificate");
      }
      copies.add(certificate.clone());
    }
    return Collections.unmodifiableList(copies);
  }

  private static byte[] encodeCertificateArray(final List<byte[]> certificates)
      throws GeneralSecurityException {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeCborHead(out, 4, certificates.size());
    for (final byte[] certificate : certificates) {
      writeCborHead(out, 2, certificate.length);
      out.write(certificate, 0, certificate.length);
    }
    final byte[] encoded = out.toByteArray();
    if (encoded.length > MAX_ATTESTATION_REPORT_BYTES) {
      throw new GeneralSecurityException(
          "Android KeyMint certificate array exceeds the registration report bound");
    }
    return encoded;
  }

  private static void writeCborHead(
      final ByteArrayOutputStream out, final int major, final int value)
      throws GeneralSecurityException {
    if (value < 0) {
      throw new GeneralSecurityException("negative CBOR length");
    }
    if (value <= 23) {
      out.write((major << 5) | value);
    } else if (value <= 0xff) {
      out.write((major << 5) | 24);
      out.write(value);
    } else if (value <= 0xffff) {
      out.write((major << 5) | 25);
      out.write((value >>> 8) & 0xff);
      out.write(value & 0xff);
    } else {
      out.write((major << 5) | 26);
      out.write((value >>> 24) & 0xff);
      out.write((value >>> 16) & 0xff);
      out.write((value >>> 8) & 0xff);
      out.write(value & 0xff);
    }
  }

  private static byte[] fixedUnsigned(final BigInteger value) throws GeneralSecurityException {
    final byte[] signed = value.toByteArray();
    final int sourceOffset = signed.length == 33 && signed[0] == 0 ? 1 : 0;
    final int length = signed.length - sourceOffset;
    if (length > 32) {
      throw new GeneralSecurityException("P-256 coordinate exceeds 32 bytes");
    }
    final byte[] fixed = new byte[32];
    System.arraycopy(signed, sourceOffset, fixed, fixed.length - length, length);
    return fixed;
  }

  private static byte[] uncompressedSec1(final java.security.PublicKey publicKey)
      throws GeneralSecurityException {
    if (!(publicKey instanceof ECPublicKey ecPublicKey)) {
      throw new GeneralSecurityException("Android KeyMint did not generate an EC public key");
    }
    final byte[] x = fixedUnsigned(ecPublicKey.getW().getAffineX());
    final byte[] y = fixedUnsigned(ecPublicKey.getW().getAffineY());
    final byte[] encoded = new byte[65];
    encoded[0] = 0x04;
    System.arraycopy(x, 0, encoded, 1, x.length);
    System.arraycopy(y, 0, encoded, 33, y.length);
    return KagemushaP256Codec.requireUncompressedPublicKey(encoded);
  }

  private static String keyId(final byte[] publicKey) {
    try {
      final byte[] digest = MessageDigest.getInstance("SHA-256").digest(publicKey);
      final StringBuilder out = new StringBuilder(digest.length * 2);
      for (final byte value : digest) out.append(String.format(Locale.ROOT, "%02x", value & 0xff));
      return out.toString();
    } catch (final GeneralSecurityException impossible) {
      throw new IllegalStateException("SHA-256 unavailable", impossible);
    }
  }

  /** Immutable material required to construct an Android device-attestation registration. */
  public static final class RegistrationMaterial {
    private final Object owner;
    private final String alias;
    private final byte[] publicKeySec1;
    private final List<byte[]> certificateChainDer;
    private final byte[] attestationReport;
    private final boolean strongBoxBacked;
    private final OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties;
    private final AssertionProfile assertionProfile;
    private final AtomicBoolean consumed = new AtomicBoolean();

    private RegistrationMaterial(
        final Object owner,
        final String alias,
        final byte[] publicKeySec1,
        final List<byte[]> certificateChainDer,
        final byte[] attestationReport,
        final boolean strongBoxBacked,
        final OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties,
        final AssertionProfile assertionProfile) {
      this.owner = owner;
      this.alias = alias;
      this.publicKeySec1 = publicKeySec1.clone();
      this.certificateChainDer = certificateChainDer;
      this.attestationReport = attestationReport.clone();
      this.strongBoxBacked = strongBoxBacked;
      this.attestedDeviceProperties = attestedDeviceProperties;
      this.assertionProfile = Objects.requireNonNull(assertionProfile, "assertionProfile");
    }

    public String alias() {
      return alias;
    }

    /** Lowercase SHA-256 of {@link #assertionPublicKeySec1()}, as required by registration. */
    public String keyId() {
      return KagemushaAndroidKeyMint.keyId(publicKeySec1);
    }

    public byte[] assertionPublicKeySec1() {
      return publicKeySec1.clone();
    }

    /** Leaf-first Android KeyMint X.509 chain, defensively copied. */
    public List<byte[]> certificateChainDer() {
      final List<byte[]> copies = new ArrayList<>(certificateChainDer.size());
      for (final byte[] certificate : certificateChainDer) copies.add(certificate.clone());
      return copies;
    }

    /** Canonical definite-length CBOR certificate array for {@code attestation_report}. */
    public byte[] attestationReport() {
      return attestationReport.clone();
    }

    public boolean strongBoxBacked() {
      return strongBoxBacked;
    }

    /**
     * Exact leaf-derived Offline V2 properties, present only for the high-level registration flow.
     */
    public OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties() {
      return attestedDeviceProperties;
    }

    public boolean isConsumed() {
      return consumed.get();
    }

    public AssertionProfile assertionProfile() {
      return assertionProfile;
    }
  }

  /** Exact fields which exist before Android KeyMint creates the assertion key. */
  public static final class RegistrationParameters {
    private final String deviceId;
    private final String accountId;
    private final String assetDefinitionId;
    private final String androidPackageName;
    private final byte[] androidSigningCertificateSha256;
    private final KagemushaDevicePublicKeyV2 deviceAuthorityPublicKey;
    private final long recentBlockHeight;
    private final byte[] recentBlockHash;
    private final long expiresAtMs;
    private final byte[] attestationChallenge;

    public RegistrationParameters(
        final String deviceId,
        final String accountId,
        final String assetDefinitionId,
        final String androidPackageName,
        final byte[] androidSigningCertificateSha256,
        final KagemushaDevicePublicKeyV2 deviceAuthorityPublicKey,
        final long recentBlockHeight,
        final byte[] recentBlockHash,
        final long expiresAtMs) {
      this.deviceId = Objects.requireNonNull(deviceId, "deviceId");
      this.accountId = Objects.requireNonNull(accountId, "accountId");
      this.assetDefinitionId = assetDefinitionId;
      this.androidPackageName =
          Objects.requireNonNull(androidPackageName, "androidPackageName");
      this.androidSigningCertificateSha256 =
          Objects.requireNonNull(
                  androidSigningCertificateSha256, "androidSigningCertificateSha256")
              .clone();
      this.deviceAuthorityPublicKey =
          Objects.requireNonNull(deviceAuthorityPublicKey, "deviceAuthorityPublicKey");
      this.recentBlockHeight = recentBlockHeight;
      this.recentBlockHash = Objects.requireNonNull(recentBlockHash, "recentBlockHash").clone();
      this.expiresAtMs = expiresAtMs;
      this.attestationChallenge =
          DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash(
              DeviceAttestationRegistration.REGISTRATION_VERSION,
              this.deviceId,
              this.accountId,
              this.assetDefinitionId,
              this.androidPackageName,
              this.androidSigningCertificateSha256,
              this.deviceAuthorityPublicKey,
              this.recentBlockHeight,
              this.recentBlockHash,
              this.expiresAtMs);
    }

    public byte[] attestationChallenge() {
      return attestationChallenge.clone();
    }

    private byte[] attestationChallenge(final AssertionProfile assertionProfile) {
      final AssertionProfile profile =
          Objects.requireNonNull(assertionProfile, "assertionProfile");
      return DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash(
          DeviceAttestationRegistration.REGISTRATION_VERSION,
          deviceId,
          accountId,
          assetDefinitionId,
          androidPackageName,
          androidSigningCertificateSha256,
          deviceAuthorityPublicKey,
          recentBlockHeight,
          recentBlockHash,
          expiresAtMs,
          profile.scheme(),
          profile.usageCountLimit());
    }

    private DeviceAttestationRegistration registration(
        final RegistrationMaterial material) {
      return new DeviceAttestationRegistration(
          DeviceAttestationRegistration.REGISTRATION_VERSION,
          DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM,
          material.keyId(),
          deviceId,
          accountId,
          assetDefinitionId,
          null,
          null,
          null,
          androidPackageName,
          androidSigningCertificateSha256,
          Objects.requireNonNull(
              material.attestedDeviceProperties(),
              "high-level Android registration requires leaf-derived device properties"),
          deviceAuthorityPublicKey,
          material.assertionProfile.scheme(),
          DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
          material.assertionPublicKeySec1(),
          material.assertionProfile.usageCountLimit(),
          true,
          attestationChallenge(material.assertionProfile),
          null,
          material.attestationReport(),
          null,
          null,
          recentBlockHeight,
          recentBlockHash,
          expiresAtMs);
    }
  }

  /** Registration plus the retained one-use key handle needed for online authorization. */
  public static final class GeneratedRegistration {
    private final DeviceAttestationRegistration registration;
    private final RegistrationMaterial material;

    private GeneratedRegistration(
        final DeviceAttestationRegistration registration,
        final RegistrationMaterial material) {
      this.registration = Objects.requireNonNull(registration, "registration");
      this.material = Objects.requireNonNull(material, "material");
    }

    public DeviceAttestationRegistration registration() {
      return registration;
    }

    public RegistrationMaterial material() {
      return material;
    }
  }

  static final class GenerationRequest {
    private final String alias;
    private final byte[] challenge;
    private final boolean strongBoxRequired;
    private final AssertionProfile assertionProfile;
    private final String expectedPackageName;
    private final byte[] expectedSigningCertificateSha256;

    GenerationRequest(
        final String alias,
        final byte[] challenge,
        final boolean strongBoxRequired,
        final AssertionProfile assertionProfile,
        final String expectedPackageName,
        final byte[] expectedSigningCertificateSha256) {
      this.alias = alias;
      this.challenge = challenge.clone();
      this.strongBoxRequired = strongBoxRequired;
      this.assertionProfile =
          Objects.requireNonNull(assertionProfile, "assertionProfile");
      if ((expectedPackageName == null) != (expectedSigningCertificateSha256 == null)) {
        throw new IllegalArgumentException(
            "Offline V2 package and signing-certificate bindings must be supplied together");
      }
      this.expectedPackageName = expectedPackageName;
      this.expectedSigningCertificateSha256 =
          expectedSigningCertificateSha256 == null
              ? null
              : expectedSigningCertificateSha256.clone();
    }

    String alias() {
      return alias;
    }

    byte[] challenge() {
      return challenge.clone();
    }

    boolean strongBoxRequired() {
      return strongBoxRequired;
    }

    AssertionProfile assertionProfile() {
      return assertionProfile;
    }

    boolean requiresDevicePropertiesProjection() {
      return expectedPackageName != null && expectedSigningCertificateSha256 != null;
    }

    String expectedPackageName() {
      return expectedPackageName;
    }

    byte[] expectedSigningCertificateSha256() {
      return expectedSigningCertificateSha256 == null
          ? null
          : expectedSigningCertificateSha256.clone();
    }

    String keyAlgorithm() {
      return KEY_ALGORITHM;
    }

    String curveName() {
      return CURVE_NAME;
    }

    int purposes() {
      return PURPOSES;
    }

    String digest() {
      return DIGEST;
    }

    Integer maxUsageCount() {
      return assertionProfile.usageCountLimit();
    }
  }

  static final class GeneratedKey {
    private final byte[] publicKeySec1;
    private final List<byte[]> certificateChainDer;
    private final boolean insideSecureHardware;
    private final boolean strongBoxBacked;
    private final Integer remainingUsageCount;
    private final OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties;

    GeneratedKey(
        final byte[] publicKeySec1,
        final List<byte[]> certificateChainDer,
        final boolean insideSecureHardware,
        final boolean strongBoxBacked,
        final Integer remainingUsageCount,
        final OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties) {
      this.publicKeySec1 = publicKeySec1.clone();
      this.certificateChainDer = certificateChainDer;
      this.insideSecureHardware = insideSecureHardware;
      this.strongBoxBacked = strongBoxBacked;
      this.remainingUsageCount = remainingUsageCount;
      this.attestedDeviceProperties = attestedDeviceProperties;
    }

    byte[] publicKeySec1() {
      return publicKeySec1.clone();
    }

    List<byte[]> certificateChainDer() {
      return certificateChainDer;
    }

    boolean insideSecureHardware() {
      return insideSecureHardware;
    }

    boolean strongBoxBacked() {
      return strongBoxBacked;
    }

    Integer remainingUsageCount() {
      return remainingUsageCount;
    }

    OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties() {
      return attestedDeviceProperties;
    }
  }

  interface Backend {
    int apiLevel();

    boolean supportsHardwareSingleUse();

    boolean supportsStrongBox();

    default boolean supportsManagedDevicePropertiesAttestation() {
      return false;
    }

    GeneratedKey generate(GenerationRequest request) throws GeneralSecurityException;

    byte[] sign(
        String alias,
        String algorithm,
        byte[] message,
        AssertionProfile assertionProfile)
        throws GeneralSecurityException;

    void delete(String alias, AssertionProfile assertionProfile) throws GeneralSecurityException;
  }

  private static final class PlatformBackend implements Backend {
    private final Context context;
    private final DevicePolicyManager devicePolicyManager;
    private final ComponentName deviceAdmin;

    private PlatformBackend(final Context context, final ComponentName deviceAdmin)
        throws GeneralSecurityException {
      this.context = Objects.requireNonNull(context, "context");
      this.devicePolicyManager =
          Objects.requireNonNull(
              context.getSystemService(DevicePolicyManager.class),
              "DevicePolicyManager unavailable");
      this.deviceAdmin = Objects.requireNonNull(deviceAdmin, "deviceAdmin");
      if (!context.getPackageName().equals(deviceAdmin.getPackageName())) {
        throw new GeneralSecurityException(
            "managed device-admin component must belong to the embedding application");
      }
      loadKeyStore();
    }

    @Override
    public int apiLevel() {
      return Build.VERSION.SDK_INT;
    }

    @Override
    public boolean supportsHardwareSingleUse() {
      return Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
          && context
              .getPackageManager()
              .hasSystemFeature(PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY);
    }

    @Override
    public boolean supportsStrongBox() {
      return Build.VERSION.SDK_INT >= Build.VERSION_CODES.P
          && context
              .getPackageManager()
              .hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE);
    }

    @Override
    public boolean supportsManagedDevicePropertiesAttestation() {
      return Build.VERSION.SDK_INT >= Build.VERSION_CODES.P
          && Build.VERSION.SDK_INT < Build.VERSION_CODES.S
          && supportsStrongBox()
          && devicePolicyManager.isAdminActive(deviceAdmin)
          && (devicePolicyManager.isDeviceOwnerApp(context.getPackageName())
              || devicePolicyManager.isProfileOwnerApp(context.getPackageName()));
    }

    @Override
    public GeneratedKey generate(final GenerationRequest request)
        throws GeneralSecurityException {
      if (request.assertionProfile()
          == AssertionProfile.MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P
            || Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
          throw new GeneralSecurityException(
              "managed pre-Android-12 assertions require API 28--30");
        }
        return generateManagedPreAndroid12(request);
      }
      if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
        throw new GeneralSecurityException(
            "hardware usage-count assertions require Android 12 / API 31");
      }
      return generateHardwareUsageLimit(request);
    }

    @TargetApi(Build.VERSION_CODES.S)
    private GeneratedKey generateHardwareUsageLimit(final GenerationRequest request)
        throws GeneralSecurityException {
      final KeyStore keyStore = loadKeyStore();
      // An existing alias is not owned by this generation attempt and must never be removed by
      // its failure cleanup.
      if (keyStore.containsAlias(request.alias())) {
        throw new GeneralSecurityException(
            "AndroidKeyStore alias already exists: " + request.alias());
      }
      try {
        final KeyGenParameterSpec.Builder builder =
            new KeyGenParameterSpec.Builder(request.alias(), KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(new ECGenParameterSpec("secp256r1"))
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setAttestationChallenge(request.challenge())
                .setDevicePropertiesAttestationIncluded(true)
                .setMaxUsageCount(1);
        if (request.strongBoxRequired()) {
          builder.setIsStrongBoxBacked(true);
        }

        final KeyPairGenerator generator =
            KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, ANDROID_KEYSTORE);
        generator.initialize(builder.build());
        final KeyPair keyPair = generator.generateKeyPair();
        final PrivateKey privateKey = keyPair.getPrivate();
        final KeyFactory keyFactory =
            KeyFactory.getInstance(privateKey.getAlgorithm(), ANDROID_KEYSTORE);
        final KeyInfo keyInfo = keyFactory.getKeySpec(privateKey, KeyInfo.class);
        final boolean strongBox =
            keyInfo.getSecurityLevel() == KeyProperties.SECURITY_LEVEL_STRONGBOX;
        final boolean acceptedSecurityLevel =
            strongBox
                || keyInfo.getSecurityLevel()
                    == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT;
        if (!keyInfo.isInsideSecureHardware() || !acceptedSecurityLevel) {
          throw new GeneralSecurityException(
              "AndroidKeyStore generated a software-backed Kagemusha assertion key");
        }
        if (keyInfo.getPurposes() != KeyProperties.PURPOSE_SIGN
            || keyInfo.getKeySize() != 256
            || keyInfo.getRemainingUsageCount() != 1
            || !Arrays.equals(keyInfo.getDigests(), new String[] {KeyProperties.DIGEST_SHA256})) {
          throw new GeneralSecurityException(
              "AndroidKeyStore generated a key outside the Kagemusha KeyMint profile");
        }

        final Certificate[] chain = keyStore.getCertificateChain(request.alias());
        if (chain == null || chain.length == 0) {
          throw new GeneralSecurityException(
              "AndroidKeyStore did not return a KeyMint attestation chain");
        }
        final List<byte[]> certificateChain = new ArrayList<>(chain.length);
        for (final Certificate certificate : chain) {
          certificateChain.add(certificate.getEncoded());
        }
        final byte[] generatedPublicKey = uncompressedSec1(keyPair.getPublic());
        final byte[] attestedPublicKey = uncompressedSec1(chain[0].getPublicKey());
        if (!MessageDigest.isEqual(generatedPublicKey, attestedPublicKey)) {
          throw new GeneralSecurityException(
              "KeyMint attestation leaf does not bind the generated assertion key");
        }
        final OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties;
        if (request.requiresDevicePropertiesProjection()) {
          try {
            attestedDeviceProperties =
                AttestationVerifier.projectOfflineDeviceRegistrationProperties(
                    certificateChain.get(0),
                    request.challenge(),
                    request.expectedPackageName(),
                    request.expectedSigningCertificateSha256(),
                    AttestationVerifier.OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT);
          } catch (final AttestationVerificationException failure) {
            throw new GeneralSecurityException(
                "Android KeyMint leaf does not satisfy Offline Device Attestation V2",
                failure);
          }
        } else {
          attestedDeviceProperties = null;
        }
        return new GeneratedKey(
            generatedPublicKey,
            certificateChain,
            true,
            strongBox,
            keyInfo.getRemainingUsageCount(),
            attestedDeviceProperties);
      } catch (final CertificateEncodingException | ProviderException failure) {
        final GeneralSecurityException wrapped =
            new GeneralSecurityException("Android KeyMint key generation failed", failure);
        deleteWithSuppressed(request.alias(), request.assertionProfile(), wrapped);
        throw wrapped;
      } catch (final GeneralSecurityException failure) {
        deleteWithSuppressed(request.alias(), request.assertionProfile(), failure);
        throw failure;
      } catch (final RuntimeException | Error failure) {
        deleteWithSuppressed(request.alias(), request.assertionProfile(), failure);
        throw failure;
      }
    }

    @Override
    public byte[] sign(
        final String alias,
        final String algorithm,
        final byte[] message,
        final AssertionProfile assertionProfile)
        throws GeneralSecurityException {
      final PrivateKey privateKey;
      if (assertionProfile
          == AssertionProfile.MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST) {
        requireManagedOwnership();
        privateKey = managedPrivateKey(alias);
        final X509Certificate[] certificateChain = managedCertificateChain(alias);
        if (privateKey == null || certificateChain == null || certificateChain.length == 0) {
          throw new GeneralSecurityException("managed Kagemusha assertion alias is unavailable");
        }
        requireHardwareBacked(privateKey);
      } else {
        final KeyStore keyStore = loadKeyStore();
        final KeyStore.Entry entry = keyStore.getEntry(alias, null);
        if (!(entry instanceof KeyStore.PrivateKeyEntry privateKeyEntry)) {
          throw new GeneralSecurityException("Kagemusha KeyMint assertion alias is unavailable");
        }
        privateKey = privateKeyEntry.getPrivateKey();
      }
      final Signature signature = Signature.getInstance(algorithm);
      signature.initSign(privateKey);
      signature.update(message);
      return signature.sign();
    }

    @Override
    public void delete(final String alias, final AssertionProfile assertionProfile)
        throws GeneralSecurityException {
      if (assertionProfile
          == AssertionProfile.MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST) {
        requireManagedOwnership();
        if (managedAliasHasAnyMaterial(alias)) {
          if (!devicePolicyManager.removeKeyPair(deviceAdmin, alias)
              || managedAliasHasAnyMaterial(alias)) {
            throw new GeneralSecurityException(
                "managed Kagemusha assertion alias could not be deleted");
          }
        }
        return;
      }
      final KeyStore keyStore = loadKeyStore();
      if (keyStore.containsAlias(alias)) keyStore.deleteEntry(alias);
    }

    @TargetApi(Build.VERSION_CODES.P)
    private GeneratedKey generateManagedPreAndroid12(final GenerationRequest request)
        throws GeneralSecurityException {
      if (!supportsManagedDevicePropertiesAttestation()
          || !request.strongBoxRequired()
          || request.maxUsageCount() != null) {
        throw new GeneralSecurityException(
            "managed pre-Android-12 generation requires device/profile ownership and StrongBox");
      }
      if (managedAliasHasAnyMaterial(request.alias())) {
        throw new GeneralSecurityException(
            "managed KeyChain alias already exists: " + request.alias());
      }
      try {
        final KeyGenParameterSpec specification =
            new KeyGenParameterSpec.Builder(request.alias(), KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(new ECGenParameterSpec("secp256r1"))
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setAttestationChallenge(request.challenge())
                .setUserAuthenticationRequired(true)
                .setUserAuthenticationValidityDurationSeconds(30)
                .setUnlockedDeviceRequired(true)
                .setIsStrongBoxBacked(true)
                .build();
        final AttestedKeyPair attested =
            devicePolicyManager.generateKeyPair(
                deviceAdmin,
                KeyProperties.KEY_ALGORITHM_EC,
                specification,
                DevicePolicyManager.ID_TYPE_BASE_INFO);
        if (attested == null || attested.getKeyPair() == null) {
          throw new GeneralSecurityException(
              "DevicePolicyManager did not return a managed StrongBox key pair");
        }
        final KeyPair keyPair = attested.getKeyPair();
        requireHardwareBacked(keyPair.getPrivate());
        final List<Certificate> certificates = attested.getAttestationRecord();
        if (certificates == null || certificates.isEmpty()) {
          throw new GeneralSecurityException(
              "managed StrongBox device-property attestation chain is unavailable");
        }
        final List<byte[]> certificateChain = new ArrayList<>(certificates.size());
        for (final Certificate certificate : certificates) {
          certificateChain.add(certificate.getEncoded());
        }
        final byte[] generatedPublicKey = uncompressedSec1(keyPair.getPublic());
        if (!MessageDigest.isEqual(
            generatedPublicKey, uncompressedSec1(certificates.get(0).getPublicKey()))) {
          throw new GeneralSecurityException(
              "managed StrongBox attestation leaf does not bind the assertion key");
        }
        final OfflineAndroidAttestedDevicePropertiesV2 properties =
            AttestationVerifier.projectOfflineDeviceRegistrationProperties(
                certificateChain.get(0),
                request.challenge(),
                request.expectedPackageName(),
                request.expectedSigningCertificateSha256(),
                AttestationVerifier.OfflineDeviceAssertionProfile
                    .MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST);
        return new GeneratedKey(
            generatedPublicKey,
            certificateChain,
            true,
            true,
            null,
            properties);
      } catch (final CertificateEncodingException | ProviderException failure) {
        final GeneralSecurityException wrapped =
            new GeneralSecurityException(
                "managed pre-Android-12 StrongBox generation failed", failure);
        deleteWithSuppressed(request.alias(), request.assertionProfile(), wrapped);
        throw wrapped;
      } catch (final AttestationVerificationException failure) {
        final GeneralSecurityException wrapped =
            new GeneralSecurityException(
                "managed StrongBox evidence does not satisfy Offline Device Attestation V2",
                failure);
        deleteWithSuppressed(request.alias(), request.assertionProfile(), wrapped);
        throw wrapped;
      } catch (final GeneralSecurityException failure) {
        deleteWithSuppressed(request.alias(), request.assertionProfile(), failure);
        throw failure;
      } catch (final RuntimeException | Error failure) {
        deleteWithSuppressed(request.alias(), request.assertionProfile(), failure);
        throw failure;
      }
    }

    private void requireManagedOwnership() throws GeneralSecurityException {
      if (!supportsManagedDevicePropertiesAttestation()) {
        throw new GeneralSecurityException(
            "managed pre-Android-12 assertion requires current device/profile ownership");
      }
    }

    private PrivateKey managedPrivateKey(final String alias) throws GeneralSecurityException {
      try {
        return KeyChain.getPrivateKey(context, alias);
      } catch (final InterruptedException failure) {
        Thread.currentThread().interrupt();
        throw new GeneralSecurityException(
            "interrupted while loading managed assertion key", failure);
      } catch (final KeyChainException failure) {
        throw new GeneralSecurityException("failed to load managed assertion key", failure);
      }
    }

    private X509Certificate[] managedCertificateChain(final String alias)
        throws GeneralSecurityException {
      try {
        return KeyChain.getCertificateChain(context, alias);
      } catch (final InterruptedException failure) {
        Thread.currentThread().interrupt();
        throw new GeneralSecurityException(
            "interrupted while loading managed assertion certificate chain", failure);
      } catch (final KeyChainException failure) {
        throw new GeneralSecurityException(
            "failed to load managed assertion certificate chain", failure);
      }
    }

    private boolean managedAliasHasAnyMaterial(final String alias)
        throws GeneralSecurityException {
      final X509Certificate[] certificateChain = managedCertificateChain(alias);
      return managedPrivateKey(alias) != null
          || (certificateChain != null && certificateChain.length != 0);
    }

    @SuppressWarnings("deprecation")
    private void requireHardwareBacked(final PrivateKey privateKey)
        throws GeneralSecurityException {
      final KeyFactory factory =
          KeyFactory.getInstance(privateKey.getAlgorithm(), ANDROID_KEYSTORE);
      final KeyInfo keyInfo = factory.getKeySpec(privateKey, KeyInfo.class);
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        final int level = keyInfo.getSecurityLevel();
        if (level != KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
            && level != KeyProperties.SECURITY_LEVEL_STRONGBOX) {
          throw new GeneralSecurityException("Kagemusha assertion key is software-backed");
        }
      } else if (!keyInfo.isInsideSecureHardware()) {
        throw new GeneralSecurityException(
            "managed pre-Android-12 assertion key is not inside secure hardware");
      }
    }

    private KeyStore loadKeyStore() throws GeneralSecurityException {
      try {
        final KeyStore keyStore = KeyStore.getInstance(ANDROID_KEYSTORE);
        keyStore.load(null);
        return keyStore;
      } catch (final IOException failure) {
        throw new GeneralSecurityException("failed to load AndroidKeyStore", failure);
      }
    }

    private void deleteWithSuppressed(
        final String alias,
        final AssertionProfile assertionProfile,
        final Throwable failure) {
      try {
        delete(alias, assertionProfile);
      } catch (final GeneralSecurityException cleanupFailure) {
        failure.addSuppressed(cleanupFailure);
      }
    }
  }
}
