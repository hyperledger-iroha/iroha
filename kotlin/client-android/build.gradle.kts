import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.util.Properties

plugins {
    alias(libs.plugins.android.library)
    `maven-publish`
}

group = "org.hyperledger.iroha.sdk"
version = providers.gradleProperty("irohaSdkVersion")
    .orElse(providers.environmentVariable("IROHA_SDK_VERSION"))
    .orElse("0.1.0")
    .get()

val mobileSdkRepoDir = providers.gradleProperty("irohaSdkRepoDir")
    .orElse(rootProject.layout.buildDirectory.dir("mobile-sdk-maven").map { it.asFile.absolutePath })

android {
    namespace = "org.hyperledger.iroha.sdk.android"
    compileSdk = 35

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
        isCoreLibraryDesugaringEnabled = true
    }

    kotlin {
        jvmToolchain(21)

        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_1_8)
            freeCompilerArgs.add("-Xjdk-release=8")
        }
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }

    packaging {
        jniLibs {
            // The mobile artifact verifier binds the published AAR to the exact
            // authenticated native bridge bytes staged in src/main/jniLibs.
            // Prevent AGP from stripping a different binary into the AAR.
            keepDebugSymbols.add("**/libconnect_norito_bridge.so")
        }
    }
}

repositories {
    google()
    mavenCentral()
}

publishing {
    repositories {
        maven {
            name = "mobileSdk"
            url = uri(mobileSdkRepoDir.get())
        }
    }
}

dependencies {
    api(project(":core-jvm"))
    implementation(libs.play.services.nearby)
    coreLibraryDesugaring(libs.desugar.jdk.libs)
    testImplementation(kotlin("test"))
    testImplementation(libs.junit.params)
    testRuntimeOnly(libs.junit.jupiter.engine)
    testRuntimeOnly(libs.junit.platform.launcher)
}

tasks.withType<Test>().configureEach {
    useJUnitPlatform()
    if (name == "testDebugUnitTest") {
        dependsOn("processDebugManifest")
        systemProperty(
            "iroha.clientAndroid.mergedManifest",
            layout.buildDirectory.file(
                "intermediates/merged_manifest/debug/processDebugManifest/AndroidManifest.xml",
            ).get().asFile.absolutePath,
        )
    }
}

val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs")
val nativeBridgeLibraries = listOf(
    jniLibsDir.file("arm64-v8a/libconnect_norito_bridge.so"),
    jniLibsDir.file("x86_64/libconnect_norito_bridge.so"),
)

fun irohaDir(): String {
    val props = Properties()
    val file = rootProject.file("local.properties")
    if (file.exists()) file.inputStream().use { props.load(it) }
    return props.getProperty("iroha.dir") ?: rootProject.file("..").absolutePath
}

val compileNativeLibs = tasks.register<Exec>("compileNativeLibs") {
    group = "native"
    description = "Compile connect_norito_bridge .so from Rust source (requires cargo-ndk + Android NDK)"

    // Pass -PprivacyProductionEnabled=true to enable real proving in the native bridge.
    // Default is off (fail-closed); flip only when all production gates have passed.
    val privacyProductionEnabled =
        project.findProperty("privacyProductionEnabled")?.toString()?.toBoolean() ?: false
    val cargoFeatureArgs = if (privacyProductionEnabled) {
        listOf("--features", "privacy-production-enabled")
    } else {
        emptyList()
    }

    workingDir = file(irohaDir())
    commandLine(
        buildList {
            addAll(listOf("cargo", "ndk", "-t", "arm64-v8a", "-t", "x86_64", "-o", jniLibsDir.asFile.absolutePath))
            addAll(listOf("build", "--release", "-p", "connect_norito_bridge"))
            addAll(cargoFeatureArgs)
        }
    )
}

val androidNdkRoot = providers.environmentVariable("ANDROID_NDK_HOME")
    .orElse(providers.environmentVariable("ANDROID_NDK_ROOT"))
    .orElse(androidComponents.sdkComponents.ndkDirectory.map { it.asFile.absolutePath })

val stripNativeLibs = tasks.register<Exec>("stripNativeLibs") {
    group = "native"
    description = "Canonically strip the compiled Android native bridge libraries"
    dependsOn(compileNativeLibs)

    doFirst {
        val prebuiltRoot = file(androidNdkRoot.get()).resolve("toolchains/llvm/prebuilt")
        val stripExecutables = prebuiltRoot
            .walkTopDown()
            .filter { it.isFile && (it.name == "llvm-strip" || it.name == "llvm-strip.exe") }
            .toList()
        require(stripExecutables.size == 1) {
            "Expected one Android NDK llvm-strip under $prebuiltRoot; found ${stripExecutables.size}"
        }
        commandLine(
            stripExecutables.single().absolutePath,
            "--strip-unneeded",
            *nativeBridgeLibraries.map { it.asFile.absolutePath }.toTypedArray(),
        )
    }
}

tasks.register("buildNativeLibs") {
    group = "native"
    description = "Build and canonically strip connect_norito_bridge Android libraries"
    dependsOn(stripNativeLibs)
}

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "org.hyperledger.iroha.sdk"
                artifactId = "client-android"
            }
        }
    }
}
