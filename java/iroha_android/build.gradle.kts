plugins {
    id("org.cyclonedx.bom") version "3.1.0" apply false
    id("com.android.library") version "8.13.2" apply false
    id("com.android.application") version "8.13.2" apply false
}

val sdkVersion = providers.gradleProperty("irohaAndroidVersion").orElse("0.1.0-SNAPSHOT")

allprojects {
    group = "org.hyperledger.iroha"
    version = sdkVersion.get()
    repositories {
        google()
        mavenCentral()
        val repoUrlProp = providers.gradleProperty("irohaAndroidRepoUrl").orNull
        val repoDirProp = providers.gradleProperty("irohaAndroidRepoDir").orNull
        if (!repoUrlProp.isNullOrBlank()) {
            maven { url = uri(repoUrlProp) }
        } else if (!repoDirProp.isNullOrBlank()) {
            maven { url = uri(file(repoDirProp)) }
        }
    }
}
