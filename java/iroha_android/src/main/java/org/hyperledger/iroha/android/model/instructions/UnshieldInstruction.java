package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Typed representation of {@code zk::Unshield}. */
public final class UnshieldInstruction implements InstructionTemplate {
  private final String asset;
  private final String to;
  private final String publicAmount;
  private final List<byte[]> inputs;
  private final List<byte[]> outputs;
  private final ProofAttachment proof;
  private final byte[] rootHint;
  private final Map<String, String> arguments;

  private UnshieldInstruction(final Builder builder) {
    this.asset = builder.asset;
    this.to = builder.to;
    this.publicAmount = builder.publicAmount;
    this.inputs = copyList(builder.inputs);
    this.outputs = copyList(builder.outputs);
    this.proof = builder.proof;
    this.rootHint = builder.rootHint == null ? null : builder.rootHint.clone();
    final LinkedHashMap<String, String> args = new LinkedHashMap<>();
    args.put("action", "Unshield");
    args.put("asset", asset);
    args.put("to", to);
    args.put("public_amount", publicAmount);
    args.put("inputs", hexJoin(inputs));
    args.put("outputs", hexJoin(outputs));
    args.put("proof", proof.toNativeJson());
    args.put("root_hint", rootHint == null ? "" : ZkInstructionUtils.hexLower(rootHint));
    this.arguments = Collections.unmodifiableMap(args);
  }

  public String asset() {
    return asset;
  }

  public String to() {
    return to;
  }

  public String publicAmount() {
    return publicAmount;
  }

  public List<byte[]> inputs() {
    return copyList(inputs);
  }

  public List<byte[]> outputs() {
    return copyList(outputs);
  }

  public ProofAttachment proof() {
    return proof;
  }

  public byte[] rootHint() {
    return rootHint == null ? null : rootHint.clone();
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static Builder builder() {
    return new Builder();
  }

  private static List<byte[]> copyList(final List<byte[]> source) {
    final ArrayList<byte[]> copy = new ArrayList<>(source.size());
    for (final byte[] value : source) {
      copy.add(value.clone());
    }
    return Collections.unmodifiableList(copy);
  }

  private static String hexJoin(final List<byte[]> values) {
    final StringBuilder builder = new StringBuilder(values.size() * 64);
    for (int i = 0; i < values.size(); i++) {
      if (i > 0) {
        builder.append(',');
      }
      builder.append(ZkInstructionUtils.hexLower(values.get(i)));
    }
    return builder.toString();
  }

  public static final class Builder {
    private String asset;
    private String to;
    private String publicAmount;
    private final ArrayList<byte[]> inputs = new ArrayList<>();
    private final ArrayList<byte[]> outputs = new ArrayList<>();
    private ProofAttachment proof;
    private byte[] rootHint;

    private Builder() {}

    public Builder setAsset(final String asset) {
      this.asset = ZkInstructionUtils.requireText(asset, "asset");
      return this;
    }

    public Builder setTo(final String to) {
      this.to = ZkInstructionUtils.requireText(to, "to");
      return this;
    }

    public Builder setPublicAmount(final String publicAmount) {
      this.publicAmount = ZkInstructionUtils.canonicalU128(publicAmount, "publicAmount");
      return this;
    }

    public Builder setPublicAmount(final Number publicAmount) {
      return setPublicAmount(publicAmount == null ? null : publicAmount.toString());
    }

    public Builder setInputs(final List<byte[]> inputs) {
      this.inputs.clear();
      if (inputs == null || inputs.isEmpty()) {
        throw new IllegalArgumentException("inputs must contain at least one nullifier");
      }
      for (int i = 0; i < inputs.size(); i++) {
        this.inputs.add(
            ZkInstructionUtils.fixedNonZeroBytes(inputs.get(i), 32, "inputs[" + i + "]"));
      }
      return this;
    }

    public Builder addInput(final byte[] input) {
      inputs.add(
          ZkInstructionUtils.fixedNonZeroBytes(input, 32, "inputs[" + inputs.size() + "]"));
      return this;
    }

    public Builder setOutputs(final List<byte[]> outputs) {
      this.outputs.clear();
      if (outputs != null) {
        for (int i = 0; i < outputs.size(); i++) {
          this.outputs.add(
              ZkInstructionUtils.fixedNonZeroBytes(outputs.get(i), 32, "outputs[" + i + "]"));
        }
      }
      return this;
    }

    public Builder addOutput(final byte[] output) {
      outputs.add(
          ZkInstructionUtils.fixedNonZeroBytes(output, 32, "outputs[" + outputs.size() + "]"));
      return this;
    }

    public Builder setProof(final ProofAttachment proof) {
      this.proof = Objects.requireNonNull(proof, "proof");
      return this;
    }

    public Builder setRootHint(final byte[] rootHint) {
      this.rootHint =
          rootHint == null ? null : ZkInstructionUtils.fixedBytes(rootHint, 32, "rootHint");
      return this;
    }

    public UnshieldInstruction build() {
      if (asset == null) {
        throw new IllegalStateException("asset must be provided");
      }
      if (to == null) {
        throw new IllegalStateException("to must be provided");
      }
      if (publicAmount == null) {
        throw new IllegalStateException("publicAmount must be provided");
      }
      if (inputs.isEmpty()) {
        throw new IllegalStateException("inputs must contain at least one nullifier");
      }
      if (proof == null) {
        throw new IllegalStateException("proof must be provided");
      }
      return new UnshieldInstruction(this);
    }
  }
}
