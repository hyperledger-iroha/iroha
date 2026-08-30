package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.io.ByteArrayInputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.security.cert.CertPathValidator
import java.security.cert.CertPathValidatorException
import java.security.cert.CertificateException
import java.security.cert.CertificateFactory
import java.security.cert.PKIXParameters
import java.security.cert.TrustAnchor
import java.security.cert.X509Certificate
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation
import org.hyperledger.iroha.sdk.offline.OfflineAndroidAttestedDevicePropertiesV2
import org.hyperledger.iroha.sdk.offline.OfflineAndroidDeviceSecurityLevelV2

private const val ATTESTATION_OID = "1.3.6.1.4.1.11129.2.1.17"
private val INT_MIN_BIG_INTEGER = BigInteger.valueOf(Int.MIN_VALUE.toLong())
private val INT_MAX_BIG_INTEGER = BigInteger.valueOf(Int.MAX_VALUE.toLong())
private val LONG_MIN_BIG_INTEGER = BigInteger.valueOf(Long.MIN_VALUE)
private val LONG_MAX_BIG_INTEGER = BigInteger.valueOf(Long.MAX_VALUE)
private const val TAG_USAGE_COUNT_LIMIT = 405
private const val TAG_ALL_APPLICATIONS = 600
private const val TAG_ROOT_OF_TRUST = 704
private const val TAG_OS_VERSION = 705
private const val TAG_OS_PATCH_LEVEL = 706
private const val TAG_ATTESTATION_APPLICATION_ID = 709
private const val TAG_ATTESTATION_ID_BRAND = 710
private const val TAG_ATTESTATION_ID_DEVICE = 711
private const val TAG_ATTESTATION_ID_PRODUCT = 712
private const val TAG_ATTESTATION_ID_MANUFACTURER = 716
private const val TAG_ATTESTATION_ID_MODEL = 717
private const val TAG_VENDOR_PATCH_LEVEL = 718
private const val TAG_BOOT_PATCH_LEVEL = 719
private const val ANDROID_12_OS_VERSION_FLOOR = 120_000L
private val DEVICE_PROPERTY_TAGS = setOf(
    TAG_ROOT_OF_TRUST,
    TAG_OS_VERSION,
    TAG_OS_PATCH_LEVEL,
    TAG_ATTESTATION_ID_BRAND,
    TAG_ATTESTATION_ID_DEVICE,
    TAG_ATTESTATION_ID_PRODUCT,
    TAG_ATTESTATION_ID_MANUFACTURER,
    TAG_ATTESTATION_ID_MODEL,
    TAG_VENDOR_PATCH_LEVEL,
    TAG_BOOT_PATCH_LEVEL,
)

/**
 * Validates Android key attestation certificate chains and extracts metadata required by higher
 * level policy checks.
 */
class AttestationVerifier private constructor(
    trustAnchors: Set<TrustAnchor>,
    private val requireStrongBox: Boolean,
) {
    /** Closed Android assertion profiles accepted by Offline Device Attestation V2. */
    enum class OfflineDeviceAssertionProfile {
        /** Android 12+ KeyMint key with hardware-enforced tag 405 equal to one. */
        HARDWARE_USAGE_LIMIT,

        /** Managed API 28--30 StrongBox key consumed receipt-first and deleted after signing. */
        MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST,
    }

    private val trustAnchors: Set<TrustAnchor> = trustAnchors.toSet()

    /** Validates `attestation` against the configured policy. */
    @Throws(AttestationVerificationException::class)
    fun verify(attestation: KeyAttestation): AttestationResult = verify(attestation, null)

    /**
     * Validates `attestation` and checks that the embedded challenge matches `expectedChallenge`
     * when provided.
     */
    @Throws(AttestationVerificationException::class)
    fun verify(attestation: KeyAttestation, expectedChallenge: ByteArray?): AttestationResult {
        return verify(
            attestation = attestation,
            expectedChallenge = expectedChallenge,
            offlineRegistrationBinding = null,
        )
    }

    /**
     * Validate the complete Offline Device Attestation V2 KeyDescription profile.
     *
     * In addition to certificate-path and challenge validation, this requires hardware-enforced
     * `usageCountLimit = 1`, rejects `allApplications`, authenticates the exact package/signing
     * identity from tag 709, and projects the exact device properties that native admission later
     * byte-compares with the submitted registration.
     */
    @Throws(AttestationVerificationException::class)
    fun verifyOfflineDeviceRegistration(
        attestation: KeyAttestation,
        expectedChallenge: ByteArray,
        expectedPackageName: String,
        expectedSigningCertificateSha256: ByteArray,
    ): AttestationResult = verifyOfflineDeviceRegistration(
        attestation,
        expectedChallenge,
        expectedPackageName,
        expectedSigningCertificateSha256,
        OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT,
    )

    /** Validate one explicit, consensus-identical Android assertion profile. */
    @Throws(AttestationVerificationException::class)
    fun verifyOfflineDeviceRegistration(
        attestation: KeyAttestation,
        expectedChallenge: ByteArray,
        expectedPackageName: String,
        expectedSigningCertificateSha256: ByteArray,
        assertionProfile: OfflineDeviceAssertionProfile,
    ): AttestationResult {
        if (expectedChallenge.size != 32) {
            throw AttestationVerificationException(
                "Offline registration challenge must contain exactly 32 bytes",
            )
        }
        if (
            expectedPackageName.isEmpty() ||
            expectedPackageName != expectedPackageName.trim()
        ) {
            throw AttestationVerificationException(
                "Offline registration package name is not canonical",
            )
        }
        if (
            expectedSigningCertificateSha256.size != 32 ||
            expectedSigningCertificateSha256.all { it == 0.toByte() }
        ) {
            throw AttestationVerificationException(
                "Offline registration signing-certificate digest must be a non-zero 32-byte value",
            )
        }
        return verify(
            attestation = attestation,
            expectedChallenge = expectedChallenge,
            offlineRegistrationBinding = OfflineRegistrationBinding(
                packageName = expectedPackageName,
                signingCertificateSha256 = expectedSigningCertificateSha256.copyOf(),
                assertionProfile = assertionProfile,
            ),
        )
    }

    private fun verify(
        attestation: KeyAttestation,
        expectedChallenge: ByteArray?,
        offlineRegistrationBinding: OfflineRegistrationBinding?,
    ): AttestationResult {
        val chain = decodeChain(attestation)
        if (chain.isEmpty()) {
            throw AttestationVerificationException("Attestation certificate chain is empty")
        }
        val leaf = chain[0]

        validateCertificatePath(chain)

        val description = parseKeyDescription(leaf, offlineRegistrationBinding)
        if (expectedChallenge != null
            && !MessageDigest.isEqual(expectedChallenge, description.attestationChallenge)
        ) {
            throw AttestationVerificationException("Attestation challenge mismatch")
        }
        if (requireStrongBox
            && description.attestationSecurityLevel != AttestationResult.SecurityLevel.STRONG_BOX
        ) {
            throw AttestationVerificationException("StrongBox attestation required by policy")
        }

        return AttestationResult(
            alias = attestation.alias,
            certificateChain = chain,
            attestationSecurityLevel = description.attestationSecurityLevel,
            keymasterSecurityLevel = description.keymasterSecurityLevel,
            attestationChallenge = description.attestationChallenge,
            uniqueId = description.uniqueId,
            softwareAuthorisationsPresent = description.softwareAuthorisationsLength > 0,
            teeAuthorisationsPresent = description.teeAuthorisationsLength > 0,
            strongBoxAuthorisationsPresent = description.strongBoxAuthorisationsLength > 0,
            attestedDeviceProperties = description.attestedDeviceProperties,
        )
    }

    private fun decodeChain(attestation: KeyAttestation): List<X509Certificate> {
        val factory: CertificateFactory
        try {
            factory = CertificateFactory.getInstance("X.509")
        } catch (ex: CertificateException) {
            throw AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex)
        }

        return attestation.certificateChain().map { certificateDer ->
            try {
                factory.generateCertificate(ByteArrayInputStream(certificateDer)) as X509Certificate
            } catch (ex: CertificateException) {
                throw AttestationVerificationException("Failed to decode attestation certificate", ex)
            }
        }
    }

    private fun validateCertificatePath(chain: List<X509Certificate>) {
        val factory: CertificateFactory
        try {
            factory = CertificateFactory.getInstance("X.509")
        } catch (ex: CertificateException) {
            throw AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex)
        }

        val certPath = try {
            factory.generateCertPath(certificatesForPath(chain))
        } catch (ex: CertificateException) {
            throw AttestationVerificationException("Failed to construct attestation CertPath", ex)
        }

        val validator = try {
            CertPathValidator.getInstance("PKIX")
        } catch (ex: Exception) {
            throw AttestationVerificationException("Unable to acquire PKIX CertPathValidator", ex)
        }

        val parameters = try {
            PKIXParameters(trustAnchors)
        } catch (ex: Exception) {
            throw AttestationVerificationException("Invalid PKIX parameters", ex)
        }
        parameters.isRevocationEnabled = false

        try {
            validator.validate(certPath, parameters)
        } catch (ex: CertPathValidatorException) {
            throw AttestationVerificationException(
                "Attestation certificate path validation failed", ex
            )
        } catch (ex: Exception) {
            throw AttestationVerificationException(
                "Unexpected failure validating attestation certificate path", ex
            )
        }
    }

    private fun certificatesForPath(chain: List<X509Certificate>): List<X509Certificate> {
        if (chain.size < 2) {
            return chain
        }
        val trailingCertificate = chain[chain.size - 1]
        for (anchor in trustAnchors) {
            val trusted = anchor.trustedCert
            if (trusted != null && sameTrustAnchorCertificate(trailingCertificate, trusted)) {
                // The configured trust anchor is not part of the PKIX CertPath. Android
                // attestation exports often include it as the final chain entry.
                return chain.dropLast(1)
            }
        }
        return chain
    }

    private fun sameTrustAnchorCertificate(
        certificate: X509Certificate,
        trusted: X509Certificate,
    ): Boolean =
        certificate.subjectX500Principal == trusted.subjectX500Principal &&
            certificate.publicKey == trusted.publicKey

    private fun parseKeyDescription(
        leaf: X509Certificate,
        offlineRegistrationBinding: OfflineRegistrationBinding?,
    ): KeyDescription {
        val extension = leaf.getExtensionValue(ATTESTATION_OID)
            ?: throw AttestationVerificationException(
                "Leaf certificate does not contain Android attestation extension"
            )

        val outer = DerReader(extension)
        val octetString = outer.readOctetString()
        if (outer.hasRemaining()) {
            throw AttestationVerificationException("Unexpected data after attestation extension")
        }

        val reader = DerReader.sequence(octetString)
        val attestationVersion = reader.readInteger64()
        if (attestationVersion !in 1..0xffff_ffffL) {
            throw AttestationVerificationException(
                "Invalid attestation version: $attestationVersion"
            )
        }

        val attestationLevel =
            AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated())
        val keymasterVersion = reader.readInteger64()
        if (keymasterVersion !in 1..0xffff_ffffL) {
            throw AttestationVerificationException(
                "Invalid keymaster version: $keymasterVersion"
            )
        }
        val keymasterLevel =
            AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated())
        val challenge = reader.readOctetString()
        val uniqueId = reader.readOctetString()
        val softwareEnforced = reader.readSequenceBytes()
        val teeEnforced = reader.readSequenceBytes()
        if (reader.hasRemaining()) {
            throw AttestationVerificationException("Unexpected trailing data in attestation")
        }

        val properties = offlineRegistrationBinding?.let { binding ->
            if (
                attestationLevel != keymasterLevel ||
                attestationLevel == AttestationResult.SecurityLevel.SOFTWARE
            ) {
                throw AttestationVerificationException(
                    "Attestation and KeyMint security levels must name the same hardware boundary",
                )
            }
            parseOfflineDeviceRegistration(
                attestationVersion = attestationVersion,
                keymasterVersion = keymasterVersion,
                securityLevel = attestationLevel,
                softwareEnforced = softwareEnforced,
                hardwareEnforced = teeEnforced,
                binding = binding,
            )
        }

        return KeyDescription(
            attestationSecurityLevel = attestationLevel,
            keymasterSecurityLevel = keymasterLevel,
            attestationChallenge = challenge,
            uniqueId = uniqueId,
            softwareAuthorisationsLength = softwareEnforced.size,
            teeAuthorisationsLength = teeEnforced.size,
            strongBoxAuthorisationsLength = if (
                attestationLevel == AttestationResult.SecurityLevel.STRONG_BOX
            ) teeEnforced.size else 0,
            attestedDeviceProperties = properties,
        )
    }

    private fun parseOfflineDeviceRegistration(
        attestationVersion: Long,
        keymasterVersion: Long,
        securityLevel: AttestationResult.SecurityLevel,
        softwareEnforced: ByteArray,
        hardwareEnforced: ByteArray,
        binding: OfflineRegistrationBinding,
    ): OfflineAndroidAttestedDevicePropertiesV2 {
        val software = parseAuthorizationList(softwareEnforced)
        val misplaced = software.keys.intersect(DEVICE_PROPERTY_TAGS)
        if (misplaced.isNotEmpty()) {
            throw AttestationVerificationException(
                "Android attested-device properties must be hardwareEnforced: " +
                    misplaced.sorted().joinToString(","),
            )
        }
        val hardware = parseAuthorizationList(hardwareEnforced)
        if (software.containsKey(TAG_USAGE_COUNT_LIMIT)) {
            throw AttestationVerificationException(
                "Android usageCountLimit must be hardwareEnforced",
            )
        }
        when (binding.assertionProfile) {
            OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT -> {
                if (readAuthorizationInteger(hardware[TAG_USAGE_COUNT_LIMIT]) != 1L) {
                    throw AttestationVerificationException(
                        "Android hardware usageCountLimit must be exactly one",
                    )
                }
            }
            OfflineDeviceAssertionProfile
                .MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST -> {
                if (hardware.containsKey(TAG_USAGE_COUNT_LIMIT)) {
                    throw AttestationVerificationException(
                        "Managed pre-Android-12 StrongBox profile must not claim usageCountLimit",
                    )
                }
                if (securityLevel != AttestationResult.SecurityLevel.STRONG_BOX) {
                    throw AttestationVerificationException(
                        "Managed pre-Android-12 profile requires StrongBox attestation",
                    )
                }
            }
        }
        if (
            software.containsKey(TAG_ALL_APPLICATIONS) ||
            hardware.containsKey(TAG_ALL_APPLICATIONS)
        ) {
            throw AttestationVerificationException(
                "Android offline registration must not authorize all applications",
            )
        }
        val softwareApplicationId = software[TAG_ATTESTATION_APPLICATION_ID]
        val hardwareApplicationId = hardware[TAG_ATTESTATION_APPLICATION_ID]
        if (softwareApplicationId != null && hardwareApplicationId != null) {
            throw AttestationVerificationException(
                "Android AuthorizationLists duplicate attestationApplicationId",
            )
        }
        verifyAttestationApplicationId(
            encoded = softwareApplicationId ?: hardwareApplicationId
                ?: throw AttestationVerificationException(
                    "Android KeyDescription is missing attestationApplicationId",
                ),
            binding = binding,
        )
        val root = parseRootOfTrust(
            hardware[TAG_ROOT_OF_TRUST]
                ?: throw AttestationVerificationException(
                    "Android KeyDescription is missing hardware rootOfTrust",
                ),
        )
        val projectedSecurityLevel = when (securityLevel) {
            AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT ->
                OfflineAndroidDeviceSecurityLevelV2.TRUSTED_ENVIRONMENT
            AttestationResult.SecurityLevel.STRONG_BOX ->
                OfflineAndroidDeviceSecurityLevelV2.STRONG_BOX
            AttestationResult.SecurityLevel.SOFTWARE ->
                throw AttestationVerificationException(
                    "Android attested-device properties are software-backed",
                )
        }
        val properties = try {
            OfflineAndroidAttestedDevicePropertiesV2(
                version = OfflineAndroidAttestedDevicePropertiesV2.VERSION_V2,
                attestationVersion = attestationVersion,
                keymintVersion = keymasterVersion,
                securityLevel = projectedSecurityLevel,
                brand = readAttestedProperty(hardware[TAG_ATTESTATION_ID_BRAND]),
                device = readAttestedProperty(hardware[TAG_ATTESTATION_ID_DEVICE]),
                product = readAttestedProperty(hardware[TAG_ATTESTATION_ID_PRODUCT]),
                manufacturer = readAttestedProperty(
                    hardware[TAG_ATTESTATION_ID_MANUFACTURER],
                ),
                model = readAttestedProperty(hardware[TAG_ATTESTATION_ID_MODEL]),
                osVersion = readAuthorizationU32(hardware[TAG_OS_VERSION]),
                osPatchLevel = readAuthorizationU32(hardware[TAG_OS_PATCH_LEVEL]),
                vendorPatchLevel = readAuthorizationU32(hardware[TAG_VENDOR_PATCH_LEVEL]),
                bootPatchLevel = readAuthorizationU32(hardware[TAG_BOOT_PATCH_LEVEL]),
                verifiedBootKey = root.first,
                verifiedBootHash = root.second,
            )
        } catch (error: IllegalArgumentException) {
            throw AttestationVerificationException(
                "Android attested-device properties exceed canonical V2 bounds",
                error,
            )
        }
        when (binding.assertionProfile) {
            OfflineDeviceAssertionProfile.HARDWARE_USAGE_LIMIT -> {
                if (properties.osVersion < ANDROID_12_OS_VERSION_FLOOR) {
                    throw AttestationVerificationException(
                        "Android hardware usage-limit profile requires Android 12 or newer",
                    )
                }
            }
            OfflineDeviceAssertionProfile
                .MANAGED_PRE_ANDROID_12_STRONGBOX_RECEIPT_FIRST -> {
                if (
                    properties.osVersion >= ANDROID_12_OS_VERSION_FLOOR ||
                    !properties.isCompleteV2()
                ) {
                    throw AttestationVerificationException(
                        "Managed pre-Android-12 StrongBox profile requires complete pre-12 hardware properties",
                    )
                }
            }
        }
        return properties
    }

    private fun parseAuthorizationList(input: ByteArray): Map<Int, ByteArray> {
        val reader = ExplicitAuthorizationReader(input)
        val fields = linkedMapOf<Int, ByteArray>()
        while (reader.hasRemaining()) {
            val field = reader.read()
            if (fields.put(field.tagNumber, field.value) != null) {
                throw AttestationVerificationException(
                    "Android AuthorizationList duplicates context tag ${field.tagNumber}",
                )
            }
        }
        return fields
    }

    private fun readAuthorizationU32(encoded: ByteArray?): Long {
        if (encoded == null) return 0
        val value = readAuthorizationInteger(encoded)
        return if (value in 1..0xffff_ffffL) value else 0
    }

    private fun readAuthorizationInteger(encoded: ByteArray?): Long {
        if (encoded == null) return 0
        val reader = DerReader(encoded)
        val value = reader.readInteger64()
        if (reader.hasRemaining()) {
            throw AttestationVerificationException(
                "Android AuthorizationList integer contains trailing data",
            )
        }
        return value
    }

    private fun verifyAttestationApplicationId(
        encoded: ByteArray,
        binding: OfflineRegistrationBinding,
    ) {
        val wrapper = DerReader(encoded)
        val applicationIdDer = wrapper.readOctetString()
        if (wrapper.hasRemaining()) {
            throw AttestationVerificationException(
                "Android attestationApplicationId wrapper contains trailing data",
            )
        }
        val applicationId = DerReader.sequence(applicationIdDer)
        val packages = DerReader(applicationId.readSetBytes())
        val signatures = DerReader(applicationId.readSetBytes())
        if (applicationId.hasRemaining()) {
            throw AttestationVerificationException(
                "Android attestationApplicationId contains trailing data",
            )
        }

        var packageCount = 0
        while (packages.hasRemaining()) {
            val info = DerReader(packages.readSequenceBytes())
            val packageBytes = info.readOctetString()
            info.readInteger64()
            if (info.hasRemaining()) {
                throw AttestationVerificationException(
                    "Android attestation package info contains trailing data",
                )
            }
            val packageName = packageBytes.toString(Charsets.UTF_8)
            if (
                !packageName.toByteArray(Charsets.UTF_8).contentEquals(packageBytes) ||
                packageName != binding.packageName
            ) {
                throw AttestationVerificationException(
                    "Android attestation package does not match the registered application",
                )
            }
            packageCount += 1
        }
        if (packageCount != 1) {
            throw AttestationVerificationException(
                "Android attestationApplicationId must bind exactly one package",
            )
        }

        var signatureCount = 0
        while (signatures.hasRemaining()) {
            val digest = signatures.readOctetString()
            if (
                digest.size != 32 ||
                !MessageDigest.isEqual(digest, binding.signingCertificateSha256)
            ) {
                throw AttestationVerificationException(
                    "Android attestation signing digest does not match the registered application",
                )
            }
            signatureCount += 1
        }
        if (signatureCount != 1) {
            throw AttestationVerificationException(
                "Android attestationApplicationId must bind exactly one signing digest",
            )
        }
    }

    private fun readAttestedProperty(encoded: ByteArray?): String {
        if (encoded == null) return ""
        val reader = DerReader(encoded)
        val bytes = reader.readOctetString()
        if (reader.hasRemaining()) {
            throw AttestationVerificationException(
                "Android attestationId property contains trailing data",
            )
        }
        val value = bytes.toString(Charsets.UTF_8)
        if (!value.toByteArray(Charsets.UTF_8).contentEquals(bytes)) {
            throw AttestationVerificationException(
                "Android attestationId property is not valid UTF-8",
            )
        }
        return value
    }

    private fun parseRootOfTrust(encoded: ByteArray): Pair<ByteArray, ByteArray> {
        val reader = DerReader.sequence(encoded)
        val verifiedBootKey = reader.readOctetString()
        if (!reader.readCanonicalBoolean()) {
            throw AttestationVerificationException(
                "Android rootOfTrust reports an unlocked bootloader",
            )
        }
        if (reader.readEnumerated() != 0) {
            throw AttestationVerificationException(
                "Android rootOfTrust is not in Verified boot state",
            )
        }
        val verifiedBootHash = reader.readOctetString()
        if (
            reader.hasRemaining() || verifiedBootKey.isEmpty() ||
            verifiedBootHash.size != OfflineAndroidAttestedDevicePropertiesV2
                .VERIFIED_BOOT_HASH_BYTES_V2
        ) {
            throw AttestationVerificationException(
                "Android rootOfTrust has invalid canonical fields",
            )
        }
        return verifiedBootKey to verifiedBootHash
    }

    /** Builder used to configure `AttestationVerifier` instances. */
    class Builder internal constructor() {
        private val trustedRoots = linkedSetOf<X509Certificate>()
        private var requireStrongBox = false

        /** Adds a trusted root certificate in DER form. */
        @Throws(AttestationVerificationException::class)
        fun addTrustedRoot(certificateDer: ByteArray): Builder = apply {
            try {
                val factory = CertificateFactory.getInstance("X.509")
                trustedRoots.add(
                    factory.generateCertificate(ByteArrayInputStream(certificateDer)) as X509Certificate
                )
            } catch (ex: CertificateException) {
                throw AttestationVerificationException("Failed to decode trusted root certificate", ex)
            }
        }

        /** Adds a trusted root certificate. */
        fun addTrustedRoot(certificate: X509Certificate): Builder = apply {
            trustedRoots.add(certificate)
        }

        /** Requires StrongBox-backed attestation when `enabled` is `true`. */
        fun requireStrongBox(enabled: Boolean): Builder = apply {
            this.requireStrongBox = enabled
        }

        fun build(): AttestationVerifier {
            check(trustedRoots.isNotEmpty()) {
                "At least one trusted root certificate is required"
            }
            val anchors = trustedRoots.mapTo(linkedSetOf()) { TrustAnchor(it, null) }
            return AttestationVerifier(anchors, requireStrongBox)
        }
    }

    private class KeyDescription(
        val attestationSecurityLevel: AttestationResult.SecurityLevel,
        val keymasterSecurityLevel: AttestationResult.SecurityLevel,
        val attestationChallenge: ByteArray,
        val uniqueId: ByteArray,
        val softwareAuthorisationsLength: Int,
        val teeAuthorisationsLength: Int,
        val strongBoxAuthorisationsLength: Int,
        val attestedDeviceProperties: OfflineAndroidAttestedDevicePropertiesV2?,
    )

    private class DerReader(private val buffer: ByteArray) {
        private var offset = 0

        fun hasRemaining(): Boolean = offset < buffer.size

        fun readEnumerated(): Int = readIntegerWithTag(TAG_ENUMERATED)

        fun readOctetString(): ByteArray = readWithExpectedTag(TAG_OCTET_STRING)

        fun readSequenceBytes(): ByteArray = readWithExpectedTag(TAG_SEQUENCE)

        fun readCanonicalBoolean(): Boolean {
            val value = readWithExpectedTag(TAG_BOOLEAN)
            if (value.size != 1 || (value[0] != 0.toByte() && value[0] != 0xff.toByte())) {
                throw AttestationVerificationException("Invalid canonical DER boolean")
            }
            return value[0] == 0xff.toByte()
        }

        fun readInteger64(): Long {
            val value = canonicalIntegerBytes(readWithExpectedTag(TAG_INTEGER))
            val integer = BigInteger(value)
            if (integer < LONG_MIN_BIG_INTEGER || integer > LONG_MAX_BIG_INTEGER) {
                throw AttestationVerificationException("Integer value out of range")
            }
            return integer.toLong()
        }

        fun readSetBytes(): ByteArray = readWithExpectedTag(TAG_SET)

        private fun readIntegerWithTag(expectedTag: Int): Int {
            val value = canonicalIntegerBytes(readWithExpectedTag(expectedTag))
            val integer = BigInteger(value)
            if (integer < INT_MIN_BIG_INTEGER || integer > INT_MAX_BIG_INTEGER) {
                throw AttestationVerificationException("Integer value out of range")
            }
            return integer.toInt()
        }

        private fun canonicalIntegerBytes(value: ByteArray): ByteArray {
            if (value.isEmpty()) {
                throw AttestationVerificationException("DER integer must not be empty")
            }
            if (value.size > 1) {
                val first = value[0].toInt() and 0xff
                val second = value[1].toInt() and 0xff
                if ((first == 0 && second and 0x80 == 0) ||
                    (first == 0xff && second and 0x80 != 0)
                ) {
                    throw AttestationVerificationException("DER integer is not minimally encoded")
                }
            }
            return value
        }

        private fun readWithExpectedTag(expectedTag: Int): ByteArray {
            val tag = readTag()
            if (tag != expectedTag) {
                throw AttestationVerificationException(
                    "Unexpected DER tag. expected=0x%02X actual=0x%02X".format(expectedTag, tag)
                )
            }
            val length = readLength()
            if (length < 0) {
                throw AttestationVerificationException("Invalid DER length")
            }
            if (length > buffer.size - offset) {
                throw AttestationVerificationException("DER value overruns buffer")
            }
            val value = buffer.copyOfRange(offset, offset + length)
            offset += length
            return value
        }

        private fun readTag(): Int {
            if (offset >= buffer.size) {
                throw AttestationVerificationException("Unexpected end of DER input")
            }
            return buffer[offset++].toInt() and 0xFF
        }

        private fun readLength(): Int {
            if (offset >= buffer.size) {
                throw AttestationVerificationException("Unexpected end of DER input")
            }
            val lengthByte = buffer[offset++].toInt() and 0xFF
            if (lengthByte and 0x80 == 0) return lengthByte
            val lengthOctets = lengthByte and 0x7F
            if (lengthOctets == 0 || lengthOctets > 4) {
                throw AttestationVerificationException("Unsupported DER length encoding")
            }
            if (offset >= buffer.size || buffer[offset] == 0.toByte()) {
                throw AttestationVerificationException("Non-canonical DER length encoding")
            }
            var length = 0
            for (i in 0 until lengthOctets) {
                if (offset >= buffer.size) {
                    throw AttestationVerificationException("Invalid DER length encoding")
                }
                length = (length shl 8) or (buffer[offset++].toInt() and 0xFF)
            }
            if (length < 128) {
                throw AttestationVerificationException("Non-minimal DER length encoding")
            }
            return length
        }

        companion object {
            private const val TAG_SEQUENCE = 0x30
            private const val TAG_BOOLEAN = 0x01
            private const val TAG_INTEGER = 0x02
            private const val TAG_ENUMERATED = 0x0A
            private const val TAG_OCTET_STRING = 0x04
            private const val TAG_SET = 0x31

            fun sequence(data: ByteArray): DerReader {
                val reader = DerReader(data)
                return DerReader(reader.readWithExpectedTag(TAG_SEQUENCE))
            }
        }
    }

    private data class ExplicitAuthorizationField(
        val tagNumber: Int,
        val value: ByteArray,
    )

    private data class OfflineRegistrationBinding(
        val packageName: String,
        val signingCertificateSha256: ByteArray,
        val assertionProfile: OfflineDeviceAssertionProfile,
    )

    /** Strict DER reader for the explicit high-number context tags in AuthorizationList. */
    private class ExplicitAuthorizationReader(private val bytes: ByteArray) {
        private var offset = 0

        fun hasRemaining(): Boolean = offset < bytes.size

        fun read(): ExplicitAuthorizationField {
            val first = readByte()
            if (first and 0xc0 != 0x80 || first and 0x20 == 0) {
                throw AttestationVerificationException(
                    "Android AuthorizationList contains a non-explicit context tag",
                )
            }
            var number = first and 0x1f
            if (number == 0x1f) {
                number = 0
                var count = 0
                while (true) {
                    val octet = readByte()
                    count += 1
                    if (count > 5 || (count == 1 && octet == 0x80)) {
                        throw AttestationVerificationException(
                            "Android AuthorizationList has a noncanonical high tag",
                        )
                    }
                    if (number > (Int.MAX_VALUE ushr 7)) {
                        throw AttestationVerificationException(
                            "Android AuthorizationList tag number overflows",
                        )
                    }
                    number = (number shl 7) or (octet and 0x7f)
                    if (octet and 0x80 == 0) break
                }
                if (number < 31) {
                    throw AttestationVerificationException(
                        "Android AuthorizationList high tag is not minimal",
                    )
                }
            }
            val length = readLength()
            if (length > bytes.size - offset) {
                throw AttestationVerificationException(
                    "Android AuthorizationList value overruns its DER input",
                )
            }
            val value = bytes.copyOfRange(offset, offset + length)
            offset += length
            return ExplicitAuthorizationField(number, value)
        }

        private fun readLength(): Int {
            val first = readByte()
            if (first and 0x80 == 0) return first
            val count = first and 0x7f
            if (count == 0 || count > 4) {
                throw AttestationVerificationException(
                    "Android AuthorizationList has an unsupported DER length",
                )
            }
            if (offset >= bytes.size || bytes[offset] == 0.toByte()) {
                throw AttestationVerificationException(
                    "Android AuthorizationList DER length is not canonical",
                )
            }
            var length = 0L
            repeat(count) {
                length = (length shl 8) or readByte().toLong()
            }
            if (length < 128 || length > Int.MAX_VALUE) {
                throw AttestationVerificationException(
                    "Android AuthorizationList DER length is outside bounds",
                )
            }
            return length.toInt()
        }

        private fun readByte(): Int {
            if (offset >= bytes.size) {
                throw AttestationVerificationException(
                    "Unexpected end of Android AuthorizationList DER",
                )
            }
            return bytes[offset++].toInt() and 0xff
        }
    }

    companion object {
        /** Creates a verifier that trusts the supplied root certificates. */
        @JvmStatic
        fun builder(): Builder = Builder()
    }
}
