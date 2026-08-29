import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlin.serialization)
    `maven-publish`
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

tasks.test {
    useJUnitPlatform()
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
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/sumeragi_v2/wire_v2.tsv"))
    inputs.file(
        rootProject.layout.projectDirectory
            .dir("..")
            .file("fixtures/sumeragi_v2/native_amx_v2_grouped.json"),
    )
    inputs.file(rootProject.layout.projectDirectory.dir("..").file("fixtures/numeric_v1_golden.json"))
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
    inputs.file(
        rootProject.layout.projectDirectory
            .dir("..")
            .file("fixtures/offline/kagemusha_peer_transport_v2.json"),
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
