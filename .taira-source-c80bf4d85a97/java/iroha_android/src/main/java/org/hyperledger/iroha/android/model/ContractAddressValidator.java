package org.hyperledger.iroha.android.model;

import java.util.Arrays;
import java.util.Objects;

/** Internal parser for canonical ABI V1 contract addresses accepted by Core. */
final class ContractAddressValidator {
  private static final int BECH32M_CHECKSUM = 0x2BC830A3;
  private static final int CONTRACT_ADDRESS_VERSION_V1 = 1;
  private static final int CONTRACT_ADDRESS_PAYLOAD_BYTES_V1 = 29;
  private static final int CHECKSUM_WORDS = 6;
  private static final int MAX_BECH32_LENGTH = 90;
  private static final int MAX_HRP_LENGTH = 83;
  private static final String BECH32_CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
  private static final int[] BECH32_GENERATORS = {
    0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3
  };

  private ContractAddressValidator() {}

  static String requireCanonicalV1(final String value) {
    final String nonNull = Objects.requireNonNull(value, "contractAddress");
    if (nonNull.trim().isEmpty()) {
      throw invalid("contractAddress must not be blank");
    }
    if (!nonNull.trim().equals(nonNull)) {
      throw invalid("contractAddress must not contain surrounding whitespace");
    }
    if (nonNull.length() > MAX_BECH32_LENGTH) {
      throw invalid("contractAddress must not exceed " + MAX_BECH32_LENGTH + " characters");
    }
    for (int index = 0; index < nonNull.length(); index++) {
      final char character = nonNull.charAt(index);
      if (character < 33 || character > 126 || (character >= 'A' && character <= 'Z')) {
        throw invalid("contractAddress must be canonical lowercase Bech32m");
      }
    }

    final int separator = nonNull.lastIndexOf('1');
    if (separator < 1
        || separator > MAX_HRP_LENGTH
        || nonNull.length() - separator - 1 < CHECKSUM_WORDS) {
      throw invalid("contractAddress must contain a valid Bech32m human-readable prefix");
    }
    final String hrp = nonNull.substring(0, separator);
    final int[] data = new int[nonNull.length() - separator - 1];
    for (int index = 0; index < data.length; index++) {
      data[index] = BECH32_CHARSET.indexOf(nonNull.charAt(separator + 1 + index));
      if (data[index] < 0) {
        throw invalid("contractAddress contains an invalid Bech32m character");
      }
    }
    if (bech32Polymod(hrp, data) != BECH32M_CHECKSUM) {
      throw invalid("contractAddress has an invalid Bech32m checksum");
    }

    final byte[] payload = decodeBase32(Arrays.copyOf(data, data.length - CHECKSUM_WORDS));
    if (payload.length != CONTRACT_ADDRESS_PAYLOAD_BYTES_V1) {
      throw invalid(
          "contractAddress must contain a "
              + CONTRACT_ADDRESS_PAYLOAD_BYTES_V1
              + "-byte V1 payload");
    }
    if ((payload[0] & 0xFF) != CONTRACT_ADDRESS_VERSION_V1) {
      throw invalid("contractAddress uses an unsupported payload version");
    }
    return nonNull;
  }

  private static int bech32Polymod(final String hrp, final int[] data) {
    int checksum = 1;
    for (int index = 0; index < hrp.length(); index++) {
      checksum = polymodStep(checksum, hrp.charAt(index) >>> 5);
    }
    checksum = polymodStep(checksum, 0);
    for (int index = 0; index < hrp.length(); index++) {
      checksum = polymodStep(checksum, hrp.charAt(index) & 0x1F);
    }
    for (final int value : data) {
      checksum = polymodStep(checksum, value);
    }
    return checksum;
  }

  private static int polymodStep(final int checksum, final int value) {
    final int top = checksum >>> 25;
    int next = ((checksum & 0x01FF_FFFF) << 5) ^ value;
    for (int index = 0; index < BECH32_GENERATORS.length; index++) {
      if (((top >>> index) & 1) != 0) {
        next ^= BECH32_GENERATORS[index];
      }
    }
    return next;
  }

  private static byte[] decodeBase32(final int[] words) {
    final byte[] decoded = new byte[words.length * 5 / 8];
    int accumulator = 0;
    int bits = 0;
    int outputIndex = 0;
    for (final int word : words) {
      accumulator = ((accumulator << 5) | word) & 0x0FFF;
      bits += 5;
      if (bits >= 8) {
        bits -= 8;
        decoded[outputIndex++] = (byte) ((accumulator >>> bits) & 0xFF);
      }
    }
    if (bits >= 5 || ((accumulator << (8 - bits)) & 0xFF) != 0) {
      throw invalid("contractAddress has non-canonical Bech32m padding");
    }
    return outputIndex == decoded.length ? decoded : Arrays.copyOf(decoded, outputIndex);
  }

  private static IllegalArgumentException invalid(final String message) {
    return new IllegalArgumentException(message);
  }
}
