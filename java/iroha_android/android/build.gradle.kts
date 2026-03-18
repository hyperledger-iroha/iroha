import groovy.json.JsonOutput
import java.security.MessageDigest
import java.time.Instant
import org.gradle.api.GradleException
import org.gradle.api.file.FileSystemLocation
import org.gradle.api.publish.maven.tasks.PublishToMavenLocal
import org.gradle.api.publish.maven.tasks.PublishToMavenRepository

plugins {
    id("com.android.library")
    `maven-publish`
}

buildscript {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
    dependencies {
        classpath("org.cyclonedx:cyclonedx-gradle-plugin:3.1.0")
    }
}

val sdkVersion = providers.gradleProperty("irohaAndroidVersion").orElse("0.1.0-SNAPSHOT")
val androidArtifactId = "iroha-android"
group = "org.hyperledger.iroha"
version = sdkVersion.get()

val enableCycloneDx =
    providers.gradleProperty("enableCycloneDx").map { it.toBoolean() }.orElse(true)
if (enableCycloneDx.get()) {
    pluginManager.apply("org.cyclonedx.bom")
}

fun sha256(file: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
    file.inputStream().use { input ->
        val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
        while (true) {
            val read = input.read(buffer)
            if (read <= 0) break
            digest.update(buffer, 0, read)
        }
    }
    return digest.digest().joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xFF) }
}

fun renderDependencyManifest(
    configurationName: String,
    coordinates: String,
    primary: File,
    artifacts: Set<org.gradle.api.artifacts.ResolvedArtifact>,
): String {
    val dependencies =
        artifacts
            .sortedBy { artifact ->
                val id = artifact.moduleVersion.id
                listOf(id.group, artifact.name, id.version, artifact.classifier ?: "", artifact.type)
                    .joinToString(":")
            }
            .map { artifact ->
                val id = artifact.moduleVersion.id
                mapOf(
                    "group" to id.group,
                    "name" to artifact.name,
                    "version" to id.version,
                    "type" to artifact.type,
                    "classifier" to artifact.classifier,
                    "sha256" to sha256(artifact.file),
                )
            }
    val manifest =
        linkedMapOf(
            "artifact" to coordinates,
            "version" to project.version.toString(),
            "generatedAt" to Instant.now().toString(),
            "configuration" to configurationName,
            "primaryArtifact" to
                mapOf(
                    "path" to primary.absolutePath,
                    "sha256" to sha256(primary),
                ),
            "dependencies" to dependencies,
        )
    return JsonOutput.prettyPrint(JsonOutput.toJson(manifest))
}

android {
    namespace = "org.hyperledger.iroha.android"
    compileSdk = 34

    defaultConfig {
        minSdk = 24
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
        isCoreLibraryDesugaringEnabled = true
    }

    // Keep packaging simple for now; publishing wiring will follow.
    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }

    testOptions {
        unitTests.all {
            it.jvmArgs("-ea")
            val harnessFilter = System.getenv("ANDROID_HARNESS_MAINS")
            if (!harnessFilter.isNullOrBlank()) {
                it.systemProperty("android.test.mains", harnessFilter)
            }
        }
    }
}

dependencies {
    api(project(":core"))
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.0.4")
    implementation("org.bouncycastle:bcprov-jdk18on:1.78.1")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("androidx.camera:camera-core:1.3.3")
    implementation("androidx.camera:camera-camera2:1.3.3")
    implementation("androidx.camera:camera-lifecycle:1.3.3")
    implementation("com.google.zxing:core:3.5.3")
    testImplementation("junit:junit:4.13.2")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
}

val androidDependencyManifest =
    tasks.register("androidDependencyManifest") {
        description = "Generates runtime dependency manifest for the Android AAR."
        group = "verification"
        val androidCoordinates = "${project.group}:$androidArtifactId"
        val manifestOut =
            layout.buildDirectory
                .file(
                    "reports/publishing/${androidArtifactId}-${sdkVersion.get()}-releaseRuntimeClasspath.json",
                )
        inputs.files(configurations.named("releaseRuntimeClasspath"))
        inputs.property("version", sdkVersion)
        outputs.file(manifestOut)
        dependsOn(tasks.named("assembleRelease"))

        doLast {
            val runtimeArtifacts =
                configurations
                    .getByName("releaseRuntimeClasspath")
                    .resolvedConfiguration
                    .resolvedArtifacts
            val aarFile =
                layout.buildDirectory.file("outputs/aar/android-release.aar").get().asFile
            if (!aarFile.exists()) {
                throw GradleException("Expected AAR at ${aarFile.absolutePath} after assembleRelease.")
            }
            val rendered =
                renderDependencyManifest(
                    configurationName = "releaseRuntimeClasspath",
                    coordinates = androidCoordinates,
                    primary = aarFile,
                    artifacts = runtimeArtifacts,
                )
            val outputFile = manifestOut.get().asFile
            outputFile.parentFile.mkdirs()
            outputFile.writeText("$rendered\n")
        }
    }

val releaseAar = layout.buildDirectory.file("outputs/aar/android-release.aar")
val hasCycloneDx = enableCycloneDx.get() && plugins.hasPlugin("org.cyclonedx.bom")
if (hasCycloneDx) {
    val cycloneTasks = setOf("cyclonedxBom", "cyclonedxDirectBom")
    tasks.matching { it.name in cycloneTasks }.configureEach {
        val schemaProperty =
            javaClass.methods.firstOrNull { method ->
                method.name == "getSchemaVersion" && method.parameterCount == 0
            }?.invoke(this)
        val versionValue =
            runCatching {
                val versionClass = javaClass.classLoader.loadClass("org.cyclonedx.Version")
                versionClass.enumConstants.firstOrNull { constant ->
                    (constant as? Enum<*>)?.name == "VERSION_15"
                }
            }.getOrNull()
        val providerClass =
            runCatching { javaClass.classLoader.loadClass("org.gradle.api.provider.Provider") }
                .getOrNull()
        val setter =
            if (schemaProperty != null && versionValue != null) {
                schemaProperty.javaClass.methods.firstOrNull { method ->
                    method.name == "set" &&
                        method.parameterCount == 1 &&
                        (providerClass == null ||
                            !providerClass.isAssignableFrom(method.parameterTypes[0])) &&
                        method.parameterTypes[0].isInstance(versionValue)
                }
            } else {
                null
            }
        val applied =
            if (setter != null && versionValue != null) {
                runCatching { setter.invoke(schemaProperty, versionValue) }.isSuccess
            } else {
                false
            }
        if (!applied && schemaProperty != null) {
            schemaProperty.javaClass.methods
                .firstOrNull { method ->
                    method.name == "set" &&
                        method.parameterCount == 1 &&
                        method.parameterTypes[0] == String::class.java
                }
                ?.let { method -> runCatching { method.invoke(schemaProperty, "1.5") } }
        }
    }
}
val androidBomFile =
    if (hasCycloneDx) {
        tasks.named("cyclonedxBom").flatMap { task ->
            task.outputs.files.elements.map { elements: Set<FileSystemLocation> ->
                elements
                    .map { it.asFile }
                    .firstOrNull { it.extension == "json" }
                    ?: elements.first().asFile
            }
        }
    } else {
        providers.provider { releaseAar.get().asFile }
    }
val androidRuntimeManifest =
    layout.buildDirectory.file("reports/publishing/${androidArtifactId}-runtime-manifest.json")
val androidRuntimeChecksum =
    layout.buildDirectory.file("reports/publishing/${androidArtifactId}-${project.version}-runtime.sha256")
val androidCoordinates = "${project.group}:$androidArtifactId:${project.version}"
val projectRoot = project.rootProject.rootDir.toPath()
val projectVersion = project.version.toString()

tasks.register("writeAndroidRuntimeManifest") {
    description = "Emits runtime manifest + checksum for the Android release AAR."
    group = "distribution"
    dependsOn("assembleRelease")
    if (hasCycloneDx) {
        dependsOn("cyclonedxBom")
    }
    inputs.files(releaseAar, androidBomFile)
    outputs.files(androidRuntimeManifest, androidRuntimeChecksum)

    doLast {
        val aarFile = releaseAar.get().asFile
        if (!aarFile.exists()) {
            throw GradleException("Expected AAR at ${aarFile.absolutePath} after assembleRelease.")
        }
        val bomFile = androidBomFile.get()
        val checksum = sha256(aarFile)
        androidRuntimeChecksum.get().asFile.apply {
            parentFile.mkdirs()
            writeText("$checksum  ${aarFile.name}\n")
        }
        val payload =
            linkedMapOf(
                "schema_version" to 1,
                "generatedAt" to Instant.now().toString(),
                "artifact" to
                    mapOf(
                        "type" to "aar",
                        "coordinates" to androidCoordinates,
                        "path" to projectRoot.relativize(aarFile.toPath()).toString(),
                        "version" to projectVersion,
                    ),
                "checksums" to
                    mapOf(
                        "sha256" to checksum,
                        "file" to
                            projectRoot
                                .relativize(androidRuntimeChecksum.get().asFile.toPath())
                                .toString(),
                    ),
                "sbom" to
                    mapOf(
                        "format" to if (hasCycloneDx) "cyclonedx" else "absent",
                        "path" to projectRoot.relativize(bomFile.toPath()).toString(),
                    ),
            )
        androidRuntimeManifest.get().asFile.apply {
            parentFile.mkdirs()
            writeText(JsonOutput.prettyPrint(JsonOutput.toJson(payload)) + "\n")
        }
    }
}

publishing {
    publications {
        register<MavenPublication>("androidSdk") {
            afterEvaluate { from(components["release"]) }
            artifactId = androidArtifactId
            pom {
                name.set("Iroha Android SDK (Android)")
                description.set(
                    "Hyperledger Iroha Android bindings for Norito codecs, key management, and Torii networking clients (Android AAR).",
                )
                url.set("https://github.com/hyperledger/iroha")
                licenses {
                    license {
                        name.set("Apache License 2.0")
                        url.set("https://www.apache.org/licenses/LICENSE-2.0")
                        distribution.set("repo")
                    }
                }
                developers {
                    developer {
                        name.set("Hyperledger Iroha Contributors")
                        organization.set("Hyperledger")
                        organizationUrl.set("https://www.hyperledger.org/projects/iroha")
                    }
                }
                scm {
                    connection.set("scm:git:https://github.com/hyperledger/iroha")
                    developerConnection.set("scm:git:ssh://git@github.com/hyperledger/iroha.git")
                    url.set("https://github.com/hyperledger/iroha")
                }
            }
        }
    }
    repositories {
        val repoDirProp = providers.gradleProperty("irohaAndroidRepoDir").orNull
        val repoUrlProp = providers.gradleProperty("irohaAndroidRepoUrl").orNull
        val targetUri = when {
            !repoUrlProp.isNullOrBlank() -> uri(repoUrlProp)
            !repoDirProp.isNullOrBlank() -> file(repoDirProp).toURI()
            else -> layout.buildDirectory.dir("maven").get().asFile.toURI()
        }
        maven {
            name = "target"
            url = targetUri
            val repoUser = providers.gradleProperty("irohaAndroidRepoUsername").orNull
            val repoPass = providers.gradleProperty("irohaAndroidRepoPassword").orNull
            if (!repoUser.isNullOrBlank() && !repoPass.isNullOrBlank()) {
                credentials {
                    username = repoUser
                    password = repoPass
                }
            }
            isAllowInsecureProtocol = targetUri.scheme.equals("http", ignoreCase = true)
        }
    }
}

tasks.withType<PublishToMavenRepository>().configureEach {
    if (hasCycloneDx) {
        dependsOn(androidDependencyManifest, tasks.named("cyclonedxBom"), tasks.named("writeAndroidRuntimeManifest"))
    } else {
        dependsOn(androidDependencyManifest, tasks.named("writeAndroidRuntimeManifest"))
    }
}

tasks.withType<PublishToMavenLocal>().configureEach {
    if (hasCycloneDx) {
        dependsOn(androidDependencyManifest, tasks.named("cyclonedxBom"), tasks.named("writeAndroidRuntimeManifest"))
    } else {
        dependsOn(androidDependencyManifest, tasks.named("writeAndroidRuntimeManifest"))
    }
}
