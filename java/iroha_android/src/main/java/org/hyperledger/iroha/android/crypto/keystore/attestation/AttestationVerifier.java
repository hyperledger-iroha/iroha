package org.hyperledger.iroha.android.crypto.keystore.attestation;

import java.io.ByteArrayInputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.cert.CertPath;
import java.security.cert.CertPathValidator;
import java.security.cert.CertPathValidatorException;
import java.security.cert.CertificateException;
import java.security.cert.CertificateFactory;
import java.security.cert.PKIXParameters;
import java.security.cert.TrustAnchor;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.android.offline.OfflineAndroidAttestedDevicePropertiesV2;

/**
 * Validates Android key attestation certificate chains and extracts metadata required by higher
 * level policy checks.
 */
public final class AttestationVerifier {

  private static final String ATTESTATION_OID = "1.3.6.1.4.1.11129.2.1.17";
  private static final BigInteger LONG_MIN = BigInteger.valueOf(Long.MIN_VALUE);
  private static final BigInteger LONG_MAX = BigInteger.valueOf(Long.MAX_VALUE);
  private static final int TAG_USAGE_COUNT_LIMIT = 405;
  private static final int TAG_ALL_APPLICATIONS = 600;
  private static final int TAG_ROOT_OF_TRUST = 704;
  private static final int TAG_OS_VERSION = 705;
  private static final int TAG_OS_PATCH_LEVEL = 706;
  private static final int TAG_ATTESTATION_APPLICATION_ID = 709;
  private static final int TAG_ATTESTATION_ID_BRAND = 710;
  private static final int TAG_ATTESTATION_ID_DEVICE = 711;
  private static final int TAG_ATTESTATION_ID_PRODUCT = 712;
  private static final int TAG_ATTESTATION_ID_MANUFACTURER = 716;
  private static final int TAG_ATTESTATION_ID_MODEL = 717;
  private static final int TAG_VENDOR_PATCH_LEVEL = 718;
  private static final int TAG_BOOT_PATCH_LEVEL = 719;
  private static final long ANDROID_12_OS_VERSION_FLOOR = 120_000L;
  private static final Set<Integer> DEVICE_PROPERTY_TAGS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  TAG_ROOT_OF_TRUST,
                  TAG_OS_VERSION,
                  TAG_OS_PATCH_LEVEL,
                  TAG_ATTESTATION_ID_BRAND,
                  TAG_ATTESTATION_ID_DEVICE,
                  TAG_ATTESTATION_ID_PRODUCT,
                  TAG_ATTESTATION_ID_MANUFACTURER,
                  TAG_ATTESTATION_ID_MODEL,
                  TAG_VENDOR_PATCH_LEVEL,
                  TAG_BOOT_PATCH_LEVEL)));

  private final Set<TrustAnchor> trustAnchors;
  private final boolean requireStrongBox;

  /** Closed Android assertion profiles accepted by Offline Device Attestation V2. */
  public enum OfflineDeviceAssertionProfile {
    /** Android 12+ KeyMint key with hardware-enforced tag 405 equal to one. */
    HARDWARE_USAGE_LIMIT,
    /** Managed API 28--30 StrongBox key consumed receipt-first and deleted after signing. */
    MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST
  }

  private AttestationVerifier(final Builder builder) {
    if (builder.trustedRoots.isEmpty()) {
      throw new IllegalStateException("At least one trusted root certificate is required");
    }
    final Set<TrustAnchor> anchors = new LinkedHashSet<>();
    for (final X509Certificate certificate : builder.trustedRoots) {
      anchors.add(new TrustAnchor(certificate, null));
    }
    this.trustAnchors = Collections.unmodifiableSet(new LinkedHashSet<>(anchors));
    this.requireStrongBox = builder.requireStrongBox;
  }

  /** Creates a verifier that trusts the supplied root certificates. */
  public static Builder builder() {
    return new Builder();
  }

  /** Validates {@code attestation} against the configured policy. */
  public AttestationResult verify(final KeyAttestation attestation)
      throws AttestationVerificationException {
    return verify(attestation, null);
  }

  /**
   * Validates {@code attestation} and checks that the embedded challenge matches {@code
   * expectedChallenge} when provided.
   */
  public AttestationResult verify(final KeyAttestation attestation, final byte[] expectedChallenge)
      throws AttestationVerificationException {
    return verify(attestation, expectedChallenge, null);
  }

  /**
   * Validate the complete Offline Device Attestation V2 KeyDescription profile.
   *
   * <p>In addition to certificate-path and challenge validation, this requires
   * hardware-enforced {@code usageCountLimit = 1}, rejects {@code allApplications}, authenticates
   * the exact package/signing identity from tag 709, and projects the exact device properties that
   * native admission later byte-compares with the submitted registration.
   */
  public AttestationResult verifyOfflineDeviceRegistration(
      final KeyAttestation attestation,
      final byte[] expectedChallenge,
      final String expectedPackageName,
      final byte[] expectedSigningCertificateSha256)
      throws AttestationVerificationException {
    return verifyOfflineDeviceRegistration(
        attestation,
        expectedChallenge,
        expectedPackageName,
        expectedSigningCertificateSha256,
        OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT);
  }

  /** Validate one explicit, consensus-identical Android assertion profile. */
  public AttestationResult verifyOfflineDeviceRegistration(
      final KeyAttestation attestation,
      final byte[] expectedChallenge,
      final String expectedPackageName,
      final byte[] expectedSigningCertificateSha256,
      final OfflineDeviceAssertionProfile assertionProfile)
      throws AttestationVerificationException {
    if (expectedChallenge == null || expectedChallenge.length != 32) {
      throw new AttestationVerificationException(
          "Offline registration challenge must contain exactly 32 bytes");
    }
    if (expectedPackageName == null
        || expectedPackageName.isEmpty()
        || !expectedPackageName.equals(expectedPackageName.trim())) {
      throw new AttestationVerificationException(
          "Offline registration package name is not canonical");
    }
    if (expectedSigningCertificateSha256 == null
        || expectedSigningCertificateSha256.length != 32
        || allZero(expectedSigningCertificateSha256)) {
      throw new AttestationVerificationException(
          "Offline registration signing-certificate digest must be a non-zero 32-byte value");
    }
    return verify(
        attestation,
        expectedChallenge,
        new OfflineRegistrationBinding(
            expectedPackageName,
            expectedSigningCertificateSha256.clone(),
            Objects.requireNonNull(assertionProfile, "assertionProfile")));
  }

  private AttestationResult verify(
      final KeyAttestation attestation,
      final byte[] expectedChallenge,
      final OfflineRegistrationBinding offlineRegistrationBinding)
      throws AttestationVerificationException {
    Objects.requireNonNull(attestation, "attestation");
    final List<X509Certificate> chain = decodeChain(attestation);
    if (chain.isEmpty()) {
      throw new AttestationVerificationException("Attestation certificate chain is empty");
    }
    final X509Certificate leaf = chain.get(0);

    validateCertificatePath(chain);

    final KeyDescription description =
        parseKeyDescription(leaf, offlineRegistrationBinding);
    if (expectedChallenge != null
        && !MessageDigest.isEqual(expectedChallenge, description.attestationChallenge)) {
      throw new AttestationVerificationException("Attestation challenge mismatch");
    }
    if (requireStrongBox
        && description.attestationSecurityLevel != AttestationResult.SecurityLevel.STRONG_BOX) {
      throw new AttestationVerificationException("StrongBox attestation required by policy");
    }

    return new AttestationResult(
        attestation.alias(),
        chain,
        description.attestationSecurityLevel,
        description.keymasterSecurityLevel,
        description.attestationChallenge,
        description.uniqueId,
        description.softwareAuthorisationsLength > 0,
        description.teeAuthorisationsLength > 0,
        description.strongBoxAuthorisationsLength > 0,
        description.attestedDeviceProperties);
  }

  private List<X509Certificate> decodeChain(final KeyAttestation attestation)
      throws AttestationVerificationException {
    final CertificateFactory factory;
    try {
      factory = CertificateFactory.getInstance("X.509");
    } catch (final CertificateException ex) {
      throw new AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex);
    }

    final List<X509Certificate> certificates = new ArrayList<>();
    for (final byte[] certificateDer : attestation.certificateChain()) {
      try {
        certificates.add(
            (X509Certificate) factory.generateCertificate(new ByteArrayInputStream(certificateDer)));
      } catch (final CertificateException ex) {
        throw new AttestationVerificationException("Failed to decode attestation certificate", ex);
      }
    }
    return certificates;
  }

  private void validateCertificatePath(final List<X509Certificate> chain)
      throws AttestationVerificationException {
    final CertificateFactory factory;
    try {
      factory = CertificateFactory.getInstance("X.509");
    } catch (final CertificateException ex) {
      throw new AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex);
    }

    final CertPath certPath;
    try {
      certPath = factory.generateCertPath(certificatesForPath(chain));
    } catch (final CertificateException ex) {
      throw new AttestationVerificationException("Failed to construct attestation CertPath", ex);
    }

    final CertPathValidator validator;
    try {
      validator = CertPathValidator.getInstance("PKIX");
    } catch (final Exception ex) {
      throw new AttestationVerificationException("Unable to acquire PKIX CertPathValidator", ex);
    }

    final PKIXParameters parameters;
    try {
      parameters = new PKIXParameters(trustAnchors);
    } catch (final Exception ex) {
      throw new AttestationVerificationException("Invalid PKIX parameters", ex);
    }
    parameters.setRevocationEnabled(false);

    try {
      validator.validate(certPath, parameters);
    } catch (final CertPathValidatorException ex) {
      throw new AttestationVerificationException("Attestation certificate path validation failed", ex);
    } catch (final Exception ex) {
      throw new AttestationVerificationException(
          "Unexpected failure validating attestation certificate path", ex);
    }
  }

  private List<X509Certificate> certificatesForPath(final List<X509Certificate> chain) {
    if (chain.size() < 2) {
      return chain;
    }
    final X509Certificate trailingCertificate = chain.get(chain.size() - 1);
    for (final TrustAnchor anchor : trustAnchors) {
      final X509Certificate trusted = anchor.getTrustedCert();
      if (trusted != null && sameTrustAnchorCertificate(trailingCertificate, trusted)) {
        // The configured trust anchor is not part of the PKIX CertPath. Android
        // attestation exports often include it as the final chain entry.
        return chain.subList(0, chain.size() - 1);
      }
    }
    return chain;
  }

  private static boolean sameTrustAnchorCertificate(
      final X509Certificate certificate, final X509Certificate trusted) {
    return certificate.getSubjectX500Principal().equals(trusted.getSubjectX500Principal())
        && certificate.getPublicKey().equals(trusted.getPublicKey());
  }

  private static KeyDescription parseKeyDescription(
      final X509Certificate leaf,
      final OfflineRegistrationBinding offlineRegistrationBinding)
      throws AttestationVerificationException {
    final byte[] extension = leaf.getExtensionValue(ATTESTATION_OID);
    if (extension == null) {
      throw new AttestationVerificationException(
          "Leaf certificate does not contain Android attestation extension");
    }

    final DerReader outer = new DerReader(extension);
    final byte[] octetString = outer.readOctetString();
    if (outer.hasRemaining()) {
      throw new AttestationVerificationException("Unexpected data after attestation extension");
    }

    final DerReader reader = DerReader.sequence(octetString);
    final long attestationVersion = reader.readInteger64();
    if (attestationVersion <= 0 || attestationVersion > 0xffff_ffffL) {
      throw new AttestationVerificationException("Invalid attestation version: " + attestationVersion);
    }

    final AttestationResult.SecurityLevel attestationLevel =
        AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated());
    final long keymasterVersion = reader.readInteger64();
    if (keymasterVersion <= 0 || keymasterVersion > 0xffff_ffffL) {
      throw new AttestationVerificationException("Invalid keymaster version: " + keymasterVersion);
    }
    final AttestationResult.SecurityLevel keymasterLevel =
        AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated());
    final byte[] challenge = reader.readOctetString();
    final byte[] uniqueId = reader.readOctetString();
    final byte[] softwareEnforced = reader.readSequenceBytes();
    final byte[] teeEnforced = reader.readSequenceBytes();
    if (reader.hasRemaining()) {
      throw new AttestationVerificationException("Unexpected trailing data in attestation");
    }

    OfflineAndroidAttestedDevicePropertiesV2 properties = null;
    if (offlineRegistrationBinding != null) {
      if (attestationLevel != keymasterLevel
          || attestationLevel == AttestationResult.SecurityLevel.SOFTWARE) {
        throw new AttestationVerificationException(
            "Attestation and KeyMint security levels must name the same hardware boundary");
      }
      properties =
          parseOfflineDeviceRegistration(
              attestationVersion,
              keymasterVersion,
              attestationLevel,
              softwareEnforced,
              teeEnforced,
              offlineRegistrationBinding);
    }

    return new KeyDescription(
        attestationLevel, keymasterLevel, challenge, uniqueId, softwareEnforced.length,
        teeEnforced.length,
        attestationLevel == AttestationResult.SecurityLevel.STRONG_BOX
            ? teeEnforced.length : 0,
        properties);
  }

  private static OfflineAndroidAttestedDevicePropertiesV2 parseOfflineDeviceRegistration(
      final long attestationVersion,
      final long keymasterVersion,
      final AttestationResult.SecurityLevel securityLevel,
      final byte[] softwareEnforced,
      final byte[] hardwareEnforced,
      final OfflineRegistrationBinding binding)
      throws AttestationVerificationException {
    final Map<Integer, byte[]> software = parseAuthorizationList(softwareEnforced);
    for (final int tag : DEVICE_PROPERTY_TAGS) {
      if (software.containsKey(tag)) {
        throw new AttestationVerificationException(
            "Android attested-device property must be hardwareEnforced: " + tag);
      }
    }
    final Map<Integer, byte[]> hardware = parseAuthorizationList(hardwareEnforced);
    if (software.containsKey(TAG_USAGE_COUNT_LIMIT)) {
      throw new AttestationVerificationException(
          "Android usageCountLimit must be hardwareEnforced");
    }
    if (binding.assertionProfile == OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT) {
      if (readAuthorizationInteger(hardware.get(TAG_USAGE_COUNT_LIMIT)) != 1L) {
        throw new AttestationVerificationException(
            "Android hardware usageCountLimit must be exactly one");
      }
    } else {
      if (hardware.containsKey(TAG_USAGE_COUNT_LIMIT)) {
        throw new AttestationVerificationException(
            "Managed pre-Android-12 StrongBox profile must not claim usageCountLimit");
      }
      if (securityLevel != AttestationResult.SecurityLevel.STRONG_BOX) {
        throw new AttestationVerificationException(
            "Managed pre-Android-12 profile requires StrongBox attestation");
      }
    }
    if (software.containsKey(TAG_ALL_APPLICATIONS)
        || hardware.containsKey(TAG_ALL_APPLICATIONS)) {
      throw new AttestationVerificationException(
          "Android offline registration must not authorize all applications");
    }
    final byte[] softwareApplicationId = software.get(TAG_ATTESTATION_APPLICATION_ID);
    final byte[] hardwareApplicationId = hardware.get(TAG_ATTESTATION_APPLICATION_ID);
    if (softwareApplicationId != null && hardwareApplicationId != null) {
      throw new AttestationVerificationException(
          "Android AuthorizationLists duplicate attestationApplicationId");
    }
    final byte[] applicationId =
        softwareApplicationId != null ? softwareApplicationId : hardwareApplicationId;
    if (applicationId == null) {
      throw new AttestationVerificationException(
          "Android KeyDescription is missing attestationApplicationId");
    }
    verifyAttestationApplicationId(applicationId, binding);
    final byte[] encodedRoot = hardware.get(TAG_ROOT_OF_TRUST);
    if (encodedRoot == null) {
      throw new AttestationVerificationException(
          "Android KeyDescription is missing hardware rootOfTrust");
    }
    final RootOfTrust root = parseRootOfTrust(encodedRoot);
    final OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel projectedLevel;
    if (securityLevel == AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT) {
      projectedLevel = OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.TRUSTED_ENVIRONMENT;
    } else if (securityLevel == AttestationResult.SecurityLevel.STRONG_BOX) {
      projectedLevel = OfflineAndroidAttestedDevicePropertiesV2.SecurityLevel.STRONG_BOX;
    } else {
      throw new AttestationVerificationException(
          "Android attested-device properties are software-backed");
    }
    try {
      final OfflineAndroidAttestedDevicePropertiesV2 properties =
          new OfflineAndroidAttestedDevicePropertiesV2(
              OfflineAndroidAttestedDevicePropertiesV2.VERSION_V2,
              attestationVersion,
              keymasterVersion,
              projectedLevel,
              readAttestedProperty(hardware.get(TAG_ATTESTATION_ID_BRAND)),
              readAttestedProperty(hardware.get(TAG_ATTESTATION_ID_DEVICE)),
              readAttestedProperty(hardware.get(TAG_ATTESTATION_ID_PRODUCT)),
              readAttestedProperty(hardware.get(TAG_ATTESTATION_ID_MANUFACTURER)),
              readAttestedProperty(hardware.get(TAG_ATTESTATION_ID_MODEL)),
              readAuthorizationU32(hardware.get(TAG_OS_VERSION)),
              readAuthorizationU32(hardware.get(TAG_OS_PATCH_LEVEL)),
              readAuthorizationU32(hardware.get(TAG_VENDOR_PATCH_LEVEL)),
              readAuthorizationU32(hardware.get(TAG_BOOT_PATCH_LEVEL)),
              root.verifiedBootKey,
              root.verifiedBootHash);
      if (binding.assertionProfile == OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT) {
        if (properties.osVersion() < ANDROID_12_OS_VERSION_FLOOR) {
          throw new AttestationVerificationException(
              "Android hardware usage-limit profile requires Android 12 or newer");
        }
      } else if (properties.osVersion() >= ANDROID_12_OS_VERSION_FLOOR
          || !properties.isCompleteV2()) {
        throw new AttestationVerificationException(
            "Managed pre-Android-12 StrongBox profile requires complete pre-12 hardware properties");
      }
      return properties;
    } catch (final IllegalArgumentException error) {
      throw new AttestationVerificationException(
          "Android attested-device properties exceed canonical V2 bounds", error);
    }
  }

  private static Map<Integer, byte[]> parseAuthorizationList(final byte[] input)
      throws AttestationVerificationException {
    final ExplicitAuthorizationReader reader = new ExplicitAuthorizationReader(input);
    final Map<Integer, byte[]> fields = new LinkedHashMap<>();
    while (reader.hasRemaining()) {
      final ExplicitAuthorizationField field = reader.read();
      if (fields.put(field.tagNumber, field.value) != null) {
        throw new AttestationVerificationException(
            "Android AuthorizationList duplicates context tag " + field.tagNumber);
      }
    }
    return fields;
  }

  private static long readAuthorizationU32(final byte[] encoded)
      throws AttestationVerificationException {
    if (encoded == null) return 0;
    final long value = readAuthorizationInteger(encoded);
    return value > 0 && value <= 0xffff_ffffL ? value : 0;
  }

  private static long readAuthorizationInteger(final byte[] encoded)
      throws AttestationVerificationException {
    if (encoded == null) return 0;
    final DerReader reader = new DerReader(encoded);
    final long value = reader.readInteger64();
    if (reader.hasRemaining()) {
      throw new AttestationVerificationException(
          "Android AuthorizationList integer contains trailing data");
    }
    return value;
  }

  private static void verifyAttestationApplicationId(
      final byte[] encoded, final OfflineRegistrationBinding binding)
      throws AttestationVerificationException {
    final DerReader wrapper = new DerReader(encoded);
    final byte[] applicationIdDer = wrapper.readOctetString();
    if (wrapper.hasRemaining()) {
      throw new AttestationVerificationException(
          "Android attestationApplicationId wrapper contains trailing data");
    }
    final DerReader applicationId = DerReader.sequence(applicationIdDer);
    final DerReader packages = new DerReader(applicationId.readSetBytes());
    final DerReader signatures = new DerReader(applicationId.readSetBytes());
    if (applicationId.hasRemaining()) {
      throw new AttestationVerificationException(
          "Android attestationApplicationId contains trailing data");
    }

    int packageCount = 0;
    while (packages.hasRemaining()) {
      final DerReader info = new DerReader(packages.readSequenceBytes());
      final byte[] packageBytes = info.readOctetString();
      info.readInteger64();
      if (info.hasRemaining()) {
        throw new AttestationVerificationException(
            "Android attestation package info contains trailing data");
      }
      final String packageName = new String(packageBytes, StandardCharsets.UTF_8);
      if (!Arrays.equals(packageName.getBytes(StandardCharsets.UTF_8), packageBytes)
          || !packageName.equals(binding.packageName)) {
        throw new AttestationVerificationException(
            "Android attestation package does not match the registered application");
      }
      packageCount++;
    }
    if (packageCount != 1) {
      throw new AttestationVerificationException(
          "Android attestationApplicationId must bind exactly one package");
    }

    int signatureCount = 0;
    while (signatures.hasRemaining()) {
      final byte[] digest = signatures.readOctetString();
      if (digest.length != 32
          || !MessageDigest.isEqual(digest, binding.signingCertificateSha256)) {
        throw new AttestationVerificationException(
            "Android attestation signing digest does not match the registered application");
      }
      signatureCount++;
    }
    if (signatureCount != 1) {
      throw new AttestationVerificationException(
          "Android attestationApplicationId must bind exactly one signing digest");
    }
  }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte item : value) aggregate |= item;
    return aggregate == 0;
  }

  private static String readAttestedProperty(final byte[] encoded)
      throws AttestationVerificationException {
    if (encoded == null) return "";
    final DerReader reader = new DerReader(encoded);
    final byte[] bytes = reader.readOctetString();
    if (reader.hasRemaining()) {
      throw new AttestationVerificationException(
          "Android attestationId property contains trailing data");
    }
    final String value = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(value.getBytes(StandardCharsets.UTF_8), bytes)) {
      throw new AttestationVerificationException(
          "Android attestationId property is not valid UTF-8");
    }
    return value;
  }

  private static RootOfTrust parseRootOfTrust(final byte[] encoded)
      throws AttestationVerificationException {
    final DerReader reader = DerReader.sequence(encoded);
    final byte[] verifiedBootKey = reader.readOctetString();
    if (!reader.readCanonicalBoolean()) {
      throw new AttestationVerificationException(
          "Android rootOfTrust reports an unlocked bootloader");
    }
    if (reader.readEnumerated() != 0) {
      throw new AttestationVerificationException(
          "Android rootOfTrust is not in Verified boot state");
    }
    final byte[] verifiedBootHash = reader.readOctetString();
    if (reader.hasRemaining()
        || verifiedBootKey.length == 0
        || verifiedBootHash.length
            != OfflineAndroidAttestedDevicePropertiesV2.VERIFIED_BOOT_HASH_BYTES_V2) {
      throw new AttestationVerificationException(
          "Android rootOfTrust has invalid canonical fields");
    }
    return new RootOfTrust(verifiedBootKey, verifiedBootHash);
  }

  /**
   * Project the exact Offline V2 fields from one AndroidKeyStore leaf certificate.
   *
   * <p>This helper validates the complete KeyDescription/challenge/application/one-use shape, but
   * deliberately does not authenticate the certificate path. It exists for the physical
   * AndroidKeyStore producer, which must place the exact leaf-derived bytes beside the full chain
   * in a registration; native admission then authenticates/revocation-checks that chain and
   * byte-compares the projection. Call {@link #verifyOfflineDeviceRegistration} whenever governed
   * trust roots are available locally.
   */
  public static OfflineAndroidAttestedDevicePropertiesV2
      projectOfflineDeviceRegistrationProperties(
          final byte[] leafCertificateDer,
          final byte[] expectedChallenge,
          final String expectedPackageName,
          final byte[] expectedSigningCertificateSha256)
          throws AttestationVerificationException {
    return projectOfflineDeviceRegistrationProperties(
        leafCertificateDer,
        expectedChallenge,
        expectedPackageName,
        expectedSigningCertificateSha256,
        OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT);
  }

  /** Project an explicit Offline V2 assertion profile without authenticating the chain. */
  public static OfflineAndroidAttestedDevicePropertiesV2
      projectOfflineDeviceRegistrationProperties(
          final byte[] leafCertificateDer,
          final byte[] expectedChallenge,
          final String expectedPackageName,
          final byte[] expectedSigningCertificateSha256,
          final OfflineDeviceAssertionProfile assertionProfile)
          throws AttestationVerificationException {
    if (leafCertificateDer == null || leafCertificateDer.length == 0) {
      throw new AttestationVerificationException(
          "Offline registration leaf certificate is missing");
    }
    if (expectedChallenge == null || expectedChallenge.length != 32) {
      throw new AttestationVerificationException(
          "Offline registration challenge must contain exactly 32 bytes");
    }
    if (expectedPackageName == null
        || expectedPackageName.isEmpty()
        || !expectedPackageName.equals(expectedPackageName.trim())) {
      throw new AttestationVerificationException(
          "Offline registration package name is not canonical");
    }
    if (expectedSigningCertificateSha256 == null
        || expectedSigningCertificateSha256.length != 32
        || allZero(expectedSigningCertificateSha256)) {
      throw new AttestationVerificationException(
          "Offline registration signing-certificate digest must be a non-zero 32-byte value");
    }
    final X509Certificate leaf;
    try {
      final CertificateFactory factory = CertificateFactory.getInstance("X.509");
      leaf =
          (X509Certificate)
              factory.generateCertificate(new ByteArrayInputStream(leafCertificateDer));
    } catch (final CertificateException ex) {
      throw new AttestationVerificationException(
          "Failed to decode Offline registration leaf certificate", ex);
    }
    final KeyDescription description =
        parseKeyDescription(
            leaf,
            new OfflineRegistrationBinding(
                expectedPackageName,
                expectedSigningCertificateSha256.clone(),
                Objects.requireNonNull(assertionProfile, "assertionProfile")));
    if (!MessageDigest.isEqual(expectedChallenge, description.attestationChallenge)) {
      throw new AttestationVerificationException("Attestation challenge mismatch");
    }
    if (description.attestedDeviceProperties == null) {
      throw new AttestationVerificationException(
          "Offline V2 device-property projection is unavailable");
    }
    return description.attestedDeviceProperties;
  }

  /** Builder used to configure {@link AttestationVerifier} instances. */
  public static final class Builder {
    private final Set<X509Certificate> trustedRoots = new LinkedHashSet<>();
    private boolean requireStrongBox = false;

    private Builder() {}

    /** Adds a trusted root certificate in DER form. */
    public Builder addTrustedRoot(final byte[] certificateDer)
        throws AttestationVerificationException {
      Objects.requireNonNull(certificateDer, "certificateDer");
      try {
        final CertificateFactory factory = CertificateFactory.getInstance("X.509");
        trustedRoots.add(
            (X509Certificate) factory.generateCertificate(new ByteArrayInputStream(certificateDer)));
        return this;
      } catch (final CertificateException ex) {
        throw new AttestationVerificationException("Failed to decode trusted root certificate", ex);
      }
    }

    /** Adds a trusted root certificate. */
    public Builder addTrustedRoot(final X509Certificate certificate) {
      trustedRoots.add(Objects.requireNonNull(certificate, "certificate"));
      return this;
    }

    /** Requires StrongBox-backed attestation when {@code enabled} is {@code true}. */
    public Builder requireStrongBox(final boolean enabled) {
      this.requireStrongBox = enabled;
      return this;
    }

    public AttestationVerifier build() {
      return new AttestationVerifier(this);
    }
  }

  private static final class KeyDescription {
    private final AttestationResult.SecurityLevel attestationSecurityLevel;
    private final AttestationResult.SecurityLevel keymasterSecurityLevel;
    private final byte[] attestationChallenge;
    private final byte[] uniqueId;
    private final int softwareAuthorisationsLength;
    private final int teeAuthorisationsLength;
    private final int strongBoxAuthorisationsLength;
    private final OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties;

    private KeyDescription(
        final AttestationResult.SecurityLevel attestationSecurityLevel,
        final AttestationResult.SecurityLevel keymasterSecurityLevel,
        final byte[] attestationChallenge,
        final byte[] uniqueId,
        final int softwareAuthorisationsLength,
        final int teeAuthorisationsLength,
        final int strongBoxAuthorisationsLength,
        final OfflineAndroidAttestedDevicePropertiesV2 attestedDeviceProperties) {
      this.attestationSecurityLevel = attestationSecurityLevel;
      this.keymasterSecurityLevel = keymasterSecurityLevel;
      this.attestationChallenge = attestationChallenge == null ? new byte[0] : attestationChallenge;
      this.uniqueId = uniqueId == null ? new byte[0] : uniqueId;
      this.softwareAuthorisationsLength = softwareAuthorisationsLength;
      this.teeAuthorisationsLength = teeAuthorisationsLength;
      this.strongBoxAuthorisationsLength = strongBoxAuthorisationsLength;
      this.attestedDeviceProperties = attestedDeviceProperties;
    }
  }

  private static final class RootOfTrust {
    private final byte[] verifiedBootKey;
    private final byte[] verifiedBootHash;

    private RootOfTrust(final byte[] verifiedBootKey, final byte[] verifiedBootHash) {
      this.verifiedBootKey = verifiedBootKey;
      this.verifiedBootHash = verifiedBootHash;
    }
  }

  private static final class OfflineRegistrationBinding {
    private final String packageName;
    private final byte[] signingCertificateSha256;
    private final OfflineDeviceAssertionProfile assertionProfile;

    private OfflineRegistrationBinding(
        final String packageName,
        final byte[] signingCertificateSha256,
        final OfflineDeviceAssertionProfile assertionProfile) {
      this.packageName = packageName;
      this.signingCertificateSha256 = signingCertificateSha256;
      this.assertionProfile = assertionProfile;
    }
  }

  private static final class DerReader {
    private static final int TAG_SEQUENCE = 0x30;
    private static final int TAG_BOOLEAN = 0x01;
    private static final int TAG_INTEGER = 0x02;
    private static final int TAG_ENUMERATED = 0x0A;
    private static final int TAG_OCTET_STRING = 0x04;
    private static final int TAG_SET = 0x31;

    private final byte[] buffer;
    private int offset = 0;

    static DerReader sequence(final byte[] data) throws AttestationVerificationException {
      final DerReader reader = new DerReader(data);
      return new DerReader(reader.readWithExpectedTag(TAG_SEQUENCE));
    }

    DerReader(final byte[] buffer) {
      this.buffer = Objects.requireNonNull(buffer, "buffer");
    }

    boolean hasRemaining() {
      return offset < buffer.length;
    }

    int readEnumerated() throws AttestationVerificationException {
      return readIntegerWithTag(TAG_ENUMERATED);
    }

    byte[] readOctetString() throws AttestationVerificationException {
      return readWithExpectedTag(TAG_OCTET_STRING);
    }

    byte[] readSequenceBytes() throws AttestationVerificationException {
      return readWithExpectedTag(TAG_SEQUENCE);
    }

    boolean readCanonicalBoolean() throws AttestationVerificationException {
      final byte[] value = readWithExpectedTag(TAG_BOOLEAN);
      if (value.length != 1 || (value[0] != 0 && value[0] != (byte) 0xff)) {
        throw new AttestationVerificationException("Invalid canonical DER boolean");
      }
      return value[0] == (byte) 0xff;
    }

    long readInteger64() throws AttestationVerificationException {
      final BigInteger integer = new BigInteger(canonicalIntegerBytes(
          readWithExpectedTag(TAG_INTEGER)));
      if (integer.compareTo(LONG_MIN) < 0 || integer.compareTo(LONG_MAX) > 0) {
        throw new AttestationVerificationException("Integer value out of range");
      }
      return integer.longValue();
    }

    byte[] readSetBytes() throws AttestationVerificationException {
      return readWithExpectedTag(TAG_SET);
    }

    private int readIntegerWithTag(final int expectedTag) throws AttestationVerificationException {
      final byte[] value = canonicalIntegerBytes(readWithExpectedTag(expectedTag));
      try {
        return new BigInteger(value).intValueExact();
      } catch (final ArithmeticException ex) {
        throw new AttestationVerificationException("Integer value out of range", ex);
      }
    }

    private static byte[] canonicalIntegerBytes(final byte[] value)
        throws AttestationVerificationException {
      if (value.length == 0) {
        throw new AttestationVerificationException("DER integer must not be empty");
      }
      if (value.length > 1) {
        final int first = value[0] & 0xff;
        final int second = value[1] & 0xff;
        if ((first == 0 && (second & 0x80) == 0)
            || (first == 0xff && (second & 0x80) != 0)) {
          throw new AttestationVerificationException(
              "DER integer is not minimally encoded");
        }
      }
      return value;
    }

    private byte[] readWithExpectedTag(final int expectedTag)
        throws AttestationVerificationException {
      final int tag = readTag();
      if (tag != expectedTag) {
        throw new AttestationVerificationException(
            String.format("Unexpected DER tag. expected=0x%02X actual=0x%02X", expectedTag, tag));
      }
      final int length = readLength();
      if (length < 0) {
        throw new AttestationVerificationException("Invalid DER length");
      }
      if (length > buffer.length - offset) {
        throw new AttestationVerificationException("DER value overruns buffer");
      }
      final byte[] value = Arrays.copyOfRange(buffer, offset, offset + length);
      offset += length;
      return value;
    }

    private int readTag() throws AttestationVerificationException {
      if (offset >= buffer.length) {
        throw new AttestationVerificationException("Unexpected end of DER input");
      }
      return buffer[offset++] & 0xFF;
    }

    private int readLength() throws AttestationVerificationException {
      if (offset >= buffer.length) {
        throw new AttestationVerificationException("Unexpected end of DER input");
      }
      final int lengthByte = buffer[offset++] & 0xFF;
      if ((lengthByte & 0x80) == 0) {
        return lengthByte;
      }
      final int lengthOctets = lengthByte & 0x7F;
      if (lengthOctets == 0 || lengthOctets > 4) {
        throw new AttestationVerificationException("Unsupported DER length encoding");
      }
      if (offset >= buffer.length || buffer[offset] == 0) {
        throw new AttestationVerificationException(
            "Non-canonical DER length encoding");
      }
      int length = 0;
      for (int i = 0; i < lengthOctets; i++) {
        if (offset >= buffer.length) {
          throw new AttestationVerificationException("Invalid DER length encoding");
        }
        length = (length << 8) | (buffer[offset++] & 0xFF);
      }
      if (length < 128) {
        throw new AttestationVerificationException(
            "Non-minimal DER length encoding");
      }
      return length;
    }
  }

  private static final class ExplicitAuthorizationField {
    private final int tagNumber;
    private final byte[] value;

    private ExplicitAuthorizationField(final int tagNumber, final byte[] value) {
      this.tagNumber = tagNumber;
      this.value = value;
    }
  }

  /** Strict DER reader for explicit high-number context tags in AuthorizationList. */
  private static final class ExplicitAuthorizationReader {
    private final byte[] bytes;
    private int offset;

    private ExplicitAuthorizationReader(final byte[] bytes) {
      this.bytes = Objects.requireNonNull(bytes, "bytes");
    }

    private boolean hasRemaining() {
      return offset < bytes.length;
    }

    private ExplicitAuthorizationField read() throws AttestationVerificationException {
      final int first = readByte();
      if ((first & 0xc0) != 0x80 || (first & 0x20) == 0) {
        throw new AttestationVerificationException(
            "Android AuthorizationList contains a non-explicit context tag");
      }
      int number = first & 0x1f;
      if (number == 0x1f) {
        number = 0;
        int count = 0;
        while (true) {
          final int octet = readByte();
          count++;
          if (count > 5 || (count == 1 && octet == 0x80)) {
            throw new AttestationVerificationException(
                "Android AuthorizationList has a noncanonical high tag");
          }
          if (number > (Integer.MAX_VALUE >>> 7)) {
            throw new AttestationVerificationException(
                "Android AuthorizationList tag number overflows");
          }
          number = (number << 7) | (octet & 0x7f);
          if ((octet & 0x80) == 0) break;
        }
        if (number < 31) {
          throw new AttestationVerificationException(
              "Android AuthorizationList high tag is not minimal");
        }
      }
      final int length = readLength();
      if (length > bytes.length - offset) {
        throw new AttestationVerificationException(
            "Android AuthorizationList value overruns its DER input");
      }
      final byte[] value = Arrays.copyOfRange(bytes, offset, offset + length);
      offset += length;
      return new ExplicitAuthorizationField(number, value);
    }

    private int readLength() throws AttestationVerificationException {
      final int first = readByte();
      if ((first & 0x80) == 0) return first;
      final int count = first & 0x7f;
      if (count == 0 || count > 4) {
        throw new AttestationVerificationException(
            "Android AuthorizationList has an unsupported DER length");
      }
      if (offset >= bytes.length || bytes[offset] == 0) {
        throw new AttestationVerificationException(
            "Android AuthorizationList DER length is not canonical");
      }
      long length = 0;
      for (int index = 0; index < count; index++) {
        length = (length << 8) | readByte();
      }
      if (length < 128 || length > Integer.MAX_VALUE) {
        throw new AttestationVerificationException(
            "Android AuthorizationList DER length is outside bounds");
      }
      return (int) length;
    }

    private int readByte() throws AttestationVerificationException {
      if (offset >= bytes.length) {
        throw new AttestationVerificationException(
            "Unexpected end of Android AuthorizationList DER");
      }
      return bytes[offset++] & 0xff;
    }
  }
}
