import java.net.URI
import java.nio.file.Files
import java.nio.file.LinkOption
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

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
    namespace = "org.hyperledger.iroha.sdk.offline.wallet"
    compileSdk = 35

    defaultConfig {
        minSdk = 24
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")
    }

    sourceSets {
        getByName("test") {
            kotlin.srcDir(rootProject.file("test-support/src"))
        }
        getByName("androidTest") {
            assets.srcDir("../../fixtures/offline")
        }
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
        providers.environmentVariable("IROHA_SDK_MAVEN_URL").orNull?.let { remoteUrl ->
            val endpoint = URI(remoteUrl)
            require(endpoint.scheme == "https" && !endpoint.host.isNullOrBlank() &&
                endpoint.rawUserInfo == null && endpoint.rawQuery == null && endpoint.rawFragment == null) {
                "IROHA_SDK_MAVEN_URL must be an HTTPS repository without embedded credentials"
            }
            val remoteUsername = providers.environmentVariable("IROHA_SDK_MAVEN_USERNAME").orNull
            val remotePassword = providers.environmentVariable("IROHA_SDK_MAVEN_PASSWORD").orNull
            require((remoteUsername == null) == (remotePassword == null) &&
                (remoteUsername == null || remoteUsername.isNotBlank() && remotePassword!!.isNotBlank())) {
                "Remote Maven credentials must be a complete runtime-only pair"
            }
            maven {
                name = "remoteSdk"
                url = endpoint
                isAllowInsecureProtocol = false
                if (remoteUsername != null) {
                    credentials {
                        username = remoteUsername
                        password = remotePassword
                    }
                }
            }
        }
    }
}

dependencies {
    api(project(":client-android"))
    coreLibraryDesugaring(libs.desugar.jdk.libs)
    testImplementation(kotlin("test"))
    testImplementation(libs.coroutines.core.jvm)
    testImplementation(libs.junit.params)
    testRuntimeOnly(libs.junit.jupiter.engine)
    testRuntimeOnly(libs.junit.platform.launcher)
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
}

tasks.withType<Test>().configureEach {
    useJUnitPlatform {
        if (name != "testDebugHostNative") excludeTags("host-native")
    }
    // The wallet custody backup test asserts the processed library manifest, not only its source.
    if (name == "testDebugUnitTest") {
        dependsOn("processDebugManifest")
        systemProperty(
            "iroha.kagemushaWalletAndroid.mergedManifest",
            layout.buildDirectory.file(
                "intermediates/merged_manifest/debug/processDebugManifest/AndroidManifest.xml",
            ).get().asFile.absolutePath,
        )
    }
}

// Execute only the actual main-wallet JNI owner against an explicitly supplied
// rebuilt host artifact. This is a negative host check, not Native-root admission.
afterEvaluate {
    tasks.register<Test>("testDebugHostNative") {
        description = "Check exact host JNI ABI and missing ordinary Native root refusal."
        group = "verification"
        val managed = tasks.named<Test>("testDebugUnitTest").get()
        testClassesDirs = managed.testClassesDirs
        classpath = managed.classpath
        dependsOn(provider { managed.taskDependencies.getDependencies(managed) })
        useJUnitPlatform { includeTags("host-native") }
        filter {
            includeTestsMatching("org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeHostNativeV1Test")
            isFailOnNoMatchingTests = true
        }
        outputs.upToDateWhen { false }
        outputs.doNotCacheIf("Actual host JNI negatives must execute against the supplied artifact") { true }
        val nativeDirectory = providers.environmentVariable("IROHA_NATIVE_LIBRARY_PATH")
        doFirst {
            val configured = nativeDirectory.orNull
            require(!configured.isNullOrBlank()) {
                "testDebugHostNative requires the rebuilt IROHA_NATIVE_LIBRARY_PATH"
            }
            val directory = File(configured).toPath()
            require(directory.isAbsolute && directory.normalize() == directory &&
                directory.toRealPath() == directory &&
                Files.isDirectory(directory, LinkOption.NOFOLLOW_LINKS)) {
                "IROHA_NATIVE_LIBRARY_PATH must be one canonical existing directory"
            }
            val library = directory.resolve(System.mapLibraryName("connect_norito_bridge"))
            require(Files.isRegularFile(library, LinkOption.NOFOLLOW_LINKS) &&
                !Files.isSymbolicLink(library)) { "The actual rebuilt host JNI bridge is missing" }
            systemProperty("java.library.path", directory.toString())
        }
    }
}

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "org.hyperledger.iroha.sdk"
                artifactId = "kagemusha-wallet-android"
            }
        }
    }
}
