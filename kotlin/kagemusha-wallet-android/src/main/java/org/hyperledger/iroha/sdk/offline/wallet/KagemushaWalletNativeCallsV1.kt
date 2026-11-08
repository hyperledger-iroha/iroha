// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Synchronous ABI33 SDK boundary. Exchange controls bypass worker queues. */
internal object KagemushaWalletNativeCallsV1 {
    private val worker get() = KagemushaWalletNativeWorkerV1.shared

    fun revision() = KagemushaWalletNativeV1.revision()
    fun openBegin(runtime: Long, credential: ByteArray, certificates: ByteArray, account: ByteArray, asset: ByteArray) =
        WalletCallObservationsV1.shared.call(WalletCallBoundaryV1.OPEN_BEGIN) {
            worker.run { KagemushaWalletNativeV1.openBegin(runtime, credential, certificates, account, asset) }
        }
    fun openFinish(runtime: Long, signature: ByteArray) =
        WalletCallObservationsV1.shared.call(WalletCallBoundaryV1.OPEN_FINISH) {
            worker.run { KagemushaWalletNativeV1.openFinish(runtime, signature) }
        }
    fun openCancel(runtime: Long) = KagemushaWalletNativeV1.openCancel(runtime)
    // Never queue control behind a fold or payment which it must cancel/join.
    fun close(handle: Long) = KagemushaWalletNativeV1.close(handle)
    fun beginExchange(handle: Long) = KagemushaWalletNativeV1.beginExchange(handle)
    fun endExchange(handle: Long, exchangeId: Long) = KagemushaWalletNativeV1.endExchange(handle, exchangeId)
    fun activity(handle: Long, foreground: Int, charging: Int) =
        KagemushaWalletNativeV1.activity(handle, foreground, charging)
    fun call(handle: Long, operation: Int, first: ByteArray, second: ByteArray) =
        worker.run(fold = operation == 3) { KagemushaWalletNativeV1.call(handle, operation, first, second) }
    fun enrollment(runtime: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>) =
        WalletCallObservationsV1.shared.call(WalletCallBoundaryV1.ENROLLMENT, selector) {
            worker.run { KagemushaWalletNativeV1.enrollment(runtime, selector, first, second, third, certificates) }
        }
    fun enrollmentRefusal(code: Int, android: Boolean) = KagemushaWalletNativeV1.enrollmentRefusal(code, android)
    fun setup(handle: Long, exchangeId: Long, setupId: ByteArray, selector: Int, amountLow: Long, amountHigh: Long, token: Long, first: ByteArray, second: ByteArray, third: ByteArray) =
        worker.setup(selector) { KagemushaWalletNativeV1.setup(handle, exchangeId, setupId, selector, amountLow, amountHigh, token, first, second, third) }
    fun execute(handle: Long, exchangeId: Long, requestId: ByteArray, selector: Int, amountLow: Long, amountHigh: Long, first: ByteArray, second: ByteArray, third: ByteArray) =
        worker.run { KagemushaWalletNativeV1.execute(handle, exchangeId, requestId, selector, amountLow, amountHigh, first, second, third) }
    fun snapshot(handle: Long) = worker.snapshot { KagemushaWalletNativeV1.snapshot(handle) }
    fun review(handle: Long, exchangeId: Long, requestId: ByteArray, selector: Int, amountLow: Long, amountHigh: Long, expectation: Int, expectedProjection: ByteArray, first: ByteArray, second: ByteArray) =
        worker.run { KagemushaWalletNativeV1.review(handle, exchangeId, requestId, selector, amountLow, amountHigh, expectation, expectedProjection, first, second) }
    fun suffixUnloadStatus(handle: Long) = worker.run { KagemushaWalletNativeV1.suffixUnloadStatus(handle) }
    fun suffixUnloadPrepare(handle: Long, exchangeId: Long, request: KagemushaWalletSuffixUnloadRequestV1) = worker.run {
        KagemushaWalletNativeV1.suffixUnloadPrepare(handle, exchangeId, request.requestId(), request.amount.low,
            request.amount.high, request.quote() ?: byteArrayOf(), request.certificates() ?: byteArrayOf(),
            request.beneficiary() ?: byteArrayOf())
    }
    fun suffixUnloadResume(handle: Long, exchangeId: Long, requestId: ByteArray) = worker.run {
        KagemushaWalletNativeV1.suffixUnloadResume(handle, exchangeId, requestId)
    }
    fun executeReviewedSuffixUnload(handle: Long, exchangeId: Long, token: Long) = worker.run {
        KagemushaWalletNativeV1.executeReviewedSuffixUnload(handle, exchangeId, token)
    }
    fun suffixUnloadInstruction(handle: Long, requestId: ByteArray, ordinal: KagemushaWalletUInt128V1) = worker.run {
        KagemushaWalletNativeV1.suffixUnloadInstruction(handle, requestId, ordinal.low, ordinal.high)
    }
    fun suffixUnloadAttemptOutcome(handle: Long, requestId: ByteArray, ordinal: KagemushaWalletUInt128V1, signedWire: ByteArray) = worker.run {
        KagemushaWalletNativeV1.suffixUnloadAttemptOutcome(handle, requestId, ordinal.low, ordinal.high, signedWire)
    }
    fun suffixUnloadIngestFinality(handle: Long, exchangeId: Long, requestId: ByteArray, ordinal: KagemushaWalletUInt128V1,
        signedWire: ByteArray, finality: ByteArray) = worker.run {
        KagemushaWalletNativeV1.suffixUnloadIngestFinality(handle, exchangeId, requestId, ordinal.low, ordinal.high, signedWire, finality)
    }
    fun suffixUnloadSignedWire(handle: Long, requestId: ByteArray, ordinal: KagemushaWalletUInt128V1, attemptIndex: Long) = worker.run {
        KagemushaWalletNativeV1.suffixUnloadSignedWire(handle, requestId, ordinal.low, ordinal.high, attemptIndex)
    }

    fun executeReviewed(handle: Long, exchangeId: Long, token: Long) =
        worker.run { KagemushaWalletNativeV1.executeReviewed(handle, exchangeId, token) }
    fun discardReview(handle: Long, exchangeId: Long, token: Long) = KagemushaWalletNativeV1.discardReview(handle, exchangeId, token)

}
