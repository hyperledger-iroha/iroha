import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.jvm)
    application
}

repositories {
    mavenCentral()
}

dependencies {
    implementation(project(":core-jvm"))
    testImplementation(libs.junit.params)
    testRuntimeOnly(libs.junit.jupiter.engine)
    testRuntimeOnly(libs.junit.platform.launcher)
}

java {
    sourceCompatibility = JavaVersion.VERSION_1_8
    targetCompatibility = JavaVersion.VERSION_1_8
}

kotlin {
    jvmToolchain(21)
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_1_8)
        freeCompilerArgs.add("-Xjdk-release=8")
    }
}

tasks.withType<JavaCompile>().configureEach {
    options.release.set(8)
}

application {
    applicationName = "iroha-attestation"
    mainClass.set("org.hyperledger.iroha.sdk.tools.AndroidAttestationCommand")
}

tasks.test {
    useJUnitPlatform()
    enableAssertions = true
    inputs.dir(rootProject.layout.projectDirectory.dir("../fixtures/android/attestation"))
}
