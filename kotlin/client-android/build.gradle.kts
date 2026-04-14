import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.util.Properties

plugins {
    alias(libs.plugins.android.library)
    `maven-publish`
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"

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

dependencies {
    api(project(":core-jvm"))
}

val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs")

fun irohaDir(): String {
    val props = Properties()
    val file = rootProject.file("local.properties")
    if (file.exists()) file.inputStream().use { props.load(it) }
    return props.getProperty("iroha.dir") ?: rootProject.file("../..").absolutePath
}

tasks.register<Exec>("buildNativeLibs") {
    group = "native"
    description = "Build connect_norito_bridge .so from Rust source (requires cargo-ndk + Android NDK)"

    workingDir = file(irohaDir())
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a",
        "-t", "x86_64",
        "-o", jniLibsDir.asFile.absolutePath,
        "build", "--release",
        "-p", "connect_norito_bridge",
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
