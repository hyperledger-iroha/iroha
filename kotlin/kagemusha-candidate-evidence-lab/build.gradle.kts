import com.android.build.api.artifact.SingleArtifact
import groovy.json.JsonSlurper
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.io.File
import java.io.FileInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.attribute.PosixFilePermission
import java.security.MessageDigest
import java.util.zip.ZipFile

plugins {
    alias(libs.plugins.android.application)
}

repositories {
    google()
    mavenCentral()
}

val candidateLabMarker = "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2"
val lowercaseSha256 = Regex("^[0-9a-f]{64}$")
val lowercaseGitCommit = Regex("^[0-9a-f]{40}$")
val compileOnlyContract = providers.gradleProperty("kagemushaCandidateCompileOnly")
    .orNull
    ?.let { value ->
        when (value) {
            "true" -> true
            "false" -> false
            else -> throw GradleException(
                "-PkagemushaCandidateCompileOnly must be exactly true or false",
            )
        }
    }
    ?: false

if (compileOnlyContract) {
    val exactCompileTasks = setOf(
        ":kagemusha-candidate-evidence-lab:compileDebugKotlin",
        ":kagemusha-candidate-evidence-lab:compileDebugAndroidTestKotlin",
    )
    if (gradle.startParameter.taskNames.toSet() != exactCompileTasks) {
        throw GradleException(
            "candidate compile-only mode permits exactly the main and androidTest Kotlin " +
                "compile tasks; packaging, staging, installation, execution, and export " +
                "are forbidden",
        )
    }
}

fun requiredProperty(name: String): String =
    providers.gradleProperty(name).orNull
        ?: throw GradleException("-P$name is required for the candidate evidence lab")

fun sha256(file: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
    FileInputStream(file).use { input ->
        val buffer = ByteArray(1024 * 1024)
        while (true) {
            val count = input.read(buffer)
            if (count < 0) break
            digest.update(buffer, 0, count)
        }
    }
    return digest.digest().joinToString("") { "%02x".format(it) }
}

fun File.requireRegularFile(label: String): File {
    if (!isFile || Files.isSymbolicLink(toPath())) {
        throw GradleException("$label must be a real regular file: $this")
    }
    return this
}

fun File.containsAscii(needleText: String): Boolean {
    val needle = needleText.toByteArray(Charsets.US_ASCII)
    require(needle.isNotEmpty())
    var matched = 0
    FileInputStream(this).use { input ->
        val buffer = ByteArray(1024 * 1024)
        while (true) {
            val count = input.read(buffer)
            if (count < 0) return false
            for (index in 0 until count) {
                val byte = buffer[index]
                if (byte == needle[matched]) {
                    matched += 1
                    if (matched == needle.size) return true
                } else {
                    matched = if (byte == needle[0]) 1 else 0
                }
            }
        }
    }
}

fun File.requireWithin(root: File, label: String): File {
    val canonicalRoot = root.canonicalFile.toPath()
    val canonicalPath = canonicalFile.toPath()
    if (!canonicalPath.startsWith(canonicalRoot)) {
        throw GradleException("$label must stay inside $canonicalRoot: $canonicalPath")
    }
    return this
}

fun File.requireMode0600(label: String): File {
    val expected = setOf(
        PosixFilePermission.OWNER_READ,
        PosixFilePermission.OWNER_WRITE,
    )
    if (Files.getPosixFilePermissions(toPath()) != expected) {
        throw GradleException("$label must have exact mode 0600: $this")
    }
    return this
}

fun decodeLowerHex(value: String): ByteArray {
    if (!value.matches(Regex("^[0-9a-f]+$")) || value.length % 2 != 0) {
        throw GradleException("invalid lowercase hexadecimal value")
    }
    return ByteArray(value.length / 2) { index ->
        value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
}

val candidateSha256 = requiredProperty("kagemushaCandidateSha256")
if (!lowercaseSha256.matches(candidateSha256)) {
    throw GradleException("-PkagemushaCandidateSha256 must be one lowercase SHA-256")
}
val candidateStageSha256 = requiredProperty("kagemushaCandidateStageSha256")
if (!lowercaseSha256.matches(candidateStageSha256)) {
    throw GradleException("-PkagemushaCandidateStageSha256 must be one lowercase SHA-256")
}

val repositoryRoot = rootProject.projectDir.parentFile.canonicalFile
val expectedEvidenceRoot =
    repositoryRoot.resolve(
        "artifacts/kagemusha-candidate-evidence/$candidateSha256/$candidateStageSha256",
    ).canonicalFile
val evidenceRoot = file(requiredProperty("kagemushaCandidateEvidenceRoot")).canonicalFile
if (evidenceRoot != expectedEvidenceRoot) {
    throw GradleException(
        "candidate lab root must be exactly $expectedEvidenceRoot (got $evidenceRoot)",
    )
}
if (evidenceRoot.name != candidateStageSha256 || evidenceRoot.parentFile.name != candidateSha256) {
    throw GradleException("candidate lab directory names must equal candidate/stage SHA-256")
}

val candidateStageManifest =
    evidenceRoot.resolve("candidate-stage-manifest-v1.json")
        .requireWithin(evidenceRoot, "candidate stage manifest")
        .requireRegularFile("candidate stage manifest")
        .requireMode0600("candidate stage manifest")
if (sha256(candidateStageManifest) != candidateStageSha256) {
    throw GradleException("candidate stage manifest SHA-256 does not match its stage directory")
}
val candidateStageManifestJson =
    JsonSlurper().parse(candidateStageManifest) as? Map<*, *>
        ?: throw GradleException("candidate stage manifest must be one JSON object")
val candidateStageManifestFields = setOf(
    "schema",
    "version",
    "stage_manifest_path",
    "stage_manifest_mode",
    "stage_manifest_size_bytes",
    "candidate_record_sha256",
    "candidate_manifest_sha256",
    "candidate_validation_report_sha256",
    "scenario_inventory_sha256",
    "source_commit",
    "source_tree_sha256",
    "source_repo_dirty",
    "validator",
    "entry_count",
    "scenario_entry_count",
    "entries",
)
if (candidateStageManifestJson.keys != candidateStageManifestFields ||
    candidateStageManifestJson["schema"] !=
    "iroha.kagemusha.android_candidate_stage_manifest.v1" ||
    candidateStageManifestJson["version"] != 1 ||
    candidateStageManifestJson["stage_manifest_path"] != candidateStageManifest.name ||
    candidateStageManifestJson["stage_manifest_mode"] != "0600" ||
    (candidateStageManifestJson["stage_manifest_size_bytes"] as? Number)?.toLong() !=
    candidateStageManifest.length() ||
    candidateStageManifestJson["candidate_record_sha256"] != candidateSha256 ||
    candidateStageManifestJson["source_repo_dirty"] != false ||
    candidateStageManifestJson["entry_count"] != 44 ||
    candidateStageManifestJson["scenario_entry_count"] != 33
) {
    throw GradleException("candidate stage manifest top-level identity is not exact")
}

val candidateRecord =
    evidenceRoot.resolve("evidence/candidate/candidate-v4.norito")
        .requireWithin(evidenceRoot, "candidate record")
        .requireRegularFile("candidate record")
if (sha256(candidateRecord) != candidateSha256) {
    throw GradleException("candidate record SHA-256 does not match -PkagemushaCandidateSha256")
}
val candidateManifest =
    evidenceRoot.resolve("evidence/candidate/manifest-v4.norito")
        .requireWithin(evidenceRoot, "candidate manifest")
        .requireRegularFile("candidate manifest")
val candidateManifestSha256 = sha256(candidateManifest)
if (candidateStageManifestJson["candidate_manifest_sha256"] != candidateManifestSha256) {
    throw GradleException("candidate stage manifest does not bind the exact inner manifest")
}

val sourceCommit = requiredProperty("kagemushaCandidateSourceCommit")
if (!lowercaseGitCommit.matches(sourceCommit)) {
    throw GradleException("-PkagemushaCandidateSourceCommit must be lowercase git hex")
}
val sourceTreeSha256 = requiredProperty("kagemushaCandidateSourceTreeSha256")
if (!lowercaseSha256.matches(sourceTreeSha256)) {
    throw GradleException("-PkagemushaCandidateSourceTreeSha256 must be one lowercase SHA-256")
}
if (candidateStageManifestJson["source_commit"] != sourceCommit ||
    candidateStageManifestJson["source_tree_sha256"] != sourceTreeSha256
) {
    throw GradleException("candidate stage manifest source identity does not match Gradle properties")
}
val generation = requiredProperty("kagemushaCandidateGeneration")
if (!generation.matches(Regex("^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$"))) {
    throw GradleException("-PkagemushaCandidateGeneration has invalid characters")
}
val slotId = requiredProperty("kagemushaCandidateSlotId")
if (!slotId.matches(Regex("^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$"))) {
    throw GradleException("-PkagemushaCandidateSlotId has invalid characters")
}

val artifactFiles = listOf(
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
val maxCandidateLabApkBytes = 64L * 1024 * 1024

fun File.requireSmallArtifactFreeApk(label: String): File {
    requireRegularFile(label)
    if (length() <= 0L || length() > maxCandidateLabApkBytes) {
        throw GradleException("$label must remain within the 64 MiB installation corridor: $this")
    }
    ZipFile(this).use { archive ->
        val entries = archive.entries()
        while (entries.hasMoreElements()) {
            val name = entries.nextElement().name
            val baseName = name.substringAfterLast('/').substringAfterLast('\\')
            if (baseName in artifactFiles) {
                throw GradleException("$label embeds forbidden external KRV4 artifact $name")
            }
        }
    }
    return this
}

val scenarioFiles = listOf(
    "init-top-up-anchor-v4.norito",
    "init-top-up-finality-proof-v2.norito",
    "init-top-up-finality-roster-artifact-v2.norito",
    "init-opening-v2.norito",
    "init-output-membership-v4.norito",
    "transfer-verifier-commitment-v2.bin",
    "append-hop-01-recipient-request-v2.norito",
    "append-hop-01-recipient-opening-v2.norito",
    "append-hop-01-change-opening-v2.norito",
    "append-hop-01-output-membership-v4.norito",
    "append-hop-01-operation-id.bin",
    "append-hop-01-block-height.txt",
    "append-hop-01-verified-at-ms.txt",
    "append-hop-02-recipient-request-v2.norito",
    "append-hop-02-recipient-opening-v2.norito",
    "append-hop-02-change-opening-v2.norito",
    "append-hop-02-output-membership-v4.norito",
    "append-hop-02-operation-id.bin",
    "append-hop-02-block-height.txt",
    "append-hop-02-verified-at-ms.txt",
    "redeem-recipient-account-id.txt",
    "unshield-verifier-commitment-v2.bin",
    "redeem-hop-01-operation-id.bin",
    "redeem-hop-01-block-height.txt",
    "redeem-hop-02-operation-id.bin",
    "redeem-hop-02-block-height.txt",
    "redeem-sender-change-operation-id.bin",
    "redeem-sender-change-block-height.txt",
    "duplicate-input-recipient-request-v2.norito",
    "duplicate-input-output-membership-v4.norito",
    "duplicate-input-operation-id.bin",
    "duplicate-input-block-height.txt",
    "duplicate-input-verified-at-ms.txt",
)
artifactFiles.forEach { name ->
    evidenceRoot.resolve("evidence/candidate/artifacts/$name")
        .requireWithin(evidenceRoot, "candidate artifact $name")
        .requireRegularFile("candidate artifact $name")
}
scenarioFiles.forEach { name ->
    evidenceRoot.resolve("scenario/$name")
        .requireWithin(evidenceRoot, "candidate scenario $name")
        .requireRegularFile("candidate scenario $name")
}

val candidateValidationReport =
    evidenceRoot.resolve("evidence/candidate/candidate-validation-v1.json")
        .requireWithin(evidenceRoot, "candidate validation report")
        .requireRegularFile("candidate validation report")
val expectedStagePaths = (
    listOf(
        "evidence/candidate/candidate-v4.norito",
        "evidence/candidate/manifest-v4.norito",
        "evidence/candidate/candidate-validation-v1.json",
    ) +
        artifactFiles.map { "evidence/candidate/artifacts/$it" } +
        scenarioFiles.map { "scenario/$it" }
).sorted()
val stageEntries = candidateStageManifestJson["entries"] as? List<*>
    ?: throw GradleException("candidate stage manifest entries must be an array")
if (stageEntries.size != 44) {
    throw GradleException("candidate stage manifest must contain exactly 44 entries")
}
val measuredStageEntries = linkedMapOf<String, Pair<Long, String>>()
stageEntries.forEachIndexed { index, rawEntry ->
    val entry = rawEntry as? Map<*, *>
        ?: throw GradleException("candidate stage entry $index must be an object")
    if (entry.keys != setOf("path", "mode", "size_bytes", "sha256")) {
        throw GradleException("candidate stage entry $index must have exact V1 fields")
    }
    val relative = entry["path"] as? String
        ?: throw GradleException("candidate stage entry $index path must be text")
    if (relative !in expectedStagePaths || entry["mode"] != "0600") {
        throw GradleException("candidate stage entry $index path/mode is not canonical")
    }
    val stagedFile = evidenceRoot.resolve(relative)
        .requireWithin(evidenceRoot, "candidate stage entry $relative")
        .requireRegularFile("candidate stage entry $relative")
        .requireMode0600("candidate stage entry $relative")
    val declaredSize = (entry["size_bytes"] as? Number)?.toLong()
    val declaredSha256 = entry["sha256"] as? String
    if (declaredSize != stagedFile.length() ||
        declaredSize <= 0L ||
        declaredSha256 == null ||
        !lowercaseSha256.matches(declaredSha256) ||
        sha256(stagedFile) != declaredSha256
    ) {
        throw GradleException("candidate stage entry $relative size/SHA-256 is not exact")
    }
    measuredStageEntries[relative] = declaredSize to declaredSha256
}
if (measuredStageEntries.keys.toList() != expectedStagePaths ||
    measuredStageEntries.size != expectedStagePaths.size
) {
    throw GradleException("candidate stage manifest inventory is missing, extra, or unsorted")
}
if (candidateStageManifestJson["candidate_validation_report_sha256"] !=
    sha256(candidateValidationReport)
) {
    throw GradleException("candidate stage manifest does not bind its validation report")
}
val scenarioInventory = MessageDigest.getInstance("SHA-256")
scenarioInventory.update(
    "iroha.kagemusha.android-candidate-scenario-inventory.v1\u0000"
        .toByteArray(Charsets.US_ASCII),
)
scenarioInventory.update(ByteBuffer.allocate(4).order(ByteOrder.BIG_ENDIAN).putInt(33).array())
expectedStagePaths.filter { it.startsWith("scenario/") }.forEach { relative ->
    val pathBytes = relative.toByteArray(Charsets.UTF_8)
    val (size, digest) = checkNotNull(measuredStageEntries[relative])
    scenarioInventory.update(
        ByteBuffer.allocate(4).order(ByteOrder.BIG_ENDIAN).putInt(pathBytes.size).array(),
    )
    scenarioInventory.update(pathBytes)
    scenarioInventory.update(ByteBuffer.allocate(8).order(ByteOrder.BIG_ENDIAN).putLong(size).array())
    scenarioInventory.update(decodeLowerHex(digest))
}
val measuredScenarioInventorySha256 =
    scenarioInventory.digest().joinToString("") { "%02x".format(it) }
if (candidateStageManifestJson["scenario_inventory_sha256"] !=
    measuredScenarioInventorySha256
) {
    throw GradleException("candidate stage manifest scenario inventory digest is not exact")
}
val stageValidator = candidateStageManifestJson["validator"] as? Map<*, *>
    ?: throw GradleException("candidate stage validator identity must be an object")
val stageValidatorFields = setOf(
    "schema", "candidate_binary_name", "candidate_binary_sha256",
    "scenario_binary_name", "scenario_binary_sha256", "cargo_binary_sha256",
    "cargo_version_verbose", "rustc_binary_sha256", "rustc_version_verbose",
    "locked", "offline", "isolated_target", "build_jobs", "candidate_package",
    "scenario_package", "features", "profile",
)
if (stageValidator.keys != stageValidatorFields ||
    stageValidator["schema"] != "iroha.kagemusha.android_candidate_validator.v1" ||
    stageValidator["candidate_binary_name"] != "kagemusha_recursive_spend_v4_bundle" ||
    stageValidator["scenario_binary_name"] != "kagemusha_candidate_scenario_validator" ||
    stageValidator["locked"] != true ||
    stageValidator["offline"] != true ||
    stageValidator["isolated_target"] != true ||
    stageValidator["build_jobs"] != 2 ||
    stageValidator["candidate_package"] != "iroha_core" ||
    stageValidator["scenario_package"] != "connect_norito_bridge" ||
    stageValidator["features"] != listOf("kagemusha-candidate-evidence-lab") ||
    stageValidator["profile"] != "debug"
) {
    throw GradleException("candidate stage validator identity is not exact")
}
listOf(
    "candidate_binary_sha256", "scenario_binary_sha256",
    "cargo_binary_sha256", "rustc_binary_sha256",
).forEach { key ->
    val value = stageValidator[key] as? String
    if (value == null || !lowercaseSha256.matches(value) || value == "0".repeat(64)) {
        throw GradleException("candidate stage validator $key is invalid")
    }
}
listOf("cargo_version_verbose", "rustc_version_verbose").forEach { key ->
    val value = stageValidator[key] as? String
    if (value == null || value.isBlank() || value.toByteArray().size > 64 * 1024 ||
        !value.endsWith("\n") || value.any { it == '\u0000' || it == '\r' }
    ) {
        throw GradleException("candidate stage validator $key is invalid")
    }
}

val nativeLibrary =
    file(requiredProperty("kagemushaCandidateLabNativeLibrary"))
        .canonicalFile
        .requireWithin(evidenceRoot, "candidate lab native library")
        .requireRegularFile("candidate lab native library")
val expectedNativeLibrary =
    evidenceRoot.resolve(
        "evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so",
    ).canonicalFile
if (nativeLibrary != expectedNativeLibrary) {
    throw GradleException("candidate lab native library must be exactly $expectedNativeLibrary")
}
val nativeLibrarySha256 = sha256(nativeLibrary)
val generatedAssets = layout.buildDirectory.dir("generated/candidateLabAssets")
val generatedJni = layout.buildDirectory.dir("generated/candidateLabJni")
val markerApkName = "kagemusha-candidate-evidence-lab-DO-NOT-SHIP-$candidateSha256-debug.apk"
val stagedApkName = markerApkName
val stagedTestApkName =
    "kagemusha-candidate-evidence-lab-DO-NOT-SHIP-$candidateSha256-debug-androidTest.apk"

// All intermediates and final APKs are candidate-bound and remain beneath the
// evidence root.  Nothing from this module is emitted into kotlin/*/build or a
// Maven repository.
layout.buildDirectory.set(evidenceRoot.resolve("gradle/kagemusha-candidate-evidence-lab"))

val prepareCandidateLabAssets by tasks.registering(Sync::class) {
    from(candidateStageManifest) {
        into("stage")
    }
    from(evidenceRoot.resolve("evidence/candidate")) {
        into("candidate")
        include(
            "candidate-v4.norito",
            "manifest-v4.norito",
            "candidate-validation-v1.json",
        )
    }
    from(evidenceRoot.resolve("scenario")) {
        into("scenario")
        include(scenarioFiles)
    }
    into(generatedAssets)
    includeEmptyDirs = false
}

val prepareCandidateLabNative by tasks.registering(Sync::class) {
    from(nativeLibrary) {
        rename { "libconnect_norito_bridge_candidate_lab.so" }
    }
    into(generatedJni.map { it.dir("arm64-v8a") })
    doFirst {
        if (!nativeLibrary.containsAscii(candidateLabMarker) ||
            !nativeLibrary.containsAscii(
                "Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_",
            )
        ) {
            throw GradleException(
                "candidate lab native library is missing its DO-NOT-SHIP marker or JNI namespace",
            )
        }
    }
}

android {
    namespace = "org.hyperledger.iroha.sdk.kagemusha.candidate.lab"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.hyperledger.iroha.sdk.kagemusha.candidate.lab"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.0.0-candidate-lab"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        testInstrumentationRunnerArguments["clearPackageData"] = "false"
        ndk {
            abiFilters += "arm64-v8a"
        }
        buildConfigField("String", "CANDIDATE_LAB_MARKER", "\"$candidateLabMarker\"")
        buildConfigField("String", "CANDIDATE_RECORD_SHA256", "\"$candidateSha256\"")
        buildConfigField("String", "CANDIDATE_MANIFEST_SHA256", "\"$candidateManifestSha256\"")
        buildConfigField("String", "CANDIDATE_STAGE_MANIFEST_SHA256", "\"$candidateStageSha256\"")
        buildConfigField("String", "SOURCE_COMMIT", "\"$sourceCommit\"")
        buildConfigField("String", "SOURCE_TREE_SHA256", "\"$sourceTreeSha256\"")
        buildConfigField("String", "GENERATION", "\"$generation\"")
        buildConfigField("String", "SLOT_ID", "\"$slotId\"")
        buildConfigField("String", "NATIVE_LIBRARY_SHA256", "\"$nativeLibrarySha256\"")
        buildConfigField("String", "LAB_APK_FILE_NAME", "\"$stagedApkName\"")
        buildConfigField("String", "LAB_TEST_APK_FILE_NAME", "\"$stagedTestApkName\"")
    }

    buildTypes {
        debug {
            isDebuggable = true
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        buildConfig = true
    }

    sourceSets {
        getByName("main") {
            assets.srcDir(generatedAssets)
            jniLibs.srcDir(generatedJni)
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }

    kotlin {
        jvmToolchain(21)
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_1_8)
            freeCompilerArgs.add("-Xjdk-release=8")
        }
    }

    packaging {
        jniLibs.useLegacyPackaging = true
    }
}

androidComponents {
    beforeVariants(selector().withBuildType("release")) { variant ->
        variant.enable = false
    }
    onVariants(selector().withBuildType("debug")) { variant ->
        tasks.register<Copy>("stageCandidateLabApk") {
            from(variant.artifacts.get(SingleArtifact.APK))
            include("*.apk")
            into(evidenceRoot.resolve("evidence"))
            rename { stagedApkName }
            doLast {
                evidenceRoot.resolve("evidence/$stagedApkName")
                    .requireSmallArtifactFreeApk("candidate lab main APK")
            }
        }
    }
}

val candidateLabTestApkOutput =
    layout.buildDirectory.dir("outputs/apk/androidTest/debug")
tasks.register<Copy>("stageCandidateLabTestApk") {
    dependsOn("assembleDebugAndroidTest")
    from(candidateLabTestApkOutput)
    include("*-androidTest.apk")
    into(evidenceRoot.resolve("evidence"))
    rename { stagedTestApkName }
    doFirst {
        val testApks = candidateLabTestApkOutput.get().asFile
            .listFiles()
            .orEmpty()
            .filter { it.isFile && it.name.endsWith("-androidTest.apk") }
        if (testApks.size != 1) {
            throw GradleException(
                "expected exactly one debug androidTest APK, found ${testApks.size}",
            )
        }
    }
    doLast {
        evidenceRoot.resolve("evidence/$stagedTestApkName")
            .requireSmallArtifactFreeApk("candidate lab androidTest APK")
    }
}

tasks.configureEach {
    if (name.contains("Debug", ignoreCase = true)) {
        dependsOn(prepareCandidateLabAssets, prepareCandidateLabNative)
    }
}

dependencies {
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
}
