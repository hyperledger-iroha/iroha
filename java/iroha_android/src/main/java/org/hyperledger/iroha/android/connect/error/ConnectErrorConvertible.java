package org.hyperledger.iroha.android.connect.error;

/** Types that can be converted into {@link ConnectError}. */
public interface ConnectErrorConvertible {
  ConnectError toConnectError();
}
