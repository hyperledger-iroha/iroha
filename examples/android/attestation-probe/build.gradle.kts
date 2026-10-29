// Build plugins execute locally and need the same advisory hygiene as published SDKs.
// Keep compatible patched transitive releases until upstream plugins carry them.
buildscript {
    dependencies {
        // Jackson 2.22.3+ fixes parser/number/forward-reference/type-ID DoS advisories.
        classpath(platform("com.fasterxml.jackson:jackson-bom:2.22.4"))
        constraints {
            add("classpath", "com.google.guava:guava:33.7.2-jre") {
                because("GHSA-xxph-c9ww-hj94: bound native deserialization allocations")
            }
            add("classpath", "org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.21") {
                because("GHSA-r937-wjx7-w2jp: align AGP tooling with the current Kotlin plugin")
            }
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
    id("com.android.application") version "9.4.1"
}

android {
    namespace = "org.hyperledger.iroha.samples.attestationprobe"
    compileSdk = 37

    defaultConfig {
        applicationId = "org.hyperledger.iroha.samples.attestationprobe"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8)
        }
    }
}

dependencies {
    testImplementation("junit:junit:4.13.2")
}

// Android Lint resolves its own tool classpath, separately from the AGP plugin.
allprojects {
    configurations.configureEach {
        if (name == "androidLintTool") {
            project.dependencies.constraints.add(name, "com.google.guava:guava:33.7.2-jre") {
                because("GHSA-xxph-c9ww-hj94: bound native deserialization allocations")
            }
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
