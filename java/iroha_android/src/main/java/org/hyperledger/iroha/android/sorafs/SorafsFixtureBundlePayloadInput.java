package org.hyperledger.iroha.android.sorafs;

/** Immutable typed input for heterogeneous SoraFS fixture-bundle validation. */
public final class SorafsFixtureBundlePayloadInput {
  private final SorafsFixtureBundlePayloadKind kind;
  private final byte[] noritoBytes;
  private final String label;

  /** Create an input using the payload kind's canonical diagnostic label. */
  public SorafsFixtureBundlePayloadInput(
      final SorafsFixtureBundlePayloadKind kind, final byte[] noritoBytes) {
    this(kind, noritoBytes, null);
  }

  /** Create an input with an explicit diagnostic label. */
  public SorafsFixtureBundlePayloadInput(
      final SorafsFixtureBundlePayloadKind kind,
      final byte[] noritoBytes,
      final String label) {
    if (kind == null) {
      throw new IllegalArgumentException("kind must be provided");
    }
    if (noritoBytes == null) {
      throw new IllegalArgumentException("noritoBytes must be provided");
    }
    this.kind = kind;
    this.noritoBytes = noritoBytes.clone();
    this.label = label;
  }

  /** Return the canonical payload kind. */
  public SorafsFixtureBundlePayloadKind kind() {
    return kind;
  }

  /** Return a detached copy of the canonical Norito bytes. */
  public byte[] noritoBytes() {
    return noritoBytes.clone();
  }

  /** Return the explicit label, or {@code null} to select the canonical default. */
  public String label() {
    return label;
  }
}
