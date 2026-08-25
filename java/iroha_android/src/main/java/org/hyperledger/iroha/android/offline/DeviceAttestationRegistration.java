// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;
import java.util.Base64;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/**
 * Strict first-release model for one finalized platform device attestation.
 *
 * <p>This type mirrors {@code OfflineDeviceAttestationRegistration} exactly. Native clients use
 * bridge ABI 22; the registration's on-chain format marker remains version 1.
 */
final class DeviceAttestationRegistration {

  /** Sole native bridge ABI supported by the first-release Kagemusha client. */
  public static final int REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 22;

  /** Sole on-chain registration format marker. */
  public static final int REGISTRATION_VERSION = 1;

  public static final String ANDROID_KEYMINT_PLATFORM = "android-keymint";
  public static final String ANDROID_KEYMINT_ASSERTION_SCHEME =
      "android-keymint-ecdsa-p256-usage-limit-v1";
  public static final String ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM = "ecdsa-p256-sha256";
  public static final String IOS_APP_ATTEST_PLATFORM = "ios-appattest";
  public static final String IOS_APP_ATTEST_ASSERTION_SCHEME = "apple-appattest-counter-v1";
  public static final String IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM = "app-attest-p256";
  public static final String DEVICE_ATTESTATION_CHALLENGE_DOMAIN =
      "iroha:kagemusha:device-attestation-challenge:v1";
  public static final String DEVICE_ATTESTATION_EVIDENCE_PREFIX =
      "offline-device-attestation-evidence-v1";

  private static final int MAX_REPORT_BYTES = 64 * 1024;
  private static final int MAX_EVIDENCE_BYTES = 128 * 1024;
  private static final byte[] EVIDENCE_PREFIX_BYTES =
      DEVICE_ATTESTATION_EVIDENCE_PREFIX.getBytes(StandardCharsets.UTF_8);

  private final int version;
  private final String platform;
  private final String keyId;
  private final String deviceId;
  private final String accountId;
  private final String assetDefinitionId;
  private final String iosTeamId;
  private final String iosBundleId;
  private final String iosEnvironment;
  private final String androidPackageName;
  private final byte[] androidSigningCertificateSha256;
  private final OfflineAndroidAttestedDevicePropertiesV2 androidAttestedDeviceProperties;
  private final KagemushaDevicePublicKeyV2 publicKey;
  private final String assertionScheme;
  private final String assertionKeyAlgorithm;
  private final byte[] assertionPublicKey;
  private final Integer assertionUsageCountLimit;
  private final boolean oneUse;
  private final byte[] challengeHash;
  private final byte[] attestationReportHash;
  private final byte[] attestationReport;
  private final byte[] evidenceHash;
  private final byte[] evidence;
  private final long recentBlockHeight;
  private final byte[] recentBlockHash;
  private final long expiresAtMs;

  /**
   * Construct and validate the exact current registration.
   *
   * <p>Hash arguments may be {@code null}; in that case the canonical hash is derived. Empty
   * evidence with no supplied evidence hash is replaced by the required domain-separated report
   * envelope. The platform report itself must always be present.
   */
  public DeviceAttestationRegistration(
      final int version,
      final String platform,
      final String keyId,
      final String deviceId,
      final String accountId,
      final String assetDefinitionId,
      final String iosTeamId,
      final String iosBundleId,
      final String iosEnvironment,
      final String androidPackageName,
      final byte[] androidSigningCertificateSha256,
      final KagemushaDevicePublicKeyV2 publicKey,
      final String assertionScheme,
      final String assertionKeyAlgorithm,
      final byte[] assertionPublicKey,
      final Integer assertionUsageCountLimit,
      final boolean oneUse,
      final byte[] challengeHash,
      final byte[] attestationReportHash,
      final byte[] attestationReport,
      final byte[] evidenceHash,
      final byte[] evidence,
      final long recentBlockHeight,
      final byte[] recentBlockHash,
      final long expiresAtMs) {
    this(
        version,
        platform,
        keyId,
        deviceId,
        accountId,
        assetDefinitionId,
        iosTeamId,
        iosBundleId,
        iosEnvironment,
        androidPackageName,
        androidSigningCertificateSha256,
        null,
        publicKey,
        assertionScheme,
        assertionKeyAlgorithm,
        assertionPublicKey,
        assertionUsageCountLimit,
        oneUse,
        challengeHash,
        attestationReportHash,
        attestationReport,
        evidenceHash,
        evidence,
        recentBlockHeight,
        recentBlockHash,
        expiresAtMs);
  }

  /** Construct the exact current registration, including optional ABI22 Android properties. */
  public DeviceAttestationRegistration(
      final int version,
      final String platform,
      final String keyId,
      final String deviceId,
      final String accountId,
      final String assetDefinitionId,
      final String iosTeamId,
      final String iosBundleId,
      final String iosEnvironment,
      final String androidPackageName,
      final byte[] androidSigningCertificateSha256,
      final OfflineAndroidAttestedDevicePropertiesV2 androidAttestedDeviceProperties,
      final KagemushaDevicePublicKeyV2 publicKey,
      final String assertionScheme,
      final String assertionKeyAlgorithm,
      final byte[] assertionPublicKey,
      final Integer assertionUsageCountLimit,
      final boolean oneUse,
      final byte[] challengeHash,
      final byte[] attestationReportHash,
      final byte[] attestationReport,
      final byte[] evidenceHash,
      final byte[] evidence,
      final long recentBlockHeight,
      final byte[] recentBlockHash,
      final long expiresAtMs) {
    this.version = version;
    this.platform = requireExactText(platform, "platform");
    this.keyId = requireExactText(keyId, "key_id");
    this.deviceId = requireExactText(deviceId, "device_id");
    this.accountId = requireExactText(accountId, "account_id");
    this.assetDefinitionId = assetDefinitionId;
    this.iosTeamId = requireOptionalExactText(iosTeamId, "ios_team_id");
    this.iosBundleId = requireOptionalExactText(iosBundleId, "ios_bundle_id");
    this.iosEnvironment = requireOptionalExactText(iosEnvironment, "ios_environment");
    this.androidPackageName =
        requireOptionalExactText(androidPackageName, "android_package_name");
    this.androidSigningCertificateSha256 = copyNullable(androidSigningCertificateSha256);
    this.androidAttestedDeviceProperties = androidAttestedDeviceProperties;
    this.publicKey = Objects.requireNonNull(publicKey, "public_key");
    this.assertionScheme = requireExactText(assertionScheme, "assertion_scheme");
    this.assertionKeyAlgorithm =
        requireExactText(assertionKeyAlgorithm, "assertion_key_algorithm");
    this.assertionPublicKey = copy(assertionPublicKey, "assertion_public_key");
    this.assertionUsageCountLimit = assertionUsageCountLimit;
    this.oneUse = oneUse;
    this.attestationReport = copy(attestationReport, "attestation_report");
    this.recentBlockHeight = recentBlockHeight;
    this.recentBlockHash = copy(recentBlockHash, "recent_block_hash");
    this.expiresAtMs = expiresAtMs;

    requireCore();
    requirePlatformProfile();

    final byte[] resolvedChallengeHash = canonicalChallengeHash();
    if (challengeHash != null) {
      requireHash(challengeHash, "challenge_hash");
      if (!Arrays.equals(challengeHash, resolvedChallengeHash)) {
        throw new IllegalArgumentException(
            "challenge_hash does not match the canonical attestation preimage");
      }
    }
    this.challengeHash = resolvedChallengeHash;

    final byte[] expectedReportHash = IrohaHash.prehash(this.attestationReport);
    final byte[] resolvedReportHash =
        attestationReportHash == null ? expectedReportHash : copy(attestationReportHash, "attestation_report_hash");
    requireHash(resolvedReportHash, "attestation_report_hash");
    if (!Arrays.equals(resolvedReportHash, expectedReportHash)) {
      throw new IllegalArgumentException(
          "attestation_report_hash does not match attestation_report");
    }
    this.attestationReportHash = resolvedReportHash;

    final byte[] submittedEvidence = evidence == null ? new byte[0] : evidence.clone();
    this.evidence =
        submittedEvidence.length == 0 && evidenceHash == null
            ? evidenceEnvelope(resolvedReportHash)
            : submittedEvidence;
    requireEvidenceEnvelope(this.evidence, resolvedReportHash);
    if (this.evidence.length > MAX_EVIDENCE_BYTES) {
      throw new IllegalArgumentException("evidence exceeds the on-chain size limit");
    }
    final byte[] expectedEvidenceHash = IrohaHash.prehash(this.evidence);
    final byte[] resolvedEvidenceHash =
        evidenceHash == null ? expectedEvidenceHash : copy(evidenceHash, "evidence_hash");
    requireHash(resolvedEvidenceHash, "evidence_hash");
    if (!Arrays.equals(resolvedEvidenceHash, expectedEvidenceHash)) {
      throw new IllegalArgumentException("evidence_hash does not match evidence");
    }
    this.evidenceHash = resolvedEvidenceHash;
  }

  /** Decode a canonical framed registration and reject any alternate representation. */
  public static DeviceAttestationRegistration decodeCanonical(
      final byte[] archive, final int chainDiscriminant) {
    return OfflineDeviceAttestationCodec.decodeRegistrationCanonical(
        archive, chainDiscriminant);
  }

  /** Encode the exact current registration as a framed Norito archive. */
  public byte[] noritoEncoded() {
    return OfflineDeviceAttestationCodec.encodeRegistration(this);
  }

  /** Deterministic challenge hash that the platform report must bind. */
  public byte[] canonicalChallengeHash() {
    return OfflineDeviceAttestationCodec.canonicalChallengeHash(this);
  }

  /** Canonical Iroha Hash/registration ID of the exact framed Norito registration archive. */
  public byte[] canonicalRegistrationHash() {
    return IrohaHash.prehash(noritoEncoded());
  }

  /**
   * Build the canonical Android challenge before KeyMint generates the assertion key.
   *
   * <p>The pre-key challenge intentionally excludes {@code key_id} and the assertion public key.
   */
  public static byte[] androidPreKeyGenerationChallengeHash(
      final int version,
      final String deviceId,
      final String accountId,
      final String assetDefinitionId,
      final String androidPackageName,
      final byte[] androidSigningCertificateSha256,
      final KagemushaDevicePublicKeyV2 publicKey,
      final long recentBlockHeight,
      final byte[] recentBlockHash,
      final long expiresAtMs) {
    return OfflineDeviceAttestationCodec.androidPreKeyGenerationChallengeHash(
        version,
        deviceId,
        accountId,
        assetDefinitionId,
        androidPackageName,
        androidSigningCertificateSha256,
        Objects.requireNonNull(publicKey, "publicKey").sec1Bytes(),
        recentBlockHeight,
        recentBlockHash,
        expiresAtMs);
  }

  private void requireCore() {
    if (version != REGISTRATION_VERSION) {
      throw new IllegalArgumentException("registration version must be exactly 1");
    }
    if (!oneUse) {
      throw new IllegalArgumentException("device attestation authority must be one-use");
    }
    // Canonical account and optional asset decoding also rejects noncanonical literals.
    OfflineDeviceAttestationCodec.validateAccountId(accountId);
    if (assetDefinitionId != null) {
      AssetDefinitionIdEncoder.parseAddressBytes(assetDefinitionId);
    }
    if (recentBlockHeight <= 0) {
      throw new IllegalArgumentException("recent_block_height must be positive");
    }
    requireHash(recentBlockHash, "recent_block_hash");
    if (expiresAtMs <= 0) {
      throw new IllegalArgumentException("expires_at_ms must be positive");
    }
    if (attestationReport.length == 0 || attestationReport.length > MAX_REPORT_BYTES) {
      throw new IllegalArgumentException(
          "attestation_report must be non-empty and within the on-chain size limit");
    }
  }

  private void requirePlatformProfile() {
    KagemushaP256Codec.requireUncompressedPublicKey(assertionPublicKey);
    if (ANDROID_KEYMINT_PLATFORM.equals(platform)) {
      if (!ANDROID_KEYMINT_ASSERTION_SCHEME.equals(assertionScheme)
          || !ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM.equals(assertionKeyAlgorithm)
          || !Integer.valueOf(1).equals(assertionUsageCountLimit)) {
        throw new IllegalArgumentException(
            "Android KeyMint requires the canonical one-use P-256 assertion profile");
      }
      if (androidPackageName == null) {
        throw new IllegalArgumentException("Android KeyMint requires android_package_name");
      }
      if (androidSigningCertificateSha256 == null
          || androidSigningCertificateSha256.length != 32
          || allZero(androidSigningCertificateSha256)) {
        throw new IllegalArgumentException(
            "Android KeyMint requires a non-zero 32-byte signing certificate SHA-256");
      }
      if (iosTeamId != null || iosBundleId != null || iosEnvironment != null) {
        throw new IllegalArgumentException("Android KeyMint must not carry iOS app metadata");
      }
      if (!keyId.equals(hexLower(sha256(assertionPublicKey)))) {
        throw new IllegalArgumentException(
            "Android KeyMint key_id must be lowercase SHA-256 of assertion_public_key");
      }
      return;
    }
    if (IOS_APP_ATTEST_PLATFORM.equals(platform)) {
      if (!IOS_APP_ATTEST_ASSERTION_SCHEME.equals(assertionScheme)
          || !IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM.equals(assertionKeyAlgorithm)
          || assertionUsageCountLimit != null) {
        throw new IllegalArgumentException(
            "iOS App Attest requires the canonical P-256 assertion profile");
      }
      final byte[] credentialId;
      try {
        credentialId = Base64.getDecoder().decode(keyId);
      } catch (final IllegalArgumentException ex) {
        throw new IllegalArgumentException("iOS App Attest key_id must be canonical base64", ex);
      }
      if (credentialId.length == 0
          || !Base64.getEncoder().encodeToString(credentialId).equals(keyId)) {
        throw new IllegalArgumentException("iOS App Attest key_id must be canonical base64");
      }
      if (iosTeamId == null || iosBundleId == null || iosEnvironment == null) {
        throw new IllegalArgumentException("iOS App Attest requires complete app metadata");
      }
      if (!"production".equals(iosEnvironment) && !"development".equals(iosEnvironment)) {
        throw new IllegalArgumentException(
            "ios_environment must be production or development");
      }
      if (androidPackageName != null
          || androidSigningCertificateSha256 != null
          || androidAttestedDeviceProperties != null) {
        throw new IllegalArgumentException("iOS App Attest must not carry Android app metadata");
      }
      return;
    }
    throw new IllegalArgumentException("unsupported device attestation platform: " + platform);
  }

  static void requireHash(final byte[] value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.length != 32 || (value[31] & 1) != 1) {
      throw new IllegalArgumentException(field + " must be a canonical 32-byte Iroha hash");
    }
  }

  private static byte[] evidenceEnvelope(final byte[] reportHash) {
    final byte[] out = Arrays.copyOf(EVIDENCE_PREFIX_BYTES, EVIDENCE_PREFIX_BYTES.length + 32);
    System.arraycopy(reportHash, 0, out, EVIDENCE_PREFIX_BYTES.length, 32);
    return out;
  }

  private static void requireEvidenceEnvelope(final byte[] value, final byte[] reportHash) {
    if (value.length != EVIDENCE_PREFIX_BYTES.length + 32) {
      throw new IllegalArgumentException("evidence must bind exactly one attestation report hash");
    }
    for (int index = 0; index < EVIDENCE_PREFIX_BYTES.length; index++) {
      if (value[index] != EVIDENCE_PREFIX_BYTES[index]) {
        throw new IllegalArgumentException("evidence prefix is not canonical");
      }
    }
    for (int index = 0; index < 32; index++) {
      if (value[EVIDENCE_PREFIX_BYTES.length + index] != reportHash[index]) {
        throw new IllegalArgumentException("evidence does not bind attestation_report_hash");
      }
    }
  }

  private static String requireExactText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }

  private static String requireOptionalExactText(final String value, final String field) {
    return value == null ? null : requireExactText(value, field);
  }

  private static byte[] copy(final byte[] value, final String field) {
    return Objects.requireNonNull(value, field).clone();
  }

  private static byte[] copyNullable(final byte[] value) {
    return value == null ? null : value.clone();
  }

  private static byte[] sha256(final byte[] value) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(value);
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 is unavailable", ex);
    }
  }

  private static String hexLower(final byte[] value) {
    final char[] alphabet = "0123456789abcdef".toCharArray();
    final char[] out = new char[value.length * 2];
    for (int index = 0; index < value.length; index++) {
      out[index * 2] = alphabet[(value[index] >>> 4) & 0x0f];
      out[index * 2 + 1] = alphabet[value[index] & 0x0f];
    }
    return new String(out);
  }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte item : value) {
      aggregate |= item;
    }
    return aggregate == 0;
  }

  public int version() {
    return version;
  }

  public String platform() {
    return platform;
  }

  public String keyId() {
    return keyId;
  }

  public String deviceId() {
    return deviceId;
  }

  public String accountId() {
    return accountId;
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public String iosTeamId() {
    return iosTeamId;
  }

  public String iosBundleId() {
    return iosBundleId;
  }

  public String iosEnvironment() {
    return iosEnvironment;
  }

  public String androidPackageName() {
    return androidPackageName;
  }

  public byte[] androidSigningCertificateSha256() {
    return copyNullable(androidSigningCertificateSha256);
  }

  public OfflineAndroidAttestedDevicePropertiesV2 androidAttestedDeviceProperties() {
    return androidAttestedDeviceProperties;
  }

  public KagemushaDevicePublicKeyV2 publicKey() {
    return publicKey;
  }

  public String assertionScheme() {
    return assertionScheme;
  }

  public String assertionKeyAlgorithm() {
    return assertionKeyAlgorithm;
  }

  public byte[] assertionPublicKey() {
    return assertionPublicKey.clone();
  }

  public Integer assertionUsageCountLimit() {
    return assertionUsageCountLimit;
  }

  public boolean oneUse() {
    return oneUse;
  }

  public byte[] challengeHash() {
    return challengeHash.clone();
  }

  public byte[] attestationReportHash() {
    return attestationReportHash.clone();
  }

  public byte[] attestationReport() {
    return attestationReport.clone();
  }

  public byte[] evidenceHash() {
    return evidenceHash.clone();
  }

  public byte[] evidence() {
    return evidence.clone();
  }

  public long recentBlockHeight() {
    return recentBlockHeight;
  }

  public byte[] recentBlockHash() {
    return recentBlockHash.clone();
  }

  public long expiresAtMs() {
    return expiresAtMs;
  }

  @Override
  public boolean equals(final Object object) {
    if (this == object) {
      return true;
    }
    if (!(object instanceof DeviceAttestationRegistration other)) {
      return false;
    }
    return version == other.version
        && oneUse == other.oneUse
        && recentBlockHeight == other.recentBlockHeight
        && expiresAtMs == other.expiresAtMs
        && platform.equals(other.platform)
        && keyId.equals(other.keyId)
        && deviceId.equals(other.deviceId)
        && accountId.equals(other.accountId)
        && Objects.equals(assetDefinitionId, other.assetDefinitionId)
        && Objects.equals(iosTeamId, other.iosTeamId)
        && Objects.equals(iosBundleId, other.iosBundleId)
        && Objects.equals(iosEnvironment, other.iosEnvironment)
        && Objects.equals(androidPackageName, other.androidPackageName)
        && Arrays.equals(androidSigningCertificateSha256, other.androidSigningCertificateSha256)
        && Objects.equals(androidAttestedDeviceProperties, other.androidAttestedDeviceProperties)
        && publicKey.equals(other.publicKey)
        && assertionScheme.equals(other.assertionScheme)
        && assertionKeyAlgorithm.equals(other.assertionKeyAlgorithm)
        && Arrays.equals(assertionPublicKey, other.assertionPublicKey)
        && Objects.equals(assertionUsageCountLimit, other.assertionUsageCountLimit)
        && Arrays.equals(challengeHash, other.challengeHash)
        && Arrays.equals(attestationReportHash, other.attestationReportHash)
        && Arrays.equals(attestationReport, other.attestationReport)
        && Arrays.equals(evidenceHash, other.evidenceHash)
        && Arrays.equals(evidence, other.evidence)
        && Arrays.equals(recentBlockHash, other.recentBlockHash);
  }

  @Override
  public int hashCode() {
    int result =
        Objects.hash(
            version,
            platform,
            keyId,
            deviceId,
            accountId,
            assetDefinitionId,
            iosTeamId,
            iosBundleId,
            iosEnvironment,
            androidPackageName,
            androidAttestedDeviceProperties,
            publicKey,
            assertionScheme,
            assertionKeyAlgorithm,
            assertionUsageCountLimit,
            oneUse,
            recentBlockHeight,
            expiresAtMs);
    result = 31 * result + Arrays.hashCode(androidSigningCertificateSha256);
    result = 31 * result + Arrays.hashCode(assertionPublicKey);
    result = 31 * result + Arrays.hashCode(challengeHash);
    result = 31 * result + Arrays.hashCode(attestationReportHash);
    result = 31 * result + Arrays.hashCode(attestationReport);
    result = 31 * result + Arrays.hashCode(evidenceHash);
    result = 31 * result + Arrays.hashCode(evidence);
    result = 31 * result + Arrays.hashCode(recentBlockHash);
    return result;
  }
}
