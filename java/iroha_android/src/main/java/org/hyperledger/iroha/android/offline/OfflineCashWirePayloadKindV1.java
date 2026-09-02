// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

/** Canonical Offline Cash V1 value whose opaque Norito bytes are transported. */
public enum OfflineCashWirePayloadKindV1 {
  PAYMENT_REQUEST(
      OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES,
      OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES),
  ACCEPTANCE_INTENT(
      OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_BYTES,
      OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_TEXT_BYTES),
  ACCEPTANCE_INTENT_AUTHORIZATION(
      OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_BYTES,
      OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_BYTES),
  ACCEPTANCE_TICKET(
      OfflineCashWireV1.MAXIMUM_ACCEPTANCE_TICKET_BYTES,
      OfflineCashWireV1.MAXIMUM_ACCEPTANCE_TICKET_TEXT_BYTES),
  PAYMENT(OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES, OfflineCashWireV1.MAXIMUM_PAYMENT_TEXT_BYTES),
  ACKNOWLEDGEMENT(
      OfflineCashWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES,
      OfflineCashWireV1.MAXIMUM_ACKNOWLEDGEMENT_TEXT_BYTES),
  MINT_AUTHORIZATION(
      OfflineCashWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES,
      OfflineCashWireV1.MAXIMUM_MINT_AUTHORIZATION_TEXT_BYTES),
  MINT_CREDIT(
      OfflineCashWireV1.MAXIMUM_MINT_CREDIT_BYTES,
      OfflineCashWireV1.MAXIMUM_MINT_CREDIT_TEXT_BYTES),
  REDEMPTION_VOUCHER(
      OfflineCashWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES,
      OfflineCashWireV1.MAXIMUM_REDEMPTION_VOUCHER_TEXT_BYTES);

  private final int maximumRawBytes;
  private final int maximumTextBytes;

  OfflineCashWirePayloadKindV1(final int maximumRawBytes, final int maximumTextBytes) {
    this.maximumRawBytes = maximumRawBytes;
    this.maximumTextBytes = maximumTextBytes;
  }

  /** Return the maximum canonical Norito bytes for this value. */
  public int maximumRawBytes() {
    return maximumRawBytes;
  }

  /** Return the maximum complete {@code oc1:} text bytes for this value. */
  public int maximumTextBytes() {
    return maximumTextBytes;
  }
}
