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

publishing {
    publications {
        create<MavenPublication>("mavenJava") {
            from(components["java"])
            artifactId = "norito-java"
        }
    }
}
