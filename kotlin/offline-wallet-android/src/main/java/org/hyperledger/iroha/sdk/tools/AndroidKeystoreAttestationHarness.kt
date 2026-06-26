package org.hyperledger.iroha.sdk.tools

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.IOException
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
        val chainPem = File(bundleDir, "chain.pem")
        return if (chainPem.isFile) {
            readCertificates(chainPem)
        } else {
            readCertificatesFromDirectory(bundleDir)
        }
    }

    /** Loads trusted root certificates from the provided paths. */
    @JvmStatic
    @Throws(IOException::class, CertificateException::class)
    fun loadTrustedRoots(
        rootPaths: List<File>,
        rootDirectories: List<File>,
        rootBundles: List<File>,
    ): Set<X509Certificate> {
        require(rootPaths.isNotEmpty() || rootDirectories.isNotEmpty() || rootBundles.isNotEmpty()) {
            "At least one --trust-root, --trust-root-dir, or --trust-root-bundle must be supplied"
        }
        val roots = LinkedHashSet<X509Certificate>()
        for (path in rootPaths) {
            require(path.isFile) { "--trust-root does not refer to a file: $path" }
            roots.addAll(readCertificates(path))
        }
        for (directory in rootDirectories) {
            require(directory.isDirectory) { "--trust-root-dir does not refer to a directory: $directory" }
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

    private fun readCertificatesFromDirectory(directory: File): List<X509Certificate> {
        val candidates = mutableListOf<File>()
        for (file in listFilesOrThrow(directory)) {
            if (!file.isFile) continue
            val lower = file.name.lowercase(Locale.US)
            if (lower.endsWith(".pem") || lower.endsWith(".crt") || lower.endsWith(".cer") || lower.endsWith(".der")) {
                candidates.add(file)
            }
        }
        candidates.sort()
        return candidates.flatMap { readCertificates(it) }
    }

    private fun readCertificates(file: File): List<X509Certificate> {
        val bytes = file.inputStream().use { it.readBytes() }
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
            // Decode as a single certificate below.
        }
        ByteArrayInputStream(data).use { single ->
            val certificate = factory.generateCertificate(single) as X509Certificate
            return listOf(certificate)
        }
    }

    private fun loadCertificatesFromBundle(bundle: File, roots: MutableSet<X509Certificate>) {
        require(bundle.isFile) { "--trust-root-bundle does not refer to a file: $bundle" }
        val filename = bundle.name.lowercase(Locale.US)
        require(filename.endsWith(".zip")) { "--trust-root-bundle must reference a .zip archive: $bundle" }
        var added = false
        ZipInputStream(bundle.inputStream()).use { zip ->
            var entry = zip.nextEntry
            while (entry != null) {
                if (!entry.isDirectory && isCertificateFilename(entry.name)) {
                    val buffer = ByteArrayOutputStream()
                    zip.copyTo(buffer)
                    roots.addAll(readCertificates(buffer.toByteArray()))
                    added = true
                }
                entry = zip.nextEntry
            }
        }
        require(added) { "No certificates were found inside bundle: ${bundle.absolutePath}" }
    }

    private fun collectCertificatesFromDirectory(directory: File, roots: MutableSet<X509Certificate>) {
        for (entry in listFilesOrThrow(directory)) {
            when {
                entry.isDirectory -> collectCertificatesFromDirectory(entry, roots)
                isCertificateFilename(entry.name) -> roots.addAll(readCertificates(entry))
                isZipFilename(entry.name) -> loadCertificatesFromBundle(entry, roots)
            }
        }
    }

    private fun isCertificateFilename(filename: String): Boolean {
        val lower = filename.lowercase(Locale.US)
        return lower.endsWith(".pem") || lower.endsWith(".der") || lower.endsWith(".crt") || lower.endsWith(".cer")
    }

    private fun isZipFilename(filename: String): Boolean =
        filename.lowercase(Locale.US).endsWith(".zip")

    private fun listFilesOrThrow(directory: File): Array<File> =
        directory.listFiles() ?: throw IOException("Unable to list directory: ${directory.absolutePath}")

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
        val bundleDir: File?,
        val chainFile: File?,
        val trustedRoots: List<File>,
        val trustedRootDirs: List<File>,
        val trustedRootBundles: List<File>,
        val requireStrongBox: Boolean,
        val alias: String,
        val challenge: ByteArray?,
        val output: File?,
    ) {
        companion object {
            @Throws(IOException::class)
            fun parse(args: Array<String>): Arguments {
                var bundleDir: File? = null
                var chainFile: File? = null
                val trustedRoots = mutableListOf<File>()
                val trustedRootDirs = mutableListOf<File>()
                val trustedRootBundles = mutableListOf<File>()
                var requireStrongBox = false
                var alias = "android-keystore-alias"
                var challenge: ByteArray? = null
                var output: File? = null

                var i = 0
                while (i < args.size) {
                    when (args[i]) {
                        "--bundle-dir" -> bundleDir = File(requireValue(args, ++i, "--bundle-dir"))
                        "--chain" -> chainFile = File(requireValue(args, ++i, "--chain"))
                        "--trust-root" -> trustedRoots.add(File(requireValue(args, ++i, "--trust-root")))
                        "--trust-root-dir" -> trustedRootDirs.add(File(requireValue(args, ++i, "--trust-root-dir")))
                        "--trust-root-bundle" -> trustedRootBundles.add(File(requireValue(args, ++i, "--trust-root-bundle")))
                        "--require-strongbox" -> requireStrongBox = true
                        "--alias" -> alias = requireValue(args, ++i, "--alias")
                        "--challenge-hex" -> challenge = parseHex(requireValue(args, ++i, "--challenge-hex"))
                        "--challenge-file" -> challenge = parseHex(
                            File(requireValue(args, ++i, "--challenge-file")).readText().trim()
                        )
                        "--output" -> output = File(requireValue(args, ++i, "--output"))
                        "--help", "-h" -> throw IllegalArgumentException(usage())
                        else -> throw IllegalArgumentException("Unknown argument: ${args[i]}")
                    }
                    i++
                }

                require(chainFile != null || bundleDir != null) {
                    "Either --chain or --bundle-dir must be provided (bundle-dir is recommended)."
                }

                if (bundleDir != null) {
                    require(bundleDir.isDirectory) { "--bundle-dir does not refer to a directory: $bundleDir" }
                    if (challenge == null) {
                        val challengeFile = File(bundleDir, "challenge.hex")
                        if (challengeFile.isFile) {
                            challenge = parseHex(challengeFile.readText().trim())
                        }
                    }
                    val aliasFile = File(bundleDir, "alias.txt")
                    if (aliasFile.isFile) {
                        alias = aliasFile.readText().trim()
                    }
                    collectTrustRootsFromBundle(bundleDir, trustedRoots, trustedRootBundles)
                }

                if (challenge == null) challenge = ByteArray(0)

                return Arguments(
                    bundleDir = bundleDir?.absoluteFile,
                    chainFile = chainFile?.absoluteFile,
                    trustedRoots = trustedRoots.map { it.absoluteFile },
                    trustedRootDirs = trustedRootDirs.map { it.absoluteFile },
                    trustedRootBundles = trustedRootBundles.map { it.absoluteFile },
                    requireStrongBox = requireStrongBox,
                    alias = alias,
                    challenge = challenge,
                    output = output?.absoluteFile,
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
                "  --alias <alias>          Override alias from alias.txt.",
                "  --challenge-hex <hex>    Verification challenge (hex encoded).",
                "  --challenge-file <path>  File containing hex-encoded challenge.",
                "  --require-strongbox      Enforce StrongBox attestation.",
                "  --output <path>          Write result JSON to <path>.",
                "  --help                   Print this message.",
            ).joinToString(System.lineSeparator())

            @Throws(IOException::class)
            private fun collectTrustRootsFromBundle(
                bundleDir: File,
                trustedRoots: MutableList<File>,
                trustedRootBundles: MutableList<File>,
            ) {
                for (entry in listFilesOrThrow(bundleDir)) {
                    if (!entry.isFile) continue
                    val lower = entry.name.lowercase(Locale.US)
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
