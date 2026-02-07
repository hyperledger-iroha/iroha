---
lang: hy
direction: ltr
source: docs/source/sdk/android/publishing_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7a992bf18df43a7215ea129a6a44f1853ff453847060ec695669aab83286958b
source_last_modified: "2026-01-05T09:28:12.058739+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Publishing Plan (AND9)

Roadmap item **AND9 — Distribution & release packaging** requires the Android
SDK to produce signed Maven artefacts, deterministic dependency manifests, and
operator-friendly documentation. This guide tracks the reproducible publishing
flow landed in this repository so release engineering can attach the exact
evidence bundle that governance reviews expect.

## Goals

- Publish both SDK artefacts—`org.hyperledger.iroha:iroha-android`
  (Android AAR) and `org.hyperledger.iroha:iroha-android-jvm` (JVM jar)—to a
Maven-style repository (local directory, test Nexus, or production
  repository).
- Emit dependency manifest/SBOM snapshots per target that list the runtime
  classpath, cryptographic hashes, and the built SDK artefact for each release
  candidate.
- Keep the commands scriptable so CI/CD runners and human operators follow the
  same contract.

## Prerequisites

1. JDK 21+ installed and discoverable via `JAVA_HOME`.
2. Optional repository credentials supplied via the standard Gradle properties:
   - `irohaAndroidRepoDir` — local filesystem destination (e.g.
     `${projectDir}/artifacts/android/maven`).
   - `irohaAndroidRepoUrl` — remote Maven repository URL.
   - `irohaAndroidRepoUsername` / `irohaAndroidRepoPassword` — optional HTTP(S)
     basic-auth credentials when `irohaAndroidRepoUrl` points to an authenticated
     endpoint.
3. A version string provided through `-PirohaAndroidVersion=…` (defaults to
   `0.1.0-SNAPSHOT` when omitted).

Gradle reads these properties from `gradle.properties`, environment variables
(`ORG_GRADLE_PROJECT_*`), or the CLI flags shown in the examples below.

## Command Sequence

```bash
ANDROID_PUBLISH_VERSION=0.2.0-dev.1 \
ANDROID_PUBLISH_REPO_DIR=$PWD/artifacts/android/maven \
make android-publish-snapshot
```

Running the helper performs the AND9 steps end-to-end:

1. Builds the JVM jar (sources + javadoc) and the Android AAR (sources jar) and
   publishes them to the configured Maven repo (`ANDROID_PUBLISH_REPO_URL` or
   `ANDROID_PUBLISH_REPO_DIR`).
2. Executes `jvmDependencyManifest` and `androidDependencyManifest`,
   which resolve the runtime classpaths, record dependency coordinates plus
   SHA‑256 digests, and write JSON evidence bundles to:
   - `jvm/build/reports/publishing/iroha-android-jvm-<version>-runtimeClasspath.json`
   - `android/build/reports/publishing/iroha-android-<version>-releaseRuntimeClasspath.json`
   - Android runtime manifest/checksum:
     `android/build/reports/publishing/android-runtime-manifest.json` and
     `android/build/reports/publishing/android-release.aar.sha256`
3. Runs `cyclonedxBom` for each module so the release folders also contain
   CycloneDX SBOMs under `*/build/reports/bom/` (for example,
   `jvm/build/reports/bom/iroha-android-jvm*.bom.{json,xml}` and
   `android/build/reports/bom/iroha-android-aar*.bom.{json,xml}`).
4. Stages published artefacts + manifests/SBOMs under
   `artifacts/android/reports/<version>/{android,jvm}/` and, unless
   `ANDROID_PUBLISH_SKIP_SAMPLE=1`, assembles the `samples-android` app against
   the freshly published AAR for a sanity check.

Evidence artefact sets live alongside the published jars/AAR so release
engineers can attach them to governance packets or CI artefact uploads without
guesswork.

### Automated helper

For repeatable runs (local or CI), the same helper script can be driven via
environment variables:

- `ANDROID_PUBLISH_VERSION` — published version (defaults to `0.1.0-SNAPSHOT`).
- `ANDROID_PUBLISH_REPO_URL` or `ANDROID_PUBLISH_REPO_DIR` — Maven destination.
- `ANDROID_PUBLISH_REPORT_DIR` — staging directory for the copied artefacts
  (defaults to `artifacts/android/reports`).
- `ANDROID_PUBLISH_SKIP_SAMPLE=1` — skip building the sample against the
  published AAR.

CI jobs can upload `artifacts/android/reports/<version>/` directly to satisfy
the AND9 evidence requirement without custom copying logic.

### Signing (Sigstore)

Pass `ANDROID_PUBLISH_SIGN=1` to sign staged artefacts with Sigstore (keyless by
default). The helper uses `cosign` unless you override it via
`ANDROID_PUBLISH_COSIGN_BIN` (or the standard `COSIGN` env var) and pulls the
identity token from `ANDROID_PUBLISH_SIGSTORE_TOKEN_ENV` (defaults to
`SIGSTORE_ID_TOKEN` when present). Sigstore bundles land alongside the staged
artefacts so governance reviewers can verify without re-fetching from Maven.

## Dependency Manifest Format

`jvmDependencyManifest` and `androidDependencyManifest` emit
prettified JSON with the following shape:

```json
{
  "artifact": "org.hyperledger.iroha:iroha-android",
  "version": "0.2.0-dev.1",
  "generatedAt": "2026-04-29T12:34:56Z",
  "configuration": "runtimeClasspath",
  "primaryArtifact": {
    "path": ".../build/libs/iroha-android-0.2.0-dev.1.jar",
    "sha256": "…"
  },
  "dependencies": [
    {
      "group": "org.hyperledger.iroha",
      "name": "norito-java",
      "version": "0.2.0-dev.1",
      "type": "jar",
      "classifier": null,
      "sha256": "…"
    }
  ]
}
```

The Android variant uses `artifact = org.hyperledger.iroha:iroha-android`,
configuration `releaseRuntimeClasspath`, and points `primaryArtifact.path` to
the built AAR (`android/build/outputs/aar/android-release.aar`); the runtime
manifest/checksum mirror the same coordinates.

Attach this manifest and the CycloneDX SBOM to every release candidate so audit
logs prove which dependencies were bundled.

## Sample Evidence Bundle

```
artifacts/android/
├── maven/… (published jars/AARs/poms)
└── reports/0.2.0-dev.1/
    ├── android/
    │   ├── bom-android.json
    │   ├── bom-android.xml
    │   ├── iroha-android-android-0.2.0-dev.1-releaseRuntimeClasspath.json
    │   ├── iroha-android-android-runtime-manifest.json
    │   └── iroha-android-android-0.2.0-dev.1-runtime.sha256
    ├── jvm/
    │   ├── bom-jvm.json
    │   ├── bom-jvm.xml
    │   ├── iroha-android-0.2.0-dev.1-runtimeClasspath.json
    │   ├── iroha-android-jvm-runtime-manifest.json
    │   └── iroha-android-jvm-0.2.0-dev.1-runtime.sha256
    ├── artifacts/
    │   ├── android/… (aar, pom, module, sources)
    │   └── jvm/… (jar, pom, module, sources, javadoc)
    ├── checksums.txt
    ├── publish_summary.json
    └── signatures/… (optional)
```

`reports/` is safe to upload verbatim as part of the AND9 governance packet.

## Next Steps

- Integrate Sigstore/Provenance signing once the release infrastructure goes
  live.
- Mirror the `artifacts/android/reports/` directory into CI artefacts so every
  publish job exposes the SBOM + dependency manifest alongside the Maven
  repository snapshot.
