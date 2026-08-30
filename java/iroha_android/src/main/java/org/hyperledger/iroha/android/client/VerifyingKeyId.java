package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;

/** Exact identifier returned by the Torii verifying-key registry id projection. */
public final class VerifyingKeyId {
  private final String backend;
  private final String name;

  VerifyingKeyId(final String backend, final String name) {
    this.backend = Objects.requireNonNull(backend, "backend");
    this.name = Objects.requireNonNull(name, "name");
  }

  /** Exact verifier-registry backend label. */
  public String backend() {
    return backend;
  }

  /** Exact verifying-key registry name. */
  public String name() {
    return name;
  }

  /** Low-level proof engine bound to this exact verifier-registry label. */
  public VerifyingKeyBackendTag engine() {
    final VerifyingKeyBackendTag engine =
        VerifyingKeyBackendTag.verifierBackendRegistryTagV1(backend);
    if (engine == null) {
      throw new IllegalStateException("unsupported verifier-registry backend " + backend);
    }
    return engine;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof VerifyingKeyId)) {
      return false;
    }
    final VerifyingKeyId that = (VerifyingKeyId) other;
    return backend.equals(that.backend) && name.equals(that.name);
  }

  @Override
  public int hashCode() {
    return Objects.hash(backend, name);
  }

  @Override
  public String toString() {
    return backend + ":" + name;
  }
}
