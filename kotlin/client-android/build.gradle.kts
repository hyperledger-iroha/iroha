import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.util.Properties

plugins {
    alias(libs.plugins.android.library)
    `maven-publish`
}

group = "org.hyperledger.iroha.sdk"
version = providers.gradleProperty("irohaSdkVersion")
    .orElse(providers.environmentVariable("IROHA_SDK_VERSION"))
    .orElse("0.1-SNAPSHOT")
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
    coreLibraryDesugaring(libs.desugar.jdk.libs)
}

val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs")

fun irohaDir(): String {
    val props = Properties()
    val file = rootProject.file("local.properties")
    if (file.exists()) file.inputStream().use { props.load(it) }
    return props.getProperty("iroha.dir") ?: rootProject.file("..").absolutePath
}

tasks.register<Exec>("buildNativeLibs") {
    group = "native"
    description = "Build connect_norito_bridge .so from Rust source (requires cargo-ndk + Android NDK)"

    // Pass -PprivacyProductionEnabled=true to enable real proving in the native bridge.
    // Default is off (fail-closed); flip only when all production gates have passed.
    val privacyProductionEnabled =
        project.findProperty("privacyProductionEnabled")?.toString()?.toBoolean() ?: false
    val cargoFeatureArgs = if (privacyProductionEnabled) {
        listOf("--features", "privacy-production-enabled")
    } else {
        emptyList()
    }

    workingDir = file(irohaDir())
    commandLine(
        buildList {
            addAll(listOf("cargo", "ndk", "-t", "arm64-v8a", "-t", "x86_64", "-o", jniLibsDir.asFile.absolutePath))
            addAll(listOf("build", "--release", "-p", "connect_norito_bridge"))
            addAll(cargoFeatureArgs)
        }
    )
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
