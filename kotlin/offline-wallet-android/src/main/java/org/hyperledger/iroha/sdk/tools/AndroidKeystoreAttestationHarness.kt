package org.hyperledger.iroha.sdk.tools

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.cert.CertificateException
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.util.LinkedHashSet
import java.util.Locale
import java.util.zip.ZipInputStream

/**
 * Command-line harness that validates Android Keystore attestation bundles.
 *
 * Attestation verification requires the `android_crypto` module which provides
 * `AttestationVerifier` and `KeyAttestation`. This harness exposes certificate chain loading
 * and trust root management so integrators can feed chains into their own verification pipeline.
 */
object AndroidKeystoreAttestationHarness {

    /** Summary of the attestation verification. */
    class Result(
        @JvmField val alias: String,
        @JvmField val attestationSecurityLevel: String,
        @JvmField val keymasterSecurityLevel: String,
        @JvmField val strongBoxAttestation: Boolean,
        @JvmField val challengeHex: String,
        @JvmField val chainLength: Int,
    ) {
        fun toJson(): String = buildString {
            append("{\n")
            append("  \"alias\": \"${escapeJson(alias)}\",\n")
            append("  \"attestation_security_level\": \"$attestationSecurityLevel\",\n")
            append("  \"keymaster_security_level\": \"$keymasterSecurityLevel\",\n")
            append("  \"strongbox_attestation\": $strongBoxAttestation,\n")
            append("  \"challenge_hex\": \"$challengeHex\",\n")
            append("  \"chain_length\": $chainLength\n")
            append("}\n")
        }
    }

    @JvmStatic
    fun main(args: Array<String>) {
        System.err.println("[attestation] This harness requires the android_crypto module for full verification.")
        System.err.println("[attestation] Use loadCertificateChain and loadTrustedRoots to prepare inputs.")
        System.exit(1)
    }

    /** Loads a certificate chain from the provided arguments for external verification. */
    @JvmStatic
    @Throws(IOException::class, CertificateException::class)
    fun loadCertificateChain(arguments: Arguments): List<X509Certificate> {
        if (arguments.chainFile != null) {
            return readCertificates(arguments.chainFile)
        }
        val bundleDir = checkNotNull(arguments.bundleDir) { "bundle directory must be specified" }
        val chainPem = bundleDir.resolve("chain.pem")
        return if (Files.isRegularFile(chainPem)) {
            readCertificates(chainPem)
        } else {
            readCertificatesFromDirectory(bundleDir)
        }
    }

    /** Loads trusted root certificates from the provided paths. */
    @JvmStatic
    @Throws(IOException::class, CertificateException::class)
    fun loadTrustedRoots(
        rootPaths: List<Path>,
        rootDirectories: List<Path>,
        rootBundles: List<Path>,
    ): Set<X509Certificate> {
        require(rootPaths.isNotEmpty() || rootDirectories.isNotEmpty() || rootBundles.isNotEmpty()) {
            "At least one --trust-root, --trust-root-dir, or --trust-root-bundle must be supplied"
        }
        val roots = LinkedHashSet<X509Certificate>()
        for (path in rootPaths) {
            require(Files.isRegularFile(path)) { "--trust-root does not refer to a file: $path" }
            roots.addAll(readCertificates(path))
        }
        for (directory in rootDirectories) {
            require(Files.isDirectory(directory)) { "--trust-root-dir does not refer to a directory: $directory" }
            collectCertificatesFromDirectory(directory, roots)
        }
        for (bundle in rootBundles) {
            loadCertificatesFromBundle(bundle, roots)
        }
        require(roots.isNotEmpty()) {
            "No trust roots were loaded; verify the provided inputs contain certificates."
        }
        return roots
    }

    private fun readCertificatesFromDirectory(directory: Path): List<X509Certificate> {
        val candidates = mutableListOf<Path>()
        Files.newDirectoryStream(directory).use { stream ->
            for (path in stream) {
                if (!Files.isRegularFile(path)) continue
                val lower = path.fileName.toString().lowercase(Locale.US)
                if (lower.endsWith(".pem") || lower.endsWith(".crt") || lower.endsWith(".cer") || lower.endsWith(".der")) {
                    candidates.add(path)
                }
            }
        }
        candidates.sort()
        return candidates.flatMap { readCertificates(it) }
    }

    private fun readCertificates(file: Path): List<X509Certificate> {
        val bytes = Files.newInputStream(file).use { it.readAllBytes() }
        return readCertificates(bytes)
    }

    private fun readCertificates(data: ByteArray): List<X509Certificate> {
        val factory = CertificateFactory.getInstance("X.509")
        try {
            ByteArrayInputStream(data).use { input ->
                val decoded = factory.generateCertificates(input)
                val certificates = decoded.map { it as X509Certificate }
                if (certificates.isNotEmpty()) return certificates
            }
        } catch (_: CertificateException) {
            // Fall back to single-certificate decoding below.
        }
        ByteArrayInputStream(data).use { single ->
            val certificate = factory.generateCertificate(single) as X509Certificate
            return listOf(certificate)
        }
    }

    private fun loadCertificatesFromBundle(bundle: Path, roots: MutableSet<X509Certificate>) {
        require(Files.isRegularFile(bundle)) { "--trust-root-bundle does not refer to a file: $bundle" }
        val filename = bundle.fileName.toString().lowercase(Locale.US)
        require(filename.endsWith(".zip")) { "--trust-root-bundle must reference a .zip archive: $bundle" }
        var added = false
        ZipInputStream(Files.newInputStream(bundle)).use { zip ->
            var entry = zip.nextEntry
            while (entry != null) {
                if (!entry.isDirectory && isCertificateFilename(entry.name)) {
                    val buffer = ByteArrayOutputStream()
                    zip.transferTo(buffer)
                    roots.addAll(readCertificates(buffer.toByteArray()))
                    added = true
                }
                entry = zip.nextEntry
            }
        }
        require(added) { "No certificates were found inside bundle: ${bundle.toAbsolutePath()}" }
    }

    private fun collectCertificatesFromDirectory(directory: Path, roots: MutableSet<X509Certificate>) {
        Files.newDirectoryStream(directory).use { entries ->
            for (entry in entries) {
                when {
                    Files.isDirectory(entry) -> collectCertificatesFromDirectory(entry, roots)
                    isCertificateFilename(entry.fileName.toString()) -> roots.addAll(readCertificates(entry))
                    isZipFilename(entry.fileName.toString()) -> loadCertificatesFromBundle(entry, roots)
                }
            }
        }
    }

    private fun isCertificateFilename(filename: String): Boolean {
        val lower = filename.lowercase(Locale.US)
        return lower.endsWith(".pem") || lower.endsWith(".der") || lower.endsWith(".crt") || lower.endsWith(".cer")
    }

    private fun isZipFilename(filename: String): Boolean =
        filename.lowercase(Locale.US).endsWith(".zip")

    private fun parseHex(value: String?): ByteArray? {
        if (value.isNullOrEmpty()) return null
        val normalized = value.replace("\\s+".toRegex(), "")
        require(normalized.length % 2 == 0) { "Challenge hex must contain an even number of digits" }
        return ByteArray(normalized.length / 2) { i ->
            Integer.parseInt(normalized.substring(i * 2, i * 2 + 2), 16).toByte()
        }
    }

    private fun toHex(data: ByteArray?): String {
        if (data == null || data.isEmpty()) return ""
        return data.joinToString("") { "%02X".format(it) }
    }

    private fun escapeJson(value: String): String = buildString {
        for (ch in value) {
            when (ch) {
                '"' -> append("\\\"")
                '\\' -> append("\\\\")
                '\b' -> append("\\b")
                '\u000C' -> append("\\f")
                '\n' -> append("\\n")
                '\r' -> append("\\r")
                '\t' -> append("\\t")
                else -> if (ch < '\u0020') append("\\u%04X".format(ch.code)) else append(ch)
            }
        }
    }

    class Arguments(
        val bundleDir: Path?,
        val chainFile: Path?,
        val trustedRoots: List<Path>,
        val trustedRootDirs: List<Path>,
        val trustedRootBundles: List<Path>,
        val requireStrongBox: Boolean,
        val alias: String,
        val challenge: ByteArray?,
        val output: Path?,
    ) {
        companion object {
            @Throws(IOException::class)
            fun parse(args: Array<String>): Arguments {
                var bundleDir: Path? = null
                var chainFile: Path? = null
                val trustedRoots = mutableListOf<Path>()
                val trustedRootDirs = mutableListOf<Path>()
                val trustedRootBundles = mutableListOf<Path>()
                var requireStrongBox = false
                var alias = "android-keystore-alias"
                var challenge: ByteArray? = null
                var output: Path? = null

                var i = 0
                while (i < args.size) {
                    when (args[i]) {
                        "--bundle-dir" -> bundleDir = Paths.get(requireValue(args, ++i, "--bundle-dir"))
                        "--chain" -> chainFile = Paths.get(requireValue(args, ++i, "--chain"))
                        "--trust-root" -> trustedRoots.add(Paths.get(requireValue(args, ++i, "--trust-root")))
                        "--trust-root-dir" -> trustedRootDirs.add(Paths.get(requireValue(args, ++i, "--trust-root-dir")))
                        "--trust-root-bundle" -> trustedRootBundles.add(Paths.get(requireValue(args, ++i, "--trust-root-bundle")))
                        "--require-strongbox" -> requireStrongBox = true
                        "--alias" -> alias = requireValue(args, ++i, "--alias")
                        "--challenge-hex" -> challenge = parseHex(requireValue(args, ++i, "--challenge-hex"))
                        "--challenge-file" -> challenge = parseHex(
                            String(Files.readAllBytes(Paths.get(requireValue(args, ++i, "--challenge-file")))).trim()
                        )
                        "--output" -> output = Paths.get(requireValue(args, ++i, "--output"))
                        "--help", "-h" -> throw IllegalArgumentException(usage())
                        else -> throw IllegalArgumentException("Unknown argument: ${args[i]}")
                    }
                    i++
                }

                require(chainFile != null || bundleDir != null) {
                    "Either --chain or --bundle-dir must be provided (bundle-dir is recommended)."
                }

                if (bundleDir != null) {
                    require(Files.isDirectory(bundleDir)) { "--bundle-dir does not refer to a directory: $bundleDir" }
                    if (challenge == null) {
                        val challengeFile = bundleDir.resolve("challenge.hex")
                        if (Files.isRegularFile(challengeFile)) {
                            challenge = parseHex(String(Files.readAllBytes(challengeFile)).trim())
                        }
                    }
                    val aliasFile = bundleDir.resolve("alias.txt")
                    if (Files.isRegularFile(aliasFile)) {
                        alias = String(Files.readAllBytes(aliasFile)).trim()
                    }
                    collectTrustRootsFromBundle(bundleDir, trustedRoots, trustedRootBundles)
                }

                if (challenge == null) challenge = ByteArray(0)

                return Arguments(
                    bundleDir = bundleDir?.toAbsolutePath(),
                    chainFile = chainFile?.toAbsolutePath(),
                    trustedRoots = trustedRoots.map { it.toAbsolutePath() },
                    trustedRootDirs = trustedRootDirs.map { it.toAbsolutePath() },
                    trustedRootBundles = trustedRootBundles.map { it.toAbsolutePath() },
                    requireStrongBox = requireStrongBox,
                    alias = alias,
                    challenge = challenge,
                    output = output?.toAbsolutePath(),
                )
            }

            private fun requireValue(args: Array<String>, index: Int, flag: String): String {
                require(index < args.size) { "$flag requires a value" }
                return args[index]
            }

            private fun usage(): String = listOf(
                "Usage: android_keystore_attestation --bundle-dir <path> --trust-root <root.pem> [options]",
                "",
                "Required:",
                "  --bundle-dir <path>      Directory containing chain.pem/alias.txt/challenge.hex.",
                "  --trust-root <path>      Trusted root certificate (PEM/DER). Repeat as needed.",
                "",
                "Optional:",
                "  --trust-root-dir <path>  Directory containing PEM/DER/CRT trust anchors (recursively scanned).",
                "  --trust-root-bundle <zip>  ZIP archive containing trusted roots. Repeat as needed.",
                "  --chain <path>           Explicit attestation chain file (PEM/DER). Overrides bundle.",
                "  --alias <alias>          Override alias (falls back to alias.txt).",
                "  --challenge-hex <hex>    Verification challenge (hex encoded).",
                "  --challenge-file <path>  File containing hex-encoded challenge.",
                "  --require-strongbox      Enforce StrongBox attestation.",
                "  --output <path>          Write result JSON to <path>.",
                "  --help                   Print this message.",
            ).joinToString(System.lineSeparator())

            @Throws(IOException::class)
            private fun collectTrustRootsFromBundle(
                bundleDir: Path,
                trustedRoots: MutableList<Path>,
                trustedRootBundles: MutableList<Path>,
            ) {
                Files.newDirectoryStream(bundleDir).use { entries ->
                    for (entry in entries) {
                        if (!Files.isRegularFile(entry)) continue
                        val filename = entry.fileName.toString()
                        val lower = filename.lowercase(Locale.US)
                        when {
                            lower.startsWith("trust_root_bundle_") && lower.endsWith(".zip") ->
                                trustedRootBundles.add(entry)
                            lower.startsWith("trust_root_") && isCertificateFilename(lower) ->
                                trustedRoots.add(entry)
                        }
                    }
                }
            }
        }
    }
}
