import org.gradle.api.GradleException

plugins {
    id("com.android.application")
}

val sdkVersion = providers.gradleProperty("irohaAndroidVersion").orElse("0.1.0-SNAPSHOT")
val repoUrlProp = providers.gradleProperty("irohaAndroidRepoUrl").orNull
val repoDirProp = providers.gradleProperty("irohaAndroidRepoDir").orNull
val defaultRepoDir = rootProject.layout.projectDirectory.dir("../artifacts/android/maven")
val sampleRepoDir =
    providers.gradleProperty("irohaAndroidSampleRepoDir").orElse(defaultRepoDir.asFile.absolutePath)
val sampleUsePublished =
    providers.provider {
        val globalToggle = providers.gradleProperty("irohaAndroidUsePublished").orNull
        val sampleToggle = providers.gradleProperty("irohaAndroidSampleUsePublished").orNull
        val envToggle = System.getenv("ANDROID_SAMPLE_USE_PUBLISHED")
        val fallbackToggle =
            if (!repoUrlProp.isNullOrBlank() || !repoDirProp.isNullOrBlank() || defaultRepoDir.asFile.exists()) {
                "true"
            } else {
                "false"
            }
        (globalToggle ?: sampleToggle ?: envToggle ?: fallbackToggle).equals("true", ignoreCase = true)
    }
val resolvedRepoDir = repoDirProp ?: sampleRepoDir.get()

android {
    namespace = "org.hyperledger.iroha.android.samples"
    compileSdk = 34

    defaultConfig {
        applicationId = "org.hyperledger.iroha.android.samples"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
        isCoreLibraryDesugaringEnabled = true
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }
}

repositories {
    google()
    mavenCentral()
    if (sampleUsePublished.get()) {
        mavenLocal()
        if (!repoUrlProp.isNullOrBlank()) {
            maven {
                url = uri(repoUrlProp)
                isAllowInsecureProtocol = true
            }
        }
        if (resolvedRepoDir.isNotBlank()) {
            maven {
                url = uri(file(resolvedRepoDir))
                isAllowInsecureProtocol = true
            }
        }
    }
}

dependencies {
    if (sampleUsePublished.get()) {
        val repoPath = file(resolvedRepoDir)
        if (repoUrlProp.isNullOrBlank() && !repoPath.exists()) {
            throw GradleException(
                "irohaAndroidUsePublished=true but repo ${repoPath.absolutePath} is missing. " +
                    "Set irohaAndroidRepoDir/irohaAndroidRepoUrl to the published Maven repository or disable published mode.",
            )
        }
        implementation("org.hyperledger.iroha:iroha-android:${sdkVersion.get()}")
    } else {
        implementation(project(":android"))
    }
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.0.4")
    testImplementation("junit:junit:4.13.2")
}
