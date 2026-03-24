package org.hyperledger.iroha.sdk.client

import java.util.concurrent.Semaphore

/**
 * Controls the maximum number of concurrent Norito RPC calls issued by the client.
 *
 * Implementations may impose scheduling policies (for example, semaphores) or simply act as a
 * no-op when unlimited concurrency is desired.
 */
interface NoritoRpcFlowController {

    /** Blocks until the call is allowed to proceed. */
    @Throws(InterruptedException::class)
    fun acquire()

    /** Signals that the call completed and capacity may be returned to the controller. */
    fun release()

    companion object {
        /** Returns a flow controller that never blocks. */
        @JvmStatic
        fun unlimited(): NoritoRpcFlowController = NoopFlowController

        /**
         * Creates a semaphore-backed controller that limits in-flight requests.
         *
         * @param maxConcurrentRequests maximum concurrent RPC calls; must be positive
         */
        @JvmStatic
        fun semaphore(maxConcurrentRequests: Int): NoritoRpcFlowController {
            require(maxConcurrentRequests > 0) { "maxConcurrentRequests must be positive" }
            return SemaphoreFlowController(maxConcurrentRequests)
        }
    }
}

/** Semaphore-based controller with a fair scheduler. */
private class SemaphoreFlowController(permits: Int) : NoritoRpcFlowController {
    private val semaphore = Semaphore(permits, true)

    @Throws(InterruptedException::class)
    override fun acquire() {
        semaphore.acquire()
    }

    override fun release() {
        semaphore.release()
    }
}

/** No-op controller used when concurrency should not be limited. */
private object NoopFlowController : NoritoRpcFlowController {
    override fun acquire() {}
    override fun release() {}
}
