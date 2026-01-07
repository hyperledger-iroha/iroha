package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;

/** Parameters accepted by the Torii `/v1/offline/transfers/proof` endpoint; requires a transfer payload. */
public final class OfflineProofRequestParams {

  private final Map<String, Object> transferPayload;
  private final OfflineProofRequestKind kind;
  private final Long counterCheckpoint;
  private final String replayLogHeadHex;
  private final String replayLogTailHex;

  private OfflineProofRequestParams(final Builder builder) {
    if (builder.transferPayload == null) {
      throw new IllegalArgumentException("transferPayload must be provided");
    }
    this.transferPayload = new LinkedHashMap<>(builder.transferPayload);
    if (transferPayload.isEmpty()) {
      throw new IllegalArgumentException("transferPayload must not be empty");
    }
    this.kind = Objects.requireNonNull(builder.kind, "kind must be provided");
    this.counterCheckpoint = builder.counterCheckpoint;
    this.replayLogHeadHex = builder.replayLogHeadHex;
    this.replayLogTailHex = builder.replayLogTailHex;
    if (kind == OfflineProofRequestKind.REPLAY) {
      if (replayLogHeadHex == null || replayLogTailHex == null) {
        throw new IllegalArgumentException(
            "replay proofs require replayLogHeadHex and replayLogTailHex");
      }
    } else {
      if (replayLogHeadHex != null || replayLogTailHex != null) {
        throw new IllegalArgumentException(
            "replayLogHeadHex/replayLogTailHex are only valid for replay proofs");
      }
    }
  }

  public static Builder builder() {
    return new Builder();
  }

  public OfflineProofRequestKind kind() {
    return kind;
  }

  public byte[] toJsonBytes() {
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("transfer", transferPayload);
    body.put("kind", kind.asParameter());
    if (counterCheckpoint != null) {
      body.put("counter_checkpoint", counterCheckpoint);
    }
    if (replayLogHeadHex != null) {
      body.put("replay_log_head_hex", replayLogHeadHex);
    }
    if (replayLogTailHex != null) {
      body.put("replay_log_tail_hex", replayLogTailHex);
    }
    return JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8);
  }

  public static final class Builder {
    private Map<String, Object> transferPayload;
    private OfflineProofRequestKind kind;
    private Long counterCheckpoint;
    private String replayLogHeadHex;
    private String replayLogTailHex;

    private Builder() {}

    public Builder transferPayload(final Map<String, Object> transferPayload) {
      this.transferPayload = transferPayload;
      return this;
    }

    public Builder kind(final OfflineProofRequestKind kind) {
      this.kind = kind;
      return this;
    }

    public Builder counterCheckpoint(final Long counterCheckpoint) {
      this.counterCheckpoint = counterCheckpoint;
      return this;
    }

    public Builder replayLogHeadHex(final String replayLogHeadHex) {
      this.replayLogHeadHex = replayLogHeadHex;
      return this;
    }

    public Builder replayLogTailHex(final String replayLogTailHex) {
      this.replayLogTailHex = replayLogTailHex;
      return this;
    }

    public OfflineProofRequestParams build() {
      return new OfflineProofRequestParams(this);
    }
  }
}
