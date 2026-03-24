pluginManagement {
    repositories {
        google()
        gradlePluginPortal()
        mavenCentral()
    }
}

plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}
rootProject.name = "iroha_kotlin_sdk"

include(":core-jvm")
include(":client-android")
include(":offline-wallet-android")
