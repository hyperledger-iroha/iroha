package org.hyperledger.iroha.android.nexus;

import org.hyperledger.iroha.android.client.PipelineStatusOptions;

/** Options for signing finalization and Torii pipeline waiting. */
public final class NexusFinalizeOptions {

  private final boolean waitForFinalStatus;
  private final PipelineStatusOptions pipelineStatusOptions;

  public NexusFinalizeOptions() {
    this(true, null);
  }

  public NexusFinalizeOptions(
      final boolean waitForFinalStatus, final PipelineStatusOptions pipelineStatusOptions) {
    this.waitForFinalStatus = waitForFinalStatus;
    this.pipelineStatusOptions = pipelineStatusOptions;
  }

  public boolean waitForFinalStatus() {
    return waitForFinalStatus;
  }

  public PipelineStatusOptions pipelineStatusOptions() {
    return pipelineStatusOptions;
  }
}
