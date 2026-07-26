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
apply(from = "../gradle/mobile-sdk-external-android-build.settings.gradle.kts")

include(":core-jvm")
include(":client-android")
include(":offline-wallet-android")

// The candidate evidence lab is an intentionally non-shipping Android
// application.  It is absent from the normal project graph (and therefore
// every Maven/release task) unless an operator opts in explicitly.
if (providers.gradleProperty("kagemushaCandidateEvidenceLab").orNull == "true") {
    include(":kagemusha-candidate-evidence-lab")
}
