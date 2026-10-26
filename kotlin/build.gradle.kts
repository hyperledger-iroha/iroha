import org.cyclonedx.gradle.CyclonedxDirectTask

// Resolve the shared Kotlin plugins once for every SDK and tooling module.
// Build plugins execute locally and need the same advisory hygiene as published SDKs.
// Keep compatible patched transitive releases until upstream plugins carry them.
buildscript {
    dependencies {
        // Jackson 2.22.3+ fixes parser/number/forward-reference/type-ID DoS advisories.
        classpath(platform("com.fasterxml.jackson:jackson-bom:2.22.4"))
        constraints {
            add("classpath", "org.apache.commons:commons-lang3:3.21.0") {
                because("GHSA-j288-q9x7-2f5v: reject recursive-input denial of service")
            }
            add("classpath", "org.bitbucket.b_c:jose4j:0.9.7") {
                because("GHSA-3677-xxcr-wjqv: bound decompression of JWE content")
            }
            for (artifact in listOf("bcprov-jdk18on", "bcpkix-jdk18on", "bcutil-jdk18on")) {
                add("classpath", "org.bouncycastle:$artifact:1.86") {
                    because("Keep BC modules aligned with fixes for certificate, LDAP and ASN.1 advisories")
                }
            }
            add("classpath", "org.jdom:jdom2:2.0.6.1") {
                because("GHSA-2363-cqg2-863c: disable external entity expansion")
            }
        }
    }
}

plugins {
    id("org.cyclonedx.bom") version "3.5.1" apply false
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

// Android Lint resolves its own tool classpath, separately from the AGP plugin.
allprojects {
    configurations.configureEach {
        if (name == "androidLintTool") {
            project.dependencies.constraints.add(name, "org.apache.commons:commons-lang3:3.21.0") {
                because("GHSA-j288-q9x7-2f5v: reject recursive-input denial of service")
            }
            project.dependencies.constraints.add(name, "org.apache.httpcomponents:httpclient:4.5.14") {
                because("GHSA-7r82-7xv7-xcpj: preserve the intended HTTP request authority")
            }
            for (artifact in listOf("bcprov-jdk18on", "bcpkix-jdk18on", "bcutil-jdk18on")) {
                project.dependencies.constraints.add(name, "org.bouncycastle:$artifact:1.86") {
                    because("Keep Lint's BC modules aligned with certificate, LDAP and ASN.1 fixes")
                }
            }
        }
    }
}
