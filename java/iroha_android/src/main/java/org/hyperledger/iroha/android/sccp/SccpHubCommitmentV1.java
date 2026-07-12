package org.hyperledger.iroha.android.sccp;

import java.util.Arrays;
import java.util.Objects;

/** Exact fixed-width SORA hub commitment. */
public final class SccpHubCommitmentV1 {
  private final SccpHubMessageKindV1 kind;
  private final SccpOutboundMessageContextV1 context;
  private final byte[] messageId;
  private final byte[] payloadHash;

  SccpHubCommitmentV1(
      final SccpHubMessageKindV1 kind,
      final SccpOutboundMessageContextV1 context,
      final byte[] messageId,
      final byte[] payloadHash) {
    this.kind = Objects.requireNonNull(kind, "kind");
    this.context = Objects.requireNonNull(context, "context");
    this.messageId = SccpV1.requireHash(messageId, "messageId");
    this.payloadHash = SccpV1.requireHash(payloadHash, "payloadHash");
  }

  public SccpHubMessageKindV1 kind() {
    return kind;
  }

  public SccpOutboundMessageContextV1 context() {
    return context;
  }

  public byte[] messageId() {
    return Arrays.copyOf(messageId, messageId.length);
  }

  public byte[] payloadHash() {
    return Arrays.copyOf(payloadHash, payloadHash.length);
  }
}
