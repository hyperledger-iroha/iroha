import java.io.File
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlin.serialization)
    `maven-publish`
    `java-library`
}

group = "org.hyperledger.iroha.sdk"
version = providers.gradleProperty("irohaSdkVersion")
    .orElse(providers.environmentVariable("IROHA_SDK_VERSION"))
    .orElse("0.1.0")
    .get()

val mobileSdkRepoDir = providers.gradleProperty("irohaSdkRepoDir")
    .orElse(rootProject.layout.buildDirectory.dir("mobile-sdk-maven").map { it.asFile.absolutePath })

repositories {
    mavenCentral()
}

dependencies {
    api(libs.okhttp)
    api(platform(libs.netty.bom))
    api(libs.netty.transport)
    implementation(libs.netty.handler)
    implementation(libs.netty.codec.http)
    testImplementation(libs.mockwebserver)
    implementation(libs.zstd.jni)
    implementation(libs.bcprov)
    implementation(libs.serialization.json)
    testImplementation(kotlin("test"))
    testImplementation(libs.junit.params)
}

java {
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

// Java consumers are compiled against JDK 8 APIs, just like the Kotlin API.
tasks.withType<JavaCompile>().configureEach {
    options.release.set(8)
}

tasks.test {
    enableAssertions = true
    useJUnitPlatform {
        excludeTags("cuda-hardware")
    }
    inputs.file(
        rootProject.layout.projectDirectory
            .dir("..")
            .file("fixtures/norito_rpc/atomic_private_settlement_sdk_v1.json"),
    )
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/musubi/sdk_v1.json"))
    inputs.file(
        rootProject.layout.projectDirectory
            .dir("..")
            .file("fixtures/musubi/instructions_v1.json"),
    )
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/crypto/ed25519_public_key_admission_v1.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/account/multisig_wire_v1.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/multisig/instruction_batch_hash_v1.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/sumeragi_v2/wire_v2.tsv"))
    inputs.file(
        rootProject.layout.projectDirectory
            .dir("..")
            .file("fixtures/sumeragi_v2/native_amx_v2_grouped.json"),
    )
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/numeric_v1_golden.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/offline/kagemusha_v1.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/offline/kagemusha_enrolled_open_selector_v1.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/offline/kagemusha_enrolled_open_challenge_v1.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/offline/kagemusha_sender_reservation_v1.json"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv"))
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/offline/kagemusha_core_coordinator_archives_v1.json"))
    inputs.file(
        rootProject.layout.projectDirectory
            .dir("..")
            .file("fixtures/kotodama/entrypoint_argument_record_v1.json"),
    )
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/zk/verifying_key_record_v1.json"))
    inputs.file(
        rootProject.layout.projectDirectory
            .dir("..")
            .file("fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64"),
    )
    inputs.dir(
        rootProject.layout.projectDirectory
            .dir("..")
            .dir("fixtures/sorafs_manifest/appeal_finance"),
    )
    inputs.dir(
        rootProject.layout.projectDirectory
            .dir("..")
            .dir("fixtures/sorafs_manifest/reference_sdk"),
    )

    // Release CI supplies a freshly built, isolated ABI-23 bridge. Local
    // development retains the conventional root target/debug fallback.
    val configuredNativeDir = System.getenv("IROHA_NATIVE_LIBRARY_PATH")
    val hostNativeDir = if (configuredNativeDir.isNullOrBlank()) {
        rootProject.projectDir.parentFile.resolve("target/debug")
    } else {
        file(configuredNativeDir)
    }
    systemProperty("java.library.path", hostNativeDir.absolutePath)
}

// Device qualification is explicit and must execute against the bridge built by
// the calling job. A cached result or a missing CUDA device is not qualification.
val cudaNativeDirectory = providers.environmentVariable("IROHA_NATIVE_LIBRARY_PATH")
tasks.register<Test>("cudaHardwareTest") {
    description = "Qualify every Kotlin/Java CUDA operation against CPU reference results."
    group = "verification"
    testClassesDirs = sourceSets["test"].output.classesDirs
    classpath = sourceSets["test"].runtimeClasspath
    enableAssertions = true
    useJUnitPlatform {
        includeTags("cuda-hardware")
    }
    filter {
        includeTestsMatching("org.hyperledger.iroha.sdk.gpu.CudaAcceleratorsHardwareTest")
        isFailOnNoMatchingTests = true
    }
    maxParallelForks = 1
    outputs.upToDateWhen { false }
    outputs.doNotCacheIf("CUDA device state must be qualified on every invocation") { true }
    doFirst {
        val nativeDirectory = cudaNativeDirectory.orNull
        require(!nativeDirectory.isNullOrBlank()) {
            "cudaHardwareTest requires IROHA_NATIVE_LIBRARY_PATH for the freshly built CUDA bridge"
        }
        val directory = File(nativeDirectory)
        require(directory.isAbsolute && directory.isDirectory) {
            "IROHA_NATIVE_LIBRARY_PATH must be an absolute existing directory"
        }
        val library = directory.resolve(System.mapLibraryName("connect_norito_bridge"))
        require(library.isFile) { "Fresh CUDA bridge is missing: $library" }
        systemProperty("iroha.cuda.nativeLibrary", library.absolutePath)
    }
}

publishing {
    repositories {
        maven {
            name = "mobileSdk"
            url = uri(mobileSdkRepoDir.get())
        }
    }

    publications {
        create<MavenPublication>("release") {
            from(components["java"])
            groupId = "org.hyperledger.iroha.sdk"
            artifactId = "core-jvm"
        }
    }
}
