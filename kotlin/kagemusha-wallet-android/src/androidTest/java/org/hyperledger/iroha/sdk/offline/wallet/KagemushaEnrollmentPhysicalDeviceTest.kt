// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.os.Build
import android.os.Process
import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileInputStream
import java.security.MessageDigest
import org.junit.AssumptionViolatedException
import org.junit.Test
import org.junit.runner.RunWith

/** Broad class/suite selection never opts into additional physical monetary fault trials. */
internal fun enrollmentPublicationTrialSelected(selection: String?, method: String): Boolean =
    method in setOf("retainedReceiveBeforePublicationDriverReconstruction", "retainedReceiveAfterPublicationDriverReconstruction") &&
        selection == "org.hyperledger.iroha.sdk.offline.wallet.KagemushaEnrollmentPhysicalDeviceTest#$method"

/** Explicit disposable-app campaign; ordinary instrumentation does not opt into this lifecycle. */
@RunWith(AndroidJUnit4::class)
class KagemushaEnrollmentPhysicalDeviceTest {
    @Test fun retainedNativeEnrollmentMailbox() = runMailbox(null)

    @Test fun retainedReceiveBeforePublicationDriverReconstruction() = publicationTrial(
        "retainedReceiveBeforePublicationDriverReconstruction", EnrollmentProbePublicationBoundary.BEFORE_PUBLICATION)

    @Test fun retainedReceiveAfterPublicationDriverReconstruction() = publicationTrial(
        "retainedReceiveAfterPublicationDriverReconstruction", EnrollmentProbePublicationBoundary.AFTER_ACKNOWLEDGED_PUBLICATION)

    private fun publicationTrial(method: String, boundary: EnrollmentProbePublicationBoundary) {
        // This standard runner selector is checked before any filesystem or Native action.
        if (!enrollmentPublicationTrialSelected(InstrumentationRegistry.getArguments().getString("class"), method)) {
            throw AssumptionViolatedException("Select this exact class#method for the physical publication trial")
        }
        runMailbox(ReceivePublicationTrial(boundary))
    }

    private fun runMailbox(trial: ReceivePublicationTrial?) {
        val args = InstrumentationRegistry.getArguments()
        if (args.getString("kagemushaProbe") == null) {
            throw AssumptionViolatedException("Explicit enrollment-v1 campaign required; no device evidence collected")
        }
        var files: EnrollmentProbeFiles? = null
        var invocation: String? = null
        try {
            probeRequire(args.getString("kagemushaProbe") == "enrollment-v1")
            fun required(name: String): String = args.getString(name)?.takeIf { it.isNotEmpty() }
                ?: throw EnrollmentProbeFailure("input")
            val run = EnrollmentProbeConfiguration.uuid(required("kagemushaRun"))
            val currentInvocation = EnrollmentProbeConfiguration.uuid(required("kagemushaInvocation"))
            invocation = currentInvocation
            val context = InstrumentationRegistry.getInstrumentation().targetContext
            val testContext = InstrumentationRegistry.getInstrumentation().context
            probeRequire(context.packageName.startsWith("org.hyperledger.iroha.kagemusha.enrollmentprobe."), "namespace")
            probeRequire(Build.DEVICE == required("kagemushaDevice") && Build.MODEL == required("kagemushaModel") &&
                Build.FINGERPRINT == required("kagemushaFingerprint"), "device")
            probeRequire(Build.VERSION.SDK_INT >= 26 && !context.isDeviceProtectedStorage, "device")
            probeRequire(context.applicationInfo.splitSourceDirs.isNullOrEmpty() &&
                testContext.applicationInfo.splitSourceDirs.isNullOrEmpty(), "apk-layout")
            val targetAPK = apkDigest(context.applicationInfo.sourceDir)
            val testAPK = apkDigest(testContext.applicationInfo.sourceDir)
            probeRequire(targetAPK == required("kagemushaTargetAPK_SHA256") &&
                testAPK == required("kagemushaTestAPK_SHA256"), "candidate")
            // Resolve Android's own app-data alias before selecting the strict mailbox leaf.
            val ownedFiles = EnrollmentProbeFiles(File(context.noBackupFilesDir.canonicalFile, "kagemusha-enrollment-probe"),
                trial?.let { it::publication })
            files = ownedFiles
            ownedFiles.lock().use {
                val bytes = ownedFiles.read("inbox/$currentInvocation/configuration.json", 8_388_608)
                val digest = required("kagemushaConfigurationSHA256")
                probeRequire(EnrollmentProbeFiles.isDigest(digest) && EnrollmentProbeFiles.digest(bytes) == digest, "collision")
                val config = EnrollmentProbeConfiguration(ownedFiles, probeObject(bytes), run,
                    currentInvocation, context.packageName, required("kagemushaGenesisSHA256"))
                trial?.bind(config)
                ownedFiles.publish("identity.json", config.identity)
                ownedFiles.mkdir("public/$currentInvocation", exclusive = true)
                ownedFiles.mkdir("private/$currentInvocation", exclusive = true)
                ownedFiles.publish("private/$currentInvocation/configuration.json", bytes)
                ownedFiles.publish("public/$currentInvocation/invocation.json", probeJson(
                    "scope" to "native-enrollment-and-monetary-stages", "run" to run, "role" to config.role,
                    "actorUUID" to config.actor, "invocation" to currentInvocation,
                    "pid" to Process.myPid().toString(), "applicationNamespace" to context.packageName,
                    "keystoreNamespace" to "application-uid-${Process.myUid()}",
                    "device" to Build.DEVICE, "model" to Build.MODEL, "fingerprint" to Build.FINGERPRINT,
                    "api" to Build.VERSION.SDK_INT.toString(), "osRelease" to Build.VERSION.RELEASE,
                    "configurationSHA256" to digest, "catalogSHA256" to config.catalogDigest,
                    "targetAPKSHA256" to targetAPK, "testAPKSHA256" to testAPK,
                    "accountSHA256" to EnrollmentProbeFiles.digest(config.account),
                    "clock" to "SystemClock.elapsedRealtimeNanos", "clockUnit" to "nanoseconds",
                    "clockScope" to "this-invocation-only", "candidateAttestationClaimed" to "false"))
                val timeout = required("kagemushaTimeoutSeconds").toLongOrNull()
                    ?: throw EnrollmentProbeFailure("input")
                probeRequire(timeout in 1..1800)
                fun newDriver() = EnrollmentDeviceDriver(config, KagemushaWalletAndroidPlatformV1.create(context)) { wallet, generation ->
                    kagemushaProbeActivityBinding(InstrumentationRegistry.getInstrumentation(), wallet, generation)
                }
                var driver = newDriver()
                var failures = 0
                val stages = mutableSetOf<String>()
                val started = SystemClock.elapsedRealtimeNanos()
                val deadline = EnrollmentProbeDeadline(started, timeout * 1_000_000_000L,
                    SystemClock::elapsedRealtimeNanos)
                fun finishCampaign(sequence: Int, after: Long) {
                    probeRequire(apkDigest(context.applicationInfo.sourceDir) == targetAPK &&
                        apkDigest(testContext.applicationInfo.sourceDir) == testAPK, "candidate")
                    ownedFiles.publish("public/$currentInvocation/closed.json", probeJson(
                        "failedCommands" to failures.toString(), "commands" to (sequence + 1).toString(),
                        "requiredStage" to config.requiredStage, "achievedStages" to stages.sorted().joinToString(","),
                        "ticks" to after.toString(), "qualificationClaimed" to "false"))
                    if (trial == null) probeRequire(failures == 0, "command-failures")
                    probeRequire(config.requiredStage in stages, "required-stage")
                    trial?.finish(failures)
                }
                try {
                    for (sequence in 0..9999) {
                        val name = sequence.toString().padStart(4, '0')
                        val base = "inbox/$currentInvocation/$name"
                        var commandBytes = deadline.select { ownedFiles.committedCommand(base, 131_072) }
                        while (commandBytes == null) {
                            driver.checkActivity()
                            Thread.sleep(200)
                            commandBytes = deadline.select { ownedFiles.committedCommand(base, 131_072) }
                        }
                        val command = EnrollmentProbeCommand(probeObject(commandBytes))
                        ownedFiles.publish("private/$currentInvocation/$name.json", commandBytes)
                        val before = SystemClock.elapsedRealtimeNanos()
                        driver.output("started.json", probeJson("sequence" to sequence.toString(),
                            "action" to command.action, "ticks" to before.toString(),
                            "commandSHA256" to EnrollmentProbeFiles.digest(commandBytes)), sequence)
                        // Publication may have suspended past the bound; no late Native dispatch.
                        deadline.check()
                        val outcome = try {
                            trial?.beforeCommand(command, sequence)
                            if (trial != null) deadline.check()
                            driver.execute(command, sequence)
                        } catch (failure: Throwable) {
                            failures++
                            var trialFailure: Throwable? = null
                            if (trial != null) {
                                try {
                                    if (!trial.acceptFailure(failure, sequence)) throw failure
                                    // Only an acknowledged close permits a fresh driver over the same custody.
                                    driver.close()
                                    driver = newDriver()
                                    trial.reconstructed()
                                } catch (problem: Throwable) {
                                    if (problem !== failure) problem.addSuppressed(failure)
                                    trialFailure = problem
                                }
                            }
                            val after = SystemClock.elapsedRealtimeNanos()
                            driver.output("finished.json", failureRecord(failure, sequence, command.action, before, after), sequence)
                            trialFailure?.let { throw it }
                            if (failure is InterruptedException || Thread.currentThread().isInterrupted) throw EnrollmentProbeFailure("cancelled")
                            continue
                        }
                        val after = SystemClock.elapsedRealtimeNanos()
                        try { trial?.afterCommand(command, sequence, outcome) } catch (failure: Throwable) {
                            failures++
                            // Preserve the real Native outcome without a successful controller fence.
                            driver.output("publication-trial-native-outcome.json", probeJson(
                                "sequence" to sequence.toString(), "action" to command.action, "status" to outcome,
                                "trialAccepted" to "false"), sequence)
                            driver.output("finished.json", failureRecord(failure, sequence, command.action, before, after), sequence)
                            throw failure
                        }
                        enrollmentProbeStage(outcome)?.let(stages::add)
                        val stoppingTrial = trial != null && command.action == "stop"
                        if (stoppingTrial) finishCampaign(sequence, after)
                        driver.output("finished.json", probeJson("sequence" to sequence.toString(),
                            "action" to command.action, "status" to outcome, "ticks" to after.toString(),
                            "durationNanoseconds" to (after - before).toString()), sequence)
                        if (command.action == "stop") {
                            if (!stoppingTrial) finishCampaign(sequence, after)
                            return
                        }
                    }
                    throw EnrollmentProbeFailure("command-limit")
                } finally {
                    // Close never abandons an enrollment or deletes its payment key. Failure stays visible.
                    driver.close()
                }
            }
        } catch (failure: Throwable) {
            // No arbitrary exception message, stack cause, session input or private path is exported.
            if (files != null && invocation != null) {
                try { files.publish("public/$invocation/harness-failed.json",
                    failureRecord(failure, -1, "harness", 0, SystemClock.elapsedRealtimeNanos())) }
                catch (_: Throwable) { /* An absent/partial record remains an unresolved attempt. */ }
            }
            throw EnrollmentProbeFailure("campaign-failed-see-retained-closed-diagnostics")
        }
    }

    /** One actual Receive, one failed public export, then read-only recovery on a new driver. */
    private class ReceivePublicationTrial(private val boundary: EnrollmentProbePublicationBoundary) {
        private enum class Phase {
            PREPARATION, ARMED_RECEIVE, FAULT_OBSERVED, DRIVER_RECONSTRUCTED, REINSTALLED, REOPENED,
            REQUEST_STATUS_VERIFIED, REQUEST_OUTPUT_VERIFIED, OPERATION_OUTPUT_VERIFIED, FINISHED,
        }
        private lateinit var config: EnrollmentProbeConfiguration
        private var phase = Phase.PREPARATION
        private val injected = EnrollmentProbeFailure("injected-publication-interruption")
        private var armed = false
        private var fired = 0
        private var expectedFailures = 0
        private var reconstructions = 0
        private var unknownSequence = -1
        private var unknownRequest: ByteArray? = null
        private var receiveSequence = -1
        private var target = ""
        private var inputs = emptyMap<String, ByteArray>()
        private var original = ByteArray(0)
        private var operationHex = ""
        private var operationBinding = ByteArray(0)
        private var openBinding = ByteArray(0)
        private var recoveredPeer: ByteArray? = null
        private var recoveredEnvelope: ByteArray? = null
        private val files get() = config.files
        private fun name(sequence: Int) = sequence.toString().padStart(4, '0')
        private fun outputPath(sequence: Int, suffix: String) = "public/${config.invocation}/${name(sequence)}-$suffix"
        private fun input(command: EnrollmentProbeCommand, key: String): ByteArray {
            val pin = command.pins[key] ?: throw EnrollmentProbeFailure("input")
            probeRequire(pin.path.startsWith("inbox/${config.invocation}/"))
            return files.pinned(pin, KagemushaMonetaryProbeManifest.maximum(key)).also {
                probeRequire(it.isNotEmpty())
                if (key == "request-id" || key == "operation-id") KagemushaMonetaryProbeManifest.word(it)
            }
        }
        private fun requestId() = inputs["request-id"] ?: throw EnrollmentProbeFailure("publication-trial-phase")
        private fun bindingPath() = "private/intents/operation-${EnrollmentProbeFiles.digest(requestId())}"
        private fun trialRecord(name: String, vararg fields: Pair<String, String>) {
            files.publish("public/${config.invocation}/publication-trial-$name.json", probeJson(
                "boundary" to boundary.name, "invocation" to config.invocation,
                "actorUUID" to config.actor, "applicationNamespace" to config.packageName,
                "qualificationClaimed" to "false", "carrierDeliveryClaimed" to "false",
                "originalLatencyUsable" to "false", "processDeathTested" to "false", *fields))
        }
        fun bind(value: EnrollmentProbeConfiguration) {
            probeRequire(value.requiredStage == "received", "publication-trial-stage")
            config = value
        }
        fun publication(path: String, at: EnrollmentProbePublicationBoundary, bytes: ByteArray) {
            if (!armed || path != target || at != boundary) return
            probeRequire(phase == Phase.ARMED_RECEIVE && fired == 0, "publication-trial-phase")
            original = bytes.copyOf()
            fired++
            armed = false
            throw injected
        }
        fun beforeCommand(command: EnrollmentProbeCommand, sequence: Int) {
            when (phase) {
                Phase.PREPARATION -> {
                    probeRequire(command.action != "stop", "publication-boundary-not-reached")
                    if (command.action == "receive") {
                        val selected = command.pins.keys.associateWith { input(command, it) }
                        probeRequire(unknownSequence == sequence - 1 &&
                            unknownRequest?.contentEquals(selected.getValue("request-id")) == true,
                            "publication-trial-unknown-required")
                        inputs = selected
                        receiveSequence = sequence
                        target = outputPath(sequence, "released-original.norito")
                        probeRequire(!files.exists(target), "collision")
                        phase = Phase.ARMED_RECEIVE
                        armed = true
                    }
                }
                Phase.DRIVER_RECONSTRUCTED -> probeRequire(command.action == "install", "publication-trial-recovery-order")
                Phase.REINSTALLED -> probeRequire(command.action == "open", "publication-trial-recovery-order")
                Phase.REOPENED, Phase.REQUEST_STATUS_VERIFIED -> {
                    val expected = if (phase == Phase.REOPENED) "request-status" else "released-request"
                    probeRequire(command.action == expected && input(command, "request-id").contentEquals(requestId()),
                        "publication-trial-recovery-identity")
                }
                Phase.REQUEST_OUTPUT_VERIFIED -> probeRequire(command.action == "released-operation" &&
                    EnrollmentProbeFiles.hex(input(command, "operation-id")) == operationHex,
                    "publication-trial-recovery-identity")
                Phase.OPERATION_OUTPUT_VERIFIED -> probeRequire(command.action == "stop", "publication-trial-recovery-order")
                else -> throw EnrollmentProbeFailure("publication-trial-phase")
            }
        }
        fun acceptFailure(failure: Throwable, sequence: Int): Boolean {
            if (failure !== injected) return false
            probeRequire(failure.suppressed.isEmpty() && phase == Phase.ARMED_RECEIVE && sequence == receiveSequence && fired == 1 && !armed &&
                original.size in 1..10_034 && expectedFailures == 0, "publication-trial-phase")
            val call = probeObject(files.read(outputPath(sequence, "native-call.json"), 4096))
            probeRequire(call.text("status") == KagemushaWalletCallV1.COMPLETE.toString(), "publication-trial-completion")
            val completion = probeObject(files.read(outputPath(sequence, "durable-call-return.json"), 4096))
            probeRequire(completion.text("requestID") == EnrollmentProbeFiles.hex(requestId()), "publication-trial-recovery-identity")
            operationHex = completion.text("operationID")
            probeRequire(EnrollmentProbeFiles.isDigest(operationHex) && operationHex.any { it != '0' }, "native-projection")
            if (boundary == EnrollmentProbePublicationBoundary.BEFORE_PUBLICATION) {
                probeRequire(!files.exists(target), "publication-trial-target")
            } else {
                probeRequire(files.read(target, 10_034).contentEquals(original), "publication-trial-target")
            }
            operationBinding = KagemushaMonetaryProbeManifest.binding("operation", inputs +
                ("family" to "receive".toByteArray(Charsets.UTF_8)))
            verifyRetainedInputs()
            openBinding = files.read("private/open-binding.json", 1024)
            val open = probeObject(openBinding)
            probeRequire(open.keys == setOf("accountSHA256", "resultSHA256") &&
                open.text("accountSHA256") == EnrollmentProbeFiles.digest(config.account) &&
                EnrollmentProbeFiles.isDigest(open.text("resultSHA256")), "publication-trial-admission")
            val base = "private/${config.invocation}/publication-trial"
            files.mkdir(base, exclusive = true)
            files.publish("$base/released-original.norito", original)
            files.publish("$base/durable-call-return.json", files.read(outputPath(sequence, "durable-call-return.json"), 4096))
            files.publish("$base/open-binding.json", openBinding)
            trialRecord("fault", "sequence" to sequence.toString(), "requestID" to EnrollmentProbeFiles.hex(requestId()),
                "operationID" to operationHex, "originalSHA256" to EnrollmentProbeFiles.digest(original),
                "nativeCompleteObserved" to "true", "selectedFaults" to fired.toString(), "commandStatus" to "failed")
            expectedFailures++
            phase = Phase.FAULT_OBSERVED
            return true
        }
        fun reconstructed() {
            probeRequire(phase == Phase.FAULT_OBSERVED && reconstructions == 0, "publication-trial-phase")
            trialRecord("reconstructed", "closeAcknowledged" to "true", "newDriverConstructed" to "true",
                "requestID" to EnrollmentProbeFiles.hex(requestId()), "operationID" to operationHex)
            reconstructions++
            phase = Phase.DRIVER_RECONSTRUCTED
        }
        fun afterCommand(command: EnrollmentProbeCommand, sequence: Int, outcome: String) {
            when (phase) {
                Phase.PREPARATION -> {
                    unknownRequest = null
                    unknownSequence = -1
                    if (command.action == "request-status" && outcome == "native-call-status-${KagemushaWalletCallV1.UNKNOWN}") {
                        val call = probeObject(files.read(outputPath(sequence, "native-call.json"), 4096))
                        probeRequire(call.text("status") == KagemushaWalletCallV1.UNKNOWN.toString(), "publication-trial-unknown-required")
                        unknownRequest = input(command, "request-id")
                        unknownSequence = sequence
                    }
                }
                // Pending/Preparing are real outcomes, but never evidence that this fault ran.
                Phase.ARMED_RECEIVE -> throw EnrollmentProbeFailure("publication-boundary-not-reached")
                Phase.DRIVER_RECONSTRUCTED -> {
                    probeRequire(outcome == "native-installation-registered", "publication-trial-admission")
                    phase = Phase.REINSTALLED
                }
                Phase.REINSTALLED -> {
                    probeRequire(outcome == "native-wallet-opened" &&
                        files.read("private/open-binding.json", 1024).contentEquals(openBinding), "publication-trial-admission")
                    phase = Phase.REOPENED
                }
                Phase.REOPENED -> {
                    val call = probeObject(files.read(outputPath(sequence, "native-call.json"), 4096))
                    val completion = probeObject(files.read(outputPath(sequence, "durable-call-return.json"), 4096))
                    probeRequire(call.text("status") == KagemushaWalletCallV1.COMPLETE.toString() &&
                        completion.text("requestID") == EnrollmentProbeFiles.hex(requestId()) &&
                        completion.text("operationID") == operationHex, "publication-trial-completion")
                    verifyRecovery(sequence, outcome)
                    phase = Phase.REQUEST_STATUS_VERIFIED
                }
                Phase.REQUEST_STATUS_VERIFIED -> {
                    verifyRecovery(sequence, outcome)
                    phase = Phase.REQUEST_OUTPUT_VERIFIED
                }
                Phase.REQUEST_OUTPUT_VERIFIED -> {
                    verifyRecovery(sequence, outcome)
                    phase = Phase.OPERATION_OUTPUT_VERIFIED
                }
                Phase.OPERATION_OUTPUT_VERIFIED -> {
                    probeRequire(outcome == "closed-with-custody-retained", "publication-trial-close")
                    phase = Phase.FINISHED
                }
                else -> throw EnrollmentProbeFailure("publication-trial-phase")
            }
        }
        private fun verifyRetainedInputs() {
            for ((key, bytes) in inputs) {
                probeRequire(files.read("private/${config.invocation}/${name(receiveSequence)}-$key",
                    KagemushaMonetaryProbeManifest.maximum(key)).contentEquals(bytes), "publication-trial-originals")
            }
            probeRequire(files.read(bindingPath(), 1024).contentEquals(operationBinding), "publication-trial-originals")
        }
        private fun verifyRecovery(sequence: Int, outcome: String) {
            probeRequire(outcome == "native-complete-receive" &&
                files.read(outputPath(sequence, "released-original.norito"), 10_034).contentEquals(original) &&
                EnrollmentProbeFiles.hex(files.read(outputPath(sequence, "operation-id.bin"), 32)) == operationHex,
                "publication-trial-originals")
            val released = probeObject(files.read(outputPath(sequence, "released.json"), 4096))
            probeRequire(released.text("kind") == "RECEIVE" && released.text("operationID") == operationHex &&
                released.text("originalSHA256") == EnrollmentProbeFiles.digest(original), "publication-trial-originals")
            val peer = files.read(outputPath(sequence, "peer-original.norito"), 10_034)
            val envelope = files.read(outputPath(sequence, "peer-envelope.norito"), 10_034)
            probeRequire(peer.isNotEmpty() && envelope.isNotEmpty(), "publication-trial-originals")
            if (phase == Phase.REOPENED) {
                recoveredPeer = peer
                recoveredEnvelope = envelope
            } else {
                probeRequire(recoveredPeer?.contentEquals(peer) == true && recoveredEnvelope?.contentEquals(envelope) == true,
                    "publication-trial-originals")
            }
            verifyRetainedInputs()
        }
        fun finish(failures: Int) {
            probeRequire(phase == Phase.FINISHED && fired == 1 && expectedFailures == 1 && reconstructions == 1 &&
                failures == 1 && recoveredPeer != null && recoveredEnvelope != null, "publication-trial-incomplete")
            trialRecord("complete", "deterministicDriverReconstruction" to "complete",
                "expectedPublicationFailures" to expectedFailures.toString(), "unexpectedCommandFailures" to "0",
                "sameRequestAndOperationVerified" to "true", "exactRecoveryObservations" to "3",
                "requestID" to EnrollmentProbeFiles.hex(requestId()), "operationID" to operationHex,
                "originalSHA256" to EnrollmentProbeFiles.digest(original), "peerComparisonScope" to "post-reconstruction-only")
        }
    }

    /** Installed public APK only; this never walks data/custody or reads signing material. */
    private fun apkDigest(path: String): String {
        val file = File(path)
        probeRequire(file.isFile && file.canonicalFile == file, "candidate")
        val size = file.length(); val modified = file.lastModified()
        probeRequire(size in 1..2_147_483_648L, "candidate")
        val digest = MessageDigest.getInstance("SHA-256")
        var seen = 0L
        FileInputStream(file).use { input ->
            val chunk = ByteArray(65_536)
            while (true) {
                val n = input.read(chunk)
                if (n < 0) break
                probeRequire(n > 0 && seen + n <= size, "candidate")
                digest.update(chunk, 0, n); seen += n
            }
        }
        probeRequire(seen == size && file.length() == size && file.lastModified() == modified, "candidate")
        return EnrollmentProbeFiles.hex(digest.digest())
    }

    private fun failureRecord(failure: Throwable, sequence: Int, action: String, start: Long, end: Long): ByteArray {
        val fields = mutableListOf("sequence" to sequence.toString(), "action" to action,
            "status" to "failed", "ticks" to end.toString(),
            "durationNanoseconds" to if (start == 0L) "unavailable" else (end - start).toString())
        when (failure) {
            is KagemushaWalletExceptionV1 -> fields.addAll(listOf("nativeStatus" to failure.status.toString(),
                "nativeReason" to failure.reason.toString(), "platformCode" to failure.platformCode.toString()))
            is KagemushaProbeAuthenticationFailure -> fields.add("confirmationCode" to failure.code.toString())
            is EnrollmentProbeFailure -> fields.add("harnessFailure" to failure.code)
            is android.system.ErrnoException -> fields.add("errno" to failure.errno.toString())
            is InterruptedException -> fields.add("harnessFailure" to "cancelled")
            is SecurityException -> fields.add("platformFailure" to "security-exception")
            else -> fields.add("harnessFailure" to "unclassified-retained-failure")
        }
        return probeJson(*fields.toTypedArray())
    }
}
