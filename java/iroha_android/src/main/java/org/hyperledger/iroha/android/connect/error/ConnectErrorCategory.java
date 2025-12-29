package org.hyperledger.iroha.android.connect.error;

/** Canonical Connect error categories shared across SDKs. */
public enum ConnectErrorCategory {
  TRANSPORT,
  CODEC,
  AUTHORIZATION,
  TIMEOUT,
  QUEUE_OVERFLOW,
  INTERNAL
}
