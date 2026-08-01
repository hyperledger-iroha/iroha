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

stage('Canonical Build') {
    node(amd64Label) {
        ws(dualProfileWorkspace) {
            checkout scm
            sh '''
set -euo pipefail
BUILD_PROFILE=deploy bash scripts/build_line.sh
'''
        }
    }
}

stage('Canonical Build Artifacts') {
    node(amd64Label) {
        ws(dualProfileWorkspace) {
            echo(
                'Jenkins does not create promotable release artifacts. Use the ' +
                'canonical reviewed-input release workflow and ' +
                'scripts/run_release_pipeline.py for the complete target matrix.'
            )
        }
    }
}

pipeline.runPipeline()
