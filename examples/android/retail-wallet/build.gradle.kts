import groovy.json.JsonOutput
import java.io.File
import java.security.MessageDigest
import java.time.Instant

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val walletToriiEndpoint = providers.gradleProperty("retailWalletToriiEndpoint")
    .orElse("https://wallet.torii.sora.org")
val walletFeatureFlags = providers.gradleProperty("retailWalletFeatures").orElse("")
val walletConfigProfile = providers.gradleProperty("retailWalletConfigProfile").orElse("default")
val walletGitCommit = providers.environmentVariable("GIT_COMMIT").orElse("UNKNOWN")
val walletManifestOutput = layout.buildDirectory.file("sample-manifest/sample_manifest.json")
val walletGraceOverrideMs = providers.gradleProperty("retailWalletVerdictGracePeriodMs").orElse("0")
val walletGraceProfile = providers.gradleProperty("retailWalletVerdictGraceProfile").orElse("")
val assetsDir = layout.projectDirectory.dir("src/main/assets")

android {
    namespace = "org.hyperledger.iroha.samples.wallet"
    compileSdk = 34

    defaultConfig {
        applicationId = "org.hyperledger.iroha.samples.wallet"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        buildConfigField("String", "TORII_ENDPOINT", "\"${walletToriiEndpoint.get()}\"")
        val graceOverrideValue = walletGraceOverrideMs.get().toLongOrNull() ?: 0L
        buildConfigField("long", "VERDICT_GRACE_PERIOD_OVERRIDE_MS", "${graceOverrideValue}L")
        val graceProfileValue = walletGraceProfile.get()
        val escapedProfile = graceProfileValue.replace("\\", "\\\\").replace("\"", "\\\"")
        buildConfigField("String", "VERDICT_GRACE_PERIOD_PROFILE", "\"$escapedProfile\"")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        viewBinding = true
    }
}

dependencies {
    implementation(libs.bundles.androidxUi)
    implementation(project(":android-sdk"))
    implementation(libs.jxingCore)

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidxTestExtJunit)
    androidTestImplementation(libs.androidxTestEspressoCore)
}

fun sha256Of(file: File): String =
    MessageDigest.getInstance("SHA-256")
        .digest(file.readBytes())
        .joinToString(separator = "") { "%02x".format(it) }

tasks.register("generateSampleManifest") {
    val sdkVersionProvider = providers.provider { project(":android-sdk").version.toString() }
    inputs.property("sdkVersion", sdkVersionProvider)
    inputs.property("toriiEndpoint", walletToriiEndpoint)
    inputs.property("featureFlags", walletFeatureFlags)
    inputs.property("configProfile", walletConfigProfile)
    inputs.property("gitCommit", walletGitCommit)
    inputs.property("graceOverrideMs", walletGraceOverrideMs)
    inputs.property("graceProfile", walletGraceProfile)
    inputs.files(
        assetsDir.file("pos_manifest.json"),
        assetsDir.file("security_policy.json"),
        assetsDir.file("offline_revocations.json"),
        assetsDir.file("pinned_root.pem")
    )
    outputs.file(walletManifestOutput)

    doLast {
        val features = walletFeatureFlags.orNull
            ?.split(',')
            ?.map { it.trim() }
            ?.filter { it.isNotEmpty() }
            ?: emptyList()
        val graceOverrideValue = walletGraceOverrideMs.get().toLongOrNull() ?: 0L
        val policy =
            linkedMapOf<String, Any?>(
                "verdict_grace_period_override_ms" to graceOverrideValue,
                "verdict_grace_period_profile" to walletGraceProfile.get().ifBlank { null }
            ).filterValues { it != null }

        val assets =
            linkedMapOf<String, Any?>()
        val manifestFile = assetsDir.file("pos_manifest.json").asFile
        val policyFile = assetsDir.file("security_policy.json").asFile
        val revocationsFile = assetsDir.file("offline_revocations.json").asFile
        val pinnedRootFile = assetsDir.file("pinned_root.pem").asFile
        if (manifestFile.exists()) {
            assets["pos_manifest_json"] =
                linkedMapOf(
                    "path" to manifestFile.relativeTo(projectDir).path,
                    "sha256" to sha256Of(manifestFile)
                )
        }
        if (policyFile.exists()) {
            assets["security_policy_json"] =
                linkedMapOf(
                    "path" to policyFile.relativeTo(projectDir).path,
                    "sha256" to sha256Of(policyFile)
                )
        }
        if (revocationsFile.exists()) {
            assets["offline_revocations_json"] =
                linkedMapOf(
                    "path" to revocationsFile.relativeTo(projectDir).path,
                    "sha256" to sha256Of(revocationsFile)
                )
        }
        if (pinnedRootFile.exists()) {
            assets["pinned_root_pem"] =
                linkedMapOf(
                    "path" to pinnedRootFile.relativeTo(projectDir).path,
                    "sha256" to sha256Of(pinnedRootFile)
                )
        }

        val manifestJson =
            JsonOutput.prettyPrint(
                JsonOutput.toJson(
                    linkedMapOf(
                        "sample" to "retail-wallet",
                        "sdk_version" to sdkVersionProvider.get(),
                        "git_commit" to walletGitCommit.get(),
                        "generated_at" to Instant.now().toString(),
                        "config_profile" to walletConfigProfile.get(),
                        "torii_endpoint" to walletToriiEndpoint.get(),
                        "feature_flags" to features,
                        "policy" to policy,
                        "assets" to assets
                    )
                )
            )

        val outputFile = walletManifestOutput.get().asFile
        outputFile.parentFile.mkdirs()
        outputFile.writeText("$manifestJson\n")
    }
}

tasks.register("printSampleManifest") {
    dependsOn("generateSampleManifest")
    doLast {
        println(walletManifestOutput.get().asFile.readText())
    }
}
