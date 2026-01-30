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

val noritoJavaVersion = providers.gradleProperty("noritoJavaVersion").orElse("0.1.0-SNAPSHOT")
version = noritoJavaVersion.get()

repositories {
    mavenCentral()
}

tasks.withType<JavaCompile>().configureEach {
    options.encoding = "UTF-8"
    options.release.set(21)
    val javaHome = System.getenv("JAVA_HOME")?.let { file(it) }
    if (javaHome != null) {
        options.isFork = true
        options.forkOptions.javaHome = javaHome
    }
}

val sourceSets = the<SourceSetContainer>()

val runNoritoTests =
    tasks.register<JavaExec>("runNoritoTests") {
        group = "verification"
        description = "Runs the Norito Java parity harness with assertions enabled."
        classpath = sourceSets.getByName("test").runtimeClasspath
        mainClass.set("org.hyperledger.iroha.norito.NoritoTests")
        jvmArgs("-ea")
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
