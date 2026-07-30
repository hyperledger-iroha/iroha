package org.hyperledger.iroha.sdk.core.model.instructions

/**
 * Runs the Rust `kotlin-fixture-gen` binary and returns its stdout lines.
 *
 * The binary lives at `tools/kotlin-fixture-gen/` in the Iroha repo root.
 * Set `IROHA_KOTLIN_FIXTURE_GEN_BIN` to use a prebuilt executable instead of
 * invoking Cargo. Relative override paths are resolved from the repository
 * root, and the override must name an executable file.
 *
 * Without a prebuilt override, this helper builds the generator once per
 * resolved target directory in each test JVM. It uses `${CARGO:-cargo}`, honors
 * `CARGO_TARGET_DIR` (falling back to `target/kotlin-fixture-gen-test`), always
 * passes `--locked --jobs 1`, and honors `CARGO_NET_OFFLINE=true` or `1`.
 * `IROHA_KOTLIN_FIXTURE_GEN_LOCKFILE_PATH` may select an existing absolute
 * lockfile path for Cargo's unstable `--lockfile-path` option.
 */
internal object FixtureGeneratorRunner {
    private val buildLock = Any()
    private val builtTargetDirectories = mutableSetOf<String>()

    fun run(subcommand: String): List<String> {
        val repoRoot = locateRepoRoot()
        val configuration = configurationFor(repoRoot, System.getenv())
        val binary = when (configuration) {
            is FixtureGeneratorConfiguration.Prebuilt -> {
                requireExecutable(
                    configuration.binary,
                    "IROHA_KOTLIN_FIXTURE_GEN_BIN",
                )
                configuration.binary
            }
            is FixtureGeneratorConfiguration.CargoBuild -> {
                ensureCargoBuild(repoRoot, configuration)
                configuration.binary
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

    /**
     * Resolves the runner configuration without reading the filesystem or
     * starting a process. Kept internal so its environment policy can be unit
     * tested without Cargo.
     */
    internal fun configurationFor(
        repoRoot: java.io.File,
        environment: Map<String, String>,
    ): FixtureGeneratorConfiguration {
        val configuredBinary = environment["IROHA_KOTLIN_FIXTURE_GEN_BIN"]
            ?.takeUnless { it.isBlank() }
        if (configuredBinary != null) {
            return FixtureGeneratorConfiguration.Prebuilt(
                resolvePathAgainstRepoRoot(repoRoot, configuredBinary),
            )
        }

        val targetDirectory = environment["CARGO_TARGET_DIR"]
            ?.takeUnless { it.isBlank() }
            ?.let { resolvePathAgainstRepoRoot(repoRoot, it) }
            ?: java.io.File(repoRoot, "target/kotlin-fixture-gen-test").absoluteFile
        val cargo = environment["CARGO"]?.takeUnless { it.isEmpty() } ?: "cargo"
        val offline = when (val configuredOffline = environment["CARGO_NET_OFFLINE"]) {
            null, "" -> false
            "true", "1" -> true
            else -> error(
                "CARGO_NET_OFFLINE must be `true` or `1` when set; was `$configuredOffline`",
            )
        }
        val lockfilePath = environment["IROHA_KOTLIN_FIXTURE_GEN_LOCKFILE_PATH"]
            ?.takeUnless { it.isBlank() }
            ?.let { configuredPath ->
                val lockfile = java.io.File(configuredPath)
                require(lockfile.isAbsolute) {
                    "IROHA_KOTLIN_FIXTURE_GEN_LOCKFILE_PATH must be an absolute path: $configuredPath"
                }
                lockfile.absoluteFile
            }

        return FixtureGeneratorConfiguration.CargoBuild(
            cargo = cargo,
            targetDirectory = targetDirectory,
            offline = offline,
            lockfilePath = lockfilePath,
        )
    }

    private fun ensureCargoBuild(
        repoRoot: java.io.File,
        configuration: FixtureGeneratorConfiguration.CargoBuild,
    ) {
        synchronized(buildLock) {
            val targetKey = configuration.targetDirectory.absolutePath
            if (targetKey !in builtTargetDirectories ||
                !configuration.binary.isFile ||
                !configuration.binary.canExecute()
            ) {
                withCargoBuildLock(configuration.targetDirectory) {
                    validateLockfilePath(configuration.lockfilePath)
                    // Always ask Cargo to refresh the generator once per test JVM.
                    // Merely finding a previous binary can otherwise compare the SDK
                    // against stale Rust wire types after an ABI cutover.
                    buildFixtureGenerator(repoRoot, configuration)
                    requireExecutable(configuration.binary, "cargo build output")
                    builtTargetDirectories += targetKey
                }
            }
        }
    }

    private fun withCargoBuildLock(targetDirectory: java.io.File, action: () -> Unit) {
        val lockFile = java.io.File(targetDirectory, ".kotlin-fixture-gen.lock")
        lockFile.parentFile.mkdirs()
        java.io.RandomAccessFile(lockFile, "rw").channel.use { channel ->
            channel.lock().use {
                action()
            }
        }
    }

    private fun buildFixtureGenerator(
        repoRoot: java.io.File,
        configuration: FixtureGeneratorConfiguration.CargoBuild,
    ) {
        val build = ProcessBuilder(configuration.command())
            .directory(repoRoot)
            .apply {
                environment()["CARGO_TARGET_DIR"] = configuration.targetDirectory.absolutePath
            }
            .redirectErrorStream(true)
            .start()
        val buildOutput = build.inputStream.bufferedReader().readText()
        val buildExit = build.waitFor()
        require(buildExit == 0) {
            "cargo build failed (exit $buildExit): $buildOutput"
        }
    }

    internal fun cargoBuildCommand(): List<String> =
        listOf("cargo", "build", "--locked", "--offline", "-p", "kotlin-fixture-gen")

    private fun validateLockfilePath(lockfilePath: java.io.File?) {
        if (lockfilePath != null) {
            require(lockfilePath.isFile) {
                "IROHA_KOTLIN_FIXTURE_GEN_LOCKFILE_PATH must name an existing regular file: " +
                    lockfilePath.absolutePath
            }
        }
    }

    private fun requireExecutable(binary: java.io.File, source: String) {
        require(binary.isFile) {
            "$source does not point to a regular kotlin-fixture-gen file: ${binary.absolutePath}"
        }
        require(binary.canExecute()) {
            "$source is not executable: ${binary.absolutePath}"
        }
    }

    private fun resolvePathAgainstRepoRoot(repoRoot: java.io.File, configuredPath: String): java.io.File {
        val path = java.io.File(configuredPath)
        return (if (path.isAbsolute) path else java.io.File(repoRoot, configuredPath)).absoluteFile
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

/** Test-only configuration for [FixtureGeneratorRunner]. */
internal sealed class FixtureGeneratorConfiguration {
    /** A caller-supplied executable; Cargo must not be invoked. */
    internal class Prebuilt(
        val binary: java.io.File,
    ) : FixtureGeneratorConfiguration()

    /** Cargo invocation and output location for a locally built generator. */
    internal class CargoBuild(
        private val cargo: String,
        val targetDirectory: java.io.File,
        private val offline: Boolean,
        val lockfilePath: java.io.File?,
    ) : FixtureGeneratorConfiguration() {
        val binary: java.io.File = java.io.File(targetDirectory, "debug/kotlin-fixture-gen")

        /** Returns the deterministic Cargo command for this configuration. */
        fun command(): List<String> = buildList {
            add(cargo)
            add("build")
            add("-p")
            add("kotlin-fixture-gen")
            add("--locked")
            add("--jobs")
            add("1")
            if (offline) {
                add("--offline")
            }
            if (lockfilePath != null) {
                add("-Z")
                add("unstable-options")
                add("--lockfile-path")
                add(lockfilePath.absolutePath)
            }
        }
    }
}
