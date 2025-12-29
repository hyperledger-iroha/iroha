@Library('jenkins-library') _

def pipeline = new org.iroha2PRDeploy.AppPipeline(steps: this,
    k8sPrDeploy: true,
    vaultPrPath: "argocd-cc/src/charts/iroha2/environments/tachi/",
    vaultUser: "iroha2-rw",
    vaultCredId: "iroha2VaultCreds",
    valuesDestPath: "argocd-cc/src/charts/iroha2/",
    devValuesPath: "dev/dev/",
    initialSecretName: "iroha2-eso-base",
    initialNameSpace: "iroha2-dev",
    targetNameSpace: "iroha2-${env.CHANGE_ID}-web",
    targetSecretName: "iroha2-${env.CHANGE_ID}-iroha2-pr-eso-base",
    disableSecretScanner: true
)

def dualProfileWorkspace = "dual-profiles-${env.BUILD_ID ?: env.BUILD_TAG ?: 'local'}"

def runShieldedMerkleCheck = { String arch, String label ->
    node(label) {
        ws("shielded-merkle-${arch}-${env.BUILD_ID ?: env.BUILD_TAG ?: 'local'}") {
            checkout scm
            sh 'bash scripts/ci_check_shielded_merkle.sh'
        }
    }
}

def amd64Label = (env.SHIELDED_MERKLE_AMD64_LABEL ?: 'linux-amd64').trim()
def arm64Label = (env.SHIELDED_MERKLE_ARM64_LABEL ?: 'linux-arm64').trim()

if (!amd64Label) {
    error('Shielded Merkle Determinism stage requires SHIELDED_MERKLE_AMD64_LABEL to resolve to a non-empty agent label')
}
if (!arm64Label) {
    error('Shielded Merkle Determinism stage requires SHIELDED_MERKLE_ARM64_LABEL to resolve to a non-empty agent label')
}

stage('Shielded Merkle Determinism') {
    def lanes = [:]
    lanes["amd64 (${amd64Label})"] = {
        runShieldedMerkleCheck('amd64', amd64Label)
    }
    lanes["arm64 (${arm64Label})"] = {
        runShieldedMerkleCheck('arm64', arm64Label)
    }
    parallel lanes
}

stage('Dual Profile Builds') {
    node(amd64Label) {
        ws(dualProfileWorkspace) {
            checkout scm
            sh '''
set -euo pipefail
BUILD_PROFILE=deploy bash scripts/build_line.sh --i3
BUILD_PROFILE=deploy bash scripts/build_line.sh --i2
'''
        }
    }
}

stage('Dual Profile Artifacts') {
    node(amd64Label) {
        ws(dualProfileWorkspace) {
            if (env.CHANGE_ID) {
                echo 'Skipping dual profile release artifacts for PR build'
            } else {
                checkout scm
                sh '''
set -euo pipefail

if ! command -v zstd >/dev/null 2>&1; then
    echo "zstd is required to build release bundles" >&2
    exit 1
fi

version=$(awk -F\" '/^version *=/ {print $2; exit}' Cargo.toml)
commit=$(git rev-parse --short HEAD)
timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

case "$(uname -s)" in
    Linux) os_tag=linux ;;
    Darwin) os_tag=mac ;;
    CYGWIN*|MINGW*|MSYS*) os_tag=win ;;
    *) os_tag=$(uname -s | tr '[:upper:]' '[:lower:]') ;;
esac

arch=$(uname -m)

artifacts_dir=artifacts
rm -rf "$artifacts_dir"
mkdir -p "$artifacts_dir"

./scripts/build_release_bundle.sh --profile iroha2 --config single --artifacts-dir "$artifacts_dir"
./scripts/build_release_bundle.sh --profile iroha3 --config nexus --artifacts-dir "$artifacts_dir"

./ci/dual_profile_smoke.sh "$artifacts_dir"/iroha2-*.tar.zst "$artifacts_dir"/iroha3-*.tar.zst
python3 scripts/select_release_profile.py \
    --emit-manifest "$artifacts_dir/network_profiles.json" \
    --artifact "iroha2=$artifacts_dir/iroha2-${version}-${os_tag}.tar.zst" \
    --artifact "iroha3=$artifacts_dir/iroha3-${version}-${os_tag}.tar.zst"

# Build AppImages for both profiles via Nix
nix build .#appimage_iroha2 --out-link result-iroha2
cp -L result-iroha2 "$artifacts_dir/iroha2-${version}-${arch}.AppImage"

nix build .#appimage_iroha3 --out-link result-iroha3
cp -L result-iroha3 "$artifacts_dir/iroha3-${version}-${arch}.AppImage"

rm -f result-iroha2 result-iroha3

# Generate aggregate checksums for downstream release automation
if command -v sha256sum >/dev/null 2>&1; then
    (cd "$artifacts_dir" && sha256sum *.tar.zst *.AppImage > SHA256SUMS)
elif command -v shasum >/dev/null 2>&1; then
    (cd "$artifacts_dir" && shasum -a 256 *.tar.zst *.AppImage > SHA256SUMS)
fi

python3 scripts/generate_release_manifest.py \
    --artifacts-dir "$artifacts_dir" \
    --version "$version" \
    --commit "$commit" \
    --built-at "$timestamp" \
    --os-tag "$os_tag" \
    --arch "$arch" \
    --output "$artifacts_dir/release_manifest.json"
'''
                archiveArtifacts artifacts: 'artifacts/**/*', fingerprint: true
            }
        }
    }
}

pipeline.runPipeline()
