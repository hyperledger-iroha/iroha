plugins {
    alias(libs.plugins.android.application)
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"

android {
    namespace = "org.hyperledger.iroha.sdk.offline.wallet.lab"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.hyperledger.iroha.sdk.offline.wallet.lab"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    testBuildType = "release"

    signingConfigs {
        getByName("debug")
    }

    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName("debug")
            isDebuggable = true
            isMinifyEnabled = false
        }
    }

    sourceSets {
        getByName("androidTest") {
            java.srcDir("../offline-wallet-android/src/androidTest/java")
            assets.srcDir("../../fixtures/offline")
        }
    }
}

repositories {
    google()
    mavenCentral()
}

dependencies {
    implementation(project(":offline-wallet-android"))
    androidTestImplementation(project(":offline-wallet-android"))
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
}
