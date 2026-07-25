import groovy.json.JsonOutput
import groovy.json.JsonSlurper
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.LocalState
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import org.gradle.work.DisableCachingByDefault
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.security.MessageDigest
import java.util.Properties
import javax.inject.Inject

plugins {
    alias(libs.plugins.android.library)
    `maven-publish`
}

private object NativeBridgeBuildContract {
    val abis = listOf("arm64-v8a", "x86_64")
    val rustTargets = listOf("aarch64-linux-android", "x86_64-linux-android")
    const val libraryName = "libconnect_norito_bridge.so"
    const val sourceSealSchema = "iroha.norito-bridge-source-seal.v1"
    const val buildEnvironmentSchema = "iroha.mobile-native-build-environment.v1"
    const val hermeticRunnerSchema = "iroha.mobile-hermetic-command.v1"
    const val pinnedRustToolchain = "1.93.1"
    const val pinnedCargoNdkVersion = "4.1.2"
    const val pinnedAndroidNdkRevision = "28.0.12674087"
    val androidCargoEnvironmentAllowlist = listOf(
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "CARGO",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "HOME",
        "LANG",
        "LC_ALL",
        "NORITO_SKIP_BINDINGS_SYNC",
        "PATH",
        "RUSTC",
        "RUSTUP_HOME",
        "TMPDIR",
    )

    data class BuildTools(
        val home: java.io.File,
        val temporaryDirectory: java.io.File,
        val python: java.nio.file.Path,
        val git: java.nio.file.Path,
        val rustup: java.nio.file.Path,
        val cargo: java.nio.file.Path,
        val rustc: java.nio.file.Path,
        val cargoNdk: java.nio.file.Path,
        val hermeticRunner: java.nio.file.Path,
        val androidNdk: java.nio.file.Path,
        val cargoRelease: String,
        val cargoCommitHash: String,
        val rustcRelease: String,
        val rustcCommitHash: String,
        val cargoNdkVersion: String,
        val pythonVersion: String,
        val gitVersion: String,
        val rustupVersion: String,
        val androidNdkRevision: String,
        val androidNdkSourcePropertiesSha256: String,
    )

    fun sha256Hex(path: java.nio.file.Path): String {
        val digest = MessageDigest.getInstance("SHA-256")
        Files.newInputStream(path, LinkOption.NOFOLLOW_LINKS).use { input ->
            val buffer = ByteArray(1024 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { byte ->
            "%02x".format(byte.toInt() and 0xff)
        }
    }

    private fun requireExecutable(path: java.nio.file.Path, label: String): java.nio.file.Path {
        val resolved = path.toRealPath()
        require(
            Files.isRegularFile(resolved, LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(resolved) &&
                Files.isExecutable(resolved),
        ) {
            "$label must resolve to a non-symbolic regular executable: $path -> $resolved"
        }
        return resolved
    }

    private fun trustedPython(): java.nio.file.Path {
        val candidate = listOf(
            "/opt/homebrew/bin/python3",
            "/usr/local/bin/python3",
            "/usr/bin/python3",
        ).map(::File).firstOrNull { file -> file.isFile && file.canExecute() }
            ?: throw GradleException("A pinned absolute Python 3 executable is required")
        return requireExecutable(candidate.toPath(), "Python")
    }

    private fun commandOutput(
        execOperations: ExecOperations,
        workingDirectory: File,
        environment: Map<String, String>,
        command: List<String>,
        label: String,
    ): String {
        val stdout = ByteArrayOutputStream()
        val stderr = ByteArrayOutputStream()
        val result = execOperations.exec {
            workingDir(workingDirectory)
            setEnvironment(environment)
            commandLine(command)
            standardOutput = stdout
            errorOutput = stderr
            isIgnoreExitValue = true
        }
        require(result.exitValue == 0) {
            "$label failed: ${stderr.toString(Charsets.UTF_8.name()).trim()}"
        }
        return stdout.toString(Charsets.UTF_8.name()).trim()
    }

    private fun baseToolEnvironment(
        home: File,
        temporaryDirectory: File,
        path: String,
    ): Map<String, String> = linkedMapOf(
        "HOME" to home.absolutePath,
        "PATH" to path,
        "TMPDIR" to temporaryDirectory.absolutePath,
        "LANG" to "C.UTF-8",
        "LC_ALL" to "C.UTF-8",
        "RUSTUP_HOME" to home.resolve(".rustup").absolutePath,
        "CARGO_HOME" to home.resolve(".cargo").absolutePath,
    )

    fun resolveBuildTools(
        execOperations: ExecOperations,
        irohaRoot: File,
        hermeticRunnerFile: File,
        androidNdkDirectory: File,
    ): BuildTools {
        val python = trustedPython()
        val homeText = commandOutput(
            execOperations,
            irohaRoot,
            mapOf(
                "HOME" to "/",
                "PATH" to "${python.parent}:/usr/bin:/bin",
                "TMPDIR" to "/tmp",
                "LANG" to "C.UTF-8",
                "LC_ALL" to "C.UTF-8",
            ),
            listOf(
                python.toString(),
                "-I",
                "-c",
                "import os,pwd; print(pwd.getpwuid(os.getuid()).pw_dir)",
            ),
            "system account home resolution",
        )
        val home = File(homeText).toPath().toRealPath().toFile()
        val temporaryDirectory = File("/tmp").toPath().toRealPath().toFile()
        val git = requireExecutable(Path.of("/usr/bin/git"), "Git")
        val rustup = requireExecutable(home.resolve(".cargo/bin/rustup").toPath(), "rustup")
        val hermeticRunner = hermeticRunnerFile.toPath().toRealPath()
        require(
            Files.isRegularFile(hermeticRunner, LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(hermeticRunner),
        ) {
            "Hermetic command runner must be a non-symbolic regular file: $hermeticRunner"
        }
        val rustToolchainFile = irohaRoot.resolve("rust-toolchain.toml")
        require(
            Files.isRegularFile(rustToolchainFile.toPath(), LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(rustToolchainFile.toPath()),
        ) {
            "rust-toolchain.toml must be a non-symbolic regular file"
        }
        val toolchainMatches = Regex(
            """(?m)^\s*channel\s*=\s*"([^"]+)"\s*$""",
        ).findAll(rustToolchainFile.readText()).map { match -> match.groupValues[1] }.toList()
        require(toolchainMatches == listOf(pinnedRustToolchain)) {
            "Android native builds require exact Rust $pinnedRustToolchain"
        }

        val rustupEnvironment = baseToolEnvironment(
            home,
            temporaryDirectory,
            "${rustup.parent}:/usr/bin:/bin",
        )
        val cargo = requireExecutable(
            Path.of(
                commandOutput(
                    execOperations,
                    irohaRoot,
                    rustupEnvironment,
                    listOf(
                        rustup.toString(),
                        "which",
                        "--toolchain",
                        pinnedRustToolchain,
                        "cargo",
                    ),
                    "pinned Cargo resolution",
                ),
            ),
            "Cargo",
        )
        val rustc = requireExecutable(
            Path.of(
                commandOutput(
                    execOperations,
                    irohaRoot,
                    rustupEnvironment,
                    listOf(
                        rustup.toString(),
                        "which",
                        "--toolchain",
                        pinnedRustToolchain,
                        "rustc",
                    ),
                    "pinned rustc resolution",
                ),
            ),
            "rustc",
        )
        val cargoNdk = requireExecutable(
            home.resolve(".cargo/bin/cargo-ndk").toPath(),
            "cargo-ndk",
        )
        val androidNdk = androidNdkDirectory.toPath().toRealPath()
        require(
            Files.isDirectory(androidNdk, LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(androidNdk),
        ) {
            "Android NDK must resolve to a non-symbolic directory: $androidNdk"
        }
        val sourceProperties = androidNdk.resolve("source.properties")
        require(Files.isRegularFile(sourceProperties, LinkOption.NOFOLLOW_LINKS)) {
            "Android NDK source.properties is unavailable: $sourceProperties"
        }
        val androidNdkRevision = Files.readAllLines(sourceProperties)
            .singleOrNull { line -> line.startsWith("Pkg.Revision = ") }
            ?.substringAfter("Pkg.Revision = ")
            ?.trim()
            .orEmpty()
        require(Regex("^[0-9]+(?:\\.[0-9]+){1,3}$").matches(androidNdkRevision)) {
            "Android NDK revision is not canonical"
        }
        require(androidNdkRevision == pinnedAndroidNdkRevision) {
            "Android native builds require exact NDK $pinnedAndroidNdkRevision"
        }

        val cargoPath = listOf(
            cargo.parent.toString(),
            rustc.parent.toString(),
            cargoNdk.parent.toString(),
            "/usr/bin",
            "/bin",
        ).distinct().joinToString(File.pathSeparator)
        val cargoEnvironment = baseToolEnvironment(home, temporaryDirectory, cargoPath) +
            mapOf(
                "CARGO" to cargo.toString(),
                "RUSTC" to rustc.toString(),
                "CARGO_INCREMENTAL" to "0",
                "CARGO_NET_OFFLINE" to "true",
            )
        val cargoVersion = commandOutput(
            execOperations,
            irohaRoot,
            cargoEnvironment,
            listOf(cargo.toString(), "--version", "--verbose"),
            "Cargo identity",
        )
        val rustcVersion = commandOutput(
            execOperations,
            irohaRoot,
            cargoEnvironment,
            listOf(rustc.toString(), "--version", "--verbose"),
            "rustc identity",
        )
        fun versionField(document: String, field: String): String =
            document.lineSequence().singleOrNull { line -> line.startsWith("$field: ") }
                ?.substringAfter("$field: ")
                ?.trim()
                ?: throw GradleException("Native tool identity is missing $field")
        val cargoRelease = versionField(cargoVersion, "release")
        val cargoCommitHash = versionField(cargoVersion, "commit-hash")
        val rustcRelease = versionField(rustcVersion, "release")
        val rustcCommitHash = versionField(rustcVersion, "commit-hash")
        require(cargoRelease == pinnedRustToolchain && rustcRelease == pinnedRustToolchain) {
            "Cargo/rustc do not match exact Rust $pinnedRustToolchain"
        }
        require(
            Regex("^[0-9a-f]{40}$").matches(cargoCommitHash) &&
                Regex("^[0-9a-f]{40}$").matches(rustcCommitHash),
        ) {
            "Cargo/rustc commit identity is not canonical"
        }
        val cargoNdkVersionOutput = commandOutput(
            execOperations,
            irohaRoot,
            cargoEnvironment,
            listOf(cargo.toString(), "ndk", "--version"),
            "cargo-ndk identity",
        )
        val cargoNdkVersion = Regex("""^cargo-ndk ([0-9]+(?:\.[0-9]+){1,2})$""")
            .matchEntire(cargoNdkVersionOutput)
            ?.groupValues
            ?.get(1)
            ?: throw GradleException("cargo-ndk returned a non-canonical version")
        require(cargoNdkVersion == pinnedCargoNdkVersion) {
            "Android native builds require exact cargo-ndk $pinnedCargoNdkVersion"
        }

        val pythonEnvironment = mapOf(
            "HOME" to home.absolutePath,
            "PATH" to "${python.parent}:/usr/bin:/bin",
            "TMPDIR" to temporaryDirectory.absolutePath,
            "LANG" to "C.UTF-8",
            "LC_ALL" to "C.UTF-8",
        )
        val pythonVersion = commandOutput(
            execOperations,
            irohaRoot,
            pythonEnvironment,
            listOf(
                python.toString(),
                "-I",
                "-c",
                "import platform; print(platform.python_version())",
            ),
            "Python identity",
        )
        val gitEnvironment = pythonEnvironment + mapOf(
            "GIT_CONFIG_NOSYSTEM" to "1",
            "GIT_CONFIG_GLOBAL" to "/dev/null",
            "GIT_OPTIONAL_LOCKS" to "0",
        )
        val gitVersion = commandOutput(
            execOperations,
            irohaRoot,
            gitEnvironment,
            listOf(git.toString(), "--version"),
            "Git identity",
        ).let { output ->
            Regex("""^git version ([0-9]+(?:\.[0-9]+){1,3}).*$""")
                .matchEntire(output)
                ?.groupValues
                ?.get(1)
                ?: throw GradleException("Git returned a non-canonical version")
        }
        val rustupVersion = commandOutput(
            execOperations,
            irohaRoot,
            rustupEnvironment,
            listOf(rustup.toString(), "--version"),
            "rustup identity",
        ).lineSequence().first().let { output ->
            Regex("""^rustup ([0-9]+(?:\.[0-9]+){1,2}).*$""")
                .matchEntire(output)
                ?.groupValues
                ?.get(1)
                ?: throw GradleException("rustup returned a non-canonical version")
        }
        require(Regex("^[0-9]+(?:\\.[0-9]+){1,2}$").matches(pythonVersion)) {
            "Python returned a non-canonical version"
        }

        return BuildTools(
            home = home,
            temporaryDirectory = temporaryDirectory,
            python = python,
            git = git,
            rustup = rustup,
            cargo = cargo,
            rustc = rustc,
            cargoNdk = cargoNdk,
            hermeticRunner = hermeticRunner,
            androidNdk = androidNdk,
            cargoRelease = cargoRelease,
            cargoCommitHash = cargoCommitHash,
            rustcRelease = rustcRelease,
            rustcCommitHash = rustcCommitHash,
            cargoNdkVersion = cargoNdkVersion,
            pythonVersion = pythonVersion,
            gitVersion = gitVersion,
            rustupVersion = rustupVersion,
            androidNdkRevision = androidNdkRevision,
            androidNdkSourcePropertiesSha256 = sha256Hex(sourceProperties),
        )
    }

    fun buildEnvironmentDocument(tools: BuildTools): Map<String, Any> = linkedMapOf(
        "schema" to buildEnvironmentSchema,
        "hermetic_runner_schema" to hermeticRunnerSchema,
        "hermetic_runner_sha256" to sha256Hex(tools.hermeticRunner),
        "environment_profile" to "android-cargo",
        "environment_allowlist" to androidCargoEnvironmentAllowlist,
        "rust_toolchain_channel" to pinnedRustToolchain,
        "cargo_release" to tools.cargoRelease,
        "cargo_commit_hash" to tools.cargoCommitHash,
        "cargo_binary_sha256" to sha256Hex(tools.cargo),
        "rustc_release" to tools.rustcRelease,
        "rustc_commit_hash" to tools.rustcCommitHash,
        "rustc_binary_sha256" to sha256Hex(tools.rustc),
        "cargo_ndk_version" to tools.cargoNdkVersion,
        "cargo_ndk_binary_sha256" to sha256Hex(tools.cargoNdk),
        "python_version" to tools.pythonVersion,
        "python_binary_sha256" to sha256Hex(tools.python),
        "git_version" to tools.gitVersion,
        "git_binary_sha256" to sha256Hex(tools.git),
        "rustup_version" to tools.rustupVersion,
        "rustup_binary_sha256" to sha256Hex(tools.rustup),
        "android_ndk_revision" to tools.androidNdkRevision,
        "android_ndk_source_properties_sha256" to tools.androidNdkSourcePropertiesSha256,
    )

    fun buildEnvironmentBytes(tools: BuildTools): ByteArray =
        (JsonOutput.prettyPrint(JsonOutput.toJson(buildEnvironmentDocument(tools))) + "\n")
            .toByteArray(Charsets.UTF_8)

    private fun sourceSealEnvironment(tools: BuildTools): Map<String, String> =
        linkedMapOf(
            "HOME" to tools.home.absolutePath,
            "PATH" to listOf(
                tools.python.parent.toString(),
                tools.cargo.parent.toString(),
                tools.rustc.parent.toString(),
                tools.git.parent.toString(),
                "/usr/bin",
                "/bin",
            ).distinct().joinToString(File.pathSeparator),
            "TMPDIR" to tools.temporaryDirectory.absolutePath,
            "LANG" to "C.UTF-8",
            "LC_ALL" to "C.UTF-8",
            "NORITO_BRIDGE_SEAL_HOME" to tools.home.absolutePath,
            "NORITO_BRIDGE_SEAL_CARGO_HOME" to tools.home.resolve(".cargo").absolutePath,
            "NORITO_BRIDGE_SEAL_RUSTUP_HOME" to tools.home.resolve(".rustup").absolutePath,
            "NORITO_BRIDGE_SEAL_TMPDIR" to tools.temporaryDirectory.absolutePath,
            "NORITO_BRIDGE_SEAL_CARGO" to tools.cargo.toString(),
            "NORITO_BRIDGE_SEAL_RUSTC" to tools.rustc.toString(),
        )

    fun requireLibraries(
        root: java.io.File,
        expectedAbis: List<String> = abis,
    ): List<java.io.File> {
        val rootPath = root.toPath()
        require(
            Files.isDirectory(rootPath, LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(rootPath),
        ) {
            "Android native bridge inventory root must be a non-symbolic directory: $root"
        }
        val expectedRelativePaths = expectedAbis.map { abi -> "$abi/$libraryName" }.toSet()
        val expectedDirectories = expectedAbis.toSet()
        val actualRelativePaths = mutableSetOf<String>()
        val actualDirectories = mutableSetOf<String>()
        Files.walk(rootPath).use { paths ->
            paths.forEach { candidate ->
                if (candidate == rootPath) return@forEach
                require(!Files.isSymbolicLink(candidate)) {
                    "Android native bridge inventory must not contain a symbolic link: $candidate"
                }
                if (Files.isDirectory(candidate, LinkOption.NOFOLLOW_LINKS)) {
                    actualDirectories += rootPath.relativize(candidate).toString()
                        .replace(File.separatorChar, '/')
                    return@forEach
                }
                require(Files.isRegularFile(candidate, LinkOption.NOFOLLOW_LINKS)) {
                    "Android native bridge inventory contains a non-regular entry: $candidate"
                }
                actualRelativePaths += rootPath.relativize(candidate).toString()
                    .replace(File.separatorChar, '/')
            }
        }
        require(actualDirectories == expectedDirectories) {
            "Expected exact Android native bridge directories $expectedDirectories under $root; " +
                "found $actualDirectories"
        }
        require(actualRelativePaths == expectedRelativePaths) {
            "Expected exact Android native bridge inventory $expectedRelativePaths under $root; " +
                "found $actualRelativePaths"
        }
        return expectedAbis.map { abi ->
            root.resolve("$abi/$libraryName").also { library ->
                require(Files.isRegularFile(library.toPath(), LinkOption.NOFOLLOW_LINKS)) {
                    "Android native bridge must be a non-symbolic regular file: $library"
                }
                require(library.length() > 0L) {
                    "Android native bridge must not be empty: $library"
                }
            }
        }
    }

    fun requireRegularFileInside(
        root: java.io.File,
        candidate: java.io.File,
        label: String,
    ) {
        val rootPath = root.toPath().toAbsolutePath().normalize()
        val candidatePath = candidate.toPath().toAbsolutePath().normalize()
        require(candidatePath.startsWith(rootPath)) {
            "$label escapes its allowed root: $candidate"
        }
        var current = candidatePath
        while (true) {
            require(!Files.isSymbolicLink(current)) {
                "$label must not traverse a symbolic link: $current"
            }
            if (current == rootPath) break
            current = requireNotNull(current.parent) {
                "$label has no parent inside its allowed root: $candidate"
            }
        }
        require(Files.isRegularFile(candidatePath, LinkOption.NOFOLLOW_LINKS)) {
            "$label must be a non-symbolic regular file: $candidate"
        }
    }

    fun captureSourceSeal(
        execOperations: ExecOperations,
        irohaRoot: java.io.File,
        sourceSealScript: java.io.File,
        tools: BuildTools,
    ): ByteArray {
        val stdout = ByteArrayOutputStream()
        val stderr = ByteArrayOutputStream()
        val result = execOperations.exec {
            workingDir(irohaRoot)
            setEnvironment(sourceSealEnvironment(tools))
            commandLine(
                tools.python.toString(),
                "-I",
                sourceSealScript.absolutePath,
                "snapshot",
                "--root",
                irohaRoot.absolutePath,
                "--platform",
                "android",
            )
            standardOutput = stdout
            errorOutput = stderr
            isIgnoreExitValue = true
        }
        require(result.exitValue == 0) {
            "Unable to capture Android NoritoBridge source seal: " +
                stderr.toString(Charsets.UTF_8.name()).trim()
        }
        return stdout.toByteArray().also { payload ->
            require(payload.isNotEmpty()) { "Android NoritoBridge source seal is empty" }
        }
    }

    fun assertSourceSeal(
        execOperations: ExecOperations,
        irohaRoot: java.io.File,
        sourceSealScript: java.io.File,
        sourceSealFile: java.io.File,
        phase: String,
        tools: BuildTools,
    ) {
        val stderr = ByteArrayOutputStream()
        val result = execOperations.exec {
            workingDir(irohaRoot)
            setEnvironment(sourceSealEnvironment(tools))
            commandLine(
                tools.python.toString(),
                "-I",
                sourceSealScript.absolutePath,
                "verify",
                "--root",
                irohaRoot.absolutePath,
                "--platform",
                "android",
                "--snapshot",
                sourceSealFile.absolutePath,
            )
            errorOutput = stderr
            isIgnoreExitValue = true
        }
        require(result.exitValue == 0) {
            "Android NoritoBridge source changed during $phase; refusing a mixed-source " +
                "native artifact: ${stderr.toString(Charsets.UTF_8.name()).trim()}"
        }
    }
}

@DisableCachingByDefault(
    because = "Cargo's complete workspace dependency closure is not modeled as a Gradle input",
)
abstract class CompileNativeBridgeTask @Inject constructor(
    private val execOperations: ExecOperations,
    private val fileSystemOperations: FileSystemOperations,
) : DefaultTask() {
    @get:Input
    abstract val privacyProductionEnabled: Property<Boolean>

    @get:Internal
    abstract val irohaDirectory: DirectoryProperty

    @get:LocalState
    abstract val cargoTargetDirectory: DirectoryProperty

    @get:LocalState
    abstract val cargoNdkStagingDirectory: DirectoryProperty

    @get:Internal
    abstract val sourceSealScript: RegularFileProperty

    @get:Internal
    abstract val hermeticRunner: RegularFileProperty

    @get:Internal
    abstract val androidNdkDirectory: DirectoryProperty

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @get:OutputFile
    abstract val sourceSealFile: RegularFileProperty

    @get:OutputFile
    abstract val buildEnvironmentFile: RegularFileProperty

    @TaskAction
    fun compile() {
        val irohaRoot = irohaDirectory.get().asFile
        val sealScript = sourceSealScript.get().asFile
        require(Files.isRegularFile(sealScript.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            "NoritoBridge source-seal script must be a non-symbolic regular file: $sealScript"
        }
        val tools = NativeBridgeBuildContract.resolveBuildTools(
            execOperations,
            irohaRoot,
            hermeticRunner.get().asFile,
            androidNdkDirectory.get().asFile,
        )
        val sourceSeal = NativeBridgeBuildContract.captureSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            tools,
        )
        val buildEnvironment = NativeBridgeBuildContract.buildEnvironmentBytes(tools)
        val outputRoot = outputDirectory.get().asFile
        val stagingRoot = cargoNdkStagingDirectory.get().asFile
        val sealFile = sourceSealFile.get().asFile
        val environmentFile = buildEnvironmentFile.get().asFile
        fileSystemOperations.delete {
            // Gradle's DeleteSpec.delete(...) replaces its target array on
            // every invocation; pass the complete inventory in one call.
            delete(outputRoot, stagingRoot, sealFile, environmentFile)
        }
        require(
            listOf(outputRoot, stagingRoot, sealFile, environmentFile).none { candidate ->
                Files.exists(candidate.toPath(), LinkOption.NOFOLLOW_LINKS)
            },
        ) {
            "Unable to clear stale Android native build outputs before sealing: " +
                listOf(outputRoot, stagingRoot, sealFile, environmentFile)
        }
        require(outputRoot.mkdirs() || outputRoot.isDirectory) {
            "Unable to create cargo-ndk output directory: $outputRoot"
        }
        require(stagingRoot.mkdirs() || stagingRoot.isDirectory) {
            "Unable to create transient cargo-ndk staging directory: $stagingRoot"
        }
        require(sealFile.parentFile.mkdirs() || sealFile.parentFile.isDirectory) {
            "Unable to create Android source-seal directory: ${sealFile.parentFile}"
        }
        require(
            environmentFile.parentFile.mkdirs() || environmentFile.parentFile.isDirectory,
        ) {
            "Unable to create Android build-environment directory: ${environmentFile.parentFile}"
        }
        sealFile.writeBytes(sourceSeal)
        environmentFile.writeBytes(buildEnvironment)
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "build start",
            tools,
        )
        val cargoTargetRoot = cargoTargetDirectory.get().asFile
        require(cargoTargetRoot.mkdirs() || cargoTargetRoot.isDirectory) {
            "Unable to create isolated Cargo target directory: $cargoTargetRoot"
        }

        NativeBridgeBuildContract.abis.forEachIndexed { index, abi ->
            val abiStagingRoot = stagingRoot.resolve(abi)
            fileSystemOperations.delete { delete(abiStagingRoot) }
            require(abiStagingRoot.mkdirs() || abiStagingRoot.isDirectory) {
                "Unable to create transient cargo-ndk staging directory for $abi: $abiStagingRoot"
            }
            val cargoPath = listOf(
                tools.cargo.parent.toString(),
                tools.rustc.parent.toString(),
                tools.cargoNdk.parent.toString(),
                "/usr/bin",
                "/bin",
            ).distinct().joinToString(File.pathSeparator)
            val command = buildList {
                addAll(
                    listOf(
                        tools.python.toString(),
                        "-I",
                        tools.hermeticRunner.toString(),
                        "--profile",
                        "android-cargo",
                        "--set",
                        "ANDROID_NDK_HOME=${tools.androidNdk}",
                        "--set",
                        "ANDROID_NDK_ROOT=${tools.androidNdk}",
                        "--set",
                        "CARGO=${tools.cargo}",
                        "--set",
                        "CARGO_HOME=${tools.home.resolve(".cargo")}",
                        "--set",
                        "CARGO_INCREMENTAL=0",
                        "--set",
                        "CARGO_NET_OFFLINE=true",
                        "--set",
                        "CARGO_TARGET_DIR=${cargoTargetRoot.absolutePath}",
                        "--set",
                        "HOME=${tools.home.absolutePath}",
                        "--set",
                        "LANG=C.UTF-8",
                        "--set",
                        "LC_ALL=C.UTF-8",
                        "--set",
                        "NORITO_SKIP_BINDINGS_SYNC=1",
                        "--set",
                        "PATH=$cargoPath",
                        "--set",
                        "RUSTC=${tools.rustc}",
                        "--set",
                        "RUSTUP_HOME=${tools.home.resolve(".rustup")}",
                        "--set",
                        "TMPDIR=${tools.temporaryDirectory.absolutePath}",
                        "--",
                        tools.cargo.toString(),
                        "ndk",
                        "-t",
                        abi,
                        "-o",
                        abiStagingRoot.absolutePath,
                    ),
                )
                addAll(
                    listOf(
                        "build",
                        "--locked",
                        "--offline",
                        "--release",
                        "-p",
                        "connect_norito_bridge",
                    ),
                )
                if (privacyProductionEnabled.get()) {
                    addAll(listOf("--features", "privacy-production-enabled"))
                }
            }
            execOperations.exec {
                workingDir(irohaRoot)
                setEnvironment(
                    mapOf(
                        "HOME" to tools.home.absolutePath,
                        "PATH" to "${tools.python.parent}:/usr/bin:/bin",
                        "TMPDIR" to tools.temporaryDirectory.absolutePath,
                        "LANG" to "C.UTF-8",
                        "LC_ALL" to "C.UTF-8",
                    ),
                )
                commandLine(*command.toTypedArray())
            }.assertNormalExitValue()
            NativeBridgeBuildContract.assertSourceSeal(
                execOperations,
                irohaRoot,
                sealScript,
                sealFile,
                "$abi Cargo build",
                tools,
            )

            // cargo-ndk can copy unrelated cdylib workspace outputs into its
            // destination. Treat that destination as transient and promote
            // only the exact bridge name into the authoritative raw inventory.
            val stagedLibrary = abiStagingRoot.resolve(
                "$abi/${NativeBridgeBuildContract.libraryName}",
            )
            NativeBridgeBuildContract.requireRegularFileInside(
                abiStagingRoot,
                stagedLibrary,
                "cargo-ndk ${NativeBridgeBuildContract.libraryName} for $abi",
            )
            val promotedLibrary = outputRoot.resolve(
                "$abi/${NativeBridgeBuildContract.libraryName}",
            )
            require(promotedLibrary.parentFile.mkdirs() || promotedLibrary.parentFile.isDirectory) {
                "Unable to create promoted raw native ABI directory: ${promotedLibrary.parentFile}"
            }
            NativeBridgeBuildContract.assertSourceSeal(
                execOperations,
                irohaRoot,
                sealScript,
                sealFile,
                "$abi immediate pre-promotion authentication",
                tools,
            )
            Files.copy(
                stagedLibrary.toPath(),
                promotedLibrary.toPath(),
                LinkOption.NOFOLLOW_LINKS,
                StandardCopyOption.COPY_ATTRIBUTES,
                StandardCopyOption.REPLACE_EXISTING,
            )
            NativeBridgeBuildContract.requireLibraries(
                outputRoot,
                NativeBridgeBuildContract.abis.take(index + 1),
            )
            NativeBridgeBuildContract.assertSourceSeal(
                execOperations,
                irohaRoot,
                sealScript,
                sealFile,
                "$abi immediate post-promotion authentication",
                tools,
            )
        }
        NativeBridgeBuildContract.requireLibraries(outputRoot)
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "native compile completion",
            tools,
        )
    }

    fun hasReusableSealedOutput(): Boolean = runCatching {
        val outputRoot = outputDirectory.get().asFile
        val sealFile = sourceSealFile.get().asFile
        val environmentFile = buildEnvironmentFile.get().asFile
        val tools = NativeBridgeBuildContract.resolveBuildTools(
            execOperations,
            irohaDirectory.get().asFile,
            hermeticRunner.get().asFile,
            androidNdkDirectory.get().asFile,
        )
        NativeBridgeBuildContract.requireLibraries(outputRoot)
        require(Files.isRegularFile(sealFile.toPath(), LinkOption.NOFOLLOW_LINKS))
        require(Files.isRegularFile(environmentFile.toPath(), LinkOption.NOFOLLOW_LINKS))
        require(
            environmentFile.readBytes().contentEquals(
                NativeBridgeBuildContract.buildEnvironmentBytes(tools),
            ),
        ) {
            "Reusable Android native output was built with a different toolchain environment"
        }
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaDirectory.get().asFile,
            sourceSealScript.get().asFile,
            sealFile,
            "up-to-date authentication",
            tools,
        )
        true
    }.getOrDefault(false)
}

@DisableCachingByDefault(
    because = "The manifest records live source-control provenance and must be regenerated",
)
abstract class StripNativeBridgeTask @Inject constructor(
    private val execOperations: ExecOperations,
    private val fileSystemOperations: FileSystemOperations,
) : DefaultTask() {
    @get:Input
    abstract val privacyProductionEnabled: Property<Boolean>

    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val inputDirectory: DirectoryProperty

    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val sourceSealFile: RegularFileProperty

    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val buildEnvironmentFile: RegularFileProperty

    @get:Internal
    abstract val irohaDirectory: DirectoryProperty

    @get:Internal
    abstract val sourceSealScript: RegularFileProperty

    @get:Internal
    abstract val hermeticRunner: RegularFileProperty

    @get:Internal
    abstract val androidNdkDirectory: DirectoryProperty

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @get:OutputDirectory
    abstract val provenanceDirectory: DirectoryProperty

    @TaskAction
    fun strip() {
        val irohaRoot = irohaDirectory.get().asFile
        val sealScript = sourceSealScript.get().asFile
        val sealFile = sourceSealFile.get().asFile
        val environmentFile = buildEnvironmentFile.get().asFile
        val tools = NativeBridgeBuildContract.resolveBuildTools(
            execOperations,
            irohaRoot,
            hermeticRunner.get().asFile,
            androidNdkDirectory.get().asFile,
        )
        require(Files.isRegularFile(sealFile.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            "Android source seal must be a non-symbolic regular file: $sealFile"
        }
        require(
            Files.isRegularFile(environmentFile.toPath(), LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(environmentFile.toPath()),
        ) {
            "Android build environment must be a non-symbolic regular file: $environmentFile"
        }
        require(
            environmentFile.readBytes().contentEquals(
                NativeBridgeBuildContract.buildEnvironmentBytes(tools),
            ),
        ) {
            "Android native build environment changed between compile and strip"
        }
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "strip start",
            tools,
        )
        val sourceSeal = JsonSlurper().parse(sealFile) as? Map<*, *>
            ?: throw GradleException("Android source seal root must be a JSON object")
        val expectedSealFields = setOf(
            "platform",
            "schema",
            "source_commit",
            "source_fingerprint_sha256",
            "source_status",
            "source_tree_dirty",
            "targets",
        )
        require(sourceSeal.keys == expectedSealFields) {
            "Android source seal field inventory is not exact: ${sourceSeal.keys}"
        }
        require(sourceSeal["schema"] == NativeBridgeBuildContract.sourceSealSchema) {
            "Android source seal schema is not canonical"
        }
        require(sourceSeal["platform"] == "android") {
            "Android source seal platform is not canonical"
        }
        require(sourceSeal["targets"] == NativeBridgeBuildContract.rustTargets) {
            "Android source seal target inventory is not exact"
        }
        val sourceCommit = sourceSeal["source_commit"] as? String
            ?: throw GradleException("Android source seal commit is not a string")
        require(Regex("^[0-9a-f]{40}$").matches(sourceCommit)) {
            "Iroha source commit is not a canonical 40-character Git object id"
        }
        val sourceTreeDirty = sourceSeal["source_tree_dirty"] as? Boolean
            ?: throw GradleException("Android source seal dirty state is not boolean")
        val sourceStatus = sourceSeal["source_status"] as? String
            ?: throw GradleException("Android source seal status is not a string")
        require(sourceTreeDirty == sourceStatus.isNotEmpty()) {
            "Android source seal dirty state disagrees with its exact status"
        }
        val sourceFingerprint = sourceSeal["source_fingerprint_sha256"] as? String
            ?: throw GradleException("Android source seal fingerprint is not a string")
        require(Regex("^[0-9a-f]{64}$").matches(sourceFingerprint)) {
            "Android source seal fingerprint is not canonical SHA-256"
        }
        val inputRoot = inputDirectory.get().asFile
        val rawLibraries = NativeBridgeBuildContract.requireLibraries(inputRoot)
        val rawIdentityByAbi = rawLibraries.associate { library ->
            library.parentFile.name to Pair(
                library.length(),
                NativeBridgeBuildContract.sha256Hex(library.toPath()),
            )
        }
        val outputRoot = outputDirectory.get().asFile
        val provenanceRoot = provenanceDirectory.get().asFile
        val scopedRoots = listOf(inputRoot, outputRoot, provenanceRoot)
            .map { directory -> directory.toPath().toAbsolutePath().normalize() }
        require(scopedRoots.toSet().size == scopedRoots.size) {
            "Raw, stripped, and provenance output directories must be distinct: $scopedRoots"
        }
        fileSystemOperations.delete {
            delete(outputRoot, provenanceRoot)
        }
        require(
            listOf(outputRoot, provenanceRoot).none { candidate ->
                Files.exists(candidate.toPath(), LinkOption.NOFOLLOW_LINKS)
            },
        ) {
            "Unable to clear stale stripped/provenance outputs before release packaging: " +
                listOf(outputRoot, provenanceRoot)
        }
        require(outputRoot.mkdirs() || outputRoot.isDirectory) {
            "Unable to create stripped native output directory: $outputRoot"
        }

        val outputLibraries = rawLibraries.map { rawLibrary ->
            val relativePath = rawLibrary.relativeTo(inputRoot).invariantSeparatorsPath
            val outputLibrary = outputRoot.resolve(relativePath)
            require(outputLibrary.parentFile.mkdirs() || outputLibrary.parentFile.isDirectory) {
                "Unable to create stripped native ABI directory: ${outputLibrary.parentFile}"
            }
            Files.copy(
                rawLibrary.toPath(),
                outputLibrary.toPath(),
                LinkOption.NOFOLLOW_LINKS,
                StandardCopyOption.REPLACE_EXISTING,
            )
            outputLibrary
        }

        val prebuiltRoot = androidNdkDirectory.get().asFile
            .resolve("toolchains/llvm/prebuilt")
        require(
            Files.isDirectory(prebuiltRoot.toPath(), LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(prebuiltRoot.toPath()),
        ) {
            "Android NDK LLVM prebuilt root must be a non-symbolic directory: $prebuiltRoot"
        }
        val stripLaunchers = prebuiltRoot.listFiles()
            ?.asSequence()
            ?.filter { hostDirectory ->
                Files.isDirectory(hostDirectory.toPath(), LinkOption.NOFOLLOW_LINKS) &&
                    !Files.isSymbolicLink(hostDirectory.toPath())
            }
            ?.flatMap { hostDirectory ->
                sequenceOf("llvm-strip", "llvm-strip.exe").map { launcherName ->
                    hostDirectory.resolve("bin/$launcherName")
                }
            }
            ?.filter { launcher ->
                Files.exists(launcher.toPath(), LinkOption.NOFOLLOW_LINKS) &&
                    !Files.isDirectory(launcher.toPath(), LinkOption.NOFOLLOW_LINKS)
            }
            ?.toList()
            .orEmpty()
        require(stripLaunchers.size == 1) {
            "Expected one Android NDK llvm-strip launcher under $prebuiltRoot; " +
                "found ${stripLaunchers.size}"
        }
        val stripLauncher = stripLaunchers.single()
        val stripExecutablePath = stripLauncher.toPath().toRealPath()
        val stripHostBinPath = stripLauncher.parentFile.toPath().toRealPath()
        require(
            stripExecutablePath.startsWith(stripHostBinPath) &&
                Files.isRegularFile(stripExecutablePath, LinkOption.NOFOLLOW_LINKS) &&
                Files.isExecutable(stripExecutablePath),
        ) {
            "Android NDK llvm-strip must resolve to an executable regular file inside " +
                "${stripLauncher.parentFile}: $stripLauncher -> $stripExecutablePath"
        }
        execOperations.exec {
            setEnvironment(
                mapOf(
                    "HOME" to tools.home.absolutePath,
                    "PATH" to "${stripExecutablePath.parent}:/usr/bin:/bin",
                    "TMPDIR" to tools.temporaryDirectory.absolutePath,
                    "LANG" to "C.UTF-8",
                    "LC_ALL" to "C.UTF-8",
                ),
            )
            commandLine(
                stripExecutablePath.toString(),
                "--strip-unneeded",
                *outputLibraries.map { it.absolutePath }.toTypedArray(),
            )
        }.assertNormalExitValue()
        NativeBridgeBuildContract.requireLibraries(outputRoot)
        val rawLibrariesAfterStrip = NativeBridgeBuildContract.requireLibraries(inputRoot)
            .associateBy { library -> library.parentFile.name }
        rawIdentityByAbi.forEach { (abi, identity) ->
            val rawLibrary = requireNotNull(rawLibrariesAfterStrip[abi])
            require(rawLibrary.length() == identity.first &&
                NativeBridgeBuildContract.sha256Hex(rawLibrary.toPath()) == identity.second
            ) {
                "Canonical stripping modified raw cargo-ndk output for $abi"
            }
        }

        val cargoLock = irohaDirectory.get().file("Cargo.lock").asFile
        require(Files.isRegularFile(cargoLock.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            "Cargo.lock must be a non-symbolic regular file: $cargoLock"
        }
        val ndkRevision = tools.androidNdkRevision

        val outputByAbi = outputLibraries.associateBy { library -> library.parentFile.name }
        val libraries = linkedMapOf<String, Any>()
        NativeBridgeBuildContract.abis.forEach { abi ->
            val outputLibrary = requireNotNull(outputByAbi[abi])
            val rawIdentity = requireNotNull(rawIdentityByAbi[abi])
            libraries[abi] = linkedMapOf(
                "aar_path" to "jni/$abi/${NativeBridgeBuildContract.libraryName}",
                "bytes" to outputLibrary.length(),
                "raw_bytes" to rawIdentity.first,
                "raw_sha256" to rawIdentity.second,
                "sha256" to NativeBridgeBuildContract.sha256Hex(outputLibrary.toPath()),
            )
        }
        val cargoFeatures = if (privacyProductionEnabled.get()) {
            listOf("privacy-production-enabled")
        } else {
            emptyList<String>()
        }
        val manifest = linkedMapOf<String, Any>(
            "schema" to "iroha.android-native-build-provenance.v1",
            "native_bridge_abi_version" to 21,
            "build_profile" to "release",
            "cargo_locked" to true,
            "privacy_production_enabled" to privacyProductionEnabled.get(),
            "cargo_features" to cargoFeatures,
            "build_environment" to NativeBridgeBuildContract.buildEnvironmentDocument(tools),
            "source_commit" to sourceCommit,
            "source_tree_dirty" to sourceTreeDirty,
            "source_fingerprint_sha256" to sourceFingerprint,
            "cargo_lock_sha256" to NativeBridgeBuildContract.sha256Hex(cargoLock.toPath()),
            "android_ndk_revision" to ndkRevision,
            "strip_tool_sha256" to NativeBridgeBuildContract.sha256Hex(stripExecutablePath),
            "libraries" to libraries,
        )
        val provenanceFile = provenanceRoot.resolve(
            "iroha/native-build-provenance-v1.json",
        )
        require(provenanceFile.parentFile.mkdirs() || provenanceFile.parentFile.isDirectory) {
            "Unable to create native provenance directory: ${provenanceFile.parentFile}"
        }
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "stripped artifact immediate pre-promotion authentication",
            tools,
        )
        provenanceFile.writeText(JsonOutput.prettyPrint(JsonOutput.toJson(manifest)) + "\n")
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "stripped artifact immediate post-promotion authentication",
            tools,
        )
    }
}

group = "org.hyperledger.iroha.sdk"
version = providers.gradleProperty("irohaSdkVersion")
    .orElse(providers.environmentVariable("IROHA_SDK_VERSION"))
    .orElse("0.1.0")
    .get()

val mobileSdkRepoDir = providers.gradleProperty("irohaSdkRepoDir")
    .orElse(rootProject.layout.buildDirectory.dir("mobile-sdk-maven").map { it.asFile.absolutePath })

android {
    namespace = "org.hyperledger.iroha.sdk.android"
    compileSdk = 35

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
        isCoreLibraryDesugaringEnabled = true
    }

    kotlin {
        jvmToolchain(21)

        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_1_8)
            freeCompilerArgs.add("-Xjdk-release=8")
        }
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }

    sourceSets {
        // Native bridge bytes are generated under build/ and registered with
        // the variant API below. Never package stale, ignored source-tree .so files.
        getByName("main").jniLibs.directories.clear()
    }

    packaging {
        jniLibs {
            // The mobile artifact verifier binds the published AAR to the exact
            // generated, canonically stripped bridge bytes and provenance.
            // Prevent AGP from applying a second, potentially different strip.
            keepDebugSymbols.add("**/libconnect_norito_bridge.so")
        }
    }
}

repositories {
    google()
    mavenCentral()
}

publishing {
    repositories {
        maven {
            name = "mobileSdk"
            url = uri(mobileSdkRepoDir.get())
        }
    }
}

dependencies {
    api(project(":core-jvm"))
    implementation(libs.play.services.nearby)
    coreLibraryDesugaring(libs.desugar.jdk.libs)
    testImplementation(kotlin("test"))
    testImplementation(libs.junit.params)
    testRuntimeOnly(libs.junit.jupiter.engine)
    testRuntimeOnly(libs.junit.platform.launcher)
}

tasks.withType<Test>().configureEach {
    useJUnitPlatform()
    if (name == "testDebugUnitTest") {
        dependsOn("processDebugManifest")
        systemProperty(
            "iroha.clientAndroid.mergedManifest",
            layout.buildDirectory.file(
                "intermediates/merged_manifest/debug/processDebugManifest/AndroidManifest.xml",
            ).get().asFile.absolutePath,
        )
    }
}

fun irohaDir(): String {
    val props = Properties()
    val file = rootProject.file("local.properties")
    if (file.exists()) file.inputStream().use { props.load(it) }
    return props.getProperty("iroha.dir") ?: rootProject.file("..").absolutePath
}

val privacyProductionEnabledInput =
    providers.gradleProperty("privacyProductionEnabled").orNull ?: "false"
if (privacyProductionEnabledInput != "true" && privacyProductionEnabledInput != "false") {
    throw GradleException(
        "privacyProductionEnabled must be exactly 'true' or 'false'; " +
            "received '$privacyProductionEnabledInput'",
    )
}
val privacyProductionEnabledValue = privacyProductionEnabledInput == "true"
val nativeBuildMode = if (privacyProductionEnabledValue) "production" else "default"
val mobileSdkAndroidArtifactDirectoryInput =
    providers.environmentVariable("MOBILE_SDK_ANDROID_ARTIFACT_DIR")
val requireExternalAndroidArtifactDirectory =
    tasks.register("requireExternalAndroidArtifactDirectory") {
        group = "verification"
        description =
            "Requires the canonical external artifact root used by reviewed Android releases"
        inputs.property(
            "mobileSdkAndroidArtifactDirectory",
            mobileSdkAndroidArtifactDirectoryInput.orElse("<missing>"),
        )
        doLast {
            val raw = mobileSdkAndroidArtifactDirectoryInput.orNull
                ?: throw GradleException(
                    "Reviewed Android Release builds require " +
                        "MOBILE_SDK_ANDROID_ARTIFACT_DIR.",
                )
            val supplied = Path.of(raw)
            require(supplied.isAbsolute && supplied.normalize().toString() == raw) {
                "MOBILE_SDK_ANDROID_ARTIFACT_DIR must be an absolute normalized path"
            }
            val canonical = supplied.toRealPath()
            require(
                canonical == supplied &&
                    Files.isDirectory(canonical, LinkOption.NOFOLLOW_LINKS) &&
                    !Files.isSymbolicLink(canonical) &&
                    Files.isWritable(canonical),
            ) {
                "MOBILE_SDK_ANDROID_ARTIFACT_DIR must be one canonical writable " +
                    "non-symbolic directory"
            }
            val irohaRoot = file(irohaDir()).toPath().toRealPath()
            require(canonical != irohaRoot && !canonical.startsWith(irohaRoot)) {
                "MOBILE_SDK_ANDROID_ARTIFACT_DIR must be outside the reviewed Iroha source tree"
            }
            val expectedBuildDirectory = canonical
                .resolve("gradle-build/iroha_kotlin_sdk/client-android")
                .normalize()
            require(
                layout.buildDirectory.get().asFile.toPath().toAbsolutePath().normalize() ==
                    expectedBuildDirectory,
            ) {
                "client-android Release outputs are not redirected to the canonical " +
                    "MOBILE_SDK_ANDROID_ARTIFACT_DIR"
            }
        }
    }
val androidNdkRoot = providers.environmentVariable("ANDROID_NDK_HOME")
    .orElse(providers.environmentVariable("ANDROID_NDK_ROOT"))
    .orElse(androidComponents.sdkComponents.ndkDirectory.map { it.asFile.absolutePath })

val compileNativeLibs = tasks.register<CompileNativeBridgeTask>("compileNativeLibs") {
    group = "native"
    description = "Compile connect_norito_bridge .so from Rust source (requires cargo-ndk + Android NDK)"
    privacyProductionEnabled.set(privacyProductionEnabledValue)
    irohaDirectory.set(file(irohaDir()))
    cargoTargetDirectory.set(
        layout.buildDirectory.dir("native/cargo-target/$nativeBuildMode"),
    )
    cargoNdkStagingDirectory.set(
        layout.buildDirectory.dir("native/cargo-ndk-staging/$nativeBuildMode"),
    )
    sourceSealScript.set(file(irohaDir()).resolve("scripts/norito_bridge_source_seal.py"))
    hermeticRunner.set(file(irohaDir()).resolve("scripts/run_mobile_hermetic_command.py"))
    androidNdkDirectory.set(
        layout.dir(androidNdkRoot.map { ndkPath -> file(ndkPath) }),
    )
    outputDirectory.set(layout.buildDirectory.dir("native/cargo-ndk/$nativeBuildMode"))
    sourceSealFile.set(
        layout.buildDirectory.file("native/sourceSeal/$nativeBuildMode/source-seal-v1.json"),
    )
    buildEnvironmentFile.set(
        layout.buildDirectory.file(
            "native/buildEnvironment/$nativeBuildMode/build-environment-v1.json",
        ),
    )
    outputs.upToDateWhen { hasReusableSealedOutput() }
    dependsOn(requireExternalAndroidArtifactDirectory)
}

val stripNativeLibs = tasks.register<StripNativeBridgeTask>("stripNativeLibs") {
    group = "native"
    description = "Canonically strip the compiled Android native bridge libraries"
    privacyProductionEnabled.set(privacyProductionEnabledValue)
    inputDirectory.set(compileNativeLibs.flatMap { it.outputDirectory })
    sourceSealFile.set(compileNativeLibs.flatMap { it.sourceSealFile })
    buildEnvironmentFile.set(compileNativeLibs.flatMap { it.buildEnvironmentFile })
    irohaDirectory.set(file(irohaDir()))
    sourceSealScript.set(file(irohaDir()).resolve("scripts/norito_bridge_source_seal.py"))
    hermeticRunner.set(file(irohaDir()).resolve("scripts/run_mobile_hermetic_command.py"))
    androidNdkDirectory.set(
        layout.dir(androidNdkRoot.map { ndkPath -> file(ndkPath) }),
    )
    outputDirectory.set(layout.buildDirectory.dir("generated/jniLibs/$nativeBuildMode"))
    provenanceDirectory.set(
        layout.buildDirectory.dir("generated/nativeProvenance/$nativeBuildMode"),
    )
    // Re-run the comparatively cheap strip/provenance phase so every release
    // packaging operation performs a final live source-seal check, even when
    // the expensive per-ABI Cargo output is safely reusable.
    outputs.upToDateWhen { false }
}

// Only release packaging consumes the shipping bridge. Registering generated
// JNI/assets on debug made ordinary JVM unit-test compilation run cargo-ndk,
// even though those tests never load an Android shared object.
androidComponents.onVariants(androidComponents.selector().withBuildType("release")) { variant ->
    requireNotNull(variant.sources.jniLibs) {
        "AGP did not expose jniLibs sources for ${variant.name}"
    }.addGeneratedSourceDirectory(stripNativeLibs, StripNativeBridgeTask::outputDirectory)
    requireNotNull(variant.sources.assets) {
        "AGP did not expose asset sources for ${variant.name}"
    }.addGeneratedSourceDirectory(stripNativeLibs, StripNativeBridgeTask::provenanceDirectory)
}

tasks.register("buildNativeLibs") {
    group = "native"
    description = "Build and canonically strip connect_norito_bridge Android libraries"
    dependsOn(stripNativeLibs)
    dependsOn(requireExternalAndroidArtifactDirectory)
}

tasks.matching {
    name == "assembleRelease" ||
        name == "bundleReleaseAar" ||
        name.startsWith("publishRelease")
}.configureEach {
    dependsOn(requireExternalAndroidArtifactDirectory)
}

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "org.hyperledger.iroha.sdk"
                artifactId = "client-android"
            }
        }
    }
}
