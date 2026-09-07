package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;

/** Closed canonical SCCP payload. V1 contains only transfer. */
public abstract class SccpPayloadV1 {
  private static final int TRANSFER_DISCRIMINANT = 2;

  private final SccpHubMessageKindV1 kind;

  protected SccpPayloadV1(final SccpHubMessageKindV1 kind) {
    this.kind = kind;
  }

  abstract int sourceDomain();

  abstract int targetDomain();

  abstract void encodeBody(ByteArrayOutputStream out);

  public final SccpHubMessageKindV1 kind() {
    return kind;
  }

  /** Return the exact fixed-layout payload bytes used by consensus hashing. */
  public final byte[] canonicalBytes() {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(TRANSFER_DISCRIMINANT);
    encodeBody(out);
    return out.toByteArray();
  }
}
