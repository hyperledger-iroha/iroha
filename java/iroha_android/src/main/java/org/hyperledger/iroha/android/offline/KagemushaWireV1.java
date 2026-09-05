// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Exact size contract and opaque text envelope for KAGEMUSHA V1.
 *
 * <p>This codec does not interpret or validate Norito. Callers must pass bytes produced by the
 * canonical typed encoder and must run the typed decoder and cryptographic verifier after text
 * decoding. Successful text decoding grants no monetary authority.
 */
public final class KagemushaWireV1 {
  public static final int WIRE_VERSION =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.WIRE_VERSION;
  public static final int DEVICE_LIFECYCLE_VERSION =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.DEVICE_LIFECYCLE_VERSION;
  public static final String HANDOFF_CAPABILITY =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.HANDOFF_CAPABILITY;
  public static final String TEXT_PREFIX =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.TEXT_PREFIX;
  public static final int MAXIMUM_ASSET_SCALE =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_ASSET_SCALE;
  public static final long REQUEST_MAX_TTL_MS =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.REQUEST_MAX_TTL_MS;
  public static final int MAXIMUM_AGGREGATE_STATE_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_AGGREGATE_STATE_BYTES;
  public static final int MAXIMUM_PAYMENT_REQUEST_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES;
  public static final int MAXIMUM_PAYMENT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PAYMENT_BYTES;
  public static final int MAXIMUM_ACKNOWLEDGEMENT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES;
  public static final int MAXIMUM_MINT_AUTHORIZATION_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES;
  public static final int MAXIMUM_MINT_CREDIT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES;
  public static final int MAXIMUM_REDEMPTION_VOUCHER_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES;
  public static final int MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES;
  public static final int MAXIMUM_PAYMENT_TEXT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PAYMENT_TEXT_BYTES;
  public static final int MAXIMUM_ACKNOWLEDGEMENT_TEXT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_TEXT_BYTES;
  public static final int MAXIMUM_MINT_AUTHORIZATION_TEXT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_TEXT_BYTES;
  public static final int MAXIMUM_MINT_CREDIT_TEXT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_MINT_CREDIT_TEXT_BYTES;
  public static final int MAXIMUM_REDEMPTION_VOUCHER_TEXT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_REDEMPTION_VOUCHER_TEXT_BYTES;
  public static final int MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES;
  public static final int MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES;

  public static final int MAXIMUM_PAIRED_PROOF_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES;
  public static final int MAXIMUM_REDEMPTION_PROOF_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES;
  public static final int MAXIMUM_COMMIT_CERTIFICATE_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_COMMIT_CERTIFICATE_BYTES;
  public static final int MAXIMUM_CURRENT_PROOFS_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_CURRENT_PROOFS_BYTES;
  public static final int MAXIMUM_PARITY_PROOF_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_PARITY_PROOF_BYTES;
  public static final int HISTORY_ACCUMULATOR_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES;
  public static final int MAXIMUM_ENCRYPTED_CREDIT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES;
  public static final int MAXIMUM_CREDIT_OPENING_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.MAXIMUM_CREDIT_OPENING_BYTES;
  public static final int CREDIT_OPENING_CANONICAL_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.CREDIT_OPENING_CANONICAL_BYTES;
  public static final int X25519_PUBLIC_KEY_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.X25519_PUBLIC_KEY_BYTES;
  public static final int XCHACHA20_POLY1305_NONCE_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.XCHACHA20_POLY1305_NONCE_BYTES;
  public static final int XCHACHA20_POLY1305_TAG_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.XCHACHA20_POLY1305_TAG_BYTES;
  public static final int ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES;
  public static final int PAYMENT_OUTBOX_MIN_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.PAYMENT_OUTBOX_MIN_BYTES;
  public static final int REDEMPTION_OUTBOX_MIN_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.REDEMPTION_OUTBOX_MIN_BYTES;
  public static final int INBOX_STAGE_MIN_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaWireV1.INBOX_STAGE_MIN_BYTES;
  private KagemushaWireV1() {}

  /** Encode bounded canonical bytes as exact unpadded base64url with the {@code kgm1:} prefix. */
  public static String encodeText(
      final KagemushaWirePayloadKindV1 kind,
      final byte[] canonicalPayload) {
    return org.hyperledger.iroha.sdk.offline.KagemushaWireV1.encodeText(
        Objects.requireNonNull(kind, "kind").canonicalKind(),
        Objects.requireNonNull(canonicalPayload, "canonicalPayload").clone());
  }

  /** Decode one strict {@code kgm1:} envelope into opaque canonical bytes. */
  public static byte[] decodeText(
      final KagemushaWirePayloadKindV1 kind,
      final String text) {
    return org.hyperledger.iroha.sdk.offline.KagemushaWireV1.decodeText(
        Objects.requireNonNull(kind, "kind").canonicalKind(),
        Objects.requireNonNull(text, "text"));
  }
}
