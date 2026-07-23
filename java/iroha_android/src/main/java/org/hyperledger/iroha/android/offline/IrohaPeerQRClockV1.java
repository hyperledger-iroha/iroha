package org.hyperledger.iroha.android.offline;

/** Injectable scanner clock; callers should supply a monotonic millisecond source. */
@FunctionalInterface
public interface IrohaPeerQRClockV1 {
  long nowMillis();
}
