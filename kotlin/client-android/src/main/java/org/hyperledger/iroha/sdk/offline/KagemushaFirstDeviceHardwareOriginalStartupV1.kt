// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executor

/** One owned startup future exists before the worker starts Native policy/clock custody.
 * View cancellation, Compose recreation, delay or a failed result cannot launch another open.
 * This kernel supplies no Native policy, measurement, clock or hardware admission.
 */
internal class KagemushaFirstDeviceHardwareOriginalStartupV1<T>(private val executor: Executor) {
    private var original: CompletableFuture<T>? = null
    @Synchronized fun openOriginal(work: () -> T): CompletableFuture<T> {
        original?.let { return detachedOriginalView(it) { value -> value } }
        val owned=CompletableFuture<T>();original=owned
        try { executor.execute {
            try { owned.complete(work()) } catch(failure: Throwable) { owned.completeExceptionally(failure) }
        } } catch(failure: Throwable) { owned.completeExceptionally(failure) }
        return detachedOriginalView(owned) { value -> value }
    }
}
