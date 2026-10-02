package org.hyperledger.iroha.sdk.tools

import java.nio.charset.StandardCharsets
import java.math.BigInteger
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.file.StandardCopyOption
import java.security.MessageDigest
import java.security.cert.CertificateException
import java.io.IOException
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.crypto.keystore.KeyAttestation
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AndroidAttestationRevocationPolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationResult
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerificationException
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.AttestationVerifier
import kotlin.system.exitProcess

/** Offline attestation verification with separately supplied trust and identity commitments. */
object AndroidAttestationCommand {
    /** Verified summary; mutable evidence and certificate objects are never exposed. */
    class Result internal constructor(
        val alias: String,
        val attestationSecurityLevel: AttestationResult.SecurityLevel,
        val keymasterSecurityLevel: AttestationResult.SecurityLevel,
        val strongBoxAttestation: Boolean,
        val challengeHex: String,
        val chainLength: Int,
        val evaluationTimeMillis: Long,
        val revocationSnapshotSha256: String,
        val leafSpkiSha256: String,
    ) {
        /** One deterministic JSON object containing the verified result and trusted commitments. */
        fun toJson(): String = JsonEncoder.encode(linkedMapOf(
            "schema" to "iroha.android.attestation.verification.v1",
            "alias" to alias,
            "attestation_security_level" to attestationSecurityLevel.name,
            "keymaster_security_level" to keymasterSecurityLevel.name,
            "strongbox_attestation" to strongBoxAttestation,
            "challenge_hex" to challengeHex,
            "chain_length" to chainLength,
            "evaluation_time_ms" to evaluationTimeMillis,
            "revocation_snapshot_sha256" to revocationSnapshotSha256,
            "leaf_spki_sha256" to leafSpkiSha256,
        )) + "\n"
    }

    /** Lower DATA summary only; never a signed raw admission, receipt or issuer owner. */
    class FirstDeviceResult internal constructor(
        val alias: String,
        val securityLevel: AttestationResult.SecurityLevel,
        val challengeOriginalSha256: String,
        val appPublicKeySec1: String,
        val attestedKeyId: String,
        val appPackage: String,
        val appVersionCode: String,
        val appSigningIdentitySha256: String,
        val chainLength: Int,
        val evaluationTimeMillis: Long,
        val revocationSnapshotSha256: String,
        val measuredLeafSpkiSha256: String,
    ) {
        fun toJson(): String = JsonEncoder.encode(linkedMapOf(
            "schema" to "iroha.android.first_device_attestation.verification.v1",
            "alias" to alias,
            "security_level" to securityLevel.name,
            "challenge_original_sha256" to challengeOriginalSha256,
            "app_public_key_sec1" to appPublicKeySec1,
            "attested_key_id" to attestedKeyId,
            "app_package" to appPackage,
            "app_version_code" to appVersionCode,
            "app_signing_identity_sha256" to appSigningIdentitySha256,
            "chain_length" to chainLength,
            "evaluation_time_ms" to evaluationTimeMillis,
            "revocation_snapshot_sha256" to revocationSnapshotSha256,
            "measured_leaf_spki_sha256" to measuredLeafSpkiSha256,
        )) + "\n"
    }

    /** Command entry point: JSON on success, diagnostics and nonzero exit on failure. */
    @JvmStatic
    fun main(args: Array<String>) {
        if (args.contentEquals(arrayOf("--help"))) {
            print(USAGE)
            return
        }
        if (args.contentEquals(arrayOf("first-device", "--help"))) {
            print(FIRST_DEVICE_USAGE)
            return
        }
        try {
            print(if (args.firstOrNull() == "first-device") {
                runFirstDevice(args.copyOfRange(1, args.size)).toJson()
            } else {
                run(args).toJson()
            })
        } catch (error: Exception) {
            System.err.println("[attestation] ${error.message ?: "verification failed"}")
            exitProcess(1)
        }
    }

    /** Verify the complete command inputs; no trust is inferred from the evidence directory. */
    @JvmStatic
    @Throws(IOException::class, CertificateException::class, AttestationVerificationException::class)
    fun run(args: Array<String>): Result {
        val arguments = Arguments.parse(args)
        val inputs = AttestationInputs()
        val snapshot = inputs.readFile(arguments.path("--revocation-snapshot"), 512 * 1024)
        val snapshotHash = digest(arguments.required("--revocation-snapshot-sha256"))
        val expectedSpki = digest(arguments.required("--expected-leaf-spki-sha256"))
        val challenge = arguments.single["--challenge-hex"]?.let(::challenge)
            ?: challenge(String(
                inputs.readFile(arguments.path("--challenge-file"), 4096), StandardCharsets.US_ASCII,
            ).trim())
        val time = arguments.required("--evaluation-time-ms").let {
            require(it.matches(Regex("[1-9][0-9]{0,18}"))) { "--evaluation-time-ms must be a positive integer" }
            it.toLongOrNull() ?: throw IllegalArgumentException("--evaluation-time-ms is outside the supported range")
        }
        val policy = AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(snapshot, snapshotHash)
        val verifier = AttestationVerifier.builder(policy, time).requireStrongBox(arguments.strongBox)
        val roots = inputs.trustedRoots(arguments.roots, arguments.rootDirectories, arguments.rootBundles)
        roots.forEach(verifier::addTrustedRoot)
        val chain = if (arguments.single.containsKey("--chain")) {
            inputs.chain(arguments.path("--chain"), false)
        } else {
            inputs.chain(arguments.path("--bundle-dir"), true)
        }
        val attestation = KeyAttestation(arguments.required("--alias"), chain.map { it.encoded })
        val verified = verifier.build().verify(attestation, challenge)
        val spki = verified.leafCertificate.publicKey.encoded
            ?: throw AttestationVerificationException("Attestation leaf has no public-key SPKI encoding")
        val actualSpki = MessageDigest.getInstance("SHA-256").digest(spki)
        if (!MessageDigest.isEqual(expectedSpki, actualSpki)) {
            throw AttestationVerificationException(
                "Attestation leaf public key does not match the separately trusted alias key commitment",
            )
        }
        val result = Result(
            verified.alias, verified.attestationSecurityLevel, verified.keymasterSecurityLevel,
            verified.isStrongBoxAttestation, hex(verified.attestationChallenge()), verified.certificateChain().size,
            time, hex(snapshotHash), hex(actualSpki),
        )
        arguments.single["--output"]?.let { output ->
            writeResult(Paths.get(output).toAbsolutePath().normalize(), result.toJson(), inputs)
        }
        return result
    }

    /**
     * Verify the original first-device chain using independent root/revocation/time/app policy.
     * Complete signed C is DATA here. No C signature, Google owner, raw archive digest, replay,
     * issuer authority or financial admission is reconstructed from this tooling result.
     */
    @JvmStatic
    @Throws(IOException::class, CertificateException::class, AttestationVerificationException::class)
    fun runFirstDevice(args: Array<String>): FirstDeviceResult {
        val arguments = Arguments.parse(args, Purpose.FIRST_DEVICE_APP)
        val inputs = AttestationInputs()
        val snapshot = inputs.readFile(arguments.path("--revocation-snapshot"), 512 * 1024)
        val snapshotHash = digest(arguments.required("--revocation-snapshot-sha256"))
        val originalC = inputs.readFile(arguments.path("--signed-challenge-original"), 192 * 1024)
        val signingIdentity = digest(arguments.required("--expected-app-signing-identity-sha256"))
        val versionText = arguments.required("--expected-app-version-code")
        require(versionText.matches(Regex("[1-9][0-9]{0,19}"))) { "Installed app version must be canonical positive decimal" }
        val version = BigInteger(versionText)
        require(version.bitLength() <= 64) { "Installed app version is outside unsigned-u64 bounds" }
        val levels = when (arguments.required("--allowed-security-levels")) {
            "TRUSTED_ENVIRONMENT" -> setOf(AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT)
            "STRONG_BOX" -> setOf(AttestationResult.SecurityLevel.STRONG_BOX)
            "TRUSTED_ENVIRONMENT,STRONG_BOX" -> setOf(
                AttestationResult.SecurityLevel.TRUSTED_ENVIRONMENT, AttestationResult.SecurityLevel.STRONG_BOX,
            )
            else -> throw IllegalArgumentException("Installed hardware levels must be canonical TEE/StrongBox selections")
        }
        val time = arguments.required("--evaluation-time-ms").let {
            require(it.matches(Regex("[1-9][0-9]{0,18}"))) { "--evaluation-time-ms must be a positive integer" }
            it.toLongOrNull() ?: throw IllegalArgumentException("--evaluation-time-ms is outside the supported range")
        }
        val policy = AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(snapshot, snapshotHash)
        val verifier = AttestationVerifier.builder(policy, time)
        inputs.trustedRoots(arguments.roots, arguments.rootDirectories, arguments.rootBundles).forEach(verifier::addTrustedRoot)
        val chain = if (arguments.single.containsKey("--chain")) {
            inputs.chain(arguments.path("--chain"), false)
        } else {
            inputs.chain(arguments.path("--bundle-dir"), true)
        }
        val attestation = KeyAttestation(arguments.required("--alias"), chain.map { it.encoded })
        val verified = verifier.build().verifyFirstDevicePersistentAppOriginals(
            attestation, originalC, arguments.required("--expected-app-package"), version, signingIdentity, levels,
        )
        val result = FirstDeviceResult(
            attestation.alias, verified.securityLevel, hex(verified.challengeOriginalSha256()),
            hex(verified.publicKeySec1()), hex(verified.attestedKeyId()), verified.packageName,
            verified.versionCode.toString(), hex(verified.signingIdentitySha256()), verified.chainLength,
            verified.evaluationTimeEpochMillis, hex(snapshotHash), hex(verified.leafSpkiSha256()),
        )
        arguments.single["--output"]?.let { output ->
            writeResult(Paths.get(output).toAbsolutePath().normalize(), result.toJson(), inputs)
        }
        return result
    }

    private fun writeResult(path: Path, json: String, inputs: AttestationInputs) {
        require(!Files.isSymbolicLink(path)) { "--output must not be a symlink" }
        val parent = path.parent ?: throw IllegalArgumentException("--output needs a parent directory")
        Files.createDirectories(parent)
        val target = parent.toRealPath().resolve(path.fileName)
        inputs.requireDistinctOutput(target)
        val temporary = Files.createTempFile(parent, ".iroha-attestation-", ".json")
        try {
            Files.write(temporary, json.toByteArray(StandardCharsets.UTF_8))
            Files.move(temporary, target, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING)
        } finally {
            Files.deleteIfExists(temporary)
        }
    }

    private fun challenge(value: String): ByteArray {
        require(value.isNotEmpty() && value.length <= 2048 && value.length % 2 == 0 &&
            value.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) {
            "Challenge must contain 1..1024 bytes of hexadecimal"
        }
        return ByteArray(value.length / 2) { value.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }

    private fun digest(value: String): ByteArray {
        require(value.matches(Regex("[0-9a-f]{64}")) && value.any { it != '0' }) {
            "Trusted SHA-256 commitments require a nonzero 32-byte value in lowercase hexadecimal"
        }
        return challenge(value)
    }

    private fun hex(value: ByteArray): String = value.joinToString("") { "%02x".format(it.toInt() and 255) }

    private enum class Purpose { ALIAS_KEY, FIRST_DEVICE_APP }

    private class Arguments(
        val single: Map<String, String>,
        val roots: List<Path>,
        val rootDirectories: List<Path>,
        val rootBundles: List<Path>,
        val strongBox: Boolean,
    ) {
        fun required(flag: String): String = single[flag] ?: throw IllegalArgumentException("$flag is required")
        fun path(flag: String): Path = Paths.get(required(flag)).toAbsolutePath().normalize()

        companion object {
            fun parse(args: Array<String>, purpose: Purpose = Purpose.ALIAS_KEY): Arguments {
                require(args.size <= 256 && args.all { it.length <= 8192 }) { "Command arguments exceed size bounds" }
                val single = linkedMapOf<String, String>()
                val repeated = mapOf(
                    "--trust-root" to mutableListOf<Path>(),
                    "--trust-root-dir" to mutableListOf<Path>(),
                    "--trust-root-bundle" to mutableListOf<Path>(),
                )
                val flags = if (purpose == Purpose.ALIAS_KEY) {
                    setOf("--chain", "--bundle-dir", "--alias", "--challenge-hex", "--challenge-file",
                        "--expected-leaf-spki-sha256", "--revocation-snapshot", "--revocation-snapshot-sha256",
                        "--evaluation-time-ms", "--output")
                } else {
                    setOf("--chain", "--bundle-dir", "--alias", "--revocation-snapshot", "--revocation-snapshot-sha256",
                        "--evaluation-time-ms", "--output", "--signed-challenge-original", "--expected-app-package",
                        "--expected-app-version-code", "--expected-app-signing-identity-sha256", "--allowed-security-levels")
                }
                var strongBox = false
                var index = 0
                while (index < args.size) {
                    val flag = args[index++]
                    if (flag == "--require-strongbox") {
                        require(purpose == Purpose.ALIAS_KEY) { "--require-strongbox is not a first-device argument; supply --allowed-security-levels" }
                        require(!strongBox) { "Duplicate --require-strongbox" }
                        strongBox = true
                        continue
                    }
                    require(flag in flags || flag in repeated) { "Unknown argument: $flag; use --help" }
                    require(index < args.size && args[index].isNotEmpty() && !args[index].startsWith("--")) {
                        "$flag requires a value"
                    }
                    val value = args[index++]
                    val paths = repeated[flag]
                    if (paths == null) {
                        require(single.put(flag, value) == null) { "Duplicate $flag" }
                    } else {
                        paths.add(Paths.get(value).toAbsolutePath().normalize())
                    }
                }
                require(single.containsKey("--chain") xor single.containsKey("--bundle-dir")) {
                    "Exactly one --chain or --bundle-dir is required"
                }
                if (purpose == Purpose.ALIAS_KEY) {
                    require(single.containsKey("--challenge-hex") xor single.containsKey("--challenge-file")) {
                        "Exactly one separately trusted --challenge-hex or --challenge-file is required"
                    }
                }
                val alias = single["--alias"]
                require(alias != null && alias.isNotBlank() && alias == alias.trim() && alias.length <= 255 &&
                    alias.none { Character.isISOControl(it) }) { "A canonical separately trusted --alias is required" }
                require(repeated.values.any { it.isNotEmpty() }) { "At least one separately trusted root source is required" }
                val requiredFlags = if (purpose == Purpose.ALIAS_KEY) {
                    listOf("--revocation-snapshot", "--revocation-snapshot-sha256",
                        "--expected-leaf-spki-sha256", "--evaluation-time-ms")
                } else {
                    listOf("--revocation-snapshot", "--revocation-snapshot-sha256", "--evaluation-time-ms",
                        "--signed-challenge-original", "--expected-app-package", "--expected-app-version-code",
                        "--expected-app-signing-identity-sha256", "--allowed-security-levels")
                }
                for (flag in requiredFlags) {
                    require(single.containsKey(flag)) { "$flag is required" }
                }
                return Arguments(single.toMap(), repeated.getValue("--trust-root").toList(),
                    repeated.getValue("--trust-root-dir").toList(), repeated.getValue("--trust-root-bundle").toList(), strongBox)
            }
        }
    }

    private const val FIRST_DEVICE_USAGE = """Usage: iroha-attestation first-device (--chain FILE | --bundle-dir DIR) [options]

Independent installed inputs:
  --trust-root FILE / --trust-root-dir DIR / --trust-root-bundle ZIP  Root sources (repeatable).
  --alias LABEL                 Supplied reservation/archive alias DATA; not authenticated by certificates.
  --signed-challenge-original FILE  Complete signed C DATA, bounded 192 KiB; nonce is its SHA256.
  --expected-app-package NAME   Independently admitted manifest package.
  --expected-app-version-code N Canonical positive unsigned-u64 decimal version.
  --expected-app-signing-identity-sha256 HEX  Independently admitted signing-certificate SHA256.
  --allowed-security-levels LEVELS  TRUSTED_ENVIRONMENT, STRONG_BOX, or TRUSTED_ENVIRONMENT,STRONG_BOX.
  --revocation-snapshot FILE / --revocation-snapshot-sha256 HEX  Governed snapshot and independent hash.
  --evaluation-time-ms TIME     Positive epoch milliseconds within snapshot freshness.
  --output FILE                Atomic distinct DATA JSON output also printed to stdout.
  --help                       Print this first-device help.

This lower verifier does not authenticate C signature/manifest/Google owner/replay or sign an admission.
The genuine issuer must own those joins and install the exact single-package/signer raw policy.
Old alias-key challenge/SPKI/require-strongbox flags are rejected in this purpose. No fallback exists.
Limits remain the shared bounded input limits; first-device requires 2..8 original DER certificates,
1..16384 bytes each. No offered leaf digest is an independently trusted SPKI selector.
"""

    private const val USAGE = """Usage: iroha-attestation (--chain FILE | --bundle-dir DIR) [options]

Required independently trusted inputs:
  --trust-root FILE             PEM/DER trust anchors (repeatable).
  --trust-root-dir DIR          Recursively scan trusted certificate/ZIP sources (repeatable).
  --trust-root-bundle ZIP       Trusted root ZIP archive (repeatable).
  --alias LABEL                 Trusted keystore alias; never read from evidence metadata.
  --challenge-hex HEX           Expected nonempty challenge, or --challenge-file FILE (hex text).
  --expected-leaf-spki-sha256 HEX  Trusted alias public-key commitment, lowercase SHA-256.
  --revocation-snapshot FILE    Canonical governed V1 certificate-status snapshot.
  --revocation-snapshot-sha256 HEX  Independently trusted snapshot SHA-256.
  --evaluation-time-ms TIME     Explicit positive epoch milliseconds within snapshot freshness.

Options:
  --require-strongbox           Require StrongBox attestation security level.
  --output FILE                Atomically write the verified JSON also printed to stdout.
  --help                       Print this help.

Supply at least one root source. Bundle metadata never supplies trust or identity.
Limits: 16 chain certificates, 256 roots, 1 MiB per certificate file, 8 MiB total
input/archive bytes, 1024 directory/archive entries, directory depth 16, 1024-byte
challenge and 512 KiB revocation snapshot. Symlink inputs are rejected.
"""
}
