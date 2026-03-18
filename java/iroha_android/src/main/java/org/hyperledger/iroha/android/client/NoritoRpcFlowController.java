package org.hyperledger.iroha.android.client;

import java.util.concurrent.Semaphore;

/**
 * Controls the maximum number of concurrent Norito RPC calls issued by the Android client.
 *
 * <p>Implementations may impose scheduling policies (for example, semaphores) or simply act as a
 * no-op when unlimited concurrency is desired.
 */
public interface NoritoRpcFlowController {

  /** Blocks until the call is allowed to proceed. */
  void acquire() throws InterruptedException;

  /** Signals that the call completed and capacity may be returned to the controller. */
  void release();

  /** Returns a flow controller that never blocks. */
  static NoritoRpcFlowController unlimited() {
    return NoopFlowController.INSTANCE;
  }

  /**
   * Creates a semaphore-backed controller that limits in-flight requests.
   *
   * @param maxConcurrentRequests maximum concurrent RPC calls; must be positive
   */
  static NoritoRpcFlowController semaphore(final int maxConcurrentRequests) {
    if (maxConcurrentRequests <= 0) {
      throw new IllegalArgumentException("maxConcurrentRequests must be positive");
    }
    return new SemaphoreFlowController(maxConcurrentRequests);
  }

  /** Semaphore-based controller with a fair scheduler. */
  final class SemaphoreFlowController implements NoritoRpcFlowController {
    private final Semaphore semaphore;

    private SemaphoreFlowController(final int permits) {
      this.semaphore = new Semaphore(permits, true);
    }

    @Override
    public void acquire() throws InterruptedException {
      semaphore.acquire();
    }

    @Override
    public void release() {
      semaphore.release();
    }
  }

  /** No-op controller used when concurrency should not be limited. */
  enum NoopFlowController implements NoritoRpcFlowController {
    INSTANCE;

    @Override
    public void acquire() {
      // Intentionally left blank.
    }

    @Override
    public void release() {
      // Intentionally left blank.
    }
  }
}
