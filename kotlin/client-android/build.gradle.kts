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
    ): ByteArray {
        val stdout = ByteArrayOutputStream()
        val stderr = ByteArrayOutputStream()
        val result = execOperations.exec {
            workingDir(irohaRoot)
            commandLine(
                "python3",
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
    ) {
        val stderr = ByteArrayOutputStream()
        val result = execOperations.exec {
            workingDir(irohaRoot)
            commandLine(
                "python3",
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

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @get:OutputFile
    abstract val sourceSealFile: RegularFileProperty

    @TaskAction
    fun compile() {
        val irohaRoot = irohaDirectory.get().asFile
        val sealScript = sourceSealScript.get().asFile
        require(Files.isRegularFile(sealScript.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            "NoritoBridge source-seal script must be a non-symbolic regular file: $sealScript"
        }
        val sourceSeal = NativeBridgeBuildContract.captureSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
        )
        val outputRoot = outputDirectory.get().asFile
        val stagingRoot = cargoNdkStagingDirectory.get().asFile
        val sealFile = sourceSealFile.get().asFile
        fileSystemOperations.delete {
            // Gradle's DeleteSpec.delete(...) replaces its target array on
            // every invocation; pass the complete inventory in one call.
            delete(outputRoot, stagingRoot, sealFile)
        }
        require(
            listOf(outputRoot, stagingRoot, sealFile).none { candidate ->
                Files.exists(candidate.toPath(), LinkOption.NOFOLLOW_LINKS)
            },
        ) {
            "Unable to clear stale Android native build outputs before sealing: " +
                listOf(outputRoot, stagingRoot, sealFile)
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
        sealFile.writeBytes(sourceSeal)
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "build start",
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
            val command = buildList {
                addAll(listOf("cargo", "ndk", "-t", abi, "-o", abiStagingRoot.absolutePath))
                addAll(
                    listOf(
                        "build",
                        "--locked",
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
                environment("CARGO_TARGET_DIR", cargoTargetRoot.absolutePath)
                commandLine(*command.toTypedArray())
            }.assertNormalExitValue()
            NativeBridgeBuildContract.assertSourceSeal(
                execOperations,
                irohaRoot,
                sealScript,
                sealFile,
                "$abi Cargo build",
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
                "$abi promotion",
            )
        }
        NativeBridgeBuildContract.requireLibraries(outputRoot)
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "native compile completion",
        )
    }

    fun hasReusableSealedOutput(): Boolean = runCatching {
        val outputRoot = outputDirectory.get().asFile
        val sealFile = sourceSealFile.get().asFile
        NativeBridgeBuildContract.requireLibraries(outputRoot)
        require(Files.isRegularFile(sealFile.toPath(), LinkOption.NOFOLLOW_LINKS))
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaDirectory.get().asFile,
            sourceSealScript.get().asFile,
            sealFile,
            "up-to-date authentication",
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

    @get:Internal
    abstract val irohaDirectory: DirectoryProperty

    @get:Internal
    abstract val sourceSealScript: RegularFileProperty

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
        require(Files.isRegularFile(sealFile.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            "Android source seal must be a non-symbolic regular file: $sealFile"
        }
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "strip start",
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
            commandLine(
                stripLauncher.absolutePath,
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
        val ndkRevision = androidNdkDirectory.get().file("source.properties").asFile
            .takeIf { it.isFile }
            ?.readLines()
            ?.singleOrNull { line -> line.startsWith("Pkg.Revision = ") }
            ?.substringAfter("Pkg.Revision = ")
            ?.trim()
            .orEmpty()
        require(ndkRevision.isNotEmpty()) {
            "Android NDK source.properties must contain exactly one Pkg.Revision"
        }

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
        provenanceFile.writeText(JsonOutput.prettyPrint(JsonOutput.toJson(manifest)) + "\n")
        NativeBridgeBuildContract.assertSourceSeal(
            execOperations,
            irohaRoot,
            sealScript,
            sealFile,
            "strip and provenance completion",
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
    outputDirectory.set(layout.buildDirectory.dir("native/cargo-ndk/$nativeBuildMode"))
    sourceSealFile.set(
        layout.buildDirectory.file("native/sourceSeal/$nativeBuildMode/source-seal-v1.json"),
    )
    outputs.upToDateWhen { hasReusableSealedOutput() }
}

val androidNdkRoot = providers.environmentVariable("ANDROID_NDK_HOME")
    .orElse(providers.environmentVariable("ANDROID_NDK_ROOT"))
    .orElse(androidComponents.sdkComponents.ndkDirectory.map { it.asFile.absolutePath })

val stripNativeLibs = tasks.register<StripNativeBridgeTask>("stripNativeLibs") {
    group = "native"
    description = "Canonically strip the compiled Android native bridge libraries"
    privacyProductionEnabled.set(privacyProductionEnabledValue)
    inputDirectory.set(compileNativeLibs.flatMap { it.outputDirectory })
    sourceSealFile.set(compileNativeLibs.flatMap { it.sourceSealFile })
    irohaDirectory.set(file(irohaDir()))
    sourceSealScript.set(file(irohaDir()).resolve("scripts/norito_bridge_source_seal.py"))
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
