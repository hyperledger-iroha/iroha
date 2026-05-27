package org.hyperledger.iroha.android.offline;

/** Transport selected by the Offline Bearer Cash v1 payload-size policy. */
public enum OfflineBearerCashTransport {
  STATIC_QR,
  STREAMING_QR,
  FRAMED_BYTE_TRANSPORT
}
