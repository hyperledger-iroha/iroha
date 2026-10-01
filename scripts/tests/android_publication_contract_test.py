#!/usr/bin/env python3
"""Publication command/filesystem contracts using explicit synthetic build stubs.

No Gradle/native/remote/cosign execution or release qualification is claimed.
Git source checks run against a temporary genuine fixture repository only.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class PublicationContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="android-publication-contract.")
        self.directory = Path(self.temp.name).resolve()
        self.repo = self.directory / "repo"
        self.artifacts = self.directory / "artifacts"
        self.artifacts.mkdir()
        (self.repo / "scripts").mkdir(parents=True)
        for name in ["publish_android_sdk.sh", "android_publish_snapshot.sh", "mobile_sdk_android_publication.py", "mobile_sdk_android_artifacts.py"]:
            shutil.copy2(ROOT / "scripts" / name, self.repo / "scripts" / name)
        (self.repo / "kotlin").mkdir()
        self.gradle = self.repo / "kotlin/gradlew"
        self.gradle.write_text('''#!/usr/bin/env python3
import hashlib,json,os,pathlib,subprocess,sys,zipfile
args=sys.argv[1:]
root=pathlib.Path(__file__).resolve().parent.parent
artifacts=pathlib.Path(os.environ['MOBILE_SDK_ANDROID_ARTIFACT_DIR'])
version=next(value.split('=',1)[1] for value in args if value.startswith('-PirohaSdkVersion='))
repo=pathlib.Path(next(value.split('=',1)[1] for value in args if value.startswith('-PirohaSdkRepoDir=')))
remote=any('ToRemoteSdkRepository' in value for value in args)
with (artifacts/'calls.jsonl').open('a') as log:
 log.write(json.dumps({'args':args,'remote':remote,'username_present':bool(os.environ.get('IROHA_SDK_MAVEN_USERNAME')),'password_present':bool(os.environ.get('IROHA_SDK_MAVEN_PASSWORD'))})+'\\n')
if remote:
 if os.environ.get('FIXTURE_REMOTE_FAIL')=='1':sys.exit(71)
 sys.exit(0)
commit=subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()
build=artifacts/'gradle-build/iroha_kotlin_sdk'
for module,dependency in [('core-jvm',''),('client-android','core-jvm'),('kagemusha-wallet-android','client-android')]:
 extension='jar' if module=='core-jvm' else 'aar'
 output=build/module/(f'libs/core-jvm-{version}.jar' if module=='core-jvm' else f'outputs/aar/{module}-release.aar')
 output.parent.mkdir(parents=True,exist_ok=True)
 if module=='core-jvm': output.write_bytes(b'public managed fixture')
 else:
  with zipfile.ZipFile(output,'w') as archive:
   archive.writestr('AndroidManifest.xml','<manifest/>');archive.writestr('classes.jar',b'managed fixture')
   if module=='client-android':
    payload=json.dumps({'schema':'iroha.android-native-build-provenance.v1','build_profile':'release','privacy_production_enabled':True,'cargo_locked':True,'source_tree_dirty':False,'source_commit':commit}).encode()
    provenance=build/module/'generated/nativeProvenance/production/iroha/native-build-provenance-v1.json'
    provenance.parent.mkdir(parents=True,exist_ok=True);provenance.write_bytes(payload)
    archive.writestr('assets/iroha/native-build-provenance-v1.json',payload)
    for abi in ['arm64-v8a','x86_64']:
     native=build/module/f'generated/jniLibs/production/{abi}/libconnect_norito_bridge.so'
     native.parent.mkdir(parents=True,exist_ok=True);native.write_bytes(b'synthetic native fixture')
 deps=(f'<dependencies><dependency><groupId>org.hyperledger.iroha.sdk</groupId><artifactId>{dependency}</artifactId><version>{version}</version></dependency></dependencies>' if dependency else '')
 pom=f'<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>org.hyperledger.iroha.sdk</groupId><artifactId>{module}</artifactId><version>{version}</version><packaging>{extension}</packaging>{deps}</project>'
 generated=build/module/'publications/release/pom-default.xml';generated.parent.mkdir(parents=True,exist_ok=True);generated.write_text(pom)
 directory=repo/'org/hyperledger/iroha/sdk'/module/version;directory.mkdir(parents=True)
 published=directory/f'{module}-{version}.{extension}';published.write_bytes(output.read_bytes())
 (directory/f'{module}-{version}.pom').write_text(pom)
 variants=[]
 for usage in ['java-api','java-runtime']:
  attributes={'org.gradle.category':'library','org.gradle.dependency.bundling':'external','org.gradle.libraryelements':extension,'org.gradle.usage':usage}
  if module=='core-jvm':attributes['org.gradle.jvm.version']=8
  dependencies=[{'group':'org.hyperledger.iroha.sdk','module':dependency,'version':{'requires':version}}] if dependency else []
  variants.append({'name':usage,'attributes':attributes,'dependencies':dependencies,'files':[{'name':published.name,'url':published.name,'size':published.stat().st_size,'sha256':hashlib.sha256(published.read_bytes()).hexdigest()}]})
 (directory/f'{module}-{version}.module').write_text(json.dumps({'formatVersion':'1.1','component':{'group':'org.hyperledger.iroha.sdk','module':module,'version':version},'variants':variants}))
if os.environ.get('FIXTURE_CHANGE_SOURCE')=='1':(root/'changed-source.txt').write_text('changed source')
''')
        self.gradle.chmod(0o755)
        (self.repo / "scripts/check_mobile_sdk_artifacts.sh").write_text("#!/bin/bash\nexit 0\n")
        (self.repo / "scripts/android_sbom_provenance.sh").write_text('''#!/usr/bin/env python3
import json,os,pathlib,sys
output=pathlib.Path(os.environ['MOBILE_SDK_SBOM_OUTPUT_DIR']);output.mkdir()
for module in ['core-jvm','client-android','kagemusha-wallet-android']:
 path=output/f'iroha-{module}.cyclonedx.json'
 path.write_text(json.dumps({'bomFormat':'CycloneDX','metadata':{'component':{'group':'org.hyperledger.iroha.sdk','name':module,'version':sys.argv[1]}},'components':[]}))
 path.with_suffix(path.suffix+'.sigstore').write_text('explicit synthetic signature fixture')
''')
        # The production caller explicitly executes SBOM via bash, so retain that
        # exact script process boundary while delegating only this fixture body.
        body = self.repo / "scripts/fixture_sbom.py"
        (self.repo / "scripts/android_sbom_provenance.sh").rename(body)
        (self.repo / "scripts/android_sbom_provenance.sh").write_text('#!/bin/bash\nexec "$MOBILE_SDK_PYTHON_BINARY" -I -S -B "$(dirname "$0")/fixture_sbom.py" "$@"\n')
        self.git(["init", "-q"])
        self.git(["config", "user.name", "Fixture"])
        self.git(["config", "user.email", "fixture@example.invalid"])
        self.git(["add", "."])
        self.git(["commit", "-qm", "synthetic publication command fixture"])

    def tearDown(self):
        self.temp.cleanup()

    def git(self, args):
        return subprocess.run(["git", "-C", str(self.repo), *args], check=True, capture_output=True)

    def run_publisher(self, *args, **updates):
        env = os.environ.copy()
        for name in list(env):
            if name.startswith(("ANDROID_PUBLISH_", "IROHA_SDK_MAVEN_")):
                del env[name]
        env.update(MOBILE_SDK_ANDROID_ARTIFACT_DIR=str(self.artifacts), MOBILE_SDK_PYTHON_BINARY=str(Path(sys.executable).resolve()))
        env.update(updates)
        return subprocess.run(["/bin/bash", str(self.repo / "scripts/publish_android_sdk.sh"), "--version", "1.2.3", *args], env=env, capture_output=True, text=True)

    def test_local_publication_calls_all_canonical_modules_and_keeps_receipt(self):
        result = self.run_publisher()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in (self.artifacts / "calls.jsonl").read_text().splitlines()]
        self.assertEqual(len(calls), 1)
        for module in ["core-jvm", "client-android", "kagemusha-wallet-android"]:
            self.assertIn(f":{module}:publishReleasePublicationToMobileSdkRepository", calls[0]["args"])
        self.assertIn("-PprivacyProductionEnabled=true", calls[0]["args"])
        self.assertEqual(
            [argument for argument in calls[0]["args"] if argument.startswith(":")],
            [f":{module}:publishReleasePublicationToMobileSdkRepository"
             for module in ["core-jvm", "client-android", "kagemusha-wallet-android"]],
        )
        report = json.loads((self.artifacts / "publication-1.2.3/publish_summary.json").read_text())
        self.assertEqual(report["modules"], ["core-jvm", "client-android", "kagemusha-wallet-android"])
        self.assertFalse(report["release_qualification"])
        self.assertEqual(report["version"], "1.2.3")

    def test_sbom_generation_has_no_regression_publication_prerequisite(self):
        source = (ROOT / "scripts/android_sbom_provenance.sh").read_text()
        command = source.split('"$SDK_GRADLE_WRAPPER" -p', 1)[1].split("\ncollect_sbom_reports", 1)[0]
        self.assertNotIn(":test", command)
        self.assertNotIn(":lint", command)
        for module in ["core-jvm", "client-android", "kagemusha-wallet-android"]:
            self.assertIn(f":{module}:cyclonedxDirectBom", command)

    def test_remote_tasks_follow_local_admission_with_environment_only_credentials(self):
        result = self.run_publisher("--repo-url", "https://maven.example.invalid/releases", ANDROID_PUBLISH_REPO_USERNAME="synthetic-user", ANDROID_PUBLISH_REPO_PASSWORD="synthetic-runtime-secret")
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in (self.artifacts / "calls.jsonl").read_text().splitlines()]
        self.assertEqual([entry["remote"] for entry in calls], [False, True])
        for module in ["core-jvm", "client-android", "kagemusha-wallet-android"]:
            self.assertIn(f":{module}:publishReleasePublicationToRemoteSdkRepository", calls[1]["args"])
        self.assertTrue(calls[1]["password_present"])
        public = result.stdout + result.stderr + (self.artifacts / "publication-1.2.3/publish_summary.json").read_text() + json.dumps(calls)
        self.assertNotIn("synthetic-runtime-secret", public)
        self.assertNotIn("-PirohaAndroid", public)

    def test_remote_failure_preserves_local_graph_and_does_not_claim_receipt(self):
        result = self.run_publisher("--repo-url", "https://maven.example.invalid/releases", FIXTURE_REMOTE_FAIL="1")
        self.assertEqual(result.returncode, 71)
        self.assertTrue((self.artifacts / "maven").is_dir())
        self.assertFalse((self.artifacts / "publication-1.2.3").exists())

    def test_dirty_or_changed_source_refuses_remote_publication(self):
        result = self.run_publisher("--repo-url", "https://maven.example.invalid/releases", FIXTURE_CHANGE_SOURCE="1")
        self.assertNotEqual(result.returncode, 0)
        calls = [json.loads(line) for line in (self.artifacts / "calls.jsonl").read_text().splitlines()]
        self.assertEqual(len(calls), 1)
        self.assertFalse((self.artifacts / "publication-1.2.3").exists())

    def test_existing_outputs_are_never_replaced(self):
        output = self.artifacts / "maven"
        output.mkdir(); sentinel = output / "keep.txt"; sentinel.write_text("preserve")
        result = self.run_publisher()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(sentinel.read_text(), "preserve")
        self.assertFalse((self.artifacts / "calls.jsonl").exists())

    def test_skip_sbom_and_insecure_or_embedded_credential_url_refuse_before_tasks(self):
        for args in [("--skip-sbom",), ("--repo-url", "http://maven.example.invalid/releases"),
                     ("--repo-url", "https://user:password@maven.example.invalid/releases")]:
            result = self.run_publisher(*args)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((self.artifacts / "calls.jsonl").exists())

    def test_runtime_credential_pair_and_dry_run_do_not_log_secrets_or_build(self):
        result = self.run_publisher("--dry-run", "--repo-url", "https://maven.example.invalid/releases", ANDROID_PUBLISH_REPO_USERNAME="synthetic-user", ANDROID_PUBLISH_REPO_PASSWORD="synthetic-runtime-secret")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("synthetic-runtime-secret", result.stdout + result.stderr)
        self.assertIn(":kagemusha-wallet-android:publishReleasePublicationToRemoteSdkRepository", result.stdout)
        self.assertFalse((self.artifacts / "calls.jsonl").exists())
        self.assertFalse((self.artifacts / "maven").exists())
        incomplete = self.run_publisher("--repo-url", "https://maven.example.invalid/releases", ANDROID_PUBLISH_REPO_USERNAME="synthetic-user")
        self.assertNotEqual(incomplete.returncode, 0)
        self.assertFalse((self.artifacts / "calls.jsonl").exists())

    def test_release_pipeline_keeps_credentials_out_of_public_command_and_owns_outputs(self):
        source = (ROOT / "scripts/run_release_pipeline.py").read_text()
        block = source.split("    if args.publish_android_sdk:", 1)[1].split("    fastpq_grafana_rel:", 1)[0]
        self.assertNotIn('publish_cmd.extend(["--password"', block)
        self.assertNotIn('publish_cmd.extend(["--username"', block)
        self.assertIn('publisher_env["ANDROID_PUBLISH_REPO_PASSWORD"]', block)
        self.assertIn('lambda: run(publish_cmd, env=publisher_env)', block)
        self.assertIn('"--sbom-dir", str(android_sbom_dir)', block)
        self.assertNotIn('REPO_ROOT / "artifacts" / "android"', block)
        self.assertNotIn('"--android-sdk-skip-sbom"', source)

    def test_all_three_gradle_repository_blocks_preserve_jdk8_api_gates(self):
        for module in ["core-jvm", "client-android", "kagemusha-wallet-android"]:
            source = (ROOT / f"kotlin/{module}/build.gradle.kts").read_text()
            self.assertIn('name = "remoteSdk"', source)
            self.assertIn('providers.environmentVariable("IROHA_SDK_MAVEN_PASSWORD")', source)
            self.assertIn('freeCompilerArgs.add("-Xjdk-release=8")', source)
            self.assertIn('name = "mobileSdk"', source)
            self.assertNotIn("irohaAndroidRepo", source)

if __name__ == "__main__":
    unittest.main()
