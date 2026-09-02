// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Base64;
import java.util.Objects;

/** Exact size contract and opaque text envelope for Kagemusha V1.
 *
 * <p>This codec does not interpret or validate Norito. Callers must pass bytes produced by the
 * canonical typed encoder and must run the typed decoder and cryptographic verifier after text
 * decoding. Successful text decoding grants no monetary authority.
 */
public final class KagemushaWireV1 {
  public static final int WIRE_VERSION = 1;
  public static final int DEVICE_LIFECYCLE_VERSION = 1;
  public static final String HANDOFF_CAPABILITY = "kagemusha_handoff_v1";
  public static final String TEXT_PREFIX = "kgm1:";
  public static final int MAXIMUM_ASSET_SCALE = 28;
  public static final long REQUEST_MAX_TTL_MS = 5L * 60L * 1_000L;

  public static final int MAXIMUM_AGGREGATE_STATE_BYTES = 768;
  public static final int MAXIMUM_PAYMENT_REQUEST_BYTES = 1_024;
  public static final int MAXIMUM_PAYMENT_BYTES = 7_936;
  public static final int MAXIMUM_ACKNOWLEDGEMENT_BYTES = 512;
  public static final int MAXIMUM_MINT_AUTHORIZATION_BYTES = 7_936;
  public static final int MAXIMUM_MINT_CREDIT_BYTES = 7_936;
  public static final int MAXIMUM_REDEMPTION_VOUCHER_BYTES = 7_936;
  public static final int MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES = 1_371;
  public static final int MAXIMUM_PAYMENT_TEXT_BYTES = 10_587;
  public static final int MAXIMUM_ACKNOWLEDGEMENT_TEXT_BYTES = 688;
  public static final int MAXIMUM_MINT_AUTHORIZATION_TEXT_BYTES = 10_587;
  public static final int MAXIMUM_MINT_CREDIT_TEXT_BYTES = 10_587;
  public static final int MAXIMUM_REDEMPTION_VOUCHER_TEXT_BYTES = 10_587;
  public static final int MAXIMUM_SESSION_RAW_BYTES = 9_211;
  public static final int MAXIMUM_SESSION_TEXT_BYTES = 12_288;

  public static final int MAXIMUM_PAIRED_PROOF_BYTES = 6_528;
  public static final int MAXIMUM_CURRENT_PROOFS_BYTES = 4_990;
  public static final int MAXIMUM_PARITY_PROOF_BYTES = 2_495;
  public static final int HISTORY_ACCUMULATOR_BYTES = 544;
  public static final int MAXIMUM_ENCRYPTED_CREDIT_BYTES = 384;
  public static final int MAXIMUM_CREDIT_OPENING_BYTES = 256;
  public static final int CREDIT_OPENING_CANONICAL_BYTES = 200;
  public static final int X25519_PUBLIC_KEY_BYTES = 32;
  public static final int XCHACHA20_POLY1305_NONCE_BYTES = 24;
  public static final int XCHACHA20_POLY1305_TAG_BYTES = 16;
  public static final int ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES =
      CREDIT_OPENING_CANONICAL_BYTES + XCHACHA20_POLY1305_TAG_BYTES;
  public static final int PAYMENT_OUTBOX_MIN_BYTES = 26_112;
  public static final int REDEMPTION_OUTBOX_MIN_BYTES = 26_112;

  private KagemushaWireV1() {}

  /** Encode bounded canonical bytes as exact unpadded base64url with the {@code kgm1:} prefix. */
  public static String encodeText(
      final KagemushaWirePayloadKindV1 kind,
      final byte[] canonicalPayload) {
    Objects.requireNonNull(kind, "kind");
    Objects.requireNonNull(canonicalPayload, "canonicalPayload");
    require(canonicalPayload.length != 0, "Kagemusha V1 payload is empty");
    require(
        canonicalPayload.length <= kind.maximumRawBytes(),
        "Kagemusha V1 payload exceeds " + kind.maximumRawBytes() + " bytes");
    final String text =
        TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(canonicalPayload);
    require(
        text.length() <= kind.maximumTextBytes(),
        "Kagemusha V1 text exceeds " + kind.maximumTextBytes() + " bytes");
    return text;
  }

  /** Decode one strict {@code kgm1:} envelope into opaque canonical bytes. */
  public static byte[] decodeText(
      final KagemushaWirePayloadKindV1 kind,
      final String text) {
    Objects.requireNonNull(kind, "kind");
    Objects.requireNonNull(text, "text");
    require(
        text.length() <= kind.maximumTextBytes(),
        "Kagemusha V1 text exceeds " + kind.maximumTextBytes() + " bytes");
    require(text.startsWith(TEXT_PREFIX), "Kagemusha V1 text prefix is invalid");
    final String body = text.substring(TEXT_PREFIX.length());
    require(!body.isEmpty(), "Kagemusha V1 payload is empty");
    for (int index = 0; index < body.length(); index++) {
      require(isBase64UrlCharacter(body.charAt(index)), "Kagemusha V1 text is invalid");
    }
    require(body.length() % 4 != 1, "Kagemusha V1 base64url is non-canonical");
    final byte[] raw;
    try {
      raw = Base64.getUrlDecoder().decode(body);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException("Kagemusha V1 base64url is invalid", error);
    }
    require(
        raw.length <= kind.maximumRawBytes(),
        "Kagemusha V1 payload exceeds " + kind.maximumRawBytes() + " bytes");
    require(encodeText(kind, raw).equals(text), "Kagemusha V1 base64url is non-canonical");
    return raw;
  }

  private static boolean isBase64UrlCharacter(final char character) {
    return (character >= 'A' && character <= 'Z')
        || (character >= 'a' && character <= 'z')
        || (character >= '0' && character <= '9')
        || character == '-'
        || character == '_';
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalArgumentException(message);
    }
  }
}
