import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.jvm)
    `maven-publish`
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"

repositories {
    mavenCentral()
}

dependencies {
    implementation(libs.zstd.jni)
    implementation(libs.bcprov)
    testImplementation(kotlin("test"))
    testImplementation(libs.junit.params)
}

java {
    sourceCompatibility = JavaVersion.VERSION_1_8
    targetCompatibility = JavaVersion.VERSION_1_8
}

kotlin {
    jvmToolchain(21)

    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_1_8)
    }
}

tasks.test {
    useJUnitPlatform()
}

publishing {
    publications {
        create<MavenPublication>("release") {
            from(components["java"])
            groupId = "org.hyperledger.iroha.sdk"
            artifactId = "core-jvm"
        }
    }
}
