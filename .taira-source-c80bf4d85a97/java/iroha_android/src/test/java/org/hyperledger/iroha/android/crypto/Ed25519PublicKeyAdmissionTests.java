package org.hyperledger.iroha.android.crypto;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.util.LinkedHashMap;
import java.util.Map;
import org.junit.Test;

public final class Ed25519PublicKeyAdmissionTests {

  @Test
  public void admitsValidPrimeOrderKeysAndRejectsTorsionAndMalformedEncodings() {
    assertTrue(
        Ed25519PublicKeyAdmission.isValid(
            hex("3B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29")));

    final Map<String, String> invalid = new LinkedHashMap<>();
    invalid.put("all-zero", "00".repeat(32));
    invalid.put("small-order identity", "01" + "00".repeat(31));
    invalid.put(
        "noncanonical identity",
        "EEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7F");
    invalid.put("invalid compressed point", "02".repeat(32));
    invalid.put("mixed torsion repeated-11", "11".repeat(32));
    invalid.put(
        "mixed torsion base-plus-torsion",
        "6AEBC0B955CE4A2F1344029986B775E6EA5C40F93F1112B86EC51678EB9DC0FB");
    for (final Map.Entry<String, String> entry : invalid.entrySet()) {
      assertFalse(entry.getKey(), Ed25519PublicKeyAdmission.isValid(hex(entry.getValue())));
    }
    assertFalse(Ed25519PublicKeyAdmission.isValid(null));
    assertFalse(Ed25519PublicKeyAdmission.isValid(new byte[31]));
  }

  private static byte[] hex(final String encoded) {
    if ((encoded.length() & 1) != 0) {
      throw new IllegalArgumentException("hex input must contain complete bytes");
    }
    final byte[] decoded = new byte[encoded.length() / 2];
    for (int index = 0; index < decoded.length; index++) {
      decoded[index] =
          (byte) Integer.parseInt(encoded.substring(index * 2, index * 2 + 2), 16);
    }
    return decoded;
  }
}
