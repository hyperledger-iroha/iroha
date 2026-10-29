// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.util.Log
import java.io.IOException
import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.ExecutionException

/** Closed boundary names only; originals, handles and exception text never enter diagnostics. */
internal enum class WalletCallBoundaryV1(val label: String) {
    ENROLLMENT("enrollment"), OPEN_BEGIN("open_begin"), OPEN_FINISH("open_finish"),
    ACCOUNT_SIGNATURE("account_signature"),
}

/** Failure-only observations. The original call, reply, exception and future remain authoritative. */
internal class WalletCallObservationV1(private val publish: (String) -> Unit) {
    fun call(boundary: WalletCallBoundaryV1, selector: Int? = null,
        action: () -> KagemushaWalletCallV1?): KagemushaWalletCallV1? {
        val reply = try { action() } catch (failure: Throwable) {
            failed(boundary, selector, failure)
            throw failure
        }
        if (reply == null || reply.status < 0) runCatching {
            publish(prefix(boundary, selector) + " status=${reply?.status ?: KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT}" +
                " reason=${reply?.reason ?: -1} platform_code=${reply?.platformCode ?: 0}")
        }
        return reply
    }

    /** Observe the existing callback future without replacing it or changing its completion. */
    fun <T> accountSignature(action: () -> CompletableFuture<T>): CompletableFuture<T> {
        val future = try { action() } catch (failure: Throwable) {
            failed(WalletCallBoundaryV1.ACCOUNT_SIGNATURE, null, failure)
            throw failure
        }
        runCatching {
            future.whenComplete { _, failure ->
                if (failure != null) failed(WalletCallBoundaryV1.ACCOUNT_SIGNATURE, null, failure)
            }
        }
        return future
    }

    private fun prefix(boundary: WalletCallBoundaryV1, selector: Int?): String =
        "event=refused operation=${boundary.label}" + if (boundary == WalletCallBoundaryV1.ENROLLMENT)
            " selector=${selector?.takeIf { it in 0..21 }?.toString() ?: "unknown"}" else ""

    private fun failed(boundary: WalletCallBoundaryV1, selector: Int?, original: Throwable) {
        runCatching {
            var failure = original
            var remaining = 8
            while (remaining-- > 0 && (failure.javaClass == CompletionException::class.java ||
                    failure.javaClass == ExecutionException::class.java)) {
                failure = failure.cause ?: break
            }
            val fields = when (val selected = failure) {
                is KagemushaWalletExceptionV1 -> " failure=native status=${selected.status}" +
                    " reason=${selected.reason} platform_code=${selected.platformCode}"
                is CancellationException -> " failure=cancelled"
                is SecurityException -> " failure=security"
                is IOException -> " failure=io"
                is IllegalArgumentException -> " failure=invalid_argument"
                is IllegalStateException -> " failure=invalid_state"
                is LinkageError -> " failure=linkage"
                else -> " failure=other"
            }
            publish(prefix(boundary, selector) + fields)
        }
    }
}

internal object WalletCallObservationsV1 {
    val shared = WalletCallObservationV1 { message -> Log.i("IrohaWalletCalls", message) }
}
