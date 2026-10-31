// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.os.SystemClock
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ExecutionException
import java.util.concurrent.atomic.AtomicBoolean
import org.hyperledger.iroha.sdk.client.ToriiKagemushaWalletEnrollmentActionV1
import org.hyperledger.iroha.sdk.client.ToriiKagemushaWalletEnrollmentRequestV1
import org.hyperledger.iroha.sdk.client.ToriiKagemushaWalletEnrollmentResponseV1
import org.hyperledger.iroha.sdk.json.JsonObject

/** Worker-only genuine SDK lifecycle. No direct adapter hooks, synthetic evidence or account keys. */
internal class EnrollmentDeviceDriver(
    private val config: EnrollmentProbeConfiguration, private val platform: KagemushaWalletAndroidPlatformV1,
    private val bindActivity: (KagemushaWalletV1, Long) -> KagemushaProbeActivityControl,
) : AutoCloseable {
    private var attempt: KagemushaWalletInstallationAttemptV1? = null
    private var installed: KagemushaWalletInstalledRuntimeV1? = null
    private var enrollment: KagemushaWalletEnrollmentV1? = null
    private var wallet: KagemushaWalletV1? = null
    private var activity: KagemushaProbeActivityControl? = null
    private var activityGeneration = 0L
    private var walletCloseAttempted = false
    private var exchange: KagemushaWalletExchangeV1? = null
    private var dispatch: ByteArray? = null
    private val reviews = KagemushaProbeReviewHolder()
    private val monetary = KagemushaWalletDeviceStages(config,
        { wallet ?: throw EnrollmentProbeFailure("lifecycle") }, ::exchangeOwner, ::output, ::requireWalletLive, reviews)
    private val nearby = KagemushaNearbyDeviceStages(config,
        { wallet ?: throw EnrollmentProbeFailure("lifecycle") }, ::exchangeOwner, ::output, ::requireWalletLive, reviews)
    private val live = AtomicBoolean(true)
    private var closed = false
    private fun requireLive() { probeRequire(live.get(), "lifecycle") }
    private fun requireWalletLive() { requireLive(); checkActivity() }
    /** Surface callback/control failures even when the mailbox has no next command yet. */
    fun checkActivity() {
        if (wallet != null) (activity ?: throw EnrollmentProbeFailure("activity-unbound")).checkHealthy()
    }
    private fun exchangeOwner() = exchange ?: throw EnrollmentProbeFailure("exchange-required")
    private fun endExchange(): KagemushaWalletExchangeEndV1 {
        val owner = exchangeOwner()
        owner.requestEnd()
        reviews.endRequested(owner)?.let { pending ->
            try { owner.discardReview(pending.review) }
            catch (failure: KagemushaWalletExceptionV1) { if (failure.status != -2) throw failure }
        }
        val result = owner.end()
        if (result != KagemushaWalletExchangeEndV1.ENDING) exchange = null
        return result
    }
    private fun owner() = enrollment ?: throw EnrollmentProbeFailure("lifecycle")
    private fun seqName(value: Int) = value.toString().padStart(4, '0')
    fun output(name: String, bytes: ByteArray, sequence: Int) {
        config.files.publish("public/${config.invocation}/${seqName(sequence)}-$name", bytes)
    }
    private fun event(name: String, sequence: Int, vararg fields: Pair<String, String>) =
        output("$name.json", probeJson(*fields), sequence)
    private fun input(command: EnrollmentProbeCommand, name: String, bound: Int, sequence: Int): ByteArray {
        val pin = command.pins[name] ?: throw EnrollmentProbeFailure("input")
        probeRequire(pin.path.startsWith("inbox/${config.invocation}/"))
        return config.files.pinned(pin, bound).also {
            probeRequire(it.isNotEmpty())
            config.files.publish("private/${config.invocation}/${seqName(sequence)}-$name", it)
        }
    }
    private fun request(bytes: ByteArray, sequence: Int) {
        output("e5.norito", bytes, sequence)
        val dispatch = dispatch ?: throw EnrollmentProbeFailure("lifecycle")
        output("service-evidence.norito", ToriiKagemushaWalletEnrollmentRequestV1.evidence(dispatch, bytes).canonicalWire(), sequence)
        output("service-issue.norito", ToriiKagemushaWalletEnrollmentRequestV1.issue(dispatch).canonicalWire(), sequence)
        output("service-deliver.norito", ToriiKagemushaWalletEnrollmentRequestV1.deliver(dispatch).canonicalWire(), sequence)
    }
    private fun session(command: EnrollmentProbeCommand, sequence: Int): KagemushaWalletEnrollmentSessionOriginalsV1 {
        val token = input(command, "access-token", 16_384, sequence)
        val proof = input(command, "dpop-proof", 4096, sequence)
        return try { KagemushaWalletEnrollmentSessionOriginalsV1(token, proof) }
        finally { token.fill(0); proof.fill(0) }
    }
    private fun progress(value: KagemushaWalletEnrollmentProgressV1) = when (value) {
        is KagemushaWalletEnrollmentProgressV1.Evidence -> "native-payment-key-evidence-ready"
        KagemushaWalletEnrollmentProgressV1.Pending -> "native-pending"
        KagemushaWalletEnrollmentProgressV1.Abandoned -> "native-abandoned"
        KagemushaWalletEnrollmentProgressV1.BootstrapSelected -> "native-bootstrap-selected"
    }
    fun execute(command: EnrollmentProbeCommand, sequence: Int): String {
        requireLive(); probeRequire(!closed, "lifecycle")
        val cleanup = command.action in setOf("release-wallet", "stop", "end-exchange", "discard-review", "cancel-time")
        if (!cleanup) checkActivity()
        if (command.action in KagemushaNearbyProbeManifest.schemas) return nearby.execute(command, sequence).also { checkActivity() }
        if (command.action in KagemushaMonetaryProbeManifest.schemas) return monetary.execute(command, sequence).also { checkActivity() }
        val outcome = when (command.action) {
            "begin-exchange" -> {
                probeRequire(exchange == null, "exchange-retained")
                exchange = (wallet ?: throw EnrollmentProbeFailure("lifecycle")).beginExchange()
                "native-exchange-open"
            }
            "end-exchange" -> "native-exchange-${endExchange().name.lowercase()}"
            "install" -> {
                probeRequire(enrollment == null && wallet == null, "lifecycle")
                if (installed != null) "installed-owner-retained" else {
                    if (attempt == null) {
                        config.verifyCatalog()
                        attempt = KagemushaWalletInstalledRuntimeV1.install(platform, config.originals())
                    }
                    installed = attempt!!.register(); attempt = null
                    "native-installation-registered"
                }
            }
            "start-enrollment" -> {
                probeRequire(enrollment == null && wallet == null, "lifecycle")
                val runtime = installed ?: throw EnrollmentProbeFailure("lifecycle")
                enrollment = runtime.startEnrollment(session(command, sequence)); installed = null
                "native-enrollment-session-started"
            }
            "renew-session" -> {
                owner().renewSession(session(command, sequence)); dispatch = null
                "native-enrollment-session-renewed-begin-required"
            }
            "begin" -> {
                val original = owner().begin(config.requestId, config.account)
                dispatch = original
                output("dispatch.norito", original, sequence)
                output("service-prekey.norito", ToriiKagemushaWalletEnrollmentRequestV1.preKey(original).canonicalWire(), sequence)
                "live-dispatch-exported"
            }
            "permit" -> {
                val response = ToriiKagemushaWalletEnrollmentResponseV1.decodeCanonical(
                    ToriiKagemushaWalletEnrollmentActionV1.PRE_KEY,
                    input(command, "response", ToriiKagemushaWalletEnrollmentResponseV1.MAXIMUM_BYTES, sequence))
                val original = (response as? ToriiKagemushaWalletEnrollmentResponseV1.Permit)?.unverifiedOriginal
                    ?: throw EnrollmentProbeFailure("input")
                val challenge = owner().acceptPermit(original)
                output("verified-permit.norito", original, sequence)
                output("account-challenge.bin", challenge, sequence)
                "native-permit-accepted"
            }
            "authorize" -> {
                val signature = input(command, "signature", 64, sequence); probeRequire(signature.size == 64)
                progress(owner().authorize(signature))
            }
            "collect-evidence" -> {
                val owner = owner()
                val retained = owner.retainedRequest()
                if (retained != null) { request(retained, sequence); "retained-e5-exported" } else {
                    // A definitive E1 refusal is retained; never resample a key or submit its chain.
                    probeRequire(!config.files.exists("private/e1-refused.json"), "e1-refused")
                    val target = (owner.progress() as? KagemushaWalletEnrollmentProgressV1.Evidence)?.target
                        ?: throw EnrollmentProbeFailure("lifecycle")
                    when (val evidence = platform.enrollmentEvidence(target, config.androidPolicy())) {
                        is KagemushaWalletAndroidEnrollmentEvidenceV1.Ineligible -> {
                            val row = probeJson("refusalCode" to evidence.refusal.code.toString(),
                                "holderClass" to evidence.refusal.holderClass.name,
                                "issuerAdmissionClaimed" to "false")
                            config.files.publish("private/e1-refused.json", row)
                            output("e1-refusal.json", row, sequence)
                            // Retain this refusal; only an explicit abandon command may terminalize Native custody.
                            throw EnrollmentProbeFailure("e1-refused")
                        }
                        is KagemushaWalletAndroidEnrollmentEvidenceV1.Eligible -> {
                            val certificates = evidence.certificatesDer()
                            certificates.forEachIndexed { index, bytes ->
                                config.files.publish("private/${config.invocation}/${seqName(sequence)}-chain-$index.der", bytes)
                            }
                            event("e1-client-check", sequence, "strongBox" to evidence.strongBox.toString(),
                                "issuerAdmissionClaimed" to "false", "certificates" to certificates.size.toString())
                            when (val prepared = owner.prepareRequest(certificates)) {
                                is KagemushaWalletEnrollmentRequestV1.AccountChallenge -> {
                                    output("e5-account-challenge.bin", prepared.bytes(), sequence)
                                    "native-e5-account-challenge-exported"
                                }
                                is KagemushaWalletEnrollmentRequestV1.Retained -> {
                                    request(prepared.bytes(), sequence); "retained-e5-exported"
                                }
                            }
                        }
                    }
                }
            }
            "retain-request" -> {
                val signature = input(command, "signature", 64, sequence); probeRequire(signature.size == 64)
                request(owner().retainRequest(signature), sequence); "native-durable-e5-exported"
            }
            "accept-result" -> {
                val response = ToriiKagemushaWalletEnrollmentResponseV1.decodeCanonical(
                    ToriiKagemushaWalletEnrollmentActionV1.DELIVER,
                    input(command, "response", ToriiKagemushaWalletEnrollmentResponseV1.MAXIMUM_BYTES, sequence))
                val original = (response as? ToriiKagemushaWalletEnrollmentResponseV1.Credential)?.unverifiedOriginal
                    ?: throw EnrollmentProbeFailure("input")
                val retained = owner().acceptCredential(original)
                probeRequire(retained.contentEquals(original), "collision")
                output("e6.norito", retained, sequence); "native-durable-e6-exported-not-activated"
            }
            "inspect" -> {
                var status = "native-retained-originals-inspected"
                owner().retainedRequest()?.let { request(it, sequence); status = "retained-e5-exported" }
                owner().retainedResult()?.let { output("e6.norito", it, sequence); status = "retained-e6-exported" }
                status
            }
            "open" -> {
                probeRequire(wallet == null, "lifecycle")
                val result = input(command, "result", KagemushaWalletEnrollmentV1.RESULT_MAX_BYTES, sequence)
                config.files.publish("private/open-binding.json", probeJson(
                    "accountSHA256" to EnrollmentProbeFiles.digest(config.account),
                    "resultSHA256" to EnrollmentProbeFiles.digest(result)))
                enrollment?.let { owner ->
                    probeRequire(owner.retainedResult()?.contentEquals(result) == true, "collision")
                    installed = owner.loadInstalledRuntime(); enrollment = null
                }
                val runtime = installed ?: throw EnrollmentProbeFailure("lifecycle")
                val future = runtime.openEnrolledResult(result, config.account,
                    { challenge -> signOpen(challenge, sequence) }, ::requireLive)
                val opened = try { future.get() } catch (interrupted: InterruptedException) {
                    live.set(false); future.cancel(false); Thread.currentThread().interrupt(); throw interrupted
                } catch (failure: ExecutionException) { throw failure.cause ?: failure }
                wallet = opened; installed = null
                val returned = SystemClock.elapsedRealtimeNanos()
                try {
                    activityGeneration = Math.addExact(activityGeneration, 1L)
                    val binding = bindActivity(opened, activityGeneration)
                    activity = binding
                    binding.start()
                } catch (failure: Throwable) {
                    // Initial activity failure is visible. Retain any failed cleanup owner for retry.
                    try { releaseWallet() } catch (cleanupFailure: Throwable) {
                        if (cleanupFailure !== failure) failure.addSuppressed(cleanupFailure)
                    }
                    throw failure
                }
                val metadata = opened.metadata()
                probeRequire(metadata.accountOriginal().contentEquals(config.account), "collision")
                output("native-account.norito", metadata.accountOriginal(), sequence)
                output("native-asset.norito", metadata.assetOriginal(), sequence)
                event("opened", sequence, "nativeReturnedTicks" to returned.toString(),
                    "schemeID" to EnrollmentProbeFiles.hex(metadata.schemeId()),
                    "walletID" to EnrollmentProbeFiles.hex(metadata.walletId()),
                    "assetScale" to metadata.assetScale.toString())
                "native-wallet-opened"
            }
            "abandon-expired" -> {
                val proof = input(command, "proof", KagemushaWalletEnrollmentV1.EXPIRY_PROOF_MAX_BYTES, sequence)
                config.files.publish("private/expiry-proof.norito", proof)
                exportExpiredEnrollmentAbandonment(owner(), proof) { name, bytes ->
                    config.files.publish("private/$name", bytes)
                    output(name, bytes, sequence)
                }
            }
            "abandon" -> exportEnrollmentAbandonment(owner()) { name, bytes ->
                // Stable copies make retries compare exact Native originals; no custody is removed.
                config.files.publish("private/$name", bytes)
                output(name, bytes, sequence)
            }
            "release-wallet" -> {
                probeRequire(wallet != null, "lifecycle"); releaseWallet()
                "wallet-released-custody-retained"
            }
            "stop" -> { close(); "closed-with-custody-retained" }
            else -> throw EnrollmentProbeFailure("input")
        }
        if (!cleanup) checkActivity()
        return outcome
    }
    /** Callback runs on the SDK admission worker; it performs no signing and no additional JNI. */
    private fun signOpen(challenge: ByteArray, sequence: Int): CompletableFuture<ByteArray> {
        val result = CompletableFuture<ByteArray>()
        try {
            probeRequire(challenge.size == 32 && challenge.any { it != 0.toByte() })
            output("open-account-challenge.bin", challenge, sequence)
            val base = "inbox/${config.invocation}/${seqName(sequence)}-open-signature"
            val start = SystemClock.elapsedRealtimeNanos()
            val deadline = EnrollmentProbeDeadline(start, 120_000_000_000L,
                SystemClock::elapsedRealtimeNanos, ::requireLive)
            var bytes = deadline.select { config.files.committedCommand(base, 8192) }
            while (bytes == null) {
                Thread.sleep(200)
                bytes = deadline.select { config.files.committedCommand(base, 8192) }
            }
            val pin = probeOpenSignature(bytes, challenge)
            probeRequire(pin.path.startsWith("inbox/${config.invocation}/"))
            val signature = config.files.pinned(pin, 64); probeRequire(signature.size == 64)
            config.files.publish("private/${config.invocation}/${seqName(sequence)}-open-signature.json", bytes)
            config.files.publish("private/${config.invocation}/${seqName(sequence)}-open-signature.bin", signature)
            // Retention may have suspended; never return a signature after expiry or cancellation.
            deadline.check()
            result.complete(signature)
        } catch (failure: Throwable) { result.completeExceptionally(failure) }
        return result
    }
    override fun close() {
        nearby.close()
        if (closed) return
        live.set(false)
        releaseWallet()
        enrollment?.let { it.close(); enrollment = null }
        installed?.let { it.close(); installed = null }
        attempt?.let { it.close(); attempt = null }
        closed = true
    }

    private fun releaseWallet() {
        val owner = wallet ?: return
        cleanupAfterExchangeFence(end = {
            // end() drains local tasks before its Native result. A failed Native end must
            // not prevent exact-wallet close/retry, including SDK-retired wallet cleanup.
            if (exchange != null) probeRequire(endExchange() != KagemushaWalletExchangeEndV1.ENDING, "exchange-ending")
        }, release = {
            val binding = activity
            if (binding != null) {
                try { binding.close() } finally {
                    if (binding.released) {
                        wallet = null; exchange = null; activity = null
                        monetary.released(); walletCloseAttempted = false
                    }
                }
            } else {
                // Binding construction can fail before a control executor owns the wallet.
                val retry = walletCloseAttempted; walletCloseAttempted = true
                if (retry) owner.retryCleanup() else owner.close()
                wallet = null; exchange = null; monetary.released(); walletCloseAttempted = false
            }
        })
    }
}

/** Worker-only cleanup ordering after accepted command calls finished. Preserve every failure. */
internal fun cleanupAfterExchangeFence(end: () -> Unit, release: () -> Unit) {
    var failure: Throwable? = null
    try { end() } catch (problem: Throwable) { failure = problem }
    try { release() } catch (problem: Throwable) {
        val previous = failure
        if (previous == null) failure = problem
        else if (previous !== problem) previous.addSuppressed(problem)
    }
    failure?.let { throw it }
}
