package org.hyperledger.iroha.sdk.kagemusha.candidate.lab

import android.content.Context
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.os.Process
import android.os.SystemClock
import android.system.Os
import android.system.OsConstants
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.json.JSONArray
import org.json.JSONObject

internal object CandidateLabHarness {
    private const val CHECKPOINT_SCHEMA =
        "iroha.android.device_lab.kagemusha.candidate_checkpoint.v2"
    private const val TRANSCRIPT_SCHEMA =
        "iroha.android.device_lab.kagemusha.lifecycle_transcript.v2"
    private const val BINDING_SCHEMA =
        "iroha.android.device_lab.kagemusha.candidate_binding.v2"
    private const val CHECKPOINT_FILE = "phase-1-checkpoint-v2.json"
    private const val MAX_CANDIDATE_BYTES = 1024 * 1024
    private const val MAX_STAGE_MANIFEST_BYTES = 1024 * 1024
    private const val MAX_REQUEST_BYTES = 64 * 1024 * 1024
    private const val MAX_RESULT_BYTES = 64 * 1024 * 1024
    private const val STREAM_CHUNK_BYTES = 1024 * 1024
    private const val MAX_ARTIFACT_BYTES = 5L * 1024 * 1024 * 1024
    private const val ARTIFACT_SPOOL_RESERVE_BYTES = 1024L * 1024 * 1024
    private const val EXTERNAL_ARTIFACT_ROOT = "kagemusha-candidate-artifacts-v1"
    private const val EXTERNAL_ARTIFACT_BINDING = "artifact-set-binding-v1.txt"
    private const val EXTERNAL_ARTIFACT_BINDING_SCHEMA =
        "iroha.kagemusha.android_candidate_artifact_set.v1"
    private const val ACCEPTED_IDENTITY_FIELD_COUNT = 49
    private const val MAXIMUM_PROOF_HOPS = 8
    private const val TAIRA_I105_CHAIN_DISCRIMINANT = 369
    private val STRONGBOX_CHALLENGE_DOMAIN =
        "IROHA_KAGEMUSHA_STRONGBOX_CHALLENGE_V1\u0000".toByteArray(Charsets.US_ASCII)
    private val STRONGBOX_CHALLENGE_FIELDS = listOf(
        "slot_id",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "candidate_stage_manifest_sha256",
        "candidate_lab_native_library_sha256",
        "candidate_lab_apk_sha256",
        "candidate_lab_test_apk_sha256",
        "candidate_source_commit",
        "candidate_source_tree_sha256",
    )

    private const val PRIVATE_INIT_OPENING = "private-init-opening-v2.norito"
    private const val PRIVATE_HOP_ONE_RECIPIENT_OPENING =
        "private-hop-01-recipient-opening-v2.norito"
    private const val PRIVATE_HOP_ONE_CHANGE_OPENING =
        "private-hop-01-change-opening-v2.norito"
    private const val PRIVATE_HOP_TWO_RECIPIENT_OPENING =
        "private-hop-02-recipient-opening-v2.norito"
    private const val PRIVATE_HOP_TWO_CHANGE_OPENING =
        "private-hop-02-change-opening-v2.norito"

    private val privateStateFiles = listOf(
        PRIVATE_INIT_OPENING,
        PRIVATE_HOP_ONE_RECIPIENT_OPENING,
        PRIVATE_HOP_ONE_CHANGE_OPENING,
        PRIVATE_HOP_TWO_RECIPIENT_OPENING,
        PRIVATE_HOP_TWO_CHANGE_OPENING,
    )

    private val artifacts = listOf(
        Artifact(
            "step_eq_params_ipa",
            "step-eq.params-ipa.krv4",
        ),
        Artifact(
            "step_eq_proving_key",
            "step-eq.proving-key.krv4",
        ),
        Artifact(
            "step_eq_verifying_key",
            "step-eq.verifying-key.krv4",
        ),
        Artifact(
            "step_eq_bootstrap_witness",
            "step-eq.bootstrap-witness.krv4",
        ),
        Artifact(
            "step_ep_params_ipa",
            "step-ep.params-ipa.krv4",
        ),
        Artifact(
            "step_ep_proving_key",
            "step-ep.proving-key.krv4",
        ),
        Artifact(
            "step_ep_verifying_key",
            "step-ep.verifying-key.krv4",
        ),
        Artifact(
            "step_ep_bootstrap_witness",
            "step-ep.bootstrap-witness.krv4",
        ),
    )
    private val stageScenarioFiles = listOf(
        "init-top-up-anchor-v4.norito",
        "init-top-up-finality-proof-v2.norito",
        "init-top-up-finality-roster-artifact-v2.norito",
        "init-opening-v2.norito",
        "init-output-membership-v4.norito",
        "transfer-verifier-commitment-v2.bin",
        "append-hop-01-recipient-request-v2.norito",
        "append-hop-01-recipient-opening-v2.norito",
        "append-hop-01-change-opening-v2.norito",
        "append-hop-01-output-membership-v4.norito",
        "append-hop-01-operation-id.bin",
        "append-hop-01-block-height.txt",
        "append-hop-01-verified-at-ms.txt",
        "append-hop-02-recipient-request-v2.norito",
        "append-hop-02-recipient-opening-v2.norito",
        "append-hop-02-change-opening-v2.norito",
        "append-hop-02-output-membership-v4.norito",
        "append-hop-02-operation-id.bin",
        "append-hop-02-block-height.txt",
        "append-hop-02-verified-at-ms.txt",
        "redeem-recipient-account-id.txt",
        "unshield-verifier-commitment-v2.bin",
        "redeem-hop-01-operation-id.bin",
        "redeem-hop-01-block-height.txt",
        "redeem-hop-02-operation-id.bin",
        "redeem-hop-02-block-height.txt",
        "redeem-sender-change-operation-id.bin",
        "redeem-sender-change-block-height.txt",
        "duplicate-input-recipient-request-v2.norito",
        "duplicate-input-output-membership-v4.norito",
        "duplicate-input-operation-id.bin",
        "duplicate-input-block-height.txt",
        "duplicate-input-verified-at-ms.txt",
    )

    fun runProofPhase() {
        val context = targetContext()
        requireMarker()
        assertNetworkOffline(context)
        clearCheckpoint(context)
        evidenceDirectory(context).deleteRecursively()

        val apkIdentities = installedApkIdentities()
        val runtimeEvidence = RuntimeEvidenceBinding.fromInstrumentationArguments()
        runtimeEvidence.validate(apkIdentities)
        val events = CausalEvents.forPhaseOne()
        val sensitive = mutableListOf<ByteArray>()
        var installed = false
        var completed = false
        try {
            val identity = timed(events, "candidate_install") {
                installCandidate(context)
            }
            installed = true

            val initAnchor = readAsset(
                context,
                "scenario/init-top-up-anchor-v4.norito",
                MAX_REQUEST_BYTES,
            )
            val initFinalityProof = readAsset(
                context,
                "scenario/init-top-up-finality-proof-v2.norito",
                MAX_REQUEST_BYTES,
            )
            val initRoster = readAsset(
                context,
                "scenario/init-top-up-finality-roster-artifact-v2.norito",
                MAX_REQUEST_BYTES,
            )
            val initOpening = readAsset(
                context,
                "scenario/init-opening-v2.norito",
                MAX_REQUEST_BYTES,
            ).also { sensitive += it }
            val initOutputMembership = readAsset(
                context,
                "scenario/init-output-membership-v4.norito",
                MAX_REQUEST_BYTES,
            )
            val initRequest = timedNative(
                events,
                "build_init_request",
                listOf(
                    initAnchor,
                    initFinalityProof,
                    initRoster,
                    initOpening,
                    initOutputMembership,
                ),
            ) {
                KagemushaCandidateLabNative.nativeBuildInitRequestV4(
                    initAnchor,
                    initFinalityProof,
                    initRoster,
                    initOpening,
                    initOutputMembership,
                )
            }.also { sensitive += it }
            val initResult = timedNative(
                events,
                "init",
                listOf(initRequest),
            ) {
                KagemushaCandidateLabNative.nativeInitV4(initRequest)
            }
            val init = projectInit(initResult)

            val transferVerifierCommitment = readDigestAsset(
                context,
                "scenario/transfer-verifier-commitment-v2.bin",
            )
            val hopOneRecipient = readAsset(
                context,
                "scenario/append-hop-01-recipient-request-v2.norito",
                MAX_REQUEST_BYTES,
            )
            val hopOneRecipientOpening = readAsset(
                context,
                "scenario/append-hop-01-recipient-opening-v2.norito",
                MAX_REQUEST_BYTES,
            ).also { sensitive += it }
            val hopOneChangeOpening = readAsset(
                context,
                "scenario/append-hop-01-change-opening-v2.norito",
                MAX_REQUEST_BYTES,
            ).also { sensitive += it }
            val hopOneOutputMembership = readAsset(
                context,
                "scenario/append-hop-01-output-membership-v4.norito",
                MAX_REQUEST_BYTES,
            )
            val hopOneOperationId = readDigestAsset(
                context,
                "scenario/append-hop-01-operation-id.bin",
            )
            val hopOneBlockHeight = readPositiveLongAsset(
                context,
                "scenario/append-hop-01-block-height.txt",
            )
            val hopOneVerifiedAt = readPositiveLongAsset(
                context,
                "scenario/append-hop-01-verified-at-ms.txt",
            )
            val hopOneRequest = timedNative(
                events,
                "build_append_hop_01_request",
                listOf(
                    init.branch.bundle,
                    init.branch.topUpProvenance,
                    initOpening,
                    init.branch.membershipWitness,
                    hopOneChangeOpening,
                    hopOneOutputMembership,
                    transferVerifierCommitment,
                    hopOneOperationId,
                ),
            ) {
                KagemushaCandidateLabNative.nativeBuildAppendRequestV4(
                    arrayOf(init.branch.bundle),
                    arrayOf(init.branch.topUpProvenance),
                    arrayOf(initOpening),
                    arrayOf(init.branch.membershipWitness),
                    hopOneChangeOpening,
                    hopOneOutputMembership,
                    transferVerifierCommitment,
                    hopOneOperationId,
                    hopOneBlockHeight,
                )
            }.also { sensitive += it }
            val hopOneResult = timedNative(
                events,
                "append_hop_01",
                listOf(hopOneRequest, hopOneRecipient),
            ) {
                KagemushaCandidateLabNative.nativeAppendV4(
                    hopOneRequest,
                    hopOneRecipient,
                    hopOneVerifiedAt,
                )
            }
            val hopOne = projectSplit(hopOneResult)

            val hopOneChange = checkNotNull(hopOne.change) {
                "hop one must produce independently spendable change"
            }
            val hopTwoRecipient = readAsset(
                context,
                "scenario/append-hop-02-recipient-request-v2.norito",
                MAX_REQUEST_BYTES,
            )
            val hopTwoRecipientOpening = readAsset(
                context,
                "scenario/append-hop-02-recipient-opening-v2.norito",
                MAX_REQUEST_BYTES,
            ).also { sensitive += it }
            val hopTwoChangeOpening = readAsset(
                context,
                "scenario/append-hop-02-change-opening-v2.norito",
                MAX_REQUEST_BYTES,
            ).also { sensitive += it }
            val hopTwoOutputMembership = readAsset(
                context,
                "scenario/append-hop-02-output-membership-v4.norito",
                MAX_REQUEST_BYTES,
            )
            val hopTwoOperationId = readDigestAsset(
                context,
                "scenario/append-hop-02-operation-id.bin",
            )
            val hopTwoBlockHeight = readPositiveLongAsset(
                context,
                "scenario/append-hop-02-block-height.txt",
            )
            val hopTwoVerifiedAt = readPositiveLongAsset(
                context,
                "scenario/append-hop-02-verified-at-ms.txt",
            )
            val hopTwoRequest = timedNative(
                events,
                "build_append_hop_02_request",
                listOf(
                    hopOneChange.bundle,
                    hopOneChange.topUpProvenance,
                    hopOneChangeOpening,
                    hopOneChange.membershipWitness,
                    hopTwoChangeOpening,
                    hopTwoOutputMembership,
                    transferVerifierCommitment,
                    hopTwoOperationId,
                ),
            ) {
                KagemushaCandidateLabNative.nativeBuildAppendRequestV4(
                    arrayOf(hopOneChange.bundle),
                    arrayOf(hopOneChange.topUpProvenance),
                    arrayOf(hopOneChangeOpening),
                    arrayOf(hopOneChange.membershipWitness),
                    hopTwoChangeOpening,
                    hopTwoOutputMembership,
                    transferVerifierCommitment,
                    hopTwoOperationId,
                    hopTwoBlockHeight,
                )
            }.also { sensitive += it }
            val hopTwoResult = timedNative(
                events,
                "append_hop_02",
                listOf(hopTwoRequest, hopTwoRecipient),
            ) {
                KagemushaCandidateLabNative.nativeAppendV4(
                    hopTwoRequest,
                    hopTwoRecipient,
                    hopTwoVerifiedAt,
                )
            }
            val hopTwo = projectSplit(hopTwoResult)

            val hopTwoChange = checkNotNull(hopTwo.change) {
                "hop two must produce independently spendable change"
            }
            check(init.branch.amount.atomicUnits.signum() > 0) {
                "candidate lifecycle must begin with positive atomic value"
            }
            check(
                hopOne.recipient.amount.atomicUnits.signum() > 0 &&
                    hopTwo.recipient.amount.atomicUnits.signum() > 0 &&
                    hopTwoChange.amount.atomicUnits.signum() > 0,
            ) { "every independently redeemable branch must carry positive atomic value" }
            requireConservation(init.branch.amount, hopOne.recipient.amount, hopOneChange.amount)
            requireConservation(
                hopOneChange.amount,
                hopTwo.recipient.amount,
                hopTwoChange.amount,
            )
            check(init.branch.hopCount == 0 && init.branch.proofStepCount == 1) {
                "init result must bind exact hop zero and proof step one"
            }
            check(
                hopOne.recipient.hopCount == 1 &&
                    hopOne.recipient.proofStepCount == 2 &&
                    hopOneChange.hopCount == 1 &&
                    hopOneChange.proofStepCount == 2,
            ) { "first append must bind exact hop one and proof step two on both branches" }
            check(
                hopTwo.recipient.hopCount == 2 &&
                    hopTwo.recipient.proofStepCount == 3 &&
                    hopTwoChange.hopCount == 2 &&
                    hopTwoChange.proofStepCount == 3,
            ) { "second append must bind exact hop two and proof step three on both branches" }

            val checkpointDir = checkpointDirectory(context)
            val initFile = checkpointDir.resolve("init-result-v4.norito")
            val hopOneFile = checkpointDir.resolve("append-hop-01-result-v4.norito")
            val hopTwoFile = checkpointDir.resolve("append-hop-02-result-v4.norito")
            writeAtomic(initFile, initResult)
            writeAtomic(hopOneFile, hopOneResult)
            writeAtomic(hopTwoFile, hopTwoResult)
            writePrivateAtomic(checkpointDir.resolve(PRIVATE_INIT_OPENING), initOpening)
            writePrivateAtomic(
                checkpointDir.resolve(PRIVATE_HOP_ONE_RECIPIENT_OPENING),
                hopOneRecipientOpening,
            )
            writePrivateAtomic(
                checkpointDir.resolve(PRIVATE_HOP_ONE_CHANGE_OPENING),
                hopOneChangeOpening,
            )
            writePrivateAtomic(
                checkpointDir.resolve(PRIVATE_HOP_TWO_RECIPIENT_OPENING),
                hopTwoRecipientOpening,
            )
            writePrivateAtomic(
                checkpointDir.resolve(PRIVATE_HOP_TWO_CHANGE_OPENING),
                hopTwoChangeOpening,
            )

            val checkpoint = JSONObject()
                .put("schema", CHECKPOINT_SCHEMA)
                .put("candidate_lab_marker", BuildConfig.CANDIDATE_LAB_MARKER)
                .put("candidate_record_sha256", identity.candidateRecordSha256)
                .put("candidate_manifest_sha256", identity.candidateManifestSha256)
                .put("candidate_stage_manifest_path", "candidate-stage-manifest-v2.json")
                .put(
                    "candidate_stage_manifest_sha256",
                    BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256,
                )
                .put("native_accepted_inventory_sha256", identity.inventorySha256)
                .put("production_capability_observed", false)
                .put("network_offline_observed", true)
                .put("process_id", Process.myPid())
                .put("elapsed_realtime_nanos", SystemClock.elapsedRealtimeNanos())
                .put("init_request_sha256", sha256(initRequest))
                .put("append_hop_01_request_sha256", sha256(hopOneRequest))
                .put("append_hop_02_request_sha256", sha256(hopTwoRequest))
                .put("init_result_sha256", sha256(initResult))
                .put("append_hop_01_result_sha256", sha256(hopOneResult))
                .put("append_hop_02_result_sha256", sha256(hopTwoResult))
                .put("init_input_bundle_sha256", sha256(init.branch.bundle))
                .put("append_hop_01_input_bundle_sha256", sha256(init.branch.bundle))
                .put("append_hop_02_input_bundle_sha256", sha256(hopOneChange.bundle))
                .put("init_opening_sha256", sha256(initOpening))
                .put("hop_01_recipient_opening_sha256", sha256(hopOneRecipientOpening))
                .put("hop_01_change_opening_sha256", sha256(hopOneChangeOpening))
                .put("hop_02_recipient_opening_sha256", sha256(hopTwoRecipientOpening))
                .put("hop_02_change_opening_sha256", sha256(hopTwoChangeOpening))
                .put("hop_01_recipient_request_sha256", sha256(hopOneRecipient))
                .put("hop_02_recipient_request_sha256", sha256(hopTwoRecipient))
                .put("hop_01_block_height", hopOneBlockHeight)
                .put("hop_02_block_height", hopTwoBlockHeight)
                .put("hop_01_verified_at_ms", hopOneVerifiedAt)
                .put("hop_02_verified_at_ms", hopTwoVerifiedAt)
                .put("init_amount", init.branch.amount.toJson())
                .put("hop_01_recipient_amount", hopOne.recipient.amount.toJson())
                .put("hop_01_change_amount", hopOneChange.amount.toJson())
                .put("hop_02_recipient_amount", hopTwo.recipient.amount.toJson())
                .put("hop_02_change_amount", hopTwoChange.amount.toJson())
                .put("init_hop_count", init.branch.hopCount)
                .put("hop_01_hop_count", hopOne.recipient.hopCount)
                .put("hop_02_hop_count", hopTwo.recipient.hopCount)
                .put("init_proof_step_count", init.branch.proofStepCount)
                .put("hop_01_proof_step_count", hopOne.recipient.proofStepCount)
                .put("hop_02_proof_step_count", hopTwo.recipient.proofStepCount)
                .put("lab_apk_sha256", apkIdentities.main.sha256)
                .put("lab_test_apk_sha256", apkIdentities.test.sha256)
                .put("lab_apk_signing_cert_sha256", apkIdentities.main.signingCertificateSha256)
                .put(
                    "lab_test_apk_signing_cert_sha256",
                    apkIdentities.test.signingCertificateSha256,
                )
                .put("attestation_challenge_sha256", runtimeEvidence.challengeSha256)
                .put(
                    "attestation_certificate_chain_sha256",
                    runtimeEvidence.certificateChainSha256,
                )
                .put("app_signing_certificate_sha256", runtimeEvidence.appSigningCertificateSha256)
                .put("phase_1_events", events.complete())
                .put("status", "phase_1_passed")
            writeAtomic(
                checkpointDir.resolve(CHECKPOINT_FILE),
                (checkpoint.toString(2) + "\n").toByteArray(Charsets.UTF_8),
            )
            completed = true
        } finally {
            sensitive.forEach { it.fill(0) }
            if (!completed) runCatching { clearCheckpoint(context) }
            if (installed) uninstallCandidate()
        }
    }

    fun runRestartAndExportPhase() {
        val context = targetContext()
        requireMarker()
        assertNetworkOffline(context)
        val checkpointDir = checkpointDirectory(context)
        val checkpoint = readStrictJson(checkpointDir.resolve(CHECKPOINT_FILE))
        check(checkpoint.getString("schema") == CHECKPOINT_SCHEMA)
        check(checkpoint.getString("candidate_lab_marker") == BuildConfig.CANDIDATE_LAB_MARKER)
        check(checkpoint.getString("candidate_record_sha256") == BuildConfig.CANDIDATE_RECORD_SHA256)
        check(
            checkpoint.getString("candidate_stage_manifest_sha256") ==
                BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256,
        )
        check(!checkpoint.getBoolean("production_capability_observed"))
        check(checkpoint.getBoolean("network_offline_observed"))
        val firstPid = checkpoint.getInt("process_id")
        val secondPid = Process.myPid()
        check(firstPid != secondPid) {
            "restart phase must execute in a fresh Android instrumentation process"
        }
        val firstElapsed = checkpoint.getLong("elapsed_realtime_nanos")
        check(SystemClock.elapsedRealtimeNanos() > firstElapsed) {
            "device rebooted or monotonic clock regressed between lifecycle phases"
        }
        check(checkpoint.getString("status") == "phase_1_passed")

        val apkIdentities = installedApkIdentities()
        val runtimeEvidence = RuntimeEvidenceBinding.fromInstrumentationArguments()
        runtimeEvidence.validate(apkIdentities)
        check(checkpoint.getString("lab_apk_sha256") == apkIdentities.main.sha256)
        check(checkpoint.getString("lab_test_apk_sha256") == apkIdentities.test.sha256)
        check(
            checkpoint.getString("lab_apk_signing_cert_sha256") ==
                apkIdentities.main.signingCertificateSha256,
        )
        check(
            checkpoint.getString("lab_test_apk_signing_cert_sha256") ==
                apkIdentities.test.signingCertificateSha256,
        )
        check(
            checkpoint.getString("attestation_challenge_sha256") ==
                runtimeEvidence.challengeSha256,
        )
        check(
            checkpoint.getString("attestation_certificate_chain_sha256") ==
                runtimeEvidence.certificateChainSha256,
        )
        check(
            checkpoint.getString("app_signing_certificate_sha256") ==
                runtimeEvidence.appSigningCertificateSha256,
        )
        val events = CausalEvents.forPhaseTwo(checkpoint.getJSONArray("phase_1_events"))
        val sensitive = mutableListOf<ByteArray>()
        var installed = false
        try {
            val identity = timed(events, "candidate_reinstall_after_process_restart") {
                installCandidate(context)
            }
            installed = true
            check(identity.candidateRecordSha256 == checkpoint.getString("candidate_record_sha256"))
            check(identity.candidateManifestSha256 == checkpoint.getString("candidate_manifest_sha256"))
            check(identity.inventorySha256 == checkpoint.getString("native_accepted_inventory_sha256"))

            val initResult = readObservedResult(
                checkpointDir.resolve("init-result-v4.norito"),
                checkpoint.getString("init_result_sha256"),
            )
            val hopOneResult = readObservedResult(
                checkpointDir.resolve("append-hop-01-result-v4.norito"),
                checkpoint.getString("append_hop_01_result_sha256"),
            )
            val hopTwoResult = readObservedResult(
                checkpointDir.resolve("append-hop-02-result-v4.norito"),
                checkpoint.getString("append_hop_02_result_sha256"),
            )
            val restoredInit = timed(
                events,
                "restore_init_result_after_restart",
                listOf(initResult),
            ) {
                projectInit(initResult)
            }
            val restoredHopOne = timed(
                events,
                "restore_hop_01_result_after_restart",
                listOf(hopOneResult),
            ) {
                projectSplit(hopOneResult)
            }
            val restoredHopTwo = timed(
                events,
                "restore_hop_02_result_after_restart",
                listOf(hopTwoResult),
            ) {
                projectSplit(hopTwoResult)
            }
            val restoredHopOneChange = checkNotNull(restoredHopOne.change)
            val restoredHopTwoChange = checkNotNull(restoredHopTwo.change)
            check(
                sha256(restoredInit.branch.bundle) ==
                    checkpoint.getString("append_hop_01_input_bundle_sha256"),
            ) { "restored init branch is not the exact first append input" }
            check(
                sha256(restoredHopOneChange.bundle) ==
                    checkpoint.getString("append_hop_02_input_bundle_sha256"),
            ) { "restored hop-one change is not the exact second append input" }
            requireConservation(
                restoredInit.branch.amount,
                restoredHopOne.recipient.amount,
                restoredHopOneChange.amount,
            )
            requireConservation(
                restoredHopOneChange.amount,
                restoredHopTwo.recipient.amount,
                restoredHopTwoChange.amount,
            )

            val initOpening = readObservedPrivate(
                checkpointDir.resolve(PRIVATE_INIT_OPENING),
                checkpoint.getString("init_opening_sha256"),
            ).also { sensitive += it }
            val hopOneRecipientOpening = readObservedPrivate(
                checkpointDir.resolve(PRIVATE_HOP_ONE_RECIPIENT_OPENING),
                checkpoint.getString("hop_01_recipient_opening_sha256"),
            ).also { sensitive += it }
            val hopOneChangeOpening = readObservedPrivate(
                checkpointDir.resolve(PRIVATE_HOP_ONE_CHANGE_OPENING),
                checkpoint.getString("hop_01_change_opening_sha256"),
            ).also { sensitive += it }
            val hopTwoRecipientOpening = readObservedPrivate(
                checkpointDir.resolve(PRIVATE_HOP_TWO_RECIPIENT_OPENING),
                checkpoint.getString("hop_02_recipient_opening_sha256"),
            ).also { sensitive += it }
            val hopTwoChangeOpening = readObservedPrivate(
                checkpointDir.resolve(PRIVATE_HOP_TWO_CHANGE_OPENING),
                checkpoint.getString("hop_02_change_opening_sha256"),
            ).also { sensitive += it }

            val hopOneBlockHeight = checkpoint.getLong("hop_01_block_height")
            val hopTwoBlockHeight = checkpoint.getLong("hop_02_block_height")
            check(hopOneBlockHeight > 0 && hopTwoBlockHeight > 0)
            validateBranch(
                events,
                "validate_init_branch_after_restart",
                restoredInit.branch,
                initOpening,
                hopOneBlockHeight,
            )
            validateBranch(
                events,
                "validate_hop_01_change_continuity",
                restoredHopOneChange,
                hopOneChangeOpening,
                hopTwoBlockHeight,
            )
            validateBranch(
                events,
                "validate_hop_01_recipient_branch",
                restoredHopOne.recipient,
                hopOneRecipientOpening,
                hopOneBlockHeight,
            )
            validateBranch(
                events,
                "validate_hop_02_recipient_branch",
                restoredHopTwo.recipient,
                hopTwoRecipientOpening,
                hopTwoBlockHeight,
            )
            validateBranch(
                events,
                "validate_sender_change_branch",
                restoredHopTwoChange,
                hopTwoChangeOpening,
                hopTwoBlockHeight,
            )

            val hopOneRecipientRequest = readAsset(
                context,
                "scenario/append-hop-01-recipient-request-v2.norito",
                MAX_REQUEST_BYTES,
            )
            check(
                sha256(hopOneRecipientRequest) ==
                    checkpoint.getString("hop_01_recipient_request_sha256"),
            ) { "hop-one recipient request changed across the process restart" }
            val hopTwoRecipientRequest = readAsset(
                context,
                "scenario/append-hop-02-recipient-request-v2.norito",
                MAX_REQUEST_BYTES,
            )
            check(
                sha256(hopTwoRecipientRequest) ==
                    checkpoint.getString("hop_02_recipient_request_sha256"),
            ) { "hop-two recipient request changed across the process restart" }
            val verifyFirst = verifyBranch(
                events,
                "verify_first_recipient_proof",
                restoredHopOne.recipient,
                hopOneRecipientRequest,
                hopOneBlockHeight,
                checkpoint.getLong("hop_01_verified_at_ms"),
            )
            requireVerified(verifyFirst, restoredHopOne.recipient.amount, 1, 2)
            val verifyMultiHop = verifyBranch(
                events,
                "verify_multi_hop_recipient_proof",
                restoredHopTwo.recipient,
                hopTwoRecipientRequest,
                hopTwoBlockHeight,
                checkpoint.getLong("hop_02_verified_at_ms"),
            )
            requireVerified(verifyMultiHop, restoredHopTwo.recipient.amount, 2, 3)

            val transferVerifierCommitment = readDigestAsset(
                context,
                "scenario/transfer-verifier-commitment-v2.bin",
            )
            val duplicateOutputMembership = readAsset(
                context,
                "scenario/duplicate-input-output-membership-v4.norito",
                MAX_REQUEST_BYTES,
            )
            val duplicateOperationId = readDigestAsset(
                context,
                "scenario/duplicate-input-operation-id.bin",
            )
            val duplicateBlockHeight = readPositiveLongAsset(
                context,
                "scenario/duplicate-input-block-height.txt",
            )
            val duplicateRecipient = readAsset(
                context,
                "scenario/duplicate-input-recipient-request-v2.norito",
                MAX_REQUEST_BYTES,
            )
            val duplicateVerifiedAt = readPositiveLongAsset(
                context,
                "scenario/duplicate-input-verified-at-ms.txt",
            )
            val duplicateRequest = timedNative(
                events,
                "build_duplicate_input_request_from_observed_branch",
                listOf(
                    restoredHopOne.recipient.bundle,
                    restoredHopOne.recipient.topUpProvenance,
                    hopOneRecipientOpening,
                    restoredHopOne.recipient.membershipWitness,
                    duplicateOutputMembership,
                    transferVerifierCommitment,
                    duplicateOperationId,
                ),
            ) {
                KagemushaCandidateLabNative.nativeBuildDuplicateInputAppendRequestV4(
                    arrayOf(restoredHopOne.recipient.bundle),
                    arrayOf(restoredHopOne.recipient.topUpProvenance),
                    arrayOf(hopOneRecipientOpening),
                    arrayOf(restoredHopOne.recipient.membershipWitness),
                    byteArrayOf(),
                    duplicateOutputMembership,
                    transferVerifierCommitment,
                    duplicateOperationId,
                    duplicateBlockHeight,
                )
            }.also { sensitive += it }
            val duplicateStarted = SystemClock.elapsedRealtimeNanos()
            val duplicateFailure = runCatching {
                KagemushaCandidateLabNative.nativeAppendV4(
                    duplicateRequest,
                    duplicateRecipient,
                    duplicateVerifiedAt,
                )
            }.exceptionOrNull()
            check(duplicateFailure is IllegalArgumentException) {
                "duplicate-input append must be rejected as an invalid transition"
            }
            val duplicateMessage = duplicateFailure.message.orEmpty()
            check(duplicateMessage.isNotBlank()) {
                "duplicate-input rejection must carry a native error"
            }
            events.rejectedDuplicateInput(
                durationNanos = SystemClock.elapsedRealtimeNanos() - duplicateStarted,
                request = duplicateRequest,
                recipientRequest = duplicateRecipient,
                sourceBundle = restoredHopOne.recipient.bundle,
                exceptionClass = duplicateFailure.javaClass.name,
                errorMessage = duplicateMessage,
            )

            val redeemRecipient = readAsciiAsset(
                context,
                "scenario/redeem-recipient-account-id.txt",
                4096,
            )
            val unshieldVerifierCommitment = readDigestAsset(
                context,
                "scenario/unshield-verifier-commitment-v2.bin",
            )
            val redeemFirst = redeemBranch(
                events,
                "redeem_first_recipient",
                restoredHopOne.recipient,
                hopOneRecipientOpening,
                redeemRecipient,
                unshieldVerifierCommitment,
                readDigestAsset(context, "scenario/redeem-hop-01-operation-id.bin"),
                readPositiveLongAsset(context, "scenario/redeem-hop-01-block-height.txt"),
            )
            requireFullRedemption(redeemFirst, restoredHopOne.recipient.amount)
            val redeemSecond = redeemBranch(
                events,
                "redeem_second_recipient",
                restoredHopTwo.recipient,
                hopTwoRecipientOpening,
                redeemRecipient,
                unshieldVerifierCommitment,
                readDigestAsset(context, "scenario/redeem-hop-02-operation-id.bin"),
                readPositiveLongAsset(context, "scenario/redeem-hop-02-block-height.txt"),
            )
            requireFullRedemption(redeemSecond, restoredHopTwo.recipient.amount)
            val redeemSenderChange = redeemBranch(
                events,
                "redeem_sender_change",
                restoredHopTwoChange,
                hopTwoChangeOpening,
                redeemRecipient,
                unshieldVerifierCommitment,
                readDigestAsset(context, "scenario/redeem-sender-change-operation-id.bin"),
                readPositiveLongAsset(context, "scenario/redeem-sender-change-block-height.txt"),
            )
            requireFullRedemption(redeemSenderChange, restoredHopTwoChange.amount)
            check(
                redeemFirst.redeemed.scale == redeemSecond.redeemed.scale &&
                    redeemFirst.redeemed.scale == redeemSenderChange.redeemed.scale &&
                    redeemFirst.redeemed.scale == restoredInit.branch.amount.scale,
            ) { "independent redemption scales differ" }
            val redeemedAtomic =
                redeemFirst.redeemed.atomicUnits +
                    redeemSecond.redeemed.atomicUnits +
                    redeemSenderChange.redeemed.atomicUnits
            check(redeemedAtomic == restoredInit.branch.amount.atomicUnits) {
                "independent redemptions do not consume the complete initial value"
            }

            check(!KagemushaCandidateLabNative.nativeProductionCapabilityObservedV4()) {
                "candidate lab must never enable the production proof capability"
            }
            assertNetworkOffline(context)

            val transcript = JSONObject()
                .put("schema", TRANSCRIPT_SCHEMA)
                .put("slot_id", BuildConfig.SLOT_ID)
                .put("candidate_record_sha256", identity.candidateRecordSha256)
                .put("candidate_manifest_sha256", identity.candidateManifestSha256)
                .put("candidate_stage_manifest_path", "candidate-stage-manifest-v2.json")
                .put(
                    "candidate_stage_manifest_sha256",
                    BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256,
                )
                .put("candidate_inventory_sha256", identity.inventorySha256)
                .put("source_commit", identity.sourceCommit)
                .put("source_tree_sha256", identity.sourceTreeSha256)
                .put("source_repo_dirty", false)
                .put("generation", identity.generation)
                .put("bridge_abi_version", identity.bridgeAbiVersion)
                .put("production_capability_observed", false)
                .put("initial_atomic", restoredInit.branch.amount.atomicUnits.toString())
                .put(
                    "first_recipient_atomic",
                    restoredHopOne.recipient.amount.atomicUnits.toString(),
                )
                .put(
                    "second_recipient_atomic",
                    restoredHopTwo.recipient.amount.atomicUnits.toString(),
                )
                .put("sender_change_atomic", restoredHopTwoChange.amount.atomicUnits.toString())
                .put("redeemed_atomic", redeemedAtomic.toString())
                .put("final_unspent_atomic", "0")
                .put("proof_hops", verifyMultiHop.hopCount)
                .put("init_proof_verified", true)
                .put("first_spend_verified", true)
                .put("multi_hop_proof_verified", true)
                .put("independent_branch_redemption_verified", true)
                .put("duplicate_rejected", true)
                .put("restart_recovered", true)
                .put("network_requests_during_peer_transfers", 0)
                .put("attestation_challenge_sha256", runtimeEvidence.challengeSha256)
                .put(
                    "attestation_certificate_chain_sha256",
                    runtimeEvidence.certificateChainSha256,
                )
                .put("app_signing_certificate_sha256", runtimeEvidence.appSigningCertificateSha256)
                .put("strongbox_attestation", true)
                .put("physical_device_attestation", true)
                .put("causal_events", events.complete())

            val evidenceDir = evidenceDirectory(context)
            evidenceDir.deleteRecursively()
            check(evidenceDir.mkdirs()) { "failed to create candidate evidence output directory" }
            val transcriptFile = evidenceDir.resolve("lifecycle-transcript-v2.json")
            writeAtomic(
                transcriptFile,
                (transcript.toString(2) + "\n").toByteArray(Charsets.UTF_8),
            )
            val binding = candidateBinding(context, identity, transcriptFile, apkIdentities)
            writeAtomic(
                evidenceDir.resolve("candidate-binding-v2.json"),
                (binding.toString(2) + "\n").toByteArray(Charsets.UTF_8),
            )
            check(evidenceDir.listFiles()?.map { it.name }?.sorted() == listOf(
                "candidate-binding-v2.json",
                "lifecycle-transcript-v2.json",
            )) {
                "candidate lab may export only the observed binding and lifecycle transcript"
            }
            clearCheckpoint(context)
        } finally {
            sensitive.forEach { it.fill(0) }
            privateStateFiles.forEach { checkpointDir.resolve(it).delete() }
            if (installed) uninstallCandidate()
        }
    }

    private fun candidateBinding(
        context: Context,
        identity: AcceptedIdentity,
        transcriptFile: File,
        apkIdentities: InstalledApkIdentities,
    ): JSONObject {
        val applicationInfo = context.applicationInfo
        val native = File(
            applicationInfo.nativeLibraryDir,
            "libconnect_norito_bridge_candidate_lab.so",
        )
        check(native.isFile) { "installed candidate lab native library is unavailable" }
        check(sha256(native) == BuildConfig.NATIVE_LIBRARY_SHA256) {
            "installed candidate lab native library differs from the candidate input"
        }

        val inventory = JSONArray()
        identity.artifacts.forEachIndexed { index, artifact ->
            val contract = artifacts[index]
            check(artifact.role == contract.role)
            inventory.put(
                JSONObject()
                    .put("role", artifact.role)
                    .put("path", "evidence/candidate/artifacts/${contract.fileName}")
                    .put("framed_size_bytes", artifact.framedSize)
                    .put("framed_sha256", artifact.framedSha256)
                    .put("payload_size_bytes", artifact.payloadSize)
                    .put("payload_sha256", artifact.payloadSha256),
            )
        }

        return JSONObject()
            .put("schema", BINDING_SCHEMA)
            .put("candidate_record_path", "evidence/candidate/candidate-v4.norito")
            .put("candidate_record_sha256", identity.candidateRecordSha256)
            .put("candidate_manifest_path", "evidence/candidate/manifest-v4.norito")
            .put("candidate_manifest_sha256", identity.candidateManifestSha256)
            .put("candidate_stage_manifest_path", "candidate-stage-manifest-v2.json")
            .put(
                "candidate_stage_manifest_sha256",
                BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256,
            )
            .put("source_commit", identity.sourceCommit)
            .put("source_tree_sha256", identity.sourceTreeSha256)
            .put("source_repo_dirty", false)
            .put("generation", identity.generation)
            .put("bridge_abi_version", identity.bridgeAbiVersion)
            .put(
                "lab_native_library_path",
                "evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so",
            )
            .put("lab_native_library_sha256", sha256(native))
            .put("lab_apk_path", "evidence/${BuildConfig.LAB_APK_FILE_NAME}")
            .put("lab_apk_sha256", apkIdentities.main.sha256)
            .put(
                "lab_apk_signing_cert_sha256",
                apkIdentities.main.signingCertificateSha256,
            )
            .put("lab_test_apk_path", "evidence/${BuildConfig.LAB_TEST_APK_FILE_NAME}")
            .put("lab_test_apk_sha256", apkIdentities.test.sha256)
            .put(
                "lab_test_apk_signing_cert_sha256",
                apkIdentities.test.signingCertificateSha256,
            )
            .put("production_capability_observed", false)
            .put("native_accepted_candidate_record_sha256", identity.candidateRecordSha256)
            .put("native_accepted_candidate_manifest_sha256", identity.candidateManifestSha256)
            .put("native_accepted_source_commit", identity.sourceCommit)
            .put("native_accepted_source_tree_sha256", identity.sourceTreeSha256)
            .put("native_accepted_source_repo_dirty", false)
            .put("native_accepted_generation", identity.generation)
            .put("native_accepted_bridge_abi_version", identity.bridgeAbiVersion)
            .put("native_accepted_inventory_sha256", identity.inventorySha256)
            .put("lifecycle_transcript_path", "evidence/lifecycle-transcript-v2.json")
            .put("lifecycle_transcript_sha256", sha256(transcriptFile))
            .put("artifact_inventory", inventory)
    }

    private fun installCandidate(context: Context): AcceptedIdentity {
        check(KagemushaCandidateLabNative.nativeBridgeAbiVersion() == 22)
        check(!KagemushaCandidateLabNative.nativeProductionCapabilityObservedV4()) {
            "candidate lab native library unexpectedly reports production capability"
        }
        val stageManifest = readAsset(
            context,
            "stage/candidate-stage-manifest-v2.json",
            MAX_STAGE_MANIFEST_BYTES,
        )
        check(sha256(stageManifest) == BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256) {
            "packaged candidate stage manifest differs from the staged root identity"
        }
        val stageCatalog = validatePackagedStageManifest(context, stageManifest)
        val artifactDirectory = externalArtifactDirectory(context, stageCatalog)
        val candidate = readAsset(
            context,
            "candidate/candidate-v4.norito",
            MAX_CANDIDATE_BYTES,
        )
        val candidateSha = decodeHex(BuildConfig.CANDIDATE_RECORD_SHA256)
        check(sha256(candidate) == BuildConfig.CANDIDATE_RECORD_SHA256)
        val handles = LongArray(artifacts.size)
        var opened = 0
        var installed = false
        try {
            artifacts.forEachIndexed { index, artifact ->
                val expected = stageCatalog.artifacts.getValue(artifact.fileName)
                val handle = KagemushaCandidateLabNative.nativeArtifactBeginV4(
                    candidate,
                    candidateSha,
                    decodeHex(expected.sha256),
                )
                check(handle > 0) { "native candidate artifact begin returned no handle" }
                handles[index] = handle
                opened += 1
                streamExternalArtifact(
                    artifactDirectory.resolve(artifact.fileName),
                    expected,
                ) { chunk ->
                    KagemushaCandidateLabNative.nativeArtifactWriteV4(handle, chunk)
                }
                KagemushaCandidateLabNative.nativeArtifactFinalizeV4(handle)
            }
            KagemushaCandidateLabNative.nativeArtifactSetInstallV4(
                candidate,
                candidateSha,
                handles,
            )
            installed = true
            try {
                check(
                    KagemushaCandidateLabNative.nativeArtifactSetIsInstalledV4(
                        candidate,
                        candidateSha,
                    ),
                ) { "native candidate artifact set was not installed" }
            } catch (failure: Throwable) {
                runCatching { uninstallCandidate() }
                throw failure
            }
        } finally {
            if (!installed) {
                repeat(opened) { index ->
                    runCatching {
                        KagemushaCandidateLabNative.nativeArtifactCancelV4(handles[index])
                    }
                }
            }
            candidate.fill(0)
            candidateSha.fill(0)
        }
        return try {
            parseAcceptedIdentity(
                context,
                KagemushaCandidateLabNative.nativeAcceptedIdentityV4(),
                stageCatalog,
            )
        } catch (failure: Throwable) {
            runCatching { uninstallCandidate() }
            throw failure
        }
    }

    private fun validatePackagedStageManifest(
        context: Context,
        manifestBytes: ByteArray,
    ): ValidatedStageManifest {
        fun requireFields(value: JSONObject, fields: Set<String>, label: String) {
            val actual = mutableSetOf<String>()
            val iterator = value.keys()
            while (iterator.hasNext()) actual += iterator.next()
            check(actual == fields) { "$label fields are not the exact contract" }
        }
        fun nonzeroSha256(value: String, label: String): String = value.also {
            check(it.matches(Regex("^[0-9a-f]{64}$")) && it != "0".repeat(64)) {
                "$label is not non-zero lowercase SHA-256"
            }
        }

        val manifest = JSONObject(manifestBytes.toString(Charsets.UTF_8))
        requireFields(
            manifest,
            setOf(
                "schema", "version", "stage_manifest_path", "stage_manifest_mode",
                "stage_manifest_size_bytes", "candidate_record_sha256",
                "candidate_manifest_sha256", "candidate_validation_report_sha256",
                "qualification_receipt_sha256", "qualified_candidate_sha256",
                "scenario_inventory_sha256", "source_commit", "source_tree_sha256",
                "source_repo_dirty", "validator", "entry_count", "scenario_entry_count",
                "entries",
            ),
            "candidate stage manifest",
        )
        check(
            manifest.getString("schema") ==
                "iroha.kagemusha.android_candidate_stage_manifest.v2" &&
                manifest.getInt("version") == 2 &&
                manifest.getString("stage_manifest_path") ==
                "candidate-stage-manifest-v2.json" &&
                manifest.getString("stage_manifest_mode") == "0600" &&
                manifest.getLong("stage_manifest_size_bytes") == manifestBytes.size.toLong() &&
                manifest.getString("candidate_record_sha256") ==
                BuildConfig.CANDIDATE_RECORD_SHA256 &&
                manifest.getString("candidate_manifest_sha256") ==
                BuildConfig.CANDIDATE_MANIFEST_SHA256 &&
                manifest.getString("source_commit") == BuildConfig.SOURCE_COMMIT &&
                manifest.getString("source_tree_sha256") == BuildConfig.SOURCE_TREE_SHA256 &&
                !manifest.getBoolean("source_repo_dirty") &&
                manifest.getInt("entry_count") == 45 &&
                manifest.getInt("scenario_entry_count") == 33,
        ) { "candidate stage manifest identity is not exact" }

        val validator = manifest.getJSONObject("validator")
        requireFields(
            validator,
            setOf(
                "schema", "candidate_binary_name", "candidate_binary_sha256",
                "scenario_binary_name", "scenario_binary_sha256", "cargo_binary_sha256",
                "cargo_version_verbose", "rustc_binary_sha256", "rustc_version_verbose",
                "locked", "offline", "isolated_target", "build_jobs", "candidate_package",
                "scenario_package", "features", "profile",
            ),
            "candidate stage validator",
        )
        check(
            validator.getString("schema") ==
                "iroha.kagemusha.android_candidate_validator.v1" &&
                validator.getString("candidate_binary_name") ==
                "kagemusha_recursive_spend_v4_bundle" &&
                validator.getString("scenario_binary_name") ==
                "kagemusha_candidate_scenario_validator" &&
                validator.getBoolean("locked") &&
                validator.getBoolean("offline") &&
                validator.getBoolean("isolated_target") &&
                validator.getInt("build_jobs") == 2 &&
                validator.getString("candidate_package") == "iroha_core" &&
                validator.getString("scenario_package") == "connect_norito_bridge" &&
                validator.getJSONArray("features").let {
                    it.length() == 1 &&
                        it.getString(0) == "kagemusha-candidate-evidence-lab"
                } &&
                validator.getString("profile") == "debug",
        ) { "candidate stage validator identity is not exact" }
        listOf(
            "candidate_binary_sha256", "scenario_binary_sha256",
            "cargo_binary_sha256", "rustc_binary_sha256",
        ).forEach { key -> nonzeroSha256(validator.getString(key), "validator.$key") }
        listOf("cargo_version_verbose", "rustc_version_verbose").forEach { key ->
            val version = validator.getString(key)
            check(version.isNotBlank() && version.toByteArray().size <= 64 * 1024 &&
                version.endsWith("\n") &&
                version.none { it == '\u0000' || it == '\r' })
        }

        val expectedPaths = (
            listOf(
                "evidence/candidate/candidate-v4.norito",
                "evidence/candidate/manifest-v4.norito",
                "evidence/candidate/candidate-validation-v2.json",
                "evidence/candidate/recursive-step-two-qualification-v4.norito",
            ) +
                artifacts.map { "evidence/candidate/artifacts/${it.fileName}" } +
                stageScenarioFiles.map { "scenario/$it" }
        ).sorted()
        val entries = manifest.getJSONArray("entries")
        check(entries.length() == expectedPaths.size)
        val measured = linkedMapOf<String, AssetDigest>()
        expectedPaths.forEachIndexed { index, expectedPath ->
            val entry = entries.getJSONObject(index)
            requireFields(
                entry,
                setOf("path", "mode", "size_bytes", "sha256"),
                "candidate stage entry $index",
            )
            check(entry.getString("path") == expectedPath && entry.getString("mode") == "0600")
            val declaredSize = entry.getLong("size_bytes")
            val declaredSha256 = nonzeroSha256(
                entry.getString("sha256"),
                "candidate stage entry $expectedPath",
            )
            val digest = if (expectedPath.startsWith("evidence/candidate/artifacts/")) {
                check(declaredSize in 1..MAX_ARTIFACT_BYTES) {
                    "candidate artifact $expectedPath exceeds the V4 corridor"
                }
                AssetDigest(declaredSize, declaredSha256)
            } else {
                val assetPath = when {
                    expectedPath.startsWith("evidence/candidate/") ->
                        "candidate/${expectedPath.substringAfterLast('/')}"
                    expectedPath.startsWith("scenario/") -> expectedPath
                    else -> error("unsupported candidate stage path")
                }
                sha256Asset(context, assetPath).also {
                    check(declaredSize == it.size && declaredSha256 == it.sha256)
                }
            }
            measured[expectedPath] = digest
        }
        check(
            measured.getValue("evidence/candidate/candidate-v4.norito").sha256 ==
                BuildConfig.CANDIDATE_RECORD_SHA256,
        )
        check(
            measured.getValue("evidence/candidate/manifest-v4.norito").sha256 ==
                BuildConfig.CANDIDATE_MANIFEST_SHA256,
        )
        check(
            measured.getValue("evidence/candidate/candidate-validation-v2.json").sha256 ==
                manifest.getString("candidate_validation_report_sha256"),
        )
        val receiptSha256 =
            measured.getValue(
                "evidence/candidate/recursive-step-two-qualification-v4.norito",
            ).sha256
        val qualifiedCandidateSha256 = qualifiedCandidateSha256(
            BuildConfig.CANDIDATE_RECORD_SHA256,
            receiptSha256,
        )
        check(
            receiptSha256 == manifest.getString("qualification_receipt_sha256") &&
                qualifiedCandidateSha256 == manifest.getString("qualified_candidate_sha256"),
        ) { "candidate stage does not bind its qualification receipt" }
        val validationBytes = readAsset(
            context,
            "candidate/candidate-validation-v2.json",
            MAX_STAGE_MANIFEST_BYTES,
        )
        val validation = JSONObject(validationBytes.toString(Charsets.UTF_8))
        requireFields(
            validation,
            setOf(
                "schema", "candidate_record_sha256", "candidate_manifest_sha256",
                "qualification_receipt_file_name", "qualification_receipt_sha256",
                "qualified_candidate_sha256", "source_commit", "source_tree_sha256",
                "source_repo_dirty", "reviewed_source_closure_descriptor_sha256",
                "authenticated_source_seal_projection_sha256",
                "reviewed_cargo_binary_sha256", "reviewed_rustc_binary_sha256",
                "generation", "generation_memory_limit_bytes",
                "generation_memory_enforcement_profile", "bridge_abi_version",
                "artifact_count", "artifacts", "topup_finality_roster_file_name",
                "topup_finality_roster_size_bytes", "topup_finality_roster_sha256",
            ),
            "candidate validation report",
        )
        check(
            validation.getString("schema") ==
                "iroha.kagemusha.recursive_spend.candidate_validation.v2" &&
                validation.getString("candidate_record_sha256") ==
                BuildConfig.CANDIDATE_RECORD_SHA256 &&
                validation.getString("candidate_manifest_sha256") ==
                BuildConfig.CANDIDATE_MANIFEST_SHA256 &&
                validation.getString("qualification_receipt_file_name") ==
                "recursive-step-two-qualification-v4.norito" &&
                validation.getString("qualification_receipt_sha256") == receiptSha256 &&
                validation.getString("qualified_candidate_sha256") == qualifiedCandidateSha256 &&
                validation.getString("source_commit") == BuildConfig.SOURCE_COMMIT &&
                validation.getString("source_tree_sha256") == BuildConfig.SOURCE_TREE_SHA256 &&
                !validation.getBoolean("source_repo_dirty") &&
                validation.getString("generation") == BuildConfig.GENERATION &&
                validation.getLong("generation_memory_limit_bytes") in
                1L..(64L * 1024 * 1024 * 1024) &&
                validation.getString("generation_memory_enforcement_profile") ==
                "self-physical-footprint-v1" &&
                validation.getInt("bridge_abi_version") == 22 &&
                validation.getInt("artifact_count") == artifacts.size &&
                validation.getString("topup_finality_roster_file_name") ==
                "topup-finality-roster-v4.norito" &&
                validation.getLong("topup_finality_roster_size_bytes") > 0 &&
                nonzeroSha256(
                    validation.getString("topup_finality_roster_sha256"),
                    "candidate validation roster",
                ).isNotEmpty(),
        ) { "candidate validation report identity is not exact V2" }
        listOf(
            "reviewed_source_closure_descriptor_sha256",
            "authenticated_source_seal_projection_sha256",
            "reviewed_cargo_binary_sha256",
            "reviewed_rustc_binary_sha256",
        ).forEach { key ->
            nonzeroSha256(
                validation.getString(key),
                "candidate validation report $key",
            )
        }
        val validationArtifacts = validation.getJSONArray("artifacts")
        check(validationArtifacts.length() == artifacts.size)
        artifacts.forEachIndexed { index, expectedArtifact ->
            val artifact = validationArtifacts.getJSONObject(index)
            requireFields(
                artifact,
                setOf(
                    "role", "file_name", "framed_size_bytes", "framed_sha256",
                    "payload_size_bytes", "payload_sha256",
                ),
                "candidate validation artifact $index",
            )
            val stageDigest = measured.getValue(
                "evidence/candidate/artifacts/${expectedArtifact.fileName}",
            )
            check(
                artifact.getString("role") == expectedArtifact.role &&
                    artifact.getString("file_name") == expectedArtifact.fileName &&
                    artifact.getLong("framed_size_bytes") == stageDigest.size &&
                    artifact.getString("framed_sha256") == stageDigest.sha256 &&
                    artifact.getLong("payload_size_bytes") in 1L until stageDigest.size &&
                    nonzeroSha256(
                        artifact.getString("payload_sha256"),
                        "candidate validation artifact payload $index",
                    ) != stageDigest.sha256,
            ) { "candidate validation artifact $index measurement is not exact" }
        }
        val scenarioInventory = MessageDigest.getInstance("SHA-256")
        scenarioInventory.update(
            "iroha.kagemusha.android-candidate-scenario-inventory.v1\u0000"
                .toByteArray(Charsets.US_ASCII),
        )
        scenarioInventory.update(u32be(stageScenarioFiles.size))
        expectedPaths.filter { it.startsWith("scenario/") }.forEach { path ->
            val pathBytes = path.toByteArray(Charsets.UTF_8)
            val digest = measured.getValue(path)
            scenarioInventory.update(u32be(pathBytes.size))
            scenarioInventory.update(pathBytes)
            scenarioInventory.update(
                ByteBuffer.allocate(8).order(ByteOrder.BIG_ENDIAN).putLong(digest.size).array(),
            )
            scenarioInventory.update(decodeHex(digest.sha256))
        }
        check(
            hex(scenarioInventory.digest()) ==
                nonzeroSha256(
                    manifest.getString("scenario_inventory_sha256"),
                    "scenario inventory",
                ),
        ) { "packaged candidate scenario inventory is not exact" }
        return ValidatedStageManifest(
            artifacts.associate { artifact ->
                artifact.fileName to measured.getValue(
                    "evidence/candidate/artifacts/${artifact.fileName}",
                )
            },
        )
    }

    private fun externalArtifactDirectory(
        context: Context,
        stageCatalog: ValidatedStageManifest,
    ): File {
        val noBackupRoot = context.noBackupFilesDir.canonicalFile
        val artifactRoot = noBackupRoot.resolve(EXTERNAL_ARTIFACT_ROOT)
        val candidateRoot = artifactRoot.resolve(BuildConfig.CANDIDATE_RECORD_SHA256)
        val artifactDirectory = candidateRoot.resolve(BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256)
        listOf(
            artifactRoot to "external artifact root",
            candidateRoot to "candidate-bound external artifact root",
            artifactDirectory to "stage-bound external artifact root",
        ).forEach { (directory, label) ->
            val metadata = Os.lstat(directory.absolutePath)
            check(
                OsConstants.S_ISDIR(metadata.st_mode) &&
                    metadata.st_nlink >= 2L &&
                    metadata.st_uid == Process.myUid(),
            ) {
                "$label must be an app-owned real directory"
            }
            check((metadata.st_mode and 511) == 448) {
                "$label must be app-owner-only mode 0700"
            }
            check(directory.canonicalFile == directory.absoluteFile) {
                "$label must not traverse a symbolic link"
            }
        }

        val exactNames = (artifacts.map { it.fileName } + EXTERNAL_ARTIFACT_BINDING).sorted()
        check(artifactDirectory.list()?.sorted() == exactNames) {
            "app-private artifact directory is missing, extra, or not finalized"
        }
        val binding = artifactDirectory.resolve(EXTERNAL_ARTIFACT_BINDING)
        requirePrivateRegularFile(binding, "external artifact-set binding")
        val expectedBinding = buildString {
            append(EXTERNAL_ARTIFACT_BINDING_SCHEMA)
            append('\n')
            append(BuildConfig.CANDIDATE_RECORD_SHA256)
            append('\n')
            append(BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256)
            append('\n')
            append(artifacts.size)
            append('\n')
        }.toByteArray(Charsets.US_ASCII)
        check(binding.length() == expectedBinding.size.toLong()) {
            "external artifact-set binding has the wrong size"
        }
        val observedBinding = readExactPrivateFile(
            binding,
            expectedBinding.size,
            "external artifact-set binding",
        )
        check(observedBinding.contentEquals(expectedBinding)) {
            "external artifact-set binding does not match this candidate and stage"
        }

        val artifactBytes = stageCatalog.artifacts.values.fold(0L) { total, artifact ->
            Math.addExact(total, artifact.size)
        }
        check(artifactDirectory.usableSpace >= Math.addExact(
            artifactBytes,
            ARTIFACT_SPOOL_RESERVE_BYTES,
        )) {
            "app-private storage cannot spool the authenticated artifact set"
        }
        return artifactDirectory
    }

    private fun requirePrivateRegularFile(file: File, label: String) {
        val metadata = Os.lstat(file.absolutePath)
        check(
            OsConstants.S_ISREG(metadata.st_mode) &&
                metadata.st_nlink == 1L &&
                metadata.st_uid == Process.myUid(),
        ) { "$label must be one singly-linked app-owned regular file" }
        check((metadata.st_mode and 511) == 384) { "$label must have exact mode 0600" }
        check(file.canonicalFile == file.absoluteFile) { "$label must not be a symbolic link" }
    }

    private fun readExactPrivateFile(file: File, expectedSize: Int, label: String): ByteArray {
        requirePrivateRegularFile(file, label)
        val before = Os.lstat(file.absolutePath)
        check(before.st_size == expectedSize.toLong()) { "$label has the wrong size" }
        val output = ByteArray(expectedSize)
        FileInputStream(file).use { input ->
            val opened = Os.fstat(input.fd)
            check(
                OsConstants.S_ISREG(opened.st_mode) &&
                    opened.st_nlink == 1L &&
                    opened.st_uid == Process.myUid() &&
                    (opened.st_mode and 511) == 384 &&
                    opened.st_dev == before.st_dev &&
                    opened.st_ino == before.st_ino &&
                    opened.st_size == before.st_size,
            ) { "$label changed while opening" }
            var offset = 0
            while (offset < output.size) {
                val count = input.read(output, offset, output.size - offset)
                check(count > 0) { "$label ended before its declared size" }
                offset += count
            }
            check(input.read() == -1) { "$label grew while being read" }
            val finalOpened = Os.fstat(input.fd)
            check(
                OsConstants.S_ISREG(finalOpened.st_mode) &&
                    finalOpened.st_nlink == 1L &&
                    finalOpened.st_uid == Process.myUid() &&
                    (finalOpened.st_mode and 511) == 384 &&
                    finalOpened.st_dev == before.st_dev &&
                    finalOpened.st_ino == before.st_ino &&
                    finalOpened.st_size == before.st_size,
            ) { "$label changed while being read" }
        }
        val after = Os.lstat(file.absolutePath)
        check(
            OsConstants.S_ISREG(after.st_mode) &&
                after.st_nlink == 1L &&
                after.st_uid == Process.myUid() &&
                (after.st_mode and 511) == 384 &&
                after.st_dev == before.st_dev &&
                after.st_ino == before.st_ino &&
                after.st_size == before.st_size &&
                file.canonicalFile == file.absoluteFile,
        ) { "$label path changed while being read" }
        return output
    }

    private fun streamExternalArtifact(
        file: File,
        expected: AssetDigest,
        consume: (ByteArray) -> Unit,
    ) {
        requirePrivateRegularFile(file, "external candidate artifact ${file.name}")
        val before = Os.lstat(file.absolutePath)
        check(
            OsConstants.S_ISREG(before.st_mode) &&
                before.st_nlink == 1L &&
                before.st_uid == Process.myUid() &&
                (before.st_mode and 511) == 384 &&
                before.st_size == expected.size,
        ) {
            "external candidate artifact ${file.name} size differs from the stage catalog"
        }
        val digest = MessageDigest.getInstance("SHA-256")
        var size = 0L
        FileInputStream(file).use { input ->
            val opened = Os.fstat(input.fd)
            check(
                OsConstants.S_ISREG(opened.st_mode) &&
                    opened.st_nlink == 1L &&
                    opened.st_uid == Process.myUid() &&
                    (opened.st_mode and 511) == 384 &&
                    opened.st_dev == before.st_dev &&
                    opened.st_ino == before.st_ino &&
                    opened.st_size == before.st_size,
            ) { "external candidate artifact ${file.name} changed while opening" }
            val buffer = ByteArray(STREAM_CHUNK_BYTES)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                size = Math.addExact(size, count.toLong())
                check(size <= expected.size) {
                    "external candidate artifact ${file.name} grew while streaming"
                }
                digest.update(buffer, 0, count)
                consume(if (count == buffer.size) buffer else buffer.copyOf(count))
            }
            val finalOpened = Os.fstat(input.fd)
            check(
                OsConstants.S_ISREG(finalOpened.st_mode) &&
                    finalOpened.st_nlink == 1L &&
                    finalOpened.st_uid == Process.myUid() &&
                    (finalOpened.st_mode and 511) == 384 &&
                    finalOpened.st_dev == before.st_dev &&
                    finalOpened.st_ino == before.st_ino &&
                    finalOpened.st_size == before.st_size,
            ) { "external candidate artifact ${file.name} changed while streaming" }
        }
        val after = Os.lstat(file.absolutePath)
        check(
            OsConstants.S_ISREG(after.st_mode) &&
                after.st_nlink == 1L &&
                after.st_uid == Process.myUid() &&
                (after.st_mode and 511) == 384 &&
                after.st_dev == before.st_dev &&
                after.st_ino == before.st_ino &&
                after.st_size == before.st_size &&
                file.canonicalFile == file.absoluteFile,
        ) { "external candidate artifact ${file.name} path changed while streaming" }
        check(size == expected.size && hex(digest.digest()) == expected.sha256) {
            "external candidate artifact ${file.name} failed its exact stage-catalog binding"
        }
    }

    private fun uninstallCandidate() {
        KagemushaCandidateLabNative.nativeArtifactSetUninstallV4(
            decodeHex(BuildConfig.CANDIDATE_RECORD_SHA256),
        )
    }

    private fun parseAcceptedIdentity(
        context: Context,
        fields: Array<ByteArray>,
        stageCatalog: ValidatedStageManifest,
    ): AcceptedIdentity {
        check(fields.size == ACCEPTED_IDENTITY_FIELD_COUNT) {
            "native accepted identity must contain exactly 49 fields"
        }
        val candidateSha = requireDigest(fields[0], "candidate record")
        val manifestSha = requireDigest(fields[1], "candidate manifest")
        check(fields[2].contentEquals(byteArrayOf(0))) {
            "native identity must observe production capability as false"
        }
        val inventorySha = requireDigest(fields[3], "artifact inventory")
        val generation = requireAscii(fields[4], "generation")
        val sourceCommit = requireAscii(fields[5], "source commit")
        val sourceTreeSha = normalizeSha256(fields[6], "source tree")
        check(fields[7].contentEquals(byteArrayOf(0))) {
            "native accepted candidate must report a clean source tree"
        }
        val bridgeAbi = requireAscii(fields[8], "bridge ABI").toInt()
        check(bridgeAbi == 21)
        check(candidateSha == BuildConfig.CANDIDATE_RECORD_SHA256)
        check(manifestSha == BuildConfig.CANDIDATE_MANIFEST_SHA256)
        check(generation == BuildConfig.GENERATION)
        check(sourceCommit == BuildConfig.SOURCE_COMMIT)
        check(sourceTreeSha == BuildConfig.SOURCE_TREE_SHA256)
        val observed = mutableListOf<AcceptedArtifact>()
        var cursor = 9
        artifacts.forEach { contract ->
            val role = requireAscii(fields[cursor++], "artifact role")
            val framedSize = requireAscii(fields[cursor++], "artifact framed size").toLong()
            val framedSha = requireDigest(fields[cursor++], "artifact framed digest")
            val payloadSize = requireAscii(fields[cursor++], "artifact payload size").toLong()
            val payloadSha = requireDigest(fields[cursor++], "artifact payload digest")
            check(role == contract.role) { "native artifact inventory is not in canonical order" }
            check(framedSize > 0 && payloadSize > 0 && payloadSize <= framedSize)
            val staged = stageCatalog.artifacts.getValue(contract.fileName)
            check(staged.size == framedSize && staged.sha256 == framedSha) {
                "native accepted framed artifact differs from the authenticated stage catalog"
            }
            observed += AcceptedArtifact(
                role,
                framedSize,
                framedSha,
                payloadSize,
                payloadSha,
            )
        }
        check(cursor == fields.size)
        val recomputedInventory = inventorySha256(observed)
        check(recomputedInventory == inventorySha) {
            "native accepted artifact inventory digest is not canonical"
        }
        val candidateAsset = sha256Asset(context, "candidate/candidate-v4.norito")
        val manifestAsset = sha256Asset(context, "candidate/manifest-v4.norito")
        check(candidateAsset.sha256 == candidateSha)
        check(manifestAsset.sha256 == manifestSha)
        return AcceptedIdentity(
            candidateSha,
            manifestSha,
            inventorySha,
            generation,
            sourceCommit,
            sourceTreeSha,
            bridgeAbi,
            observed,
        )
    }

    private fun inventorySha256(inventory: List<AcceptedArtifact>): String {
        val digest = MessageDigest.getInstance("SHA-256")
        inventory.forEach { artifact ->
            val record = buildString {
                append(artifact.role)
                append('\u0000')
                append(artifact.framedSize)
                append('\u0000')
                append(artifact.framedSha256)
                append('\u0000')
                append(artifact.payloadSize)
                append('\u0000')
                append(artifact.payloadSha256)
                append('\n')
            }
            digest.update(record.toByteArray(Charsets.UTF_8))
        }
        return hex(digest.digest())
    }

    private fun validateBranch(
        events: CausalEvents,
        operation: String,
        branch: BranchProjection,
        opening: ByteArray,
        blockHeight: Long,
    ) {
        check(blockHeight > 0)
        timedNative(
            events,
            operation,
            listOf(
                branch.bundle,
                branch.topUpProvenance,
                branch.membershipWitness,
                opening,
            ),
        ) {
            KagemushaCandidateLabNative.nativeValidateBranchV4(
                branch.bundle,
                branch.topUpProvenance,
                branch.membershipWitness,
                opening,
                blockHeight,
            )
        }
    }

    private fun verifyBranch(
        events: CausalEvents,
        operation: String,
        branch: BranchProjection,
        recipientRequest: ByteArray,
        blockHeight: Long,
        verifiedAtMilliseconds: Long,
    ): VerifyProjection {
        val request = timedNative(
            events,
            "build_${operation}_request",
            listOf(branch.bundle, recipientRequest, branch.topUpProvenance),
        ) {
            KagemushaCandidateLabNative.nativeBuildVerifyRequestV4(
                branch.bundle,
                recipientRequest,
                branch.topUpProvenance,
                MAXIMUM_PROOF_HOPS,
                blockHeight,
                verifiedAtMilliseconds,
            )
        }
        val result = timedNative(events, operation, listOf(request)) {
            KagemushaCandidateLabNative.nativeVerifyV4(request)
        }
        return projectVerify(result)
    }

    private fun requireVerified(
        projection: VerifyProjection,
        expectedAmount: AtomicQuantity,
        expectedHops: Int,
        expectedProofSteps: Int,
    ) {
        check(
            projection.valid &&
                projection.chainAdmissible &&
                projection.lineageRedeemable &&
                projection.witnesslessRedemptionSupported,
        ) { "native candidate verifier rejected an expected spendable branch" }
        check(
            projection.hopCount == expectedHops &&
                projection.proofStepCount == expectedProofSteps,
        ) {
            "native candidate verifier reported the wrong exact hop/proof-step pair"
        }
        check(projection.amount == expectedAmount) {
            "verified amount does not match its independently spendable branch"
        }
    }

    private fun redeemBranch(
        events: CausalEvents,
        operation: String,
        branch: BranchProjection,
        opening: ByteArray,
        recipient: ByteArray,
        verifierCommitment: ByteArray,
        operationId: ByteArray,
        blockHeight: Long,
    ): RedeemProjection {
        val atomicUnits = branch.amount.atomicUnits.toString().toByteArray(Charsets.US_ASCII)
        val request = timedNative(
            events,
            "build_${operation}_request",
            listOf(
                branch.bundle,
                branch.topUpProvenance,
                opening,
                branch.membershipWitness,
                recipient,
                u32be(TAIRA_I105_CHAIN_DISCRIMINANT),
                atomicUnits,
                verifierCommitment,
                operationId,
            ),
        ) {
            KagemushaCandidateLabNative.nativeBuildRedeemRequestV4(
                branch.bundle,
                branch.topUpProvenance,
                opening,
                branch.membershipWitness,
                recipient,
                TAIRA_I105_CHAIN_DISCRIMINANT,
                atomicUnits,
                branch.amount.scale,
                byteArrayOf(),
                byteArrayOf(),
                verifierCommitment,
                operationId,
                blockHeight,
            )
        }
        return try {
            val result = timedNative(events, operation, listOf(request)) {
                KagemushaCandidateLabNative.nativeRedeemV4(request)
            }
            projectRedeem(result)
        } finally {
            atomicUnits.fill(0)
            request.fill(0)
        }
    }

    private fun requireFullRedemption(
        projection: RedeemProjection,
        expectedAmount: AtomicQuantity,
    ) {
        check(projection.change == null) {
            "full branch redemption unexpectedly produced offline change"
        }
        check(projection.redeemed == expectedAmount) {
            "full branch redemption amount does not match the spendable branch"
        }
    }

    private fun projectInit(result: ByteArray): InitProjection {
        requireResult(result, "init")
        val cursor = ProjectionCursor(
            KagemushaCandidateLabNative.nativeProjectInitResultV4(result),
            "init",
        )
        cursor.version()
        val topUpProvenance = cursor.archive("top-up provenance")
        val branch = cursor.branch("init branch", topUpProvenance)
        requireDigest(cursor.next("public statement digest"), "public statement")
        cursor.finish()
        return InitProjection(branch)
    }

    private fun projectSplit(result: ByteArray): SplitProjection {
        requireResult(result, "split")
        val cursor = ProjectionCursor(
            KagemushaCandidateLabNative.nativeProjectSplitResultV4(result),
            "split",
        )
        cursor.version()
        cursor.next("peer payment")
        requireDigest(cursor.next("operation id"), "operation id")
        requireDigest(cursor.next("request digest"), "request digest")
        requireDigest(cursor.next("split binding digest"), "split binding digest")
        val recipientProvenance = cursor.archive("recipient provenance")
        val recipient = cursor.branch("recipient branch", recipientProvenance)
        val change = if (cursor.bool("change present")) {
            val changeProvenance = cursor.archive("change provenance")
            cursor.branch("change branch", changeProvenance)
        } else {
            null
        }
        cursor.finish()
        return SplitProjection(recipient, change)
    }

    private fun projectVerify(result: ByteArray): VerifyProjection {
        requireResult(result, "verify")
        val cursor = ProjectionCursor(
            KagemushaCandidateLabNative.nativeProjectVerifyResultV4(result),
            "verify",
        )
        cursor.version()
        val valid = cursor.bool("valid")
        val chain = cursor.bool("chain admissible")
        val lineage = cursor.bool("lineage redeemable")
        val witnessless = cursor.bool("witnessless redemption")
        requireDigest(cursor.next("commitment"), "commitment")
        requireDigest(cursor.next("nullifier"), "nullifier")
        val amount = cursor.amount("verified amount")
        val hop = cursor.decimalInt("hop count")
        val proofSteps = cursor.decimalInt("proof step count")
        requireDigest(cursor.next("bundle digest"), "bundle digest")
        cursor.ascii("asset definition")
        cursor.next("artifact binding")
        requireDigest(cursor.next("request digest"), "request digest")
        requireDigest(cursor.next("output binding digest"), "output binding")
        cursor.ascii("verifier backend")
        cursor.ascii("verifier name")
        cursor.ascii("verifier circuit id")
        cursor.next("activation height")
        cursor.next("withdrawal height")
        check(cursor.decimalLong("verified block height") > 0)
        check(cursor.decimalLong("verified timestamp") > 0)
        val claims = cursor.count("branch claim count")
        repeat(claims) { cursor.next("branch claim[$it]") }
        cursor.finish()
        return VerifyProjection(valid, chain, lineage, witnessless, amount, hop, proofSteps)
    }

    private fun projectRedeem(result: ByteArray): RedeemProjection {
        requireResult(result, "redeem")
        val fields = KagemushaCandidateLabNative.nativeProjectRedeemResultV4(result)
        check(fields.size >= 7) { "redeem projection omitted value fields" }
        val redeemedAtomic = requireAscii(fields[fields.lastIndex - 1], "redeemed atomic units")
        val redeemedScale = requireAscii(fields[fields.lastIndex], "redeemed scale")
        val redeemed = AtomicQuantity(BigInteger(redeemedAtomic), redeemedScale.toInt())
        val cursor = ProjectionCursor(fields.copyOfRange(0, fields.size - 2), "redeem")
        cursor.version()
        cursor.next("unsigned redemption")
        requireDigest(cursor.next("authorization digest"), "authorization digest")
        requireDigest(cursor.next("operation id"), "operation id")
        val change = if (cursor.bool("change present")) {
            val changeProvenance = cursor.archive("change provenance")
            cursor.branch("redemption change", changeProvenance)
        } else {
            null
        }
        cursor.finish()
        return RedeemProjection(redeemed, change)
    }

    private fun requireConservation(
        input: AtomicQuantity,
        output: AtomicQuantity,
        change: AtomicQuantity,
    ) {
        check(input.scale == output.scale && input.scale == change.scale) {
            "value conservation scales differ"
        }
        check(input.atomicUnits == output.atomicUnits + change.atomicUnits) {
            "value conservation failed"
        }
    }

    private fun requireResult(result: ByteArray, label: String) {
        check(result.isNotEmpty() && result.size <= MAX_RESULT_BYTES) {
            "native $label result is absent or too large"
        }
    }

    private fun readObservedResult(file: File, expectedSha256: String): ByteArray {
        check(file.isFile && file.length() in 1..MAX_RESULT_BYTES.toLong()) {
            "persisted observed result is absent or too large"
        }
        val bytes = FileInputStream(file).use { it.readBytes() }
        check(sha256(bytes) == expectedSha256) { "persisted observed result digest changed" }
        return bytes
    }

    private fun readObservedPrivate(file: File, expectedSha256: String): ByteArray {
        check(file.isFile && file.length() in 1..MAX_REQUEST_BYTES.toLong()) {
            "persisted private lifecycle input is absent or too large"
        }
        check((Os.stat(file.absolutePath).st_mode and 511) == 384) {
            "persisted private lifecycle input must be owner-readable and owner-writable only"
        }
        val bytes = FileInputStream(file).use { it.readBytes() }
        check(sha256(bytes) == expectedSha256) {
            "persisted private lifecycle input digest changed"
        }
        return bytes
    }

    private fun installedApkIdentities(): InstalledApkIdentities {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val main = installedApkIdentity(
            instrumentation.targetContext,
            "org.hyperledger.iroha.sdk.kagemusha.candidate.lab",
        )
        val test = installedApkIdentity(
            instrumentation.context,
            "org.hyperledger.iroha.sdk.kagemusha.candidate.lab.test",
        )
        check(main.sha256 != test.sha256) {
            "main and androidTest APKs must be distinct exact artifacts"
        }
        check(main.signingCertificateSha256 == test.signingCertificateSha256) {
            "main and androidTest APKs must have the same signing certificate"
        }
        return InstalledApkIdentities(main, test)
    }

    private fun installedApkIdentity(context: Context, expectedPackage: String): InstalledApkIdentity {
        check(context.packageName == expectedPackage) {
            "instrumentation context package does not match $expectedPackage"
        }
        val applicationInfo = context.applicationInfo
        check(applicationInfo.splitSourceDirs.isNullOrEmpty()) {
            "$expectedPackage must be installed as one exact base APK"
        }
        val sourceApk = File(applicationInfo.sourceDir)
        check(sourceApk.isFile && sourceApk.length() > 0L) {
            "installed APK for $expectedPackage is unavailable"
        }
        val packageInfo = context.packageManager.getPackageInfo(
            expectedPackage,
            PackageManager.GET_SIGNING_CERTIFICATES,
        )
        val signers = checkNotNull(packageInfo.signingInfo).apkContentsSigners
        check(signers.size == 1) {
            "$expectedPackage must have exactly one current APK signing certificate"
        }
        return InstalledApkIdentity(
            installedPath = sourceApk.absolutePath,
            sha256 = sha256(sourceApk),
            signingCertificateSha256 = sha256(signers.single().toByteArray()),
        )
    }

    private fun deriveStrongboxChallenge(apks: InstalledApkIdentities): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update(STRONGBOX_CHALLENGE_DOMAIN)
        val fields = listOf(
            "slot_id" to BuildConfig.SLOT_ID,
            "candidate_record_sha256" to BuildConfig.CANDIDATE_RECORD_SHA256,
            "candidate_manifest_sha256" to BuildConfig.CANDIDATE_MANIFEST_SHA256,
            "candidate_stage_manifest_sha256" to BuildConfig.CANDIDATE_STAGE_MANIFEST_SHA256,
            "candidate_lab_native_library_sha256" to BuildConfig.NATIVE_LIBRARY_SHA256,
            "candidate_lab_apk_sha256" to apks.main.sha256,
            "candidate_lab_test_apk_sha256" to apks.test.sha256,
            "candidate_source_commit" to BuildConfig.SOURCE_COMMIT,
            "candidate_source_tree_sha256" to BuildConfig.SOURCE_TREE_SHA256,
        )
        check(fields.map { it.first } == STRONGBOX_CHALLENGE_FIELDS)
        fields.forEach { (name, value) ->
            val nameBytes = name.toByteArray(Charsets.UTF_8)
            val valueBytes = value.toByteArray(Charsets.UTF_8)
            digest.update(u32be(nameBytes.size))
            digest.update(nameBytes)
            digest.update(u32be(valueBytes.size))
            digest.update(valueBytes)
        }
        return digest.digest()
    }

    private fun u32be(value: Int): ByteArray =
        ByteBuffer.allocate(4).order(ByteOrder.BIG_ENDIAN).putInt(value).array()

    private fun targetContext(): Context =
        InstrumentationRegistry.getInstrumentation().targetContext

    private fun requireMarker() {
        check(
            BuildConfig.CANDIDATE_LAB_MARKER ==
                "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
        ) { "candidate lab marker is missing" }
        check(BuildConfig.CANDIDATE_RECORD_SHA256.matches(Regex("^[0-9a-f]{64}$")))
    }

    private fun assertNetworkOffline(context: Context) {
        val connectivity = context.getSystemService(ConnectivityManager::class.java)
        check(connectivity.activeNetwork == null) {
            "physical-device candidate evidence requires no active Android network"
        }
    }

    private fun checkpointDirectory(context: Context): File =
        context.noBackupFilesDir.resolve("kagemusha-candidate-evidence-lab-v2").also {
            check(it.exists() || it.mkdirs()) { "failed to create candidate checkpoint directory" }
        }

    private fun evidenceDirectory(context: Context): File =
        checkNotNull(context.getExternalFilesDir(null)).resolve("evidence")

    private fun clearCheckpoint(context: Context) {
        val directory = checkpointDirectory(context)
        directory.listFiles()?.forEach { check(it.deleteRecursively()) }
    }

    private fun readAsset(context: Context, path: String, maximum: Int): ByteArray {
        context.assets.open(path).buffered().use { input ->
            val output = ArrayList<ByteArray>()
            var total = 0
            val buffer = ByteArray(STREAM_CHUNK_BYTES)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                total += count
                check(total <= maximum) { "asset $path exceeds its byte limit" }
                output += buffer.copyOf(count)
            }
            check(total > 0) { "asset $path is empty" }
            val bytes = ByteArray(total)
            var offset = 0
            output.forEach { chunk ->
                chunk.copyInto(bytes, offset)
                offset += chunk.size
            }
            return bytes
        }
    }

    private fun readPositiveLongAsset(context: Context, path: String): Long {
        val value = readAsset(context, path, 64).toString(Charsets.US_ASCII).trim()
        check(value.matches(Regex("^[1-9][0-9]{0,18}$"))) { "asset $path is not a positive decimal" }
        return value.toLong().also { check(it > 0) }
    }

    private fun readDigestAsset(context: Context, path: String): ByteArray =
        readAsset(context, path, 32).also {
            requireDigest(it, "asset $path")
        }

    private fun readAsciiAsset(context: Context, path: String, maximum: Int): ByteArray {
        val text = readAsset(context, path, maximum).toString(Charsets.US_ASCII).trim()
        check(text.isNotEmpty() && text.all { it.code in 0x21..0x7e }) {
            "asset $path must contain one non-empty printable ASCII value"
        }
        return text.toByteArray(Charsets.US_ASCII)
    }

    private fun sha256Asset(context: Context, path: String): AssetDigest {
        val digest = MessageDigest.getInstance("SHA-256")
        var size = 0L
        context.assets.open(path).buffered().use { input ->
            val buffer = ByteArray(STREAM_CHUNK_BYTES)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                size += count
                digest.update(buffer, 0, count)
            }
        }
        check(size > 0) { "asset $path is empty" }
        return AssetDigest(size, hex(digest.digest()))
    }

    private fun timedNative(
        events: CausalEvents,
        operation: String,
        inputs: List<ByteArray>,
        call: () -> ByteArray,
    ): ByteArray {
        val started = SystemClock.elapsedRealtimeNanos()
        val output = call()
        requireResult(output, operation)
        events.succeeded(
            operation = operation,
            durationNanos = SystemClock.elapsedRealtimeNanos() - started,
            inputs = inputs,
            output = output,
        )
        return output
    }

    private fun <T> timed(
        events: CausalEvents,
        operation: String,
        inputs: List<ByteArray> = emptyList(),
        call: () -> T,
    ): T {
        val started = SystemClock.elapsedRealtimeNanos()
        val result = call()
        events.succeeded(
            operation = operation,
            durationNanos = SystemClock.elapsedRealtimeNanos() - started,
            inputs = inputs,
            output = null,
        )
        return result
    }

    private fun writeAtomic(path: File, bytes: ByteArray) {
        check(path.parentFile?.exists() == true || path.parentFile?.mkdirs() == true)
        val temporary = File(path.parentFile, ".${path.name}.${Process.myPid()}.tmp")
        FileOutputStream(temporary).use { output ->
            output.write(bytes)
            output.flush()
            output.fd.sync()
        }
        check(temporary.renameTo(path)) { "failed to commit ${path.name}" }
    }

    private fun writePrivateAtomic(path: File, bytes: ByteArray) {
        writeAtomic(path, bytes)
        Os.chmod(path.absolutePath, 384)
        check((Os.stat(path.absolutePath).st_mode and 511) == 384) {
            "failed to restrict ${path.name} to the Android application owner"
        }
    }

    private fun readStrictJson(path: File): JSONObject {
        check(path.isFile && path.length() in 1..(1024 * 1024).toLong()) {
            "candidate checkpoint is absent or too large"
        }
        return JSONObject(path.readText(Charsets.UTF_8))
    }

    private fun sha256(bytes: ByteArray): String =
        hex(MessageDigest.getInstance("SHA-256").digest(bytes))

    private fun qualifiedCandidateSha256(
        candidateRecordSha256: String,
        qualificationReceiptSha256: String,
    ): String {
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update(
            "iroha:kagemusha:recursive-spend-qualified-candidate:v4"
                .toByteArray(Charsets.US_ASCII),
        )
        digest.update(byteArrayOf(0))
        digest.update(decodeHex(candidateRecordSha256))
        digest.update(decodeHex(qualificationReceiptSha256))
        return hex(digest.digest())
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        FileInputStream(file).use { input ->
            val buffer = ByteArray(STREAM_CHUNK_BYTES)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                digest.update(buffer, 0, count)
            }
        }
        return hex(digest.digest())
    }

    private fun decodeHex(value: String): ByteArray {
        check(value.matches(Regex("^[0-9a-f]{64}$")))
        return ByteArray(32) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun hex(bytes: ByteArray): String =
        bytes.joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun requireDigest(value: ByteArray, label: String): String {
        check(value.size == 32 && value.any { it.toInt() != 0 }) {
            "$label digest is invalid"
        }
        return hex(value)
    }

    private fun normalizeSha256(value: ByteArray, label: String): String =
        if (value.size == 32) {
            requireDigest(value, label)
        } else {
            requireAscii(value, label).also {
                check(it.matches(Regex("^[0-9a-f]{64}$"))) { "$label SHA-256 is invalid" }
            }
        }

    private fun requireAscii(value: ByteArray, label: String): String {
        check(value.isNotEmpty() && value.all { it.toInt() and 0xff in 0x20..0x7e }) {
            "$label is not printable ASCII"
        }
        return value.toString(Charsets.US_ASCII)
    }

    private data class InstalledApkIdentity(
        val installedPath: String,
        val sha256: String,
        val signingCertificateSha256: String,
    )

    private data class InstalledApkIdentities(
        val main: InstalledApkIdentity,
        val test: InstalledApkIdentity,
    )

    private data class RuntimeEvidenceBinding(
        val challengeHex: String,
        val challengeSha256: String,
        val certificateChainSha256: String,
        val appSigningCertificateSha256: String,
    ) {
        fun validate(apks: InstalledApkIdentities) {
            val expectedChallenge = CandidateLabHarness.deriveStrongboxChallenge(apks)
            check(challengeHex == CandidateLabHarness.hex(expectedChallenge)) {
                "instrumentation challenge is not bound to the exact candidate and APKs"
            }
            check(challengeSha256 == CandidateLabHarness.sha256(expectedChallenge)) {
                "instrumentation challenge digest does not match its exact challenge bytes"
            }
            check(appSigningCertificateSha256 != apks.main.signingCertificateSha256) {
                "non-shipping lab APK signer must be distinct from the attested wallet signer"
            }
        }

        companion object {
            private val sha256 = Regex("^[0-9a-f]{64}$")

            fun fromInstrumentationArguments(): RuntimeEvidenceBinding {
                val arguments = InstrumentationRegistry.getArguments()
                fun required(name: String): String {
                    val value = arguments.getString(name)
                    check(value != null && value == value.trim() && value.isNotEmpty()) {
                        "instrumentation argument $name is required"
                    }
                    return value
                }
                fun digest(name: String): String = required(name).also {
                    check(sha256.matches(it) && it != "0".repeat(64)) {
                        "instrumentation argument $name must be non-zero lowercase SHA-256"
                    }
                }
                check(required("kagemushaStrongboxAttestation") == "true")
                check(required("kagemushaPhysicalDeviceAttestation") == "true")
                return RuntimeEvidenceBinding(
                    challengeHex = digest("kagemushaAttestationChallengeHex"),
                    challengeSha256 = digest("kagemushaAttestationChallengeSha256"),
                    certificateChainSha256 =
                        digest("kagemushaAttestationCertificateChainSha256"),
                    appSigningCertificateSha256 =
                        digest("kagemushaAppSigningCertificateSha256"),
                )
            }
        }
    }

    private class CausalEvents private constructor(
        private val phase: String,
        historical: JSONArray,
    ) {
        private val events = JSONArray(historical.toString())
        private var nextSequence = events.length()

        fun succeeded(
            operation: String,
            durationNanos: Long,
            inputs: List<ByteArray>,
            output: ByteArray?,
        ) {
            requireNext(operation)
            val inputDigests = JSONArray()
            inputs.forEach { inputDigests.put(CandidateLabHarness.sha256(it)) }
            val event = baseEvent(operation, "succeeded", durationNanos, inputDigests)
                .put(
                    "output_sha256",
                    output?.let { CandidateLabHarness.sha256(it) } ?: JSONObject.NULL,
                )
                .put("output_size_bytes", output?.size ?: 0)
                .put("rejection_classification", JSONObject.NULL)
                .put("exception_class", JSONObject.NULL)
                .put("error_message_sha256", JSONObject.NULL)
            events.put(event)
            nextSequence += 1
        }

        fun rejectedDuplicateInput(
            durationNanos: Long,
            request: ByteArray,
            recipientRequest: ByteArray,
            sourceBundle: ByteArray,
            exceptionClass: String,
            errorMessage: String,
        ) {
            val operation = "duplicate_input_rejection"
            requireNext(operation)
            check(exceptionClass == "java.lang.IllegalArgumentException")
            check(errorMessage.isNotBlank())
            val inputs = JSONArray()
                .put(CandidateLabHarness.sha256(request))
                .put(CandidateLabHarness.sha256(recipientRequest))
                .put(CandidateLabHarness.sha256(sourceBundle))
            events.put(
                baseEvent(operation, "rejected", durationNanos, inputs)
                    .put("output_sha256", JSONObject.NULL)
                    .put("output_size_bytes", 0)
                    .put("rejection_classification", "duplicate_input_bundle")
                    .put("exception_class", exceptionClass)
                    .put(
                        "error_message_sha256",
                        CandidateLabHarness.sha256(errorMessage.toByteArray(Charsets.UTF_8)),
                    ),
            )
            nextSequence += 1
        }

        fun complete(): JSONArray {
            val expectedEnd = if (phase == "phase_1") PHASE_ONE_EVENT_COUNT else OPERATIONS.size
            check(nextSequence == expectedEnd) {
                "$phase causal event stream ended at $nextSequence instead of $expectedEnd"
            }
            return JSONArray(events.toString())
        }

        private fun baseEvent(
            operation: String,
            outcome: String,
            durationNanos: Long,
            inputDigests: JSONArray,
        ): JSONObject = JSONObject()
            .put("sequence", nextSequence)
            .put("phase", phase)
            .put("operation", operation)
            .put("outcome", outcome)
            .put("duration_nanos", maxOf(1L, durationNanos))
            .put("input_sha256", inputDigests)

        private fun requireNext(operation: String) {
            check(nextSequence in OPERATIONS.indices)
            check(OPERATIONS[nextSequence] == operation) {
                "causal event $nextSequence must be ${OPERATIONS[nextSequence]}, got $operation"
            }
            val expectedPhase = if (nextSequence < PHASE_ONE_EVENT_COUNT) "phase_1" else "phase_2"
            check(phase == expectedPhase)
        }

        companion object {
            private const val PHASE_ONE_EVENT_COUNT = 7
            private val EVENT_FIELDS = setOf(
                "sequence",
                "phase",
                "operation",
                "outcome",
                "duration_nanos",
                "input_sha256",
                "output_sha256",
                "output_size_bytes",
                "rejection_classification",
                "exception_class",
                "error_message_sha256",
            )
            private val OPERATIONS = listOf(
                "candidate_install",
                "build_init_request",
                "init",
                "build_append_hop_01_request",
                "append_hop_01",
                "build_append_hop_02_request",
                "append_hop_02",
                "candidate_reinstall_after_process_restart",
                "restore_init_result_after_restart",
                "restore_hop_01_result_after_restart",
                "restore_hop_02_result_after_restart",
                "validate_init_branch_after_restart",
                "validate_hop_01_change_continuity",
                "validate_hop_01_recipient_branch",
                "validate_hop_02_recipient_branch",
                "validate_sender_change_branch",
                "build_verify_first_recipient_proof_request",
                "verify_first_recipient_proof",
                "build_verify_multi_hop_recipient_proof_request",
                "verify_multi_hop_recipient_proof",
                "build_duplicate_input_request_from_observed_branch",
                "duplicate_input_rejection",
                "build_redeem_first_recipient_request",
                "redeem_first_recipient",
                "build_redeem_second_recipient_request",
                "redeem_second_recipient",
                "build_redeem_sender_change_request",
                "redeem_sender_change",
            )

            fun forPhaseOne(): CausalEvents = CausalEvents("phase_1", JSONArray())

            fun forPhaseTwo(phaseOne: JSONArray): CausalEvents {
                check(phaseOne.length() == PHASE_ONE_EVENT_COUNT)
                repeat(phaseOne.length()) { index ->
                    validateHistoricalEvent(phaseOne.getJSONObject(index), index)
                }
                return CausalEvents("phase_2", phaseOne)
            }

            private fun validateHistoricalEvent(event: JSONObject, sequence: Int) {
                val keys = mutableSetOf<String>()
                val iterator = event.keys()
                while (iterator.hasNext()) keys += iterator.next()
                check(keys == EVENT_FIELDS)
                check(event.getInt("sequence") == sequence)
                check(event.getString("phase") == "phase_1")
                check(event.getString("operation") == OPERATIONS[sequence])
                check(event.getString("outcome") == "succeeded")
                check(event.getLong("duration_nanos") > 0L)
                val inputs = event.getJSONArray("input_sha256")
                repeat(inputs.length()) { index ->
                    check(inputs.getString(index).matches(Regex("^[0-9a-f]{64}$")))
                }
                if (event.isNull("output_sha256")) {
                    check(event.getLong("output_size_bytes") == 0L)
                } else {
                    check(event.getString("output_sha256").matches(Regex("^[0-9a-f]{64}$")))
                    check(event.getLong("output_size_bytes") > 0L)
                }
                check(event.isNull("rejection_classification"))
                check(event.isNull("exception_class"))
                check(event.isNull("error_message_sha256"))
            }
        }
    }

    private data class Artifact(val role: String, val fileName: String)
    private data class AssetDigest(val size: Long, val sha256: String)
    private data class ValidatedStageManifest(val artifacts: Map<String, AssetDigest>)
    private data class AcceptedArtifact(
        val role: String,
        val framedSize: Long,
        val framedSha256: String,
        val payloadSize: Long,
        val payloadSha256: String,
    )
    private data class AcceptedIdentity(
        val candidateRecordSha256: String,
        val candidateManifestSha256: String,
        val inventorySha256: String,
        val generation: String,
        val sourceCommit: String,
        val sourceTreeSha256: String,
        val bridgeAbiVersion: Int,
        val artifacts: List<AcceptedArtifact>,
    )

    private data class AtomicQuantity(val atomicUnits: BigInteger, val scale: Int) {
        init {
            check(atomicUnits.signum() >= 0 && scale in 0..28)
        }

        fun toJson(): JSONObject = JSONObject()
            .put("atomic_units", atomicUnits.toString())
            .put("scale", scale)
    }

    private data class BranchProjection(
        val bundle: ByteArray,
        val topUpProvenance: ByteArray,
        val membershipWitness: ByteArray,
        val amount: AtomicQuantity,
        val hopCount: Int,
        val proofStepCount: Int,
    )
    private data class InitProjection(val branch: BranchProjection)
    private data class SplitProjection(
        val recipient: BranchProjection,
        val change: BranchProjection?,
    )
    private data class VerifyProjection(
        val valid: Boolean,
        val chainAdmissible: Boolean,
        val lineageRedeemable: Boolean,
        val witnesslessRedemptionSupported: Boolean,
        val amount: AtomicQuantity,
        val hopCount: Int,
        val proofStepCount: Int,
    )
    private data class RedeemProjection(
        val redeemed: AtomicQuantity,
        val change: BranchProjection?,
    )

    private class ProjectionCursor(
        private val fields: Array<ByteArray>,
        private val label: String,
    ) {
        private var index = 0

        fun next(field: String): ByteArray {
            check(index < fields.size) { "$label projection omitted $field" }
            return fields[index++]
        }

        fun archive(field: String): ByteArray = next(field).also {
            check(it.isNotEmpty() && it.size <= MAX_RESULT_BYTES) {
                "$label projection $field is absent or too large"
            }
        }

        fun version() {
            check(next("version").contentEquals(byteArrayOf(0, 0, 0, 1))) {
                "$label projection version is unsupported"
            }
        }

        fun bool(field: String): Boolean {
            val value = next(field)
            check(value.size == 1 && (value[0] == 0.toByte() || value[0] == 1.toByte())) {
                "$label projection $field is not boolean"
            }
            return value[0] == 1.toByte()
        }

        fun ascii(field: String): String = requireAscii(next(field), "$label $field")

        fun decimalInt(field: String): Int = ascii(field).toInt().also { check(it >= 0) }
        fun decimalLong(field: String): Long = ascii(field).toLong().also { check(it >= 0) }

        fun amount(field: String): AtomicQuantity {
            val atomic = BigInteger(ascii("$field atomic units"))
            val scale = decimalInt("$field scale")
            return AtomicQuantity(atomic, scale)
        }

        fun count(field: String): Int {
            val value = next(field)
            check(value.size == 4) { "$label $field is not u32" }
            val count = ByteBuffer.wrap(value).order(ByteOrder.BIG_ENDIAN).int
            check(count in 0..2) { "$label $field exceeds the exact-state limit" }
            return count
        }

        fun branch(field: String, topUpProvenance: ByteArray): BranchProjection {
            val bundle = archive("$field bundle")
            val membershipWitness = archive("$field membership witness")
            requireDigest(next("$field commitment"), "$field commitment")
            requireDigest(next("$field nullifier"), "$field nullifier")
            val amount = amount("$field amount")
            val hop = decimalInt("$field hop count")
            val proofSteps = decimalInt("$field proof step count")
            requireDigest(next("$field bundle digest"), "$field bundle digest")
            next("$field artifact binding")
            val claims = count("$field claim count")
            repeat(claims) { next("$field claim[$it]") }
            check(proofSteps > 0)
            return BranchProjection(
                bundle,
                topUpProvenance,
                membershipWitness,
                amount,
                hop,
                proofSteps,
            )
        }

        fun finish() {
            check(index == fields.size) { "$label projection has trailing fields" }
        }
    }
}
