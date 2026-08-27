package org.hyperledger.iroha.android.alias;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/** Canonical account-faucet claim prepared into one exact transaction. */
public final class AccountFaucetClaimV1 extends AliasJsonValue {
  private static final byte[] HASH_DOMAIN =
      "iroha:accounts:faucet:claim:v1\0".getBytes(StandardCharsets.UTF_8);

  private final String accountId;
  private final BigInteger powAnchorHeight;
  private final String powNonceHex;

  /** Constructs an exact faucet claim with a required positive proof anchor and nonce. */
  public AccountFaucetClaimV1(
      final String accountId,
      final BigInteger powAnchorHeight,
      final String powNonceHex) {
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    if (powAnchorHeight == null) {
      throw new IllegalArgumentException("powAnchorHeight is required");
    }
    this.powAnchorHeight = AliasNameSupport.requireU64(powAnchorHeight, "powAnchorHeight");
    if (this.powAnchorHeight.signum() <= 0) {
      throw new IllegalArgumentException("powAnchorHeight must be positive");
    }
    if (powNonceHex == null) {
      throw new IllegalArgumentException("powNonceHex is required");
    }
    if (powNonceHex.isEmpty() || powNonceHex.length() > 64 || (powNonceHex.length() & 1) != 0) {
      throw new IllegalArgumentException(
          "powNonceHex must contain 1..32 bytes of lowercase hexadecimal");
    }
    for (int index = 0; index < powNonceHex.length(); index++) {
      final char value = powNonceHex.charAt(index);
      if (!((value >= '0' && value <= '9') || (value >= 'a' && value <= 'f'))) {
        throw new IllegalArgumentException(
            "powNonceHex must contain 1..32 bytes of lowercase hexadecimal");
      }
    }
    this.powNonceHex = powNonceHex;
  }

  public String accountId() { return accountId; }
  public BigInteger powAnchorHeight() { return powAnchorHeight; }
  public String powNonceHex() { return powNonceHex; }

  /** Returns the domain-separated semantic hash committed by a prepared faucet transaction. */
  public String semanticHashHex() {
    final ByteArrayOutputStream encoded = new ByteArrayOutputStream();
    writeNoritoField(encoded, encodeNoritoString(accountId));
    writeNoritoField(encoded, encodeU64LittleEndian(powAnchorHeight));
    writeNoritoField(encoded, encodeNoritoString(powNonceHex));
    final byte[] payload = encoded.toByteArray();
    final byte[] preimage = new byte[HASH_DOMAIN.length + payload.length];
    System.arraycopy(HASH_DOMAIN, 0, preimage, 0, HASH_DOMAIN.length);
    System.arraycopy(payload, 0, preimage, HASH_DOMAIN.length, payload.length);
    return lowerHex(IrohaHash.prehash(preimage));
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("account_id", accountId);
    map.put("pow_anchor_height", powAnchorHeight);
    map.put("pow_nonce_hex", powNonceHex);
    return map;
  }

  private static byte[] encodeNoritoString(final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    writeCompactLength(output, bytes.length);
    output.write(bytes, 0, bytes.length);
    return output.toByteArray();
  }

  private static byte[] encodeU64LittleEndian(final BigInteger value) {
    final byte[] output = new byte[8];
    for (int index = 0; index < output.length; index++) {
      output[index] = value.shiftRight(index * 8).and(BigInteger.valueOf(0xffL)).byteValue();
    }
    return output;
  }

  private static void writeNoritoField(
      final ByteArrayOutputStream output, final byte[] value) {
    writeCompactLength(output, value.length);
    output.write(value, 0, value.length);
  }

  private static void writeCompactLength(
      final ByteArrayOutputStream output, final long raw) {
    long value = raw;
    do {
      int next = (int) (value & 0x7fL);
      value >>>= 7;
      if (value != 0L) next |= 0x80;
      output.write(next);
    } while (value != 0L);
  }

  private static String lowerHex(final byte[] bytes) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final char[] output = new char[bytes.length * 2];
    for (int index = 0; index < bytes.length; index++) {
      final int value = bytes[index] & 0xff;
      output[index * 2] = digits[value >>> 4];
      output[index * 2 + 1] = digits[value & 0xf];
    }
    return new String(output);
  }
}
