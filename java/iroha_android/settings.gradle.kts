pluginManagement {
    repositories {
        maven { url = uri("../../.gradle/local-maven") }
        google()
        gradlePluginPortal()
        mavenCentral()
    }
}

rootProject.name = "iroha-android"

includeBuild("../norito_java") {
    name = "norito_java"
    dependencySubstitution {
        substitute(module("org.hyperledger.iroha:norito-java")).using(project(":"))
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
