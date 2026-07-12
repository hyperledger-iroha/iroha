package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;
import org.hyperledger.iroha.android.numeric.NumericV1;

/** Shared canonical quantity validation for asset and RWA instruction builders. */
final class InstructionQuantity {
  private InstructionQuantity() {}

  static String requireCanonical(final String value) {
    return NumericV1.QuantityValue.parseCanonical(Objects.requireNonNull(value, "quantity"))
        .toString();
  }
}
