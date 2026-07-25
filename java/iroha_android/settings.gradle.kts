pluginManagement {
    repositories {
        maven { url = uri("../../.gradle/local-maven") }
        google()
        gradlePluginPortal()
        mavenCentral()
    }
}

rootProject.name = "iroha-android"
apply(from = "../../gradle/mobile-sdk-external-android-build.settings.gradle.kts")

includeBuild("../norito_java") {
    name = "norito_java"
    dependencySubstitution {
        substitute(module("org.hyperledger.iroha:norito-java")).using(project(":"))
    }
}

// Reuse the default pure JVM transport state machines from Java instead of
// maintaining a second cryptographic IPN1/NFC implementation.
includeBuild("../../kotlin") {
    name = "iroha_kotlin_sdk"
    dependencySubstitution {
        substitute(module("org.hyperledger.iroha.sdk:core-jvm")).using(project(":core-jvm"))
        substitute(module("org.hyperledger.iroha.sdk:client-android")).using(project(":client-android"))
    }
}

include("jvm")
project(":jvm").projectDir = file("jvm")

include("android")
project(":android").projectDir = file("android")

include("core")
project(":core").projectDir = file("core")

include("samples-android")
project(":samples-android").projectDir = file("samples-android")
