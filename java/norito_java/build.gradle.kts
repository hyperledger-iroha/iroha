import org.gradle.api.tasks.JavaExec
import org.gradle.api.tasks.SourceSetContainer
import org.gradle.api.tasks.compile.JavaCompile

plugins {
    `java-library`
    `maven-publish`
}

java {
    toolchain.languageVersion.set(JavaLanguageVersion.of(21))
    withSourcesJar()
}

group = "org.hyperledger.iroha"

val noritoJavaVersion = providers.gradleProperty("noritoJavaVersion").orElse("0.1.0")
version = noritoJavaVersion.get()

repositories {
    mavenCentral()
}

dependencies {
    implementation("com.github.luben:zstd-jni:1.5.7-7")
}

tasks.withType<JavaCompile>().configureEach {
    options.encoding = "UTF-8"
    options.release.set(21)
}

val sourceSets = the<SourceSetContainer>()
val runtimeClasspath = configurations.named("runtimeClasspath")

tasks.register("writeRuntimeClasspath") {
    group = "help"
    description = "Writes the resolved runtime dependency classpath for direct JVM tooling."
    val outputFile = layout.buildDirectory.file("runtime-classpath.txt")
    inputs.files(runtimeClasspath)
    outputs.file(outputFile)

    doLast {
        val destination = outputFile.get().asFile
        destination.parentFile.mkdirs()
        destination.writeText("${runtimeClasspath.get().asPath}\n")
    }
}

val runNoritoTests =
    tasks.register<JavaExec>("runNoritoTests") {
        group = "verification"
        description = "Runs the Norito Java parity harness with assertions enabled."
        classpath = sourceSets.getByName("test").runtimeClasspath
        mainClass.set("org.hyperledger.iroha.norito.NoritoTests")
        jvmArgs("-ea")
    }

// This module intentionally uses the assertion-based harness above instead of JUnit.
tasks.named("test") {
    enabled = false
}

tasks.named("check") {
    dependsOn(runNoritoTests)
}

publishing {
    publications {
        create<MavenPublication>("mavenJava") {
            from(components["java"])
            artifactId = "norito-java"
        }
    }
}
