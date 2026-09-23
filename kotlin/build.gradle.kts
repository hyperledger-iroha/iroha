import org.cyclonedx.gradle.CyclonedxDirectTask

// Resolve the shared Kotlin plugins once for every SDK and tooling module.
plugins {
    id("org.cyclonedx.bom") version "3.1.0" apply false
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.kotlin.serialization) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.android.library) apply false
}

// The published Kotlin modules own their runtime inventories. Tools, samples,
// and the retiring Java SDK are not substitutes for these three release BOMs.
subprojects {
    if (name in setOf("core-jvm", "client-android", "kagemusha-wallet-android")) {
        pluginManager.apply("org.cyclonedx.bom")
        tasks.named<CyclonedxDirectTask>("cyclonedxDirectBom") {
            includeConfigs.set(listOf(if (project.name == "core-jvm") "runtimeClasspath" else "releaseRuntimeClasspath"))
            componentGroup.set("org.hyperledger.iroha.sdk")
            componentName.set(project.name)
            componentVersion.set(provider { project.version.toString() })
            jsonOutput.set(layout.buildDirectory.file("reports/bom/bom.json"))
            xmlOutput.unset()
            includeBuildEnvironment.set(false)
            includeBuildSystem.set(false)
        }
    }
}
