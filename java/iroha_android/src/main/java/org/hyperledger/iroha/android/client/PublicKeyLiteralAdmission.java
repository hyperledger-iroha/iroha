package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.hyperledger.iroha.android.address.PublicKeyCodec;

final class PublicKeyLiteralAdmission {

  private PublicKeyLiteralAdmission() {}

  static String requireValid(final String value, final String fieldName) {
    Objects.requireNonNull(value, fieldName);
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be empty");
    }
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(
          fieldName + " must not contain surrounding whitespace");
    }
    if (PublicKeyCodec.decodePublicKeyLiteral(value) == null) {
      throw new IllegalArgumentException(fieldName + " is not a valid public key literal");
    }
    return value;
  }
}
