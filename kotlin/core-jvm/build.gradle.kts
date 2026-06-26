import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlin.serialization)
    `maven-publish`
}

group = "org.hyperledger.iroha.sdk"
version = providers.gradleProperty("irohaSdkVersion")
    .orElse(providers.environmentVariable("IROHA_SDK_VERSION"))
    .orElse("0.1-SNAPSHOT")
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

    // Host JNI bridge (libconnect_norito_bridge) for the opt-in localnet integration tests.
    // Build it from the iroha repo root with: cargo build -p connect_norito_bridge --lib
    // Pure-JVM tests ignore this; the localnet tests are gated on IROHA_LOCALNET_TEST=1.
    val hostNativeDir = rootProject.projectDir.parentFile.resolve("target/debug")
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
