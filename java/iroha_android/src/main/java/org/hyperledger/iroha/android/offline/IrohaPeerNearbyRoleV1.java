package org.hyperledger.iroha.android.offline;

/** Stable IPN1 endpoint role exposed without Kotlin-specific types. */
public enum IrohaPeerNearbyRoleV1 {
  SENDER(1),
  RECEIVER(2);

  private final int code;

  IrohaPeerNearbyRoleV1(final int code) {
    this.code = code;
  }

  public int code() {
    return code;
  }

  public IrohaPeerNearbyRoleV1 peer() {
    return this == SENDER ? RECEIVER : SENDER;
  }

  static IrohaPeerNearbyRoleV1 fromShared(
      final org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1 role) {
    return role == null ? null : role.getCode() == 1 ? SENDER : RECEIVER;
  }

  org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1 toShared() {
    return org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.fromCode(code);
  }
}
