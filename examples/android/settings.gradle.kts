pluginManagement {
    repositories {
        google()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "IrohaAndroidSamples"
include(":operator-console", ":retail-wallet", ":android-sdk")
project(":android-sdk").projectDir = file("../../java/iroha_android")
