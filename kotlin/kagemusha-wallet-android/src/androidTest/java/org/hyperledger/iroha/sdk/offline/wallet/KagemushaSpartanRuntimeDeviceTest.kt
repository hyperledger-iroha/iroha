// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.app.Activity
import android.content.Intent
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Bundle
import android.os.CancellationSignal
import android.os.Handler
import android.os.Looper
import android.view.WindowManager
import android.widget.TextView
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Full native engine in an isolated hardware-owned application. Fixed fixture
 * issuers and synthetic consensus events never select production funds. */
@RunWith(AndroidJUnit4::class)
class KagemushaSpartanRuntimeDeviceTest {
    @Test fun completeNativeLifecycleAndExactRestartRecovery() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        require(context.packageName.startsWith("org.hyperledger.iroha.kagemusha.enrollmentprobe."))
        val root = File(context.noBackupFilesDir, "spartan-native-runtime").apply { mkdirs() }
        val artifacts = File(context.filesDir, "spartan-programs").apply { mkdirs() }
        for (name in listOf("program-bundle.norito", "parameter-manifest.norito", "parameter-signature.bin")) {
            instrumentation.context.assets.open("spartan-artifacts/$name").use { input ->
                File(artifacts, name).outputStream().use { output -> input.copyTo(output) }
            }
        }
        instrumentation.runOnMainSync { SpartanRuntimeWindow.running = true }
        try {
            instrumentation.startActivitySync(
                Intent(instrumentation.context, KagemushaSpartanRuntimeActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            System.loadLibrary("kagemusha_spartan_device_fixture")
            val report = SpartanDeviceRuntimeNative.run(KagemushaWalletAndroidPlatformV1.create(context),
                root.absolutePath, artifacts.absolutePath, SpartanDeviceConfirmation())
            File(context.filesDir, "spartan-native-runtime-report.txt").writeText(report)
            instrumentation.sendStatus(0, Bundle().apply { putString("spartanRuntime", report) })
            assertTrue(report, report.startsWith("PASS:"))
        } finally {
            instrumentation.runOnMainSync {
                SpartanRuntimeWindow.running = false
                SpartanRuntimeWindow.activity?.finish()
                SpartanRuntimeWindow.activity = null
            }
        }
    }
}

/** Keep only this fixture's visible window awake while its native run is active. */
private object SpartanRuntimeWindow {
    // Accessed only on the main thread. Process death resets the active-run gate.
    var running = false
    var activity: KagemushaSpartanRuntimeActivity? = null
}

/** A recreated window cannot outlive the active instrumentation run. */
class KagemushaSpartanRuntimeActivity : Activity() {
    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        if (!SpartanRuntimeWindow.running) { finish(); return }
        SpartanRuntimeWindow.activity = this
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE or
            WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContentView(TextView(this).apply {
            textSize = 18f; setPadding(32, 48, 32, 32)
            text = "Experimental KAGEMUSHA native fixture\n\n" +
                "Verifying hardware custody and complete payment proofs.\n\n" +
                "The isolated Send will request Android authentication."
        })
    }
    override fun onDestroy() {
        if (SpartanRuntimeWindow.activity === this) SpartanRuntimeWindow.activity = null
        super.onDestroy()
    }
}

internal object SpartanDeviceRuntimeNative {
    @JvmStatic external fun run(platform: KagemushaWalletAndroidPlatformV1, root: String,
        artifacts: String, confirmation: SpartanDeviceConfirmation): String
}

/** JNI retains this object; the result can only come from a fresh Android prompt. */
internal class SpartanDeviceConfirmation {
    @Suppress("unused") fun confirm(review: ByteArray): Boolean {
        require(Looper.myLooper() != Looper.getMainLooper())
        val projection = KagemushaSpartanReviewProjectionV1(review)
        require(projection.operation == KagemushaSpartanOperationV1.SEND)
        require(projection.amount.low == 10L && projection.amount.high == 0L &&
            projection.fee.low == 0L && projection.fee.high == 0L)
        require(projection.receiverWalletId().contentEquals(projection.walletId()))
        val request = SpartanDevicePrompt.Request(review.copyOf(), projection)
        SpartanDevicePrompt.register(request)
        try {
            android.util.Log.i("SpartanDeviceFixture", "confirmation-required review=${request.digest}")
            val context = InstrumentationRegistry.getInstrumentation().context
            context.startActivity(Intent(context, KagemushaSpartanConfirmationActivity::class.java)
                .putExtra("ticket", request.ticket.id).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            val confirmed = request.ticket.await(120, TimeUnit.SECONDS)
            if (confirmed) {
                android.util.Log.i("SpartanDeviceFixture", "confirmation-granted review=${request.digest}")
            }
            return confirmed
        } finally {
            request.ticket.fail(IllegalStateException("Hardware confirmation closed"))
            SpartanDevicePrompt.clear(request)
            Handler(Looper.getMainLooper()).post { request.activity?.closePrompt() }
        }
    }
}
internal object SpartanDevicePrompt {
    class Request(val bytes: ByteArray, val projection: KagemushaSpartanReviewProjectionV1) {
        val ticket = KagemushaProbeConfirmationTicket<Boolean>()
        var activity: KagemushaSpartanConfirmationActivity? = null
        val digest = MessageDigest.getInstance("SHA-256").digest(bytes)
            .joinToString("") { "%02x".format(it.toInt() and 255) }
    }
    private var current: Request? = null
    @Synchronized fun register(request: Request) { check(current == null); current = request }
    @Synchronized fun claim(id: String) = current?.takeIf { it.ticket.claim(id) }
    @Synchronized fun clear(request: Request) { if (current === request) current = null }
}

/** Android's biometric/device credential prompt authenticates the exact review. */
class KagemushaSpartanConfirmationActivity : Activity() {
    private var request: SpartanDevicePrompt.Request? = null
    private var cancellation: CancellationSignal? = null
    private var started = false
    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        if (android.os.Build.VERSION.SDK_INT < 30 || state != null) { finish(); return }
        val selected = intent.getStringExtra("ticket")?.let(SpartanDevicePrompt::claim)
            ?: run { finish(); return }
        request = selected; selected.activity = this
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE or
            WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContentView(TextView(this).apply {
            textSize = 18f; setPadding(32, 48, 32, 32)
            text = "Experimental KAGEMUSHA hardware fixture\n\n" +
                "Self-transfer: 10 test units, fee 0\n\n" +
                "Exact native review SHA-256:\n${selected.digest}\n\n" +
                "Confirm this isolated Send using Android authentication."
        })
    }
    override fun onResume() {
        super.onResume()
        val selected = request ?: return
        if (started) return
        started = true
        try {
            val signal = CancellationSignal(); cancellation = signal
            BiometricPrompt.Builder(this).setTitle("Confirm experimental Send")
                .setSubtitle("Review ${selected.digest.take(12)}")
                .setDescription("Self-transfer 10 test units, fee 0")
                .setConfirmationRequired(true)
                .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG or
                    BiometricManager.Authenticators.DEVICE_CREDENTIAL).build()
                .authenticate(signal, mainExecutor, object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        if (result.authenticationType == BiometricPrompt.AUTHENTICATION_RESULT_TYPE_BIOMETRIC ||
                            result.authenticationType == BiometricPrompt.AUTHENTICATION_RESULT_TYPE_DEVICE_CREDENTIAL) {
                            selected.ticket.succeed(true)
                        } else selected.ticket.fail(IllegalStateException("Unexpected authentication type"))
                        finish()
                    }
                    override fun onAuthenticationError(code: Int, message: CharSequence) {
                        selected.ticket.fail(KagemushaProbeAuthenticationFailure(code)); finish()
                    }
                })
        } catch (failure: Throwable) { selected.ticket.fail(failure); finish() }
    }
    internal fun closePrompt() { cancellation?.cancel(); cancellation = null; finish() }
    override fun onDestroy() {
        request?.ticket?.fail(IllegalStateException("Hardware confirmation dismissed"))
        cancellation?.cancel(); super.onDestroy()
    }
}
