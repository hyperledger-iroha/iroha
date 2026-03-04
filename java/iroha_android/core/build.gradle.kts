import groovy.json.JsonOutput
import groovy.json.JsonSlurper
import java.net.URLClassLoader

plugins {
    `java-library`
}

val sdkVersion = providers.gradleProperty("irohaAndroidVersion").orElse("0.1.0-SNAPSHOT")
val noritoJavaVersion = providers.gradleProperty("noritoJavaVersion").orElse(sdkVersion)
val noritoSchemaManifest =
    rootProject.layout.projectDirectory.file("schemas/norito_schema_manifest.json")
val noritoSchemaNames =
    listOf(
        "ConnectJournalRecordV1",
        "iroha.android.transaction.Payload.v1",
    )
val noritoJar =
    rootProject.layout.projectDirectory
        .dir("../norito_java/build/libs")
        .file("norito-java-${noritoJavaVersion.get()}.jar")
val noritoJarTask = gradle.includedBuild("norito_java").task(":jar")
// Use the built jar to keep Android lint happy with composite builds.
val noritoJarDependency = files(noritoJar.asFile).builtBy(noritoJarTask)

java {
    toolchain.languageVersion.set(JavaLanguageVersion.of(21))
}

tasks.withType<Test>().configureEach {
    useJUnit()
    enableAssertions = true
    val harnessFilter = System.getenv("ANDROID_HARNESS_MAINS")
    if (!harnessFilter.isNullOrBlank()) {
        systemProperty("android.test.mains", harnessFilter)
    }
    val nativeLibPath = System.getenv("IROHA_NATIVE_LIBRARY_PATH")
    if (!nativeLibPath.isNullOrBlank()) {
        jvmArgs("-Djava.library.path=$nativeLibPath")
    }
}

sourceSets {
    main {
        java {
            srcDir("../src/main/java")
        }
    }
    test {
        java {
            srcDir("../src/test/java")
            resources.srcDir("../src/test/resources")
        }
    }
}

val noritoVerification by configurations.creating {
    isCanBeConsumed = false
    isCanBeResolved = true
    dependencies.add(
        project.dependencies.create(noritoJarDependency),
    )
}

fun computeSchemaHashes(classpath: Set<File>, schemaNames: List<String>): Map<String, String> {
    val loader =
        URLClassLoader(
            classpath.map { it.toURI().toURL() }.toTypedArray(),
            javaClass.classLoader,
        )
    loader.use {
        val schemaHashClass = it.loadClass("org.hyperledger.iroha.norito.SchemaHash")
        val hashMethod = schemaHashClass.getMethod("hash16", String::class.java)
        return schemaNames.associateWith { name ->
            val bytes = hashMethod.invoke(null, name) as ByteArray
            bytes.joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xFF) }
        }
    }
}

tasks.register("regenNoritoSchemaManifest") {
    description = "Regenerates the Norito schema manifest pinned for Android/JVM builds."
    group = "verification"
    inputs.files(noritoVerification)
    inputs.property("schemas", noritoSchemaNames)
    outputs.file(noritoSchemaManifest)

    doLast {
        val hashes =
            computeSchemaHashes(
                noritoVerification.resolve(),
                noritoSchemaNames.sorted(),
            )
        val manifestPayload =
            linkedMapOf(
                "schema_manifest_version" to 1,
                "norito_java_version" to noritoJavaVersion.get(),
                "schemas" to hashes.entries.map { (name, hash) ->
                    mapOf("name" to name, "hash_hex" to hash)
                },
            )
        val rendered = JsonOutput.prettyPrint(JsonOutput.toJson(manifestPayload))
        noritoSchemaManifest.asFile.parentFile.mkdirs()
        noritoSchemaManifest.asFile.writeText("$rendered\n")
    }
}

tasks.register("verifyNoritoSchemas") {
    description = "Validates Norito schema hashes against the pinned manifest."
    group = "verification"
    dependsOn(noritoVerification)
    inputs.files(noritoSchemaManifest)
    inputs.property("schemas", noritoSchemaNames)
    outputs.dir(layout.buildDirectory.dir("reports/norito-schema"))

    doLast {
        if (!noritoSchemaManifest.asFile.exists()) {
            throw GradleException(
                "Missing ${noritoSchemaManifest.asFile}. Regenerate via :core:regenNoritoSchemaManifest.",
            )
        }
        val manifest =
            JsonSlurper().parse(noritoSchemaManifest.asFile) as Map<*, *>
        val manifestSchemas =
            (manifest["schemas"] as? List<*>)?.associate { entry ->
                val map = entry as? Map<*, *>
                    ?: throw GradleException("Invalid schema manifest entry: $entry")
                val name = map["name"] as? String
                    ?: throw GradleException("Schema manifest entry missing name: $map")
                val hash = map["hash_hex"] as? String
                    ?: throw GradleException("Schema manifest entry missing hash_hex: $map")
                name to hash.lowercase()
            }
                ?: throw GradleException("Schema manifest missing schemas array.")

        val expectedNames = noritoSchemaNames.sorted()
        val missing = expectedNames.filterNot { manifestSchemas.containsKey(it) }
        val extra = manifestSchemas.keys.filterNot { expectedNames.contains(it) }

        val computed = computeSchemaHashes(noritoVerification.resolve(), expectedNames)
        val mismatches =
            computed.filter { (name, hash) -> manifestSchemas[name]?.lowercase() != hash }

        val reportFile =
            layout.buildDirectory
                .file("reports/norito-schema/verification.json")
                .get()
                .asFile
        reportFile.parentFile.mkdirs()
        val renderedReport =
            JsonOutput.prettyPrint(
                JsonOutput.toJson(
                    mapOf(
                        "computed" to computed,
                        "manifest" to manifestSchemas,
                        "missing" to missing,
                        "extra" to extra,
                        "mismatches" to mismatches,
                    ),
                ),
            )
        reportFile.writeText("$renderedReport\n")

        if (missing.isNotEmpty() || extra.isNotEmpty() || mismatches.isNotEmpty()) {
            throw GradleException(
                buildString {
                    append("Norito schema manifest mismatch detected.")
                    if (missing.isNotEmpty()) {
                        append(" Missing entries: $missing.")
                    }
                    if (extra.isNotEmpty()) {
                        append(" Extra entries: $extra.")
                    }
                    if (mismatches.isNotEmpty()) {
                        append(" Hash mismatches: $mismatches.")
                    }
                    append(" Regenerate with :core:regenNoritoSchemaManifest if the schema set changed.")
                },
            )
        }
    }
}

val androidFixturesRoot = layout.projectDirectory.dir("../src/test/resources")

tasks.register("checkAndroidFixtures") {
    description = "Validates checked-in Android Norito fixtures against the manifest."
    group = "verification"
    inputs.dir(androidFixturesRoot)
    outputs.file(layout.buildDirectory.file("reports/android-fixtures/check.log"))

    doLast {
        val repoRoot = rootProject.layout.projectDirectory.dir("..").dir("..")
        val script = repoRoot.file("scripts/check_android_fixtures.py")
        val resourcesDir = rootProject.layout.projectDirectory.dir("src/test/resources")
        val cmd =
            listOf(
                "python3",
                script.asFile.absolutePath,
                "--resources",
                resourcesDir.asFile.absolutePath,
                "--fixtures",
                resourcesDir.file("transaction_payloads.json").asFile.absolutePath,
                "--manifest",
                resourcesDir.file("transaction_fixtures.manifest.json").asFile.absolutePath,
                "--quiet",
            )
        val process =
            ProcessBuilder(cmd)
                .directory(rootProject.projectDir)
                .redirectErrorStream(true)
                .inheritIO()
                .start()
        val exitCode = process.waitFor()
        if (exitCode != 0) {
            throw GradleException("check_android_fixtures.py failed with exit code $exitCode")
        }
        val report =
            layout.buildDirectory.file("reports/android-fixtures/check.log").get().asFile
        report.parentFile.mkdirs()
        report.writeText("ok\n")
    }
}

tasks.named("check") {
    dependsOn("verifyNoritoSchemas", "checkAndroidFixtures")
}

dependencies {
    api(noritoJarDependency)
    implementation("org.bouncycastle:bcprov-jdk18on:1.78.1")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("com.squareup.okhttp3:okhttp:4.12.0")
    testImplementation("com.squareup.okio:okio:3.6.0")
    testImplementation("junit:junit:4.13.2")
}
