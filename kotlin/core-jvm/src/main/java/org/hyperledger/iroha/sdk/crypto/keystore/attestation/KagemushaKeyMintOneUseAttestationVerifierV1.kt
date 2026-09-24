// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.AlgorithmParameters
import java.security.MessageDigest
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECFieldFp
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import java.util.concurrent.atomic.AtomicLong
import org.bouncycastle.asn1.ASN1Boolean
import org.bouncycastle.asn1.ASN1Enumerated
import org.bouncycastle.asn1.ASN1Integer
import org.bouncycastle.asn1.ASN1Null
import org.bouncycastle.asn1.ASN1OctetString
import org.bouncycastle.asn1.ASN1Primitive
import org.bouncycastle.asn1.ASN1Sequence
import org.bouncycastle.asn1.ASN1Set
import org.bouncycastle.asn1.ASN1TaggedObject
import org.bouncycastle.asn1.BERTags
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation

private const val KEYMINT_ATTESTATION_OID_V1 = "1.3.6.1.4.1.11129.2.1.17"
private val KEYMINT_CHALLENGE_DOMAIN_V1 =
    "iroha:kagemusha:keymint-prepared-key:v1\u0000".toByteArray(StandardCharsets.UTF_8)
private val KEYMINT_SUPPORTED_AUTH_TAGS_V1 = setOf(
    1, 2, 3, 5, 10, 303, 405, 503, 701, 702, 704, 705, 706, 709, 718, 719,
)
private val KEYMINT_SET_TAGS_V1 = setOf(1, 5)
private val KEYMINT_INTEGER_TAGS_V1 = setOf(2, 3, 10, 405, 701, 702, 705, 706, 718, 719)
private val KEYMINT_HARDWARE_ONLY_TAGS_V1 = setOf(1, 2, 3, 5, 10, 303, 405, 702, 704)

/** Fixed offsets from Core's sole V1 hardware-transition selection signing layout. */
internal object KagemushaSelectionFrameV1 {
    private val domain =
        "iroha:kagemusha:v1:hardware-transition-selection\u0000".toByteArray(StandardCharsets.US_ASCII)
    private const val FRAME_BYTES = 460
    private const val BODY_BYTES = 403
    private const val RELEASE = 59
    private const val PROVIDER = 91
    private const val APP_POLICY = 123
    private const val CREDENTIAL = 155
    private const val NETWORK = 187
    private const val LANE = 219
    private const val PROFILE = 251
    private const val POLICY_EPOCH = 283
    private const val HARDWARE_EPOCH = 291
    private const val HARDWARE_GENERATION = 323
    private const val OPERATION = 331
    private const val TRANSITION = 332
    private const val CANDIDATE = 364
    private const val TERMINAL = 396
    private const val BEFORE = 428
    private const val AFTER = 444

    fun requireExact(frame: ByteArray, lane: ByteArray, before: ByteArray, after: ByteArray) {
        require(domain.size == 49 && frame.size == FRAME_BYTES) { "Core S has the wrong V1 width" }
        require(frame.copyOfRange(0, domain.size).contentEquals(domain)) {
            "Core S has the wrong V1 signing domain"
        }
        require(frame.copyOfRange(domain.size, RELEASE - 2).contentEquals(
            byteArrayOf(BODY_BYTES.toByte(), (BODY_BYTES ushr 8).toByte(), 0, 0, 0, 0, 0, 0),
        )) { "Core S has the wrong V1 body length" }
        require(frame[RELEASE - 2] == 1.toByte() && frame[RELEASE - 1] == 0.toByte()) {
            "Core S has the wrong V1 wire version"
        }
        for (offset in intArrayOf(RELEASE, PROVIDER, APP_POLICY, CREDENTIAL, NETWORK, LANE,
                PROFILE, HARDWARE_EPOCH, TRANSITION)) {
            require(frame.copyOfRange(offset, offset + 32).any { it != 0.toByte() }) {
                "Core S has an empty required identity or transition digest"
            }
        }
        require(frame.copyOfRange(POLICY_EPOCH, POLICY_EPOCH + 8).any { it != 0.toByte() } &&
            frame.copyOfRange(HARDWARE_GENERATION, HARDWARE_GENERATION + 8).any { it != 0.toByte() }
        ) { "Core S has a zero policy or hardware generation" }
        val operation = frame[OPERATION].toInt() and 0xff
        require(operation in 1..5) { "Core S has an invalid monetary operation" }
        val outgoing = operation == 2 || operation == 4
        require(frame.copyOfRange(CANDIDATE, CANDIDATE + 32).any { it != 0.toByte() } == outgoing &&
            frame.copyOfRange(TERMINAL, TERMINAL + 32).any { it != 0.toByte() } == outgoing
        ) { "Core S has the wrong outgoing commitment shape" }
        require(frame.copyOfRange(LANE, LANE + 32).contentEquals(lane) &&
            frame.copyOfRange(BEFORE, BEFORE + 16).contentEquals(before) &&
            frame.copyOfRange(AFTER, AFTER + 16).contentEquals(after)
        ) { "Core S differs from the selected lane or exact-next indices" }
    }

    fun requireAppAttest(frame: ByteArray, previous: UInt) {
        require(previous != UInt.MAX_VALUE && frame.size == FRAME_BYTES) {
            "App Attest Core S counter or width is invalid"
        }
        fun index(value: UInt): ByteArray = ByteArray(16).also { bytes ->
            for (offset in 0 until 4) {
                bytes[offset] = (value.toLong() ushr (offset * 8)).toByte()
            }
        }
        requireExact(frame, frame.copyOfRange(LANE, LANE + 32),
            index(previous), index(previous + 1u))
    }
}

/** Reject any Core S other than the exact V1 layout and prepared lane/index selection. */
fun requireKagemushaCoreSelectionFrameV1(
    frame: ByteArray,
    lane: ByteArray,
    before: ByteArray,
    after: ByteArray,
) {
    KagemushaSelectionFrameV1.requireExact(frame, lane, before, after)
}

/** One release-pinned trust anchor, committed by both DER bytes and SHA-256. */
class KagemushaKeyMintPinnedRootV1(certificateDer: ByteArray, certificateSha256: ByteArray) {
    private val der = certificateDer.copyOf()
    private val digest = certificateSha256.copyOf()

    init {
        require(der.isNotEmpty() && der.size <= 16_384) { "Root DER is outside the V1 bounds" }
        require(digest.size == 32 && digest.any { it != 0.toByte() }) {
            "Root pin must be a non-zero SHA-256 digest"
        }
        require(MessageDigest.isEqual(sha256V1(der), digest)) { "Root DER does not match its release pin" }
    }

    fun certificateDer(): ByteArray = der.copyOf()
    fun certificateSha256(): ByteArray = digest.copyOf()
}

/** Exact app identity allowed by the release; signer hashes digest signing certificates. */
class KagemushaKeyMintAppPinV1(
    val packageName: String,
    val packageVersion: Long,
    signingCertificateSha256: List<ByteArray>,
) {
    private val signerDigests = signingCertificateSha256.map(ByteArray::copyOf)

    init {
        require(packageName.isNotBlank() && packageName.toByteArray(StandardCharsets.UTF_8).size <= 255)
        require(packageVersion >= 0)
        require(signerDigests.isNotEmpty() && signerDigests.size <= 8)
        require(signerDigests.all { it.size == 32 && it.any { byte -> byte != 0.toByte() } })
        require(signerDigests.distinctBy(::hexV1).size == signerDigests.size)
    }

    fun signingCertificateSha256(): List<ByteArray> = signerDigests.map(ByteArray::copyOf)
}

/** Trusted selection binding already committed by Core before the one-use signature is collected. */
class KagemushaKeyMintExpectedSelectionV1(
    canonicalSelectionFrame: ByteArray,
    laneCommitment: ByteArray,
    secureIndexBeforeLittleEndian: ByteArray,
    secureIndexAfterLittleEndian: ByteArray,
    attestationNonce: ByteArray,
    committedPublicKeySec1: ByteArray,
) {
    private val frame = canonicalSelectionFrame.copyOf()
    private val lane = laneCommitment.copyOf()
    private val before = secureIndexBeforeLittleEndian.copyOf()
    private val after = secureIndexAfterLittleEndian.copyOf()
    private val nonce = attestationNonce.copyOf()
    private val key = committedPublicKeySec1.copyOf()

    init {
        require(lane.size == 32 && before.size == 16 && after.size == 16)
        require(nonce.size == 32 && nonce.any { it != 0.toByte() })
        require(key.size == 65 && key[0] == 0x04.toByte())
        require(isExactNextV1(before, after)) { "Selection hardware index is not exact-next" }
        KagemushaSelectionFrameV1.requireExact(frame, lane, before, after)
    }

    fun canonicalSelectionFrame(): ByteArray = frame.copyOf()
    fun laneCommitment(): ByteArray = lane.copyOf()
    fun secureIndexBeforeLittleEndian(): ByteArray = before.copyOf()
    fun secureIndexAfterLittleEndian(): ByteArray = after.copyOf()
    fun attestationNonce(): ByteArray = nonce.copyOf()
    fun committedPublicKeySec1(): ByteArray = key.copyOf()
}

/** Byte-identical Android collector output, without a monetary-authority claim. */
class KagemushaKeyMintRawSelectionEvidenceV1(
    canonicalSelectionFrame: ByteArray,
    laneCommitment: ByteArray,
    secureIndexBeforeLittleEndian: ByteArray,
    secureIndexAfterLittleEndian: ByteArray,
    attestationNonce: ByteArray,
    attestationChallenge: ByteArray,
    publicKeySec1: ByteArray,
    certificateChainDer: List<ByteArray>,
    signatureDer: ByteArray,
) {
    private val frame = canonicalSelectionFrame.copyOf()
    private val lane = laneCommitment.copyOf()
    private val before = secureIndexBeforeLittleEndian.copyOf()
    private val after = secureIndexAfterLittleEndian.copyOf()
    private val nonce = attestationNonce.copyOf()
    private val challenge = attestationChallenge.copyOf()
    private val key = publicKeySec1.copyOf()
    private val chain = certificateChainDer.map(ByteArray::copyOf)
    private val signature = signatureDer.copyOf()

    fun canonicalSelectionFrame(): ByteArray = frame.copyOf()
    fun laneCommitment(): ByteArray = lane.copyOf()
    fun secureIndexBeforeLittleEndian(): ByteArray = before.copyOf()
    fun secureIndexAfterLittleEndian(): ByteArray = after.copyOf()
    fun attestationNonce(): ByteArray = nonce.copyOf()
    fun attestationChallenge(): ByteArray = challenge.copyOf()
    fun publicKeySec1(): ByteArray = key.copyOf()
    fun certificateChainDer(): List<ByteArray> = chain.map(ByteArray::copyOf)
    fun signatureDer(): ByteArray = signature.copyOf()
}

/** Verifier output authenticates only the raw one-use candidate, never a monetary transition. */
class KagemushaKeyMintVerifiedRawCandidateV1 internal constructor(
    val attestationSecurityLevel: AttestationResult.SecurityLevel,
    val keyMintSecurityLevel: AttestationResult.SecurityLevel,
    signatureSha256: ByteArray,
) {
    private val signatureDigest = signatureSha256.copyOf()
    fun signatureSha256(): ByteArray = signatureDigest.copyOf()
}

/**
 * Offline Android KeyMint V1 profile verifier for the ordinary-app one-use candidate.
 *
 * The release must supply pinned roots, app identity, and a governed, fresh revocation snapshot.
 * [trustedTimeEpochMillis] must return a fresh, authenticated time for every verification;
 * ordinary app wall-clock time is not a monetary freshness authority. The verifier rejects an
 * in-instance clock regression but the trusted source must also prevent rollback across restarts.
 * This verifier cannot establish predecessor commitment or recursive lineage. Core must enforce
 * those independently before any monetary use; no caller may promote this result to authority.
 */
class KagemushaKeyMintOneUseAttestationVerifierV1(
    trustedRoots: List<KagemushaKeyMintPinnedRootV1>,
    private val appPin: KagemushaKeyMintAppPinV1,
    private val revocationPolicy: AndroidAttestationRevocationPolicyV1,
    private val trustedTimeEpochMillis: () -> Long,
) {
    private val rootCertificates = trustedRoots.map { it.certificateDer() }
    private val lastEvaluationTime = AtomicLong(Long.MIN_VALUE)

    init {
        require(trustedRoots.isNotEmpty() && trustedRoots.size <= 8) {
            "At least one release-pinned attestation root is required"
        }
    }

    /** Rejects any binding, attestation, app, hardware or signature mismatch. */
    @Throws(AttestationVerificationException::class)
    fun verify(
        expected: KagemushaKeyMintExpectedSelectionV1,
        raw: KagemushaKeyMintRawSelectionEvidenceV1,
    ): KagemushaKeyMintVerifiedRawCandidateV1 {
        sameV1(expected.canonicalSelectionFrame(), raw.canonicalSelectionFrame(), "Core selection frame")
        sameV1(expected.laneCommitment(), raw.laneCommitment(), "lane commitment")
        sameV1(expected.secureIndexBeforeLittleEndian(), raw.secureIndexBeforeLittleEndian(), "predecessor index")
        sameV1(expected.secureIndexAfterLittleEndian(), raw.secureIndexAfterLittleEndian(), "successor index")
        sameV1(expected.attestationNonce(), raw.attestationNonce(), "attestation nonce")
        sameV1(expected.committedPublicKeySec1(), raw.publicKeySec1(), "committed public key")
        val challenge = preparedChallengeV1(
            expected.attestationNonce(), expected.laneCommitment(),
            expected.secureIndexBeforeLittleEndian(), expected.secureIndexAfterLittleEndian(),
        )
        sameV1(challenge, raw.attestationChallenge(), "prepared-key challenge")

        val chain = raw.certificateChainDer()
        if (chain.size !in 2..8 || chain.any { it.isEmpty() || it.size > 16_384 }) {
            throw AttestationVerificationException("Attestation chain is outside V1 bounds")
        }
        val evaluationTime = trustedTimeEpochMillis()
        if (evaluationTime < 0) throw AttestationVerificationException("Trusted evaluation time is invalid")
        while (true) {
            val prior = lastEvaluationTime.get()
            if (evaluationTime < prior) {
                throw AttestationVerificationException("Trusted evaluation time regressed")
            }
            if (lastEvaluationTime.compareAndSet(prior, evaluationTime)) break
        }
        val builder = AttestationVerifier.builder(revocationPolicy, evaluationTime)
        rootCertificates.forEach(builder::addTrustedRoot)
        val verified = builder.build().verify(KeyAttestation("kagemusha-one-use-v1", chain), challenge)
        val keyUsage = verified.leafCertificate.keyUsage
        if (verified.leafCertificate.basicConstraints >= 0 || keyUsage == null ||
            keyUsage.isEmpty() || !keyUsage[0] || (keyUsage.size > 5 && keyUsage[5])
        ) {
            throw AttestationVerificationException("Attested leaf is not a non-CA digital-signature key")
        }
        if (verified.attestationSecurityLevel == AttestationResult.SecurityLevel.SOFTWARE ||
            verified.keymasterSecurityLevel == AttestationResult.SecurityLevel.SOFTWARE ||
            verified.attestationSecurityLevel != verified.keymasterSecurityLevel
        ) {
            throw AttestationVerificationException("Attestation and key must share one hardware security level")
        }
        val description = parseDescriptionV1(verified.leafCertificate.getExtensionValue(KEYMINT_ATTESTATION_OID_V1))
        // Hardware-enforced limited-use keys are a KeyMint feature. Keymaster-era
        // layouts must not acquire the V1 one-use guarantee merely by carrying
        // a syntactically valid tag 405 in a signed extension.
        if (description.version !in KEYMINT_ATTESTATION_VERSIONS_V1 ||
            description.keyMintVersion != description.version
        ) {
            throw AttestationVerificationException("Attestation does not use a supported KeyMint version")
        }
        val software = parseAuthorizationsV1(description.software, "software")
        val hardware = parseAuthorizationsV1(description.hardware, "hardware")
        if (software.keys.any { it in KEYMINT_HARDWARE_ONLY_TAGS_V1 } ||
            hardware.containsKey(709)
        ) {
            throw AttestationVerificationException("Security-critical authorization has the wrong enforcement level")
        }
        checkHardwareKeyProfileV1(hardware)
        checkVerifiedBootV1(hardware.getValue(704))
        checkAppIdentityV1(software[709] ?: throw AttestationVerificationException("Attested app identity is missing"))

        val publicKey = verified.leafCertificate.publicKey as? ECPublicKey
            ?: throw AttestationVerificationException("Attested leaf is not an EC public key")
        sameV1(expected.committedPublicKeySec1(), sec1P256V1(publicKey), "attested leaf public key")
        val signature = raw.signatureDer()
        checkDerSignatureV1(signature, publicKey.params.order)
        val verifier = Signature.getInstance("SHA256withECDSA")
        verifier.initVerify(publicKey)
        verifier.update(expected.canonicalSelectionFrame())
        if (!verifier.verify(signature)) {
            throw AttestationVerificationException("One-use KeyMint signature does not verify over Core S")
        }
        return KagemushaKeyMintVerifiedRawCandidateV1(
            verified.attestationSecurityLevel, verified.keymasterSecurityLevel, sha256V1(signature),
        )
    }

    private fun checkAppIdentityV1(value: ASN1Primitive) {
        val octets = value as? ASN1OctetString
            ?: throw AttestationVerificationException("Attested app identity is not an OCTET STRING")
        val identity = derSequenceV1(octets.octets)
        if (identity.size() != 2) throw AttestationVerificationException("Malformed attested app identity")
        val packages = identity.getObjectAt(0) as? ASN1Set
            ?: throw AttestationVerificationException("Attested app packages are not a SET")
        if (packages.size() != 1) throw AttestationVerificationException("Shared-UID or missing app identity is unsupported")
        val packageInfo = packages.getObjectAt(0) as? ASN1Sequence
            ?: throw AttestationVerificationException("Malformed attested package")
        if (packageInfo.size() != 2) throw AttestationVerificationException("Malformed attested package fields")
        val packageName = (packageInfo.getObjectAt(0) as? ASN1OctetString)?.octets
            ?: throw AttestationVerificationException("Malformed attested package name")
        sameV1(appPin.packageName.toByteArray(StandardCharsets.UTF_8), packageName, "attested package")
        val version = (packageInfo.getObjectAt(1) as? ASN1Integer)?.value
            ?: throw AttestationVerificationException("Malformed attested package version")
        if (version != BigInteger.valueOf(appPin.packageVersion)) {
            throw AttestationVerificationException("Attested package version does not match release")
        }
        val digests = identity.getObjectAt(1) as? ASN1Set
            ?: throw AttestationVerificationException("Attested signer digests are not a SET")
        if (digests.size() != appPin.signingCertificateSha256().size) {
            throw AttestationVerificationException("Attested signer count does not match release")
        }
        val observed = (0 until digests.size()).map { index ->
            val digest = (digests.getObjectAt(index) as? ASN1OctetString)?.octets
                ?: throw AttestationVerificationException("Malformed attested signer digest")
            if (digest.size != 32) throw AttestationVerificationException("Attested signer digest is not SHA-256")
            hexV1(digest)
        }.toSet()
        val expected = appPin.signingCertificateSha256().map(::hexV1).toSet()
        if (observed.size != digests.size() || observed != expected) {
            throw AttestationVerificationException("Attested signer digest does not match release")
        }
    }

    private fun checkHardwareKeyProfileV1(hardware: Map<Int, ASN1Primitive>) {
        requireSetExactlyV1(hardware[1], 2, "hardware SIGN purpose")
        requireIntegerV1(hardware[2], 3, "hardware EC algorithm")
        requireIntegerV1(hardware[3], 256, "hardware P-256 key size")
        requireSetExactlyV1(hardware[5], 4, "hardware SHA-256 digest")
        requireIntegerV1(hardware[10], 1, "hardware P-256 curve")
        if (hardware[303] !is ASN1Null) {
            throw AttestationVerificationException("Hardware rollback resistance is missing")
        }
        requireIntegerV1(hardware[405], 1, "hardware one-use limit")
        requireIntegerV1(hardware[702], 0, "hardware-generated origin")
    }
}

/** Recomputes the only accepted KeyMint preparation challenge. */
fun preparedChallengeV1(nonce: ByteArray, lane: ByteArray, before: ByteArray, after: ByteArray): ByteArray {
    require(nonce.size == 32 && nonce.any { it != 0.toByte() } && lane.size == 32 &&
        before.size == 16 && after.size == 16)
    return sha256V1(KEYMINT_CHALLENGE_DOMAIN_V1 + nonce + lane + before + after)
}

private data class KeyMintDescriptionV1(
    val version: Int,
    val keyMintVersion: Int,
    val software: ASN1Sequence,
    val hardware: ASN1Sequence,
)

private val KEYMINT_ATTESTATION_VERSIONS_V1 = setOf(100, 200, 300, 400, 500)

private fun parseDescriptionV1(extension: ByteArray?): KeyMintDescriptionV1 {
    if (extension == null) throw AttestationVerificationException("Android attestation extension is absent")
    try {
        val outer = derPrimitiveV1(extension) as? ASN1OctetString
            ?: throw AttestationVerificationException("Android attestation extension wrapper is malformed")
        val sequence = derSequenceV1(outer.octets)
        if (sequence.size() != 8) throw AttestationVerificationException("Unsupported KeyDescription layout")
        val version = (sequence.getObjectAt(0) as? ASN1Integer)?.value?.intValueExact()
            ?: throw AttestationVerificationException("Malformed attestation version")
        val attestationLevel = (sequence.getObjectAt(1) as? ASN1Enumerated)?.value?.intValueExact()
            ?: throw AttestationVerificationException("Malformed attestation security level")
        val keyMintVersion = (sequence.getObjectAt(2) as? ASN1Integer)?.value?.intValueExact()
            ?: throw AttestationVerificationException("Malformed KeyMint version")
        val keyMintLevel = (sequence.getObjectAt(3) as? ASN1Enumerated)?.value?.intValueExact()
            ?: throw AttestationVerificationException("Malformed KeyMint security level")
        if (version <= 0 || keyMintVersion < 0 || attestationLevel !in 1..2 || keyMintLevel !in 1..2) {
            throw AttestationVerificationException("Software or invalid KeyMint attestation level")
        }
        if (sequence.getObjectAt(4) !is ASN1OctetString || sequence.getObjectAt(5) !is ASN1OctetString) {
            throw AttestationVerificationException("Malformed KeyMint challenge or unique ID")
        }
        val software = sequence.getObjectAt(6) as? ASN1Sequence
            ?: throw AttestationVerificationException("Malformed software authorizations")
        val hardware = sequence.getObjectAt(7) as? ASN1Sequence
            ?: throw AttestationVerificationException("Malformed hardware authorizations")
        return KeyMintDescriptionV1(version, keyMintVersion, software, hardware)
    } catch (error: AttestationVerificationException) {
        throw error
    } catch (error: RuntimeException) {
        throw AttestationVerificationException("Malformed Android KeyMint attestation DER", error)
    }
}

private fun parseAuthorizationsV1(sequence: ASN1Sequence, level: String): Map<Int, ASN1Primitive> {
    val parsed = linkedMapOf<Int, ASN1Primitive>()
    var prior = 0
    try {
        for (index in 0 until sequence.size()) {
            val tagged = sequence.getObjectAt(index) as? ASN1TaggedObject
                ?: throw AttestationVerificationException("$level authorization is not context-tagged")
            val tag = tagged.tagNo
            if (!tagged.hasContextTag(tag) || tag <= prior || tag !in KEYMINT_SUPPORTED_AUTH_TAGS_V1) {
                throw AttestationVerificationException("Unsupported, duplicate or unsorted $level authorization tag $tag")
            }
            prior = tag
            val expectedType = when (tag) {
                in KEYMINT_SET_TAGS_V1 -> BERTags.SET
                in KEYMINT_INTEGER_TAGS_V1 -> BERTags.INTEGER
                303, 503 -> BERTags.NULL
                704 -> BERTags.SEQUENCE
                709 -> BERTags.OCTET_STRING
                else -> throw AttestationVerificationException("Unsupported $level authorization tag $tag")
            }
            parsed[tag] = tagged.getBaseUniversal(true, expectedType)
        }
        return parsed
    } catch (error: AttestationVerificationException) {
        throw error
    } catch (error: RuntimeException) {
        throw AttestationVerificationException("Malformed $level KeyMint authorization", error)
    }
}

private fun checkVerifiedBootV1(value: ASN1Primitive) {
    val root = value as? ASN1Sequence
        ?: throw AttestationVerificationException("Hardware root of trust is malformed")
    if (root.size() != 4) throw AttestationVerificationException("Hardware root of trust is incomplete")
    val bootKey = (root.getObjectAt(0) as? ASN1OctetString)?.octets
        ?: throw AttestationVerificationException("Verified boot key is malformed")
    if (bootKey.size < 32 || bootKey.all { it == 0.toByte() }) {
        throw AttestationVerificationException("Verified boot key is empty")
    }
    val locked = root.getObjectAt(1) as? ASN1Boolean
    val bootState = (root.getObjectAt(2) as? ASN1Enumerated)?.value
    if (locked?.isTrue != true || bootState != BigInteger.ZERO) {
        throw AttestationVerificationException("Device boot is not locked and verified")
    }
    val hash = (root.getObjectAt(3) as? ASN1OctetString)?.octets
    if (hash == null || hash.size < 32 || hash.all { it == 0.toByte() }) {
        throw AttestationVerificationException("Verified boot hash is malformed")
    }
}

private fun requireIntegerV1(value: ASN1Primitive?, exact: Int, label: String) {
    if ((value as? ASN1Integer)?.value != BigInteger.valueOf(exact.toLong())) {
        throw AttestationVerificationException("$label is missing or mismatched")
    }
}

private fun requireSetExactlyV1(value: ASN1Primitive?, exact: Int, label: String) {
    val set = value as? ASN1Set
    if (set?.size() != 1 || (set.getObjectAt(0) as? ASN1Integer)?.value != BigInteger.valueOf(exact.toLong())) {
        throw AttestationVerificationException("$label is missing or mismatched")
    }
}

private fun sec1P256V1(key: ECPublicKey): ByteArray {
    val params = key.params
    val standard = AlgorithmParameters.getInstance("EC").apply {
        init(ECGenParameterSpec("secp256r1"))
    }.getParameterSpec(ECParameterSpec::class.java)
    val field = params.curve.field as? ECFieldFp
        ?: throw AttestationVerificationException("Attested EC field is not prime")
    val standardField = standard.curve.field as ECFieldFp
    if (field.p != standardField.p || params.curve.a != standard.curve.a ||
        params.curve.b != standard.curve.b || params.generator != standard.generator ||
        params.order != standard.order || params.cofactor != standard.cofactor
    ) throw AttestationVerificationException("Attested curve is not secp256r1")
    val x = key.w.affineX
    val y = key.w.affineY
    if (x.signum() < 0 || y.signum() < 0 || x >= field.p || y >= field.p ||
        y.modPow(BigInteger.valueOf(2), field.p) !=
            x.modPow(BigInteger.valueOf(3), field.p)
                .add(params.curve.a.multiply(x)).add(params.curve.b).mod(field.p)
    ) throw AttestationVerificationException("Attested P-256 point is invalid")
    return byteArrayOf(0x04) + fixedUnsigned32V1(x) + fixedUnsigned32V1(y)
}

private fun fixedUnsigned32V1(value: BigInteger): ByteArray {
    val bytes = value.toByteArray()
    val unsigned = if (bytes.size == 33 && bytes[0] == 0.toByte()) bytes.copyOfRange(1, 33) else bytes
    if (unsigned.size > 32) throw AttestationVerificationException("P-256 coordinate overflows")
    return ByteArray(32 - unsigned.size) + unsigned
}

private fun checkDerSignatureV1(signature: ByteArray, order: BigInteger) {
    if (signature.size !in 8..80) throw AttestationVerificationException("ECDSA signature is outside V1 bounds")
    val sequence = derSequenceV1(signature)
    if (sequence.size() != 2) throw AttestationVerificationException("ECDSA signature is malformed")
    for (index in 0..1) {
        val scalar = (sequence.getObjectAt(index) as? ASN1Integer)?.value
            ?: throw AttestationVerificationException("ECDSA scalar is malformed")
        if (scalar.signum() <= 0 || scalar >= order) {
            throw AttestationVerificationException("ECDSA scalar is outside the P-256 order")
        }
    }
}

private fun derSequenceV1(bytes: ByteArray): ASN1Sequence = derPrimitiveV1(bytes) as? ASN1Sequence
    ?: throw AttestationVerificationException("Expected canonical DER SEQUENCE")

private fun derPrimitiveV1(bytes: ByteArray): ASN1Primitive {
    try {
        val parsed = ASN1Primitive.fromByteArray(bytes)
        if (!MessageDigest.isEqual(bytes, parsed.getEncoded("DER"))) {
            throw AttestationVerificationException("Non-canonical DER encoding")
        }
        return parsed
    } catch (error: AttestationVerificationException) {
        throw error
    } catch (error: Exception) {
        throw AttestationVerificationException("Invalid DER encoding", error)
    }
}

private fun isExactNextV1(before: ByteArray, after: ByteArray): Boolean {
    val next = before.copyOf()
    for (index in next.indices) {
        next[index] = (next[index].toInt() + 1).toByte()
        if (next[index] != 0.toByte()) return next.contentEquals(after)
    }
    return false
}

private fun sameV1(expected: ByteArray, actual: ByteArray, label: String) {
    if (!MessageDigest.isEqual(expected, actual)) {
        throw AttestationVerificationException("$label does not match the trusted selection")
    }
}

private fun sha256V1(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

private fun hexV1(bytes: ByteArray): String = buildString(bytes.size * 2) {
    val digits = "0123456789abcdef"
    bytes.forEach { byte ->
        val value = byte.toInt() and 0xff
        append(digits[value ushr 4]).append(digits[value and 0x0f])
    }
}
