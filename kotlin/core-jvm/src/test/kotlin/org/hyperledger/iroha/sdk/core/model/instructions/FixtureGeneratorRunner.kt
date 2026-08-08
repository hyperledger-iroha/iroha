package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.concurrent.Executors

/**
 * Runs an explicitly supplied Rust `kotlin-fixture-gen` executable.
 *
 * Set `IROHA_KOTLIN_FIXTURE_GEN_BIN` to an existing executable file. Relative
 * paths are resolved from the Iroha repository root. The parity tests never
 * invoke Cargo or build the generator themselves.
 */
internal object FixtureGeneratorRunner {
    internal const val BINARY_ENVIRONMENT_VARIABLE = "IROHA_KOTLIN_FIXTURE_GEN_BIN"

    fun run(subcommand: String): List<String> {
        val repoRoot = locateRepoRoot()
        val binary = executableFor(repoRoot, System.getenv())
        val process = ProcessBuilder(commandFor(binary, subcommand))
            .directory(repoRoot)
            .redirectErrorStream(false)
            .start()

        val readers = Executors.newFixedThreadPool(2)
        val stdout = readers.submit<String> {
            process.inputStream.bufferedReader().use { it.readText() }
        }
        val stderr = readers.submit<String> {
            process.errorStream.bufferedReader().use { it.readText() }
        }
        return try {
            val exitCode = process.waitFor()
            outputLines(subcommand, exitCode, stdout.get(), stderr.get())
        } catch (error: InterruptedException) {
            process.destroyForcibly()
            Thread.currentThread().interrupt()
            throw error
        } finally {
            readers.shutdownNow()
        }
    }

    /** Resolves and validates the sole fixture-generator configuration. */
    internal fun executableFor(
        repoRoot: java.io.File,
        environment: Map<String, String>,
    ): java.io.File {
        val configuredPath = environment[BINARY_ENVIRONMENT_VARIABLE]
            ?: throw IllegalArgumentException(
                "$BINARY_ENVIRONMENT_VARIABLE must be set to the kotlin-fixture-gen executable",
            )
        require(configuredPath.isNotBlank()) {
            "$BINARY_ENVIRONMENT_VARIABLE must not be blank"
        }

        val binary = resolvePathAgainstRepoRoot(repoRoot, configuredPath)
        require(binary.isFile) {
            "$BINARY_ENVIRONMENT_VARIABLE must name an existing regular file: ${binary.absolutePath}"
        }
        require(binary.canExecute()) {
            "$BINARY_ENVIRONMENT_VARIABLE is not executable: ${binary.absolutePath}"
        }
        return binary
    }

    /** Returns the complete process command; no build command is synthesized. */
    internal fun commandFor(binary: java.io.File, subcommand: String): List<String> {
        require(subcommand.isNotBlank()) { "fixture-generator subcommand must not be blank" }
        return listOf(binary.absolutePath, subcommand)
    }

    /** Keeps diagnostics out of fixture rows and exposes them only on failure. */
    internal fun outputLines(
        subcommand: String,
        exitCode: Int,
        stdout: String,
        stderr: String,
    ): List<String> {
        val output = stdout.trim()
        val diagnostic = stderr.trim()
        require(exitCode == 0) {
            val suffix = diagnostic.takeIf { it.isNotBlank() }?.let { ": $it" }.orEmpty()
            "kotlin-fixture-gen $subcommand failed (exit $exitCode)$suffix"
        }
        require(output.isNotBlank()) { "kotlin-fixture-gen $subcommand produced empty output" }
        return output.lines()
    }

    private fun resolvePathAgainstRepoRoot(
        repoRoot: java.io.File,
        configuredPath: String,
    ): java.io.File {
        val path = java.io.File(configuredPath)
        return (if (path.isAbsolute) path else java.io.File(repoRoot, configuredPath)).absoluteFile
    }

    private fun locateRepoRoot(): java.io.File {
        var dir = java.io.File("").absoluteFile
        while (!java.io.File(dir, "Cargo.toml").isFile) {
            dir = dir.parentFile
                ?: error("Could not locate Iroha repo root (Cargo.toml) from CWD")
        }
        return dir
    }

    fun hexToBytes(hex: String): ByteArray {
        val clean = hex.lowercase()
        require(clean.length % 2 == 0) { "Hex string must have even length" }
        return ByteArray(clean.length / 2) { i ->
            val hi = Character.digit(clean[i * 2], 16)
            val lo = Character.digit(clean[i * 2 + 1], 16)
            ((hi shl 4) or lo).toByte()
        }
    }

    fun bytesToHex(bytes: ByteArray): String =
        bytes.joinToString("") { "%02x".format(it) }
}
