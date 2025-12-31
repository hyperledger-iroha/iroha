import groovy.json.JsonOutput
import java.security.MessageDigest
import java.time.Instant
import org.gradle.api.GradleException
import org.gradle.api.publish.maven.tasks.PublishToMavenLocal
import org.gradle.api.publish.maven.tasks.PublishToMavenRepository
import org.gradle.api.tasks.bundling.Jar

plugins {
    `java-library`
    `maven-publish`
}

val sdkVersion = providers.gradleProperty("irohaAndroidVersion").orElse("0.1.0-SNAPSHOT")
val jvmArtifactId = "iroha-android"

group = "org.hyperledger.iroha"
version = sdkVersion.get()

val enableCycloneDx =
    providers.gradleProperty("enableCycloneDx").map { it.toBoolean() }.orElse(true)
if (enableCycloneDx.get()) {
    pluginManager.apply("org.cyclonedx.bom")
}

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

java {
    toolchain.languageVersion.set(JavaLanguageVersion.of(21))
    withSourcesJar()
    withJavadocJar()
}

dependencies {
    implementation(project(":core"))
    testImplementation(project(":core"))
    testImplementation("junit:junit:4.13.2")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("com.squareup.okhttp3:okhttp:4.12.0")
    testImplementation("com.squareup.okio:okio:3.6.0")
}

val coreMain =
    project(":core")
        .extensions
        .getByType(org.gradle.api.plugins.JavaPluginExtension::class.java)
        .sourceSets
        .getByName("main")

tasks.named<Jar>("jar").configure {
    from(coreMain.output)
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

val jvmDependencyManifest =
    tasks.register("jvmDependencyManifest") {
        description = "Generates runtime dependency manifest for the JVM artefact."
        group = "verification"
        val jvmCoordinates = "${project.group}:$jvmArtifactId"
        val manifestOut =
            layout.buildDirectory
                .file("reports/publishing/${jvmArtifactId}-${sdkVersion.get()}-runtimeClasspath.json")
        inputs.files(configurations.named("runtimeClasspath"))
        inputs.property("version", sdkVersion)
        outputs.file(manifestOut)
        dependsOn(tasks.named("jar"))

        doLast {
            val runtimeArtifacts =
                configurations.getByName("runtimeClasspath").resolvedConfiguration.resolvedArtifacts
            val primaryJar = tasks.named<Jar>("jar").get().archiveFile.get().asFile
            val rendered =
                renderDependencyManifest(
                    configurationName = "runtimeClasspath",
                    coordinates = jvmCoordinates,
                    primary = primaryJar,
                    artifacts = runtimeArtifacts,
                )
            val outputFile = manifestOut.get().asFile
            outputFile.parentFile.mkdirs()
            outputFile.writeText("$rendered\n")
        }
    }

val jvmBomFile =
    if (hasCycloneDx) {
        tasks.named("cyclonedxBom").flatMap { task ->
            task.outputs.files.elements.map { elements ->
                elements
                    .map { it.asFile }
                    .firstOrNull { it.extension == "json" }
                    ?: elements.first().asFile
            }
        }
    } else {
        providers.provider { tasks.named<Jar>("jar").get().archiveFile.get().asFile }
    }
val jvmRuntimeManifest =
    layout.buildDirectory.file("reports/publishing/${jvmArtifactId}-jvm-runtime-manifest.json")
val jvmRuntimeChecksum =
    layout.buildDirectory.file(
        "reports/publishing/${jvmArtifactId}-jvm-${project.version}-runtime.sha256",
    )
val jvmCoordinates = "${project.group}:$jvmArtifactId:${project.version}"
val projectRoot = project.rootProject.rootDir.toPath()
val projectVersion = project.version.toString()

tasks.register("writeJvmRuntimeManifest") {
    description = "Emits runtime manifest + checksum for the JVM artefact."
    group = "distribution"
    dependsOn(tasks.named("jar"))
    if (hasCycloneDx) {
        dependsOn("cyclonedxBom")
    }
    inputs.files(jvmBomFile, tasks.named<Jar>("jar").flatMap { it.archiveFile })
    outputs.files(jvmRuntimeManifest, jvmRuntimeChecksum)

    doLast {
        val jarFile = tasks.named<Jar>("jar").get().archiveFile.get().asFile
        if (!jarFile.exists()) {
            throw GradleException("Expected JAR at ${jarFile.absolutePath} after :jar.")
        }
        val bomFile = jvmBomFile.get()
        val checksum = sha256(jarFile)
        jvmRuntimeChecksum.get().asFile.apply {
            parentFile.mkdirs()
            writeText("$checksum  ${jarFile.name}\n")
        }
        val payload =
            linkedMapOf(
                "schema_version" to 1,
                "generatedAt" to Instant.now().toString(),
                "artifact" to
                    mapOf(
                        "type" to "jar",
                        "coordinates" to jvmCoordinates,
                        "path" to projectRoot.relativize(jarFile.toPath()).toString(),
                        "version" to projectVersion,
                    ),
                "checksums" to
                    mapOf(
                        "sha256" to checksum,
                        "file" to
                            projectRoot
                                .relativize(jvmRuntimeChecksum.get().asFile.toPath())
                                .toString(),
                    ),
                "sbom" to
                    mapOf(
                        "format" to if (hasCycloneDx) "cyclonedx" else "absent",
                        "path" to projectRoot.relativize(bomFile.toPath()).toString(),
                    ),
            )
        jvmRuntimeManifest.get().asFile.apply {
            parentFile.mkdirs()
            writeText(JsonOutput.prettyPrint(JsonOutput.toJson(payload)) + "\n")
        }
    }
}

publishing {
    publications {
        create<MavenPublication>("androidSdkJvm") {
            from(components["java"])
            artifactId = jvmArtifactId
            pom {
                name.set("Iroha Android SDK (JVM)")
                description.set(
                    "Hyperledger Iroha Android bindings for Norito codecs, key management, and Torii networking clients (JVM target).",
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
                    connection.set("scm:git:https://github.com/hyperledger/iroha.git")
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
        dependsOn(jvmDependencyManifest, tasks.named("cyclonedxBom"), "writeJvmRuntimeManifest")
    } else {
        dependsOn(jvmDependencyManifest, "writeJvmRuntimeManifest")
    }
}

tasks.withType<PublishToMavenLocal>().configureEach {
    if (hasCycloneDx) {
        dependsOn(jvmDependencyManifest, tasks.named("cyclonedxBom"), "writeJvmRuntimeManifest")
    } else {
        dependsOn(jvmDependencyManifest, "writeJvmRuntimeManifest")
    }
}
