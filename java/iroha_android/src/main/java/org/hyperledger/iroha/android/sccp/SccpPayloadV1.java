package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.util.Arrays;

/** Canonical closed first-release SCCP payload hierarchy. */
public abstract class SccpPayloadV1 {
  private final int discriminant;
  private final SccpHubMessageKindV1 kind;

  SccpPayloadV1(final int discriminant, final SccpHubMessageKindV1 kind) {
    this.discriminant = discriminant;
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
    out.write(discriminant);
    encodeBody(out);
    return out.toByteArray();
  }

  /** Asset registration payload. */
  public static final class AssetRegister extends SccpPayloadV1 {
    private final int target;
    private final int home;
    private final BigInteger nonce;
    private final int assetIdCodec;
    private final byte[] assetId;
    private final int decimals;

    public AssetRegister(
        final int target,
        final int home,
        final BigInteger nonce,
        final int assetIdCodec,
        final byte[] assetId,
        final int decimals) {
      super(0, SccpHubMessageKindV1.ASSET_REGISTER);
      SccpV1.requireDomain(target, "target");
      SccpV1.requireDomain(home, "home");
      if (target == home) {
        throw new IllegalArgumentException("asset registration endpoints must differ");
      }
      if (decimals < 0 || decimals > 255) {
        throw new IllegalArgumentException("decimals must fit u8");
      }
      this.target = target;
      this.home = home;
      this.nonce = SccpV1.requireUnsigned(nonce, 64, "nonce");
      this.assetIdCodec = assetIdCodec;
      this.assetId = SccpV1.requireCodecValue(assetIdCodec, assetId, "assetId");
      this.decimals = decimals;
    }

    public AssetRegister(
        final int target,
        final int home,
        final long nonce,
        final int assetIdCodec,
        final byte[] assetId,
        final int decimals) {
      this(target, home, BigInteger.valueOf(nonce), assetIdCodec, assetId, decimals);
    }

    @Override
    int sourceDomain() {
      return home;
    }

    @Override
    int targetDomain() {
      return target;
    }

    public byte[] assetId() {
      return Arrays.copyOf(assetId, assetId.length);
    }

    @Override
    void encodeBody(final ByteArrayOutputStream out) {
      out.write(1);
      SccpV1.writeU32(out, target);
      SccpV1.writeU32(out, home);
      SccpV1.writeUnsignedLe(out, nonce, 8);
      out.write(assetIdCodec);
      SccpV1.writeBytes(out, assetId);
      out.write(decimals);
    }
  }

  /** Route activation payload. */
  public static final class RouteActivate extends SccpPayloadV1 {
    private final int source;
    private final int target;
    private final BigInteger nonce;
    private final int assetIdCodec;
    private final byte[] assetId;
    private final int routeIdCodec;
    private final byte[] routeId;

    public RouteActivate(
        final int source,
        final int target,
        final BigInteger nonce,
        final int assetIdCodec,
        final byte[] assetId,
        final int routeIdCodec,
        final byte[] routeId) {
      super(1, SccpHubMessageKindV1.ROUTE_ACTIVATE);
      SccpV1.requireDomain(source, "source");
      SccpV1.requireDomain(target, "target");
      if (source == target) {
        throw new IllegalArgumentException("route endpoints must differ");
      }
      this.source = source;
      this.target = target;
      this.nonce = SccpV1.requireUnsigned(nonce, 64, "nonce");
      this.assetIdCodec = assetIdCodec;
      this.assetId = SccpV1.requireCodecValue(assetIdCodec, assetId, "assetId");
      this.routeIdCodec = routeIdCodec;
      this.routeId = SccpV1.requireCodecValue(routeIdCodec, routeId, "routeId");
    }

    public RouteActivate(
        final int source,
        final int target,
        final long nonce,
        final int assetIdCodec,
        final byte[] assetId,
        final int routeIdCodec,
        final byte[] routeId) {
      this(
          source,
          target,
          BigInteger.valueOf(nonce),
          assetIdCodec,
          assetId,
          routeIdCodec,
          routeId);
    }

    @Override
    int sourceDomain() {
      return source;
    }

    @Override
    int targetDomain() {
      return target;
    }

    @Override
    void encodeBody(final ByteArrayOutputStream out) {
      out.write(1);
      SccpV1.writeU32(out, source);
      SccpV1.writeU32(out, target);
      SccpV1.writeUnsignedLe(out, nonce, 8);
      out.write(assetIdCodec);
      SccpV1.writeBytes(out, assetId);
      out.write(routeIdCodec);
      SccpV1.writeBytes(out, routeId);
    }
  }

  /** The sole value-moving SCCP payload in V1. */
  public static final class Transfer extends SccpPayloadV1 {
    private final int source;
    private final int destination;
    private final BigInteger nonce;
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

    public Transfer(
        final int source,
        final int destination,
        final BigInteger nonce,
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
      super(2, SccpHubMessageKindV1.TRANSFER);
      SccpV1.requireDomain(source, "source");
      SccpV1.requireDomain(destination, "destination");
      SccpV1.requireDomain(assetHomeDomain, "assetHomeDomain");
      if (source == destination) {
        throw new IllegalArgumentException("transfer endpoints must differ");
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

    @Override
    int sourceDomain() {
      return source;
    }

    @Override
    int targetDomain() {
      return destination;
    }

    @Override
    void encodeBody(final ByteArrayOutputStream out) {
      out.write(1);
      SccpV1.writeU32(out, source);
      SccpV1.writeU32(out, destination);
      SccpV1.writeUnsignedLe(out, nonce, 8);
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

  /** Token creation payload. */
  public static final class TokenAdd extends SccpPayloadV1 {
    private final int target;
    private final BigInteger nonce;
    private final byte[] assetId;
    private final int decimals;
    private final byte[] name;
    private final byte[] symbol;

    public TokenAdd(
        final int target,
        final BigInteger nonce,
        final byte[] assetId,
        final int decimals,
        final byte[] name,
        final byte[] symbol) {
      super(3, SccpHubMessageKindV1.TOKEN_ADD);
      SccpV1.requireExternalDomain(target, "target");
      if (decimals < 0 || decimals > 255) {
        throw new IllegalArgumentException("decimals must fit u8");
      }
      this.target = target;
      this.nonce = SccpV1.requireUnsigned(nonce, 64, "nonce");
      this.assetId = SccpV1.requireHash(assetId, "soraAssetId");
      this.decimals = decimals;
      this.name = SccpV1.requireFixedAscii(name, "name");
      this.symbol = SccpV1.requireFixedAscii(symbol, "symbol");
    }

    @Override
    int sourceDomain() {
      return 0;
    }

    @Override
    int targetDomain() {
      return target;
    }

    @Override
    void encodeBody(final ByteArrayOutputStream out) {
      out.write(1);
      SccpV1.writeU32(out, target);
      SccpV1.writeUnsignedLe(out, nonce, 8);
      SccpV1.write(out, assetId);
      out.write(decimals);
      SccpV1.write(out, name);
      SccpV1.write(out, symbol);
    }
  }

  /** Token pause payload. */
  public static final class TokenPause extends TokenControl {
    public TokenPause(final int target, final BigInteger nonce, final byte[] assetId) {
      super(4, SccpHubMessageKindV1.TOKEN_PAUSE, target, nonce, assetId);
    }

    public TokenPause(final int target, final long nonce, final byte[] assetId) {
      this(target, BigInteger.valueOf(nonce), assetId);
    }
  }

  /** Token resume payload. */
  public static final class TokenResume extends TokenControl {
    public TokenResume(final int target, final BigInteger nonce, final byte[] assetId) {
      super(5, SccpHubMessageKindV1.TOKEN_RESUME, target, nonce, assetId);
    }

    public TokenResume(final int target, final long nonce, final byte[] assetId) {
      this(target, BigInteger.valueOf(nonce), assetId);
    }
  }

  /** Shared canonical implementation for token pause/resume. */
  public abstract static class TokenControl extends SccpPayloadV1 {
    private final int target;
    private final BigInteger nonce;
    private final byte[] assetId;

    TokenControl(
        final int discriminant,
        final SccpHubMessageKindV1 kind,
        final int target,
        final BigInteger nonce,
        final byte[] assetId) {
      super(discriminant, kind);
      SccpV1.requireExternalDomain(target, "target");
      this.target = target;
      this.nonce = SccpV1.requireUnsigned(nonce, 64, "nonce");
      this.assetId = SccpV1.requireHash(assetId, "soraAssetId");
    }

    @Override
    int sourceDomain() {
      return 0;
    }

    @Override
    int targetDomain() {
      return target;
    }

    public byte[] soraAssetId() {
      return Arrays.copyOf(assetId, assetId.length);
    }

    @Override
    void encodeBody(final ByteArrayOutputStream out) {
      out.write(1);
      SccpV1.writeU32(out, target);
      SccpV1.writeUnsignedLe(out, nonce, 8);
      SccpV1.write(out, assetId);
    }
  }
}
