// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.URL
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executor
import java.util.concurrent.ForkJoinPool
import javax.net.ssl.HttpsURLConnection

/** The fixed Native-selected Core transport. No enrolled wallet/device session is required.
 * The original Google owner credential is in the protected prepare body, never an auth header.
 * One actual HTTP attempt: redirects, automatic application retries and alternate origins refuse.
 */
class KagemushaFirstDeviceHardwareHttpsTransportV1 @JvmOverloads constructor(
    private val executor: Executor = ForkJoinPool.commonPool(),
) : KagemushaHardwareBootstrapOriginalTransportV1 {
    override fun exchange(original: KagemushaHardwareBootstrapHttpOriginalV1): CompletableFuture<ByteArray> {
        original.claimOriginalInvocation()
        val future=CompletableFuture<ByteArray>()
        executor.execute {
            try { future.complete(exchangeOriginal(original)) }
            catch(failure: Throwable) { future.completeExceptionally(failure) }
        }
        return future
    }
    private fun exchangeOriginal(original: KagemushaHardwareBootstrapHttpOriginalV1): ByteArray {
        original.requireInvocationCurrent()
        val body=original.body()
        val connection=URL(original.coreOrigin+original.path).openConnection() as HttpsURLConnection
        try {
            connection.instanceFollowRedirects=false
            connection.requestMethod="POST";connection.connectTimeout=10_000;connection.readTimeout=10_000
            connection.doOutput=true;connection.useCaches=false
            connection.setRequestProperty("Content-Type","application/json")
            connection.setRequestProperty("Accept","application/json")
            connection.setRequestProperty("Idempotency-Key",original.requestId)
            connection.setFixedLengthStreamingMode(body.size)
            original.requireInvocationCurrent()
            connection.outputStream.use { original.requireInvocationCurrent();it.write(body);it.flush() }
            original.requireCurrent()
            check(connection.responseCode==200) { "Protected hardware issuer declined the original request" }
            check(connection.contentType?.substringBefore(';')?.trim()?.lowercase()=="application/json") { "Issuer response type differs" }
            val length=connection.contentLengthLong
            require(length == -1L || length in 1..original.maximumResponseBytes.toLong())
            val reply=connection.inputStream.use { readBoundedHardwareIssuerOriginal(it,original.maximumResponseBytes,original::requireCurrent) }
            original.requireCurrent()
            return original.decodeIssuerResponse(reply)
        } finally { body.fill(0);connection.disconnect() }
    }
}
/** Complete response guard used by the production streamed path, including unknown chunk size. */
internal fun readBoundedHardwareIssuerOriginal(input: InputStream, maximum: Int, requireOwner: () -> Unit): ByteArray {
    require(maximum in 1..((192*1024+2)/3)*4+128)
    val output=ByteArrayOutputStream();val buffer=ByteArray(8192)
    while(true) {
        requireOwner()
        // Read at most the remaining allowance plus one byte, refusing an oversized chunked body.
        val count=input.read(buffer,0,minOf(buffer.size,maximum-output.size()+1))
        requireOwner();if(count==-1) break
        check(count>0) { "Issuer stream made no progress" }
        check(count<=maximum-output.size()) { "Issuer original exceeds its complete bound" }
        output.write(buffer,0,count)
    }
    requireOwner();check(output.size()>0)
    return output.toByteArray()
}
