package org.hyperledger.iroha.sdk.tools

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.Path
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.util.Locale
import java.util.zip.ZipInputStream

/** Bounded command-owned evidence reader; archives are decoded in memory and never extracted. */
internal class AttestationInputs {
    private var remainingBytes = 8 * 1024 * 1024
    private var remainingEntries = 1024
    private val inputFiles = linkedSetOf<Path>()

    fun readFile(path: Path, limit: Int): ByteArray {
        require(Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)) { "Expected a regular non-symlink file: $path" }
        inputFiles.add(path.toRealPath())
        return Files.newInputStream(path, LinkOption.NOFOLLOW_LINKS).use { readBounded(it, limit) }
    }

    fun requireDistinctOutput(path: Path) {
        require(inputFiles.none { it == path || (Files.exists(path) && Files.isSameFile(it, path)) }) {
            "--output must not replace an evidence or trusted input file"
        }
    }

    fun chain(source: Path, directory: Boolean): List<X509Certificate> {
        val certificates = mutableListOf<X509Certificate>()
        if (!directory) {
            certificates.addAll(certificates(readFile(source, 1024 * 1024)))
        } else {
            require(Files.isDirectory(source, LinkOption.NOFOLLOW_LINKS)) { "Expected a non-symlink evidence directory: $source" }
            val pem = source.resolve("chain.pem")
            if (Files.exists(pem, LinkOption.NOFOLLOW_LINKS)) {
                certificates.addAll(certificates(readFile(pem, 1024 * 1024)))
            } else {
                for (entry in entries(source)) {
                    if (isCertificate(entry.fileName.toString())) {
                        certificates.addAll(certificates(readFile(entry, 1024 * 1024)))
                        require(certificates.size <= 16) { "Attestation chain exceeds 16 certificates" }
                    }
                }
            }
        }
        require(certificates.isNotEmpty() && certificates.size <= 16) { "Attestation chain must contain 1..16 certificates" }
        return certificates
    }

    fun trustedRoots(files: List<Path>, directories: List<Path>, bundles: List<Path>): Set<X509Certificate> {
        val roots = linkedSetOf<X509Certificate>()
        fun add(data: ByteArray) {
            roots.addAll(certificates(data))
            require(roots.size <= 256) { "Trust root set exceeds 256 certificates" }
        }
        fun bundle(path: Path) {
            require(path.fileName.toString().lowercase(Locale.ROOT).endsWith(".zip")) { "Trusted root bundle must be a ZIP" }
            var found = false
            ZipInputStream(ByteArrayInputStream(readFile(path, 8 * 1024 * 1024))).use { zip ->
                while (true) {
                    val entry = zip.nextEntry ?: break
                    admitEntry()
                    require(entry.name.length <= 4096) { "ZIP entry name exceeds bounds" }
                    // Charge every decompressed entry, including ignored files, so skipping
                    // metadata cannot turn the reader into an unbounded ZIP decompressor.
                    val content = readBounded(zip, 1024 * 1024)
                    if (!entry.isDirectory && isCertificate(entry.name)) {
                        add(content)
                        found = true
                    }
                    zip.closeEntry()
                }
            }
            require(found) { "No certificates in trusted ZIP: $path" }
        }
        fun directory(path: Path, depth: Int) {
            require(depth <= 16) { "Trusted root directory exceeds depth 16" }
            require(Files.isDirectory(path, LinkOption.NOFOLLOW_LINKS)) { "Expected a non-symlink trust directory: $path" }
            for (entry in entries(path)) {
                require(!Files.isSymbolicLink(entry)) { "Symlink trust entries are forbidden: $entry" }
                when {
                    Files.isDirectory(entry, LinkOption.NOFOLLOW_LINKS) -> directory(entry, depth + 1)
                    isCertificate(entry.fileName.toString()) -> add(readFile(entry, 1024 * 1024))
                    entry.fileName.toString().lowercase(Locale.ROOT).endsWith(".zip") -> bundle(entry)
                }
            }
        }
        files.forEach { add(readFile(it, 1024 * 1024)) }
        directories.forEach { directory(it, 0) }
        bundles.forEach(::bundle)
        require(roots.isNotEmpty()) { "No independently trusted root certificates were loaded" }
        return roots
    }

    private fun entries(directory: Path): List<Path> {
        val paths = mutableListOf<Path>()
        Files.newDirectoryStream(directory).use { entries ->
            for (entry in entries) {
                admitEntry()
                paths.add(entry)
            }
        }
        return paths.sorted()
    }

    private fun admitEntry() {
        require(remainingEntries > 0) { "Directory/ZIP inputs exceed 1024 entries" }
        remainingEntries--
    }

    internal fun readBounded(input: InputStream, limit: Int): ByteArray {
        val output = ByteArrayOutputStream()
        val buffer = ByteArray(8192)
        while (true) {
            val count = input.read(buffer)
            if (count < 0) break
            require(count > 0) { "Attestation input stream made no progress" }
            require(count <= limit - output.size() && count <= remainingBytes) { "Attestation input exceeds byte bounds" }
            remainingBytes -= count
            output.write(buffer, 0, count)
        }
        return output.toByteArray()
    }

    private fun certificates(data: ByteArray): List<X509Certificate> {
        val decoded = CertificateFactory.getInstance("X.509").generateCertificates(ByteArrayInputStream(data))
        require(decoded.isNotEmpty() && decoded.size <= 256) { "Certificate file must contain 1..256 certificates" }
        return decoded.map { it as X509Certificate }
    }

    private fun isCertificate(name: String): Boolean = name.lowercase(Locale.ROOT).let {
        it.endsWith(".pem") || it.endsWith(".crt") || it.endsWith(".cer") || it.endsWith(".der")
    }
}
