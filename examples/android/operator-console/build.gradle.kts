import java.time.Instant

import groovy.json.JsonOutput

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

fun Provider<String>.escaped(defaultValue: String): String =
    this.orElse(defaultValue).get().replace("\\", "\\\\").replace("\"", "\\\"")

val envSampleProfile = providers.environmentVariable("ANDROID_SAMPLE_PROFILE")
val envSampleToriiUrl = providers.environmentVariable("ANDROID_SAMPLE_TORII_URL")
val envSampleToriiAccounts = providers.environmentVariable("ANDROID_SAMPLE_TORII_ACCOUNTS")
val envSampleToriiLog = providers.environmentVariable("ANDROID_SAMPLE_TORII_LOG")
val envSampleToriiMetrics = providers.environmentVariable("ANDROID_SAMPLE_TORII_METRICS")
val envSampleSorafsScoreboard = providers.environmentVariable("ANDROID_SAMPLE_SORAFS_SCOREBOARD")
val envSampleSorafsScoreboardSha =
    providers.environmentVariable("ANDROID_SAMPLE_SORAFS_SCOREBOARD_SHA256")
val envSampleSorafsSummary = providers.environmentVariable("ANDROID_SAMPLE_SORAFS_SUMMARY")
val envSampleSorafsSummarySha =
    providers.environmentVariable("ANDROID_SAMPLE_SORAFS_SUMMARY_SHA256")
val envSampleSorafsReceipts = providers.environmentVariable("ANDROID_SAMPLE_SORAFS_RECEIPTS")
val envSampleTelemetryLog = providers.environmentVariable("ANDROID_SAMPLE_TELEMETRY_LOG")
val envSampleHandoffEndpoint = providers.environmentVariable("ANDROID_SAMPLE_HANDOFF_ENDPOINT")
val envSampleHandoffInbox = providers.environmentVariable("ANDROID_SAMPLE_HANDOFF_INBOX")
val envSampleTelemetrySaltHex = providers.environmentVariable("ANDROID_SAMPLE_TELEMETRY_SALT_HEX")
val envSampleTelemetrySaltVersion =
    providers.environmentVariable("ANDROID_SAMPLE_TELEMETRY_SALT_VERSION")
val envSampleTelemetryRotationId =
    providers.environmentVariable("ANDROID_SAMPLE_TELEMETRY_ROTATION_ID")
val envSampleTelemetryExporter =
    providers.environmentVariable("ANDROID_SAMPLE_TELEMETRY_EXPORTER")

val defaultTelemetrySaltHex =
    "9f14c3d5d6e7f809aabbccddeeff11223344556677889900aabbccddeeff0011"
val defaultTelemetrySaltVersion = "2027Q2-sample"
val defaultTelemetryRotationId = "operator-sample"
val defaultTelemetryExporter = "operator-console"

android {
    namespace = "org.hyperledger.iroha.samples.operator"
    compileSdk = 34

    defaultConfig {
        applicationId = "org.hyperledger.iroha.samples.operator"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        buildConfigField(
            "String",
            "SAMPLE_PROFILE",
            "\"${envSampleProfile.escaped("operator")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TORII_URL",
            "\"${envSampleToriiUrl.escaped("https://torii.dev.sora.org")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TORII_ACCOUNTS",
            "\"${envSampleToriiAccounts.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TORII_LOG",
            "\"${envSampleToriiLog.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TORII_METRICS",
            "\"${envSampleToriiMetrics.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_SORAFS_SCOREBOARD",
            "\"${envSampleSorafsScoreboard.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_SORAFS_SUMMARY",
            "\"${envSampleSorafsSummary.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_SORAFS_SCOREBOARD_SHA256",
            "\"${envSampleSorafsScoreboardSha.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_SORAFS_SUMMARY_SHA256",
            "\"${envSampleSorafsSummarySha.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_SORAFS_RECEIPTS",
            "\"${envSampleSorafsReceipts.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TELEMETRY_LOG",
            "\"${envSampleTelemetryLog.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TELEMETRY_SALT_HEX",
            "\"${envSampleTelemetrySaltHex.escaped(defaultTelemetrySaltHex)}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TELEMETRY_SALT_VERSION",
            "\"${envSampleTelemetrySaltVersion.escaped(defaultTelemetrySaltVersion)}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TELEMETRY_ROTATION_ID",
            "\"${envSampleTelemetryRotationId.escaped(defaultTelemetryRotationId)}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_TELEMETRY_EXPORTER",
            "\"${envSampleTelemetryExporter.escaped(defaultTelemetryExporter)}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_HANDOFF_ENDPOINT",
            "\"${envSampleHandoffEndpoint.escaped("")}\""
        )
        buildConfigField(
            "String",
            "SAMPLE_HANDOFF_INBOX",
            "\"${envSampleHandoffInbox.escaped("")}\""
        )
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

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidxTestExtJunit)
    androidTestImplementation(libs.androidxTestEspressoCore)
}

val operatorToriiEndpoint = providers.gradleProperty("operatorConsoleToriiEndpoint")
    .orElse("https://torii.dev.sora.org")
val operatorFeatureFlags = providers.gradleProperty("operatorConsoleFeatures").orElse("")
val operatorConfigProfile = providers.gradleProperty("operatorConsoleConfigProfile").orElse("default")
val gitCommitRef = providers.environmentVariable("GIT_COMMIT").orElse("UNKNOWN")
val manifestOutput = layout.buildDirectory.file("sample-manifest/sample_manifest.json")

tasks.register("generateSampleManifest") {
    val sdkVersionProvider = providers.provider { project(":android-sdk").version.toString() }
    inputs.property("sdkVersion", sdkVersionProvider)
    inputs.property("toriiEndpoint", operatorToriiEndpoint)
    inputs.property("featureFlags", operatorFeatureFlags)
    inputs.property("configProfile", operatorConfigProfile)
    inputs.property("toriiAccounts", envSampleToriiAccounts)
    inputs.property("toriiLog", envSampleToriiLog)
    inputs.property("toriiMetrics", envSampleToriiMetrics)
    inputs.property("sorafsScoreboard", envSampleSorafsScoreboard)
    inputs.property("sorafsScoreboardSha", envSampleSorafsScoreboardSha)
    inputs.property("sorafsSummary", envSampleSorafsSummary)
    inputs.property("sorafsSummarySha", envSampleSorafsSummarySha)
    inputs.property("sorafsReceipts", envSampleSorafsReceipts)
    inputs.property("telemetryLog", envSampleTelemetryLog)
    inputs.property("telemetrySaltVersion", envSampleTelemetrySaltVersion)
    inputs.property("telemetryRotationId", envSampleTelemetryRotationId)
    inputs.property("telemetryExporter", envSampleTelemetryExporter)
    inputs.property("handoffEndpoint", envSampleHandoffEndpoint)
    inputs.property("handoffInbox", envSampleHandoffInbox)
    inputs.property("gitCommit", gitCommitRef)
    outputs.file(manifestOutput)

    doLast {
        val features = operatorFeatureFlags.orNull
            ?.split(',')
            ?.map { it.trim() }
            ?.filter { it.isNotEmpty() }
            ?: emptyList()
        val environment = linkedMapOf<String, Any>()
        environment["torii_endpoint"] = operatorToriiEndpoint.get()
        envSampleToriiAccounts.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["torii_accounts"] = it
        }
        envSampleToriiLog.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["torii_log"] = it
        }
        envSampleToriiMetrics.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["torii_metrics"] = it
        }
        envSampleSorafsScoreboard.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["sorafs_scoreboard"] = it
        }
        envSampleSorafsScoreboardSha.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["sorafs_scoreboard_sha256"] = it
        }
        envSampleSorafsSummary.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["sorafs_summary"] = it
        }
        envSampleSorafsSummarySha.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["sorafs_summary_sha256"] = it
        }
        envSampleSorafsReceipts.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["sorafs_receipts"] = it
        }
        val telemetry =
            linkedMapOf<String, Any>(
                "salt_version" to envSampleTelemetrySaltVersion.orElse(defaultTelemetrySaltVersion)
                    .get(),
                "rotation_id" to envSampleTelemetryRotationId.orElse(defaultTelemetryRotationId).get(),
                "exporter" to envSampleTelemetryExporter.orElse(defaultTelemetryExporter).get()
            )
        envSampleTelemetryLog.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["telemetry_log"] = it
            telemetry["log"] = it
        }
        environment["telemetry"] = telemetry
        envSampleTelemetrySaltHex.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            telemetry["salt_hex"] = it
        }
        envSampleHandoffEndpoint.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["handoff_endpoint"] = it
        }
        envSampleHandoffInbox.orNull?.trim()?.takeIf { it.isNotEmpty() }?.let {
            environment["handoff_inbox"] = it
        }

        val manifestJson =
            JsonOutput.prettyPrint(
                JsonOutput.toJson(
                    linkedMapOf(
                        "sample" to "operator-console",
                        "sdk_version" to sdkVersionProvider.get(),
                        "git_commit" to gitCommitRef.get(),
                        "generated_at" to Instant.now().toString(),
                        "config_profile" to operatorConfigProfile.get(),
                        "torii_endpoint" to operatorToriiEndpoint.get(),
                        "feature_flags" to features,
                        "environment" to environment
                    )
                )
            )

        val outputFile = manifestOutput.get().asFile
        outputFile.parentFile.mkdirs()
        outputFile.writeText("$manifestJson\n")
    }
}

tasks.register("printSampleManifest") {
    dependsOn("generateSampleManifest")
    doLast {
        println(manifestOutput.get().asFile.readText())
    }
}
