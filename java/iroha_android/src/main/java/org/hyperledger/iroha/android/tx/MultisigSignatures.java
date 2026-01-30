package org.hyperledger.iroha.android.tx;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/** Collection of multisig signatures attached to a transaction. */
public final class MultisigSignatures {
  private final List<MultisigSignature> signatures;

  private MultisigSignatures(final List<MultisigSignature> signatures) {
    Objects.requireNonNull(signatures, "signatures");
    this.signatures = List.copyOf(signatures);
    if (this.signatures.contains(null)) {
      throw new IllegalArgumentException("signatures must not contain null entries");
    }
  }

  /** Construct a bundle from the provided signatures. */
  public static MultisigSignatures of(final List<MultisigSignature> signatures) {
    return new MultisigSignatures(signatures);
  }

  /** Construct a bundle from the provided signatures. */
  public static MultisigSignatures of(final MultisigSignature... signatures) {
    Objects.requireNonNull(signatures, "signatures");
    final List<MultisigSignature> list = new ArrayList<>(signatures.length);
    for (final MultisigSignature signature : signatures) {
      list.add(signature);
    }
    return new MultisigSignatures(list);
  }

  /** Immutable list of signatures. */
  public List<MultisigSignature> signatures() {
    return signatures;
  }
}
