import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.library)
    `maven-publish`
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"

android {
    namespace = "org.hyperledger.iroha.sdk.offline.wallet"
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
    api(project(":client-android"))
}

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "org.hyperledger.iroha.sdk"
                artifactId = "offline-wallet-android"
            }
        }
    }
}
