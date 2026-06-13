package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;

/** Reference to a registered verifier key used by a proof attachment. */
public final class ProofVerifierKeyRef {
  private final String backend;
  private final String name;

  public ProofVerifierKeyRef(final String backend, final String name) {
    this.backend = ZkInstructionUtils.requirePortableComponent(backend, "backend");
    this.name = ZkInstructionUtils.requirePortableComponent(name, "name");
  }

  public String backend() {
    return backend;
  }

  public String name() {
    return name;
  }

  public String wireId() {
    return backend + ":" + name;
  }

  public static ProofVerifierKeyRef fromWireId(final String wireId) {
    final String text = ZkInstructionUtils.requireText(wireId, "verifyingKeyId");
    final int split = text.indexOf(':');
    if (split <= 0 || split >= text.length() - 1) {
      throw new IllegalArgumentException("verifyingKeyId must use backend:name syntax");
    }
    return new ProofVerifierKeyRef(text.substring(0, split), text.substring(split + 1));
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ProofVerifierKeyRef other)) {
      return false;
    }
    return backend.equals(other.backend) && name.equals(other.name);
  }

  @Override
  public int hashCode() {
    return Objects.hash(backend, name);
  }
}
