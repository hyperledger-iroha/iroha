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
include(":operator-console", ":retail-wallet")
includeBuild("../../kotlin") {
    dependencySubstitution {
        substitute(module("org.hyperledger.iroha.sdk:client-android"))
            .using(project(":client-android"))
    }
}
