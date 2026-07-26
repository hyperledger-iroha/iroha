package org.hyperledger.iroha.android.model.instructions;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/**
 * Typed builder for {@code RegisterPeerWithPop} instructions.
 *
 * <p>The instruction carries a peer identifier (public key) alongside a BLS proof-of-possession for
 * the peer. The proof is stored as raw bytes and serialised via base64 in the Norito arguments, mirroring
 * the wire format expected by the Rust data model.</p>
 */
public final class RegisterPeerWithPopInstruction implements InstructionTemplate {

  private final String peerPublicKey;
  private final byte[] proofOfPossession;
  private final Map<String, String> arguments;

  private RegisterPeerWithPopInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterPeerWithPopInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.peerPublicKey = builder.peerPublicKey;
    this.proofOfPossession = builder.proofOfPossession.clone();
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the peer public key encoded in multihash format. */
  public String peerPublicKey() {
    return peerPublicKey;
  }

  /** Returns the proof-of-possession bytes. */
  public byte[] proofOfPossession() {
    return proofOfPossession.clone();
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterPeerWithPopInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setPeerPublicKey(require(arguments, "peer"))
            .setProofOfPossession(decodeBase64(require(arguments, "pop")));
    return new RegisterPeerWithPopInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static byte[] decodeBase64(final String value) {
    try {
      return Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("pop must be base64 encoded", ex);
    }
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RegisterPeerWithPopInstruction other)) {
      return false;
    }
    return peerPublicKey.equals(other.peerPublicKey)
        && java.util.Arrays.equals(proofOfPossession, other.proofOfPossession);
  }

  @Override
  public int hashCode() {
    return Objects.hash(peerPublicKey, java.util.Arrays.hashCode(proofOfPossession));
  }

  public static final class Builder {
    private String peerPublicKey;
    private byte[] proofOfPossession;

    private Builder() {}

    public Builder setPeerPublicKey(final String peerPublicKey) {
      if (peerPublicKey == null || peerPublicKey.isBlank()) {
        throw new IllegalArgumentException("peerPublicKey must not be blank");
      }
      this.peerPublicKey = peerPublicKey;
      return this;
    }

    public Builder setProofOfPossession(final byte[] proofOfPossession) {
      this.proofOfPossession =
          Objects.requireNonNull(proofOfPossession, "proofOfPossession").clone();
      if (this.proofOfPossession.length == 0) {
        throw new IllegalArgumentException("proofOfPossession must not be empty");
      }
      return this;
    }

    public RegisterPeerWithPopInstruction build() {
      if (peerPublicKey == null) {
        throw new IllegalStateException("peerPublicKey must be provided");
      }
      if (proofOfPossession == null || proofOfPossession.length == 0) {
        throw new IllegalStateException("proofOfPossession must be provided");
      }
      return new RegisterPeerWithPopInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", "RegisterPeerWithPop");
      args.put("peer", peerPublicKey);
      args.put("pop", Base64.getEncoder().encodeToString(proofOfPossession));
      return args;
    }
  }
}
