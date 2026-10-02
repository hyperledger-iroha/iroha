// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import java.util.concurrent.Executors
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine
/** One workflow-owned Native worker. A cancelled UI does not cancel the actual Native call
 * or discard its eventual journal result. The caller retains the returned original before
 * its next cancellable HTTP suspension. Retirement drains already admitted work.
 */
internal class KagemushaRetainedNativeIoV1 {
 private val executor=Executors.newSingleThreadExecutor { task ->
  Thread(task,"iroha-ordinary-native-workflow").also {it.isDaemon=true}
 }
 suspend fun<T> call(operation:()->T):T=suspendCoroutine{continuation->
  try{executor.execute{try{continuation.resume(operation())}catch(failure:Throwable){continuation.resumeWithException(failure)}}}
  catch(failure:Throwable){continuation.resumeWithException(failure)}
 }
 fun retire(){executor.shutdown()}
}
