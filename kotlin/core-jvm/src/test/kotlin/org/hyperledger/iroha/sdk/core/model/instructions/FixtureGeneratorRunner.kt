package org.hyperledger.iroha.sdk.core.model.instructions

/**
 * Runs the Rust `kotlin-fixture-gen` binary and returns its stdout lines.
 *
 * The binary lives at `tools/kotlin-fixture-gen/` in the Iroha repo root.
 * If the binary is not built yet, this helper builds it automatically.
 */
internal object FixtureGeneratorRunner {

    fun run(subcommand: String): List<String> {
        val repoRoot = locateRepoRoot()
        val binary = java.io.File(repoRoot, "target/debug/kotlin-fixture-gen")
        if (!binary.exists()) {
            val build = ProcessBuilder("cargo", "build", "-p", "kotlin-fixture-gen")
                .directory(repoRoot)
                .redirectErrorStream(true)
                .start()
            val buildExit = build.waitFor()
            require(buildExit == 0) {
                "cargo build failed (exit $buildExit): ${build.inputStream.bufferedReader().readText()}"
            }
        }
        val process = ProcessBuilder(binary.absolutePath, subcommand)
            .directory(repoRoot)
            .redirectErrorStream(false)
            .start()
        val stdout = process.inputStream.bufferedReader().readText().trim()
        val exitCode = process.waitFor()
        require(exitCode == 0) { "kotlin-fixture-gen $subcommand failed (exit $exitCode)" }
        require(stdout.isNotBlank()) { "kotlin-fixture-gen $subcommand produced empty output" }
        return stdout.lines()
    }

    private fun locateRepoRoot(): java.io.File {
        var dir = java.io.File("").absoluteFile
        while (!java.io.File(dir, "Cargo.toml").exists()) {
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
