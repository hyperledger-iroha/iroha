package org.hyperledger.iroha.android.nexus;

import java.util.Objects;

/** Transfer draft containing both the normalized input and signable payload. */
public final class NexusTransferDraft {

  private final NexusTransferInput input;
  private final NexusSignableTransaction signable;

  public NexusTransferDraft(final NexusTransferInput input, final NexusSignableTransaction signable) {
    this.input = Objects.requireNonNull(input, "input");
    this.signable = Objects.requireNonNull(signable, "signable");
  }

  public NexusTransferInput input() {
    return input;
  }

  public NexusSignableTransaction signable() {
    return signable;
  }
}
