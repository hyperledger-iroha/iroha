// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.app.Activity
import android.app.Instrumentation
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.test.runner.lifecycle.ActivityLifecycleCallback
import androidx.test.runner.lifecycle.ActivityLifecycleMonitor
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry
import androidx.test.runner.lifecycle.Stage
import java.util.concurrent.Callable
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executors
import java.util.concurrent.FutureTask

/** Actual instrumentation-host observation; custody's target Context is not the Activity host. */
internal class KagemushaProbeActivityObservationAndroid(
    private val instrumentation: Instrumentation,
) : KagemushaProbeActivityObservation {
    private val context = instrumentation.context
    private val main = Handler(Looper.getMainLooper())
    private var monitor: ActivityLifecycleMonitor? = null
    private var lifecycleRegistered = false
    private var receiverAttempted = false
    private var receiverRegistered = false
    private var observing = false
    private var seeded = false
    private var batteryObserved = false
    private var foreground = false
    private var charging = false
    private var changed: ((Boolean, Boolean) -> Unit)? = null
    private var failed: ((Throwable) -> Unit)? = null

    // Keep a strong reference: the AndroidX monitor retains callbacks weakly.
    private val lifecycle = ActivityLifecycleCallback { activity, _ ->
        if (observing && owns(activity)) capture {
            foreground = visible()
            emit()
        }
    }
    private val battery = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (observing && intent?.action == Intent.ACTION_BATTERY_CHANGED) capture {
                batteryObserved = true
                charging = kagemushaProbeChargingStatus(intent)
                emit()
            }
        }
    }

    override fun start(changed: (Boolean, Boolean) -> Unit, failed: (Throwable) -> Unit) = onMain {
        check(!observing && monitor == null)
        this.changed = changed; this.failed = failed; observing = true
        val selected = ActivityLifecycleMonitorRegistry.getInstance()
        monitor = selected
        // Subscribe before sampling on the same main-thread turn as lifecycle and battery events.
        selected.addLifecycleCallback(lifecycle); lifecycleRegistered = true
        receiverAttempted = true
        val filter = IntentFilter(Intent.ACTION_BATTERY_CHANGED)
        val sticky = if (Build.VERSION.SDK_INT >= 33) {
            context.registerReceiver(battery, filter, null, main, Context.RECEIVER_NOT_EXPORTED)
        } else {
            context.registerReceiver(battery, filter, null, main)
        }
        receiverRegistered = true
        // Do not overwrite a synchronously observed newer battery event with a returned snapshot.
        if (!batteryObserved) charging = kagemushaProbeChargingStatus(sticky)
        foreground = visible()
        seeded = true
        emit()
    }

    override fun detach() = onMain {
        observing = false; seeded = false
        var problem: Throwable? = null
        if (lifecycleRegistered) {
            try { monitor!!.removeLifecycleCallback(lifecycle); lifecycleRegistered = false }
            catch (failure: Throwable) { problem = failure }
        }
        if (receiverAttempted) {
            try {
                context.unregisterReceiver(battery)
                receiverRegistered = false; receiverAttempted = false
            } catch (failure: IllegalArgumentException) {
                // Failed registration can leave no receiver. A known registration must detach.
                if (!receiverRegistered) receiverAttempted = false else if (problem == null) problem = failure
            } catch (failure: Throwable) { if (problem == null) problem = failure }
        }
        if (!lifecycleRegistered && !receiverAttempted) {
            monitor = null; changed = null; failed = null
        }
        problem?.let { throw it }
    }

    private fun visible(): Boolean = Stage.values().filter(::kagemushaProbeVisibleStage).any { stage ->
        monitor!!.getActivitiesInStage(stage).any(::owns)
    }

    private fun owns(activity: Activity): Boolean = activity is KagemushaProbeConfirmationActivity ||
        activity is KagemushaNearbyPairingActivity || activity is KagemushaNearbyPermissionsActivity

    private fun emit() { if (seeded) changed!!(foreground, charging) }
    private fun capture(action: () -> Unit) {
        try { action() } catch (problem: Throwable) { failed?.invoke(problem) }
    }
    private fun onMain(action: () -> Unit) {
        check(Looper.myLooper() != Looper.getMainLooper()) { "activity-control-thread" }
        val work = FutureTask(Callable { action() })
        instrumentation.runOnMainSync(work)
        try { work.get() }
        catch (problem: ExecutionException) { throw problem.cause ?: problem }
        catch (problem: InterruptedException) { Thread.currentThread().interrupt(); throw problem }
    }
}

/** Paused probe UI remains visible; stopped/destroyed or not-yet-started UI does not. */
internal fun kagemushaProbeVisibleStage(stage: Stage): Boolean =
    stage == Stage.STARTED || stage == Stage.RESUMED || stage == Stage.PAUSED

/** Only an actual charging/full battery status enables this axis; a plugged flag is insufficient. */
internal fun kagemushaProbeChargingStatus(intent: Intent?): Boolean {
    if (intent?.action != Intent.ACTION_BATTERY_CHANGED) return false
    return when (intent.getIntExtra(BatteryManager.EXTRA_STATUS, BatteryManager.BATTERY_STATUS_UNKNOWN)) {
        BatteryManager.BATTERY_STATUS_CHARGING, BatteryManager.BATTERY_STATUS_FULL -> true
        else -> false
    }
}

/** Exact-wallet binding; the control executor never shares the synchronous monetary command path. */
internal fun kagemushaProbeActivityBinding(
    instrumentation: Instrumentation,
    wallet: KagemushaWalletV1,
    generation: Long,
): KagemushaProbeActivityControl {
    val executor = Executors.newSingleThreadExecutor { action ->
        Thread(action, "kagemusha-probe-activity-$generation").apply { isDaemon = true }
    }
    return KagemushaProbeActivityControl(generation, executor,
        KagemushaProbeActivityObservationAndroid(instrumentation), wallet::setActivity,
        { retry -> if (retry) wallet.retryCleanup() else wallet.close() }, executor::shutdown)
}
