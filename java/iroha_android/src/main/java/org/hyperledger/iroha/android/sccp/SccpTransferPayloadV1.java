package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.util.Arrays;

/** The sole value-moving payload shipped in exact SCCP V1. */
public final class SccpTransferPayloadV1 extends SccpPayloadV1 {
  private final int source;
  private final int destination;
  private final BigInteger nonce;
  private final long routeRevision;
  private final int assetHomeDomain;
  private final int assetIdCodec;
  private final byte[] assetId;
  private final BigInteger amount;
  private final int senderCodec;
  private final byte[] sender;
  private final int recipientCodec;
  private final byte[] recipient;
  private final int routeIdCodec;
  private final byte[] routeId;

  public SccpTransferPayloadV1(
      final int source,
      final int destination,
      final BigInteger nonce,
      final long routeRevision,
      final int assetHomeDomain,
      final int assetIdCodec,
      final byte[] assetId,
      final BigInteger amount,
      final int senderCodec,
      final byte[] sender,
      final int recipientCodec,
      final byte[] recipient,
      final int routeIdCodec,
      final byte[] routeId) {
    super(SccpHubMessageKindV1.TRANSFER);
    SccpV1.requireDomain(source, "source");
    SccpV1.requireDomain(destination, "destination");
    SccpV1.requireDomain(assetHomeDomain, "assetHomeDomain");
    if (source == destination) {
      throw new IllegalArgumentException("transfer endpoints must differ");
    }
    if (routeRevision <= 0 || routeRevision > 0xffff_ffffL) {
      throw new IllegalArgumentException("routeRevision must be a nonzero u32");
    }
    if (senderCodec != SccpV1.accountCodec(source)) {
      throw new IllegalArgumentException("sender codec does not match source domain");
    }
    if (recipientCodec != SccpV1.accountCodec(destination)) {
      throw new IllegalArgumentException("recipient codec does not match destination domain");
    }
    this.source = source;
    this.destination = destination;
    this.nonce = SccpV1.requireUnsigned(nonce, 64, "nonce");
    this.routeRevision = routeRevision;
    this.assetHomeDomain = assetHomeDomain;
    this.assetIdCodec = assetIdCodec;
    this.assetId = SccpV1.requireCodecValue(assetIdCodec, assetId, "assetId");
    this.amount = SccpV1.requireUnsigned(amount, 128, "amount");
    if (this.amount.signum() == 0) {
      throw new IllegalArgumentException("amount must be nonzero");
    }
    this.senderCodec = senderCodec;
    this.sender = SccpV1.requireCodecValue(senderCodec, sender, "sender");
    this.recipientCodec = recipientCodec;
    this.recipient = SccpV1.requireCodecValue(recipientCodec, recipient, "recipient");
    this.routeIdCodec = routeIdCodec;
    this.routeId = SccpV1.requireCodecValue(routeIdCodec, routeId, "routeId");
  }

  public SccpTransferPayloadV1(
      final int source,
      final int destination,
      final long nonce,
      final long routeRevision,
      final int assetHomeDomain,
      final int assetIdCodec,
      final byte[] assetId,
      final BigInteger amount,
      final int senderCodec,
      final byte[] sender,
      final int recipientCodec,
      final byte[] recipient,
      final int routeIdCodec,
      final byte[] routeId) {
    this(
        source,
        destination,
        BigInteger.valueOf(nonce),
        routeRevision,
        assetHomeDomain,
        assetIdCodec,
        assetId,
        amount,
        senderCodec,
        sender,
        recipientCodec,
        recipient,
        routeIdCodec,
        routeId);
  }

  @Override
  int sourceDomain() {
    return source;
  }

  @Override
  int targetDomain() {
    return destination;
  }

  public long routeRevision() {
    return routeRevision;
  }

  public byte[] assetId() {
    return Arrays.copyOf(assetId, assetId.length);
  }

  public byte[] sender() {
    return Arrays.copyOf(sender, sender.length);
  }

  public byte[] recipient() {
    return Arrays.copyOf(recipient, recipient.length);
  }

  public byte[] routeId() {
    return Arrays.copyOf(routeId, routeId.length);
  }

  @Override
  void encodeBody(final ByteArrayOutputStream out) {
    out.write(1);
    SccpV1.writeU32(out, source);
    SccpV1.writeU32(out, destination);
    SccpV1.writeUnsignedLe(out, nonce, 8);
    SccpV1.writeU32Bits(out, routeRevision);
    SccpV1.writeU32(out, assetHomeDomain);
    out.write(assetIdCodec);
    SccpV1.writeBytes(out, assetId);
    SccpV1.writeUnsignedLe(out, amount, 16);
    out.write(senderCodec);
    SccpV1.writeBytes(out, sender);
    out.write(recipientCodec);
    SccpV1.writeBytes(out, recipient);
    out.write(routeIdCodec);
    SccpV1.writeBytes(out, routeId);
  }
}
