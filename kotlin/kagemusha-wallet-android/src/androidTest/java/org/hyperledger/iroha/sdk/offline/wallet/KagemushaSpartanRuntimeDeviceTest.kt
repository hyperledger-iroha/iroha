// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Intent
import android.os.Bundle
import android.os.Looper
import android.view.WindowManager
import android.widget.TextView
import androidx.fragment.app.FragmentActivity
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.security.MessageDigest
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
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
        val assets = instrumentation.context.assets
        val noBackup = KagemushaWalletAndroidStorageV1.privateNoBackupFilesDir(context)
        val selected = assets.open("spartan-artifacts/parameter-manifest.norito").use {
            SpartanDeviceFixturePaths(noBackup, context.filesDir, it.readBytes())
        }
        check(selected.custody.mkdirs() || selected.custody.isDirectory) {
            "Unable to open the Spartan fixture custody directory"
        }
        check(selected.programs.mkdirs() || selected.programs.isDirectory) {
            "Unable to open the Spartan fixture program directory"
        }
        val originals = mutableListOf("program-bundle.norito", "parameter-manifest.norito", "parameter-signature.bin")
        val worldSelection = "world-manifest-digest.bin"
        if (worldSelection in assets.list("spartan-artifacts").orEmpty()) {
            originals.add(worldSelection)
        } else {
            check(!File(selected.programs, worldSelection).exists()) {
                "A previous fixture extraction contains a World selection absent from this APK"
            }
        }
        for (name in originals) {
            assets.open("spartan-artifacts/$name").use { input ->
                File(selected.programs, name).outputStream().use { output -> input.copyTo(output) }
            }
        }
        instrumentation.runOnMainSync { SpartanRuntimeWindow.running = true }
        try {
            val activity = instrumentation.startActivitySync(
                Intent(instrumentation.context, KagemushaSpartanRuntimeActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) as KagemushaSpartanRuntimeActivity
            System.loadLibrary("kagemusha_spartan_device_fixture")
            val platform = KagemushaWalletAndroidPlatformV1.create(context)
            val report = SpartanDeviceRuntimeNative.run(platform, selected.custody.absolutePath,
                selected.programs.absolutePath, SpartanDeviceConfirmation(platform, activity))
            selected.report.writeText(report)
            instrumentation.sendStatus(0, Bundle().apply {
                putString("spartanRuntime", report)
                putString("spartanReportSha256", selected.report.inputStream().use(::fixtureSha256))
                putString("spartanManifestSha256", selected.digest)
                putString("spartanReportPath", selected.reportPath)
                putString("spartanCustodyPath", selected.custodyPath)
                listOf(
                    "spartanMeasurementsSha256" to "device-measurements.json",
                    "spartanEventsSha256" to "device-measurements.jsonl",
                ).forEach { (field, name) ->
                    val file = File(selected.custody, name)
                    if (file.isFile) file.inputStream().use { putString(field, fixtureSha256(it)) }
                }
            })
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

/** Preserve every previous scheme's custody across upgrades of this diagnostic package. */
internal class SpartanDeviceFixturePaths(noBackupDir: File, filesDir: File, manifest: ByteArray) {
    val digest: String = manifest.inputStream().use(::fixtureSha256)
    private val generation = "spartan-fixtures/$digest/protected-debit-v1"
    val custody = File(noBackupDir, generation)
    private val diagnostics = File(filesDir, generation)
    val programs = File(diagnostics, "programs")
    val report = File(diagnostics, "spartan-native-runtime-report.txt")
    val reportPath = "files/$generation/${report.name}"
    val custodyPath = "no_backup/$generation"
}

private fun fixtureSha256(input: java.io.InputStream): String {
    val digest = MessageDigest.getInstance("SHA-256")
    val buffer = ByteArray(8192)
    var count = input.read(buffer)
    while (count != -1) {
        digest.update(buffer, 0, count)
        count = input.read(buffer)
    }
    return digest.digest().joinToString("") { "%02x".format(it.toInt() and 255) }
}

/** Keep only this fixture's visible window awake while its native run is active. */
private object SpartanRuntimeWindow {
    // Accessed only on the main thread. Process death resets the active-run gate.
    var running = false
    var activity: KagemushaSpartanRuntimeActivity? = null
}

/** A recreated window cannot outlive the active instrumentation run. */
class KagemushaSpartanRuntimeActivity : FragmentActivity() {
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
    @JvmStatic external fun confirm(owner: Long, token: Long, message: ByteArray,
        der: ByteArray): KagemushaSpartanReplyV1?
}

/** Present the same exact review, hardware operation and Native retention protocol as retail. */
internal class SpartanDeviceConfirmation(
    private val platform: KagemushaWalletAndroidPlatformV1,
    private val activity: KagemushaSpartanRuntimeActivity,
) {
    @Suppress("unused") fun confirm(owner: Long, token: Long, slot: ByteArray, review: ByteArray) {
        check(Looper.myLooper() != Looper.getMainLooper())
        val projection = KagemushaSpartanReviewProjectionV1(review)
        require(projection.operation == KagemushaSpartanOperationV1.SEND)
        require(projection.amount.low == 10L && projection.amount.high == 0L &&
            projection.fee.low == 0L && projection.fee.high == 0L)
        require(!projection.receiverWalletId().contentEquals(projection.walletId()))
        val prompt = createSpartanDebitPromptV1(platform, slot, projection.sendDebitTerms()) { message, der ->
            KagemushaSpartanWalletV1.call { SpartanDeviceRuntimeNative.confirm(owner, token, message, der) }
        }
        android.util.Log.i("SpartanDeviceFixture", "Protected Send confirmation required")
        try {
            check(runBlocking { withTimeout(120_000L) { prompt.authenticate(activity) } }) {
                "Protected Send confirmation refused"
            }
            android.util.Log.i("SpartanDeviceFixture", "Native retained protected Send confirmation")
        } finally {
            prompt.cancel()
        }
    }
}
