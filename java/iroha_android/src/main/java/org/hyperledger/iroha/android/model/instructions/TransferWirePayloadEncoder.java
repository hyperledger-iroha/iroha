// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.model.instructions;

import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.TypeAdapter;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import java.util.Optional;

import org.hyperledger.iroha.android.address.AccountIdLiteral;

/**
 * Encodes asset transfer instructions in wire-framed Norito format.
 *
 * <p>This encoder creates wire payloads for transfer instructions that can be properly decoded by
 * the Rust Iroha server. The wire format uses:
 *
 * <ul>
 *   <li>Wire name: "iroha.transfer"
 *   <li>Payload: Norito-framed TransferBox enum with Asset variant
 * </ul>
 *
 * <p>The TransferBox::Asset variant contains:
 *
 * <ul>
 *   <li>source: AssetId (the asset to transfer from)
 *   <li>object: Numeric (amount to transfer)
 *   <li>destination: AccountId (recipient account)
 * </ul>
 */
public final class TransferWirePayloadEncoder {

  /** Wire name for transfer instructions in Iroha. */
  public static final String WIRE_NAME = "iroha.transfer";

  /** Schema path for TransferBox payloads. Must match Rust type name exactly. */
  private static final String SCHEMA_PATH = "iroha_data_model::isi::transfer::TransferBox";

  /** TransferBox enum discriminant for Asset variant. */
  private static final int TRANSFER_BOX_ASSET_DISCRIMINANT = 2;
  private static final int MULTISIG_POLICY_VERSION_V1 = 1;

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> UINT8_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<Long> UINT16_ADAPTER = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);

  private TransferWirePayloadEncoder() {}

  /**
   * Encodes an asset transfer instruction as a wire-framed InstructionBox.
   *
   * @param assetId The internal asset balance-bucket literal as
   *     {@code <base58-asset-definition-id>#<katakana-i105-account-id>} with an optional
   *     {@code #dataspace:<id>} suffix. Public asset ids remain bare Base58
   *     asset-definition ids.
   * @param amount The amount to transfer as a string (e.g., "10" or "10.50")
   * @param destinationAccountId The recipient's canonical Katakana i105 account ID
   * @return InstructionBox with wire payload ready for Norito encoding
   */
  public static InstructionBox encodeAssetTransfer(
      String assetId, String amount, String destinationAccountId) {
    Objects.requireNonNull(assetId, "assetId");
    Objects.requireNonNull(amount, "amount");
    Objects.requireNonNull(destinationAccountId, "destinationAccountId");

    byte[] wirePayload = encodeTransferBox(assetId, amount, destinationAccountId);
    return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload);
  }

  /**
   * Encodes an {@code AccountId} bare payload using the same layout expected by transaction
   * instruction fields.
   */
  static byte[] encodeAccountIdPayload(String accountId) {
    Objects.requireNonNull(accountId, "accountId");
    final AccountId parsed = AccountId.parse(accountId);
    final NoritoEncoder encoder = new NoritoEncoder(0);
    new AccountIdAdapter().encode(encoder, parsed);
    return encoder.toByteArray();
  }

  /**
   * Encodes the TransferBox payload with Norito framing.
   *
   * <p>Format: NoritoHeader + TransferBox::Asset(Transfer { source, object, destination })
   */
  private static byte[] encodeTransferBox(
      String assetIdStr, String amount, String destinationAccountIdStr) {
    // Parse amount into mantissa and scale
    NumericValue numeric = parseNumericAmount(amount);

    // Parse string IDs into proper struct types
    AssetId assetId = AssetId.parse(assetIdStr);
    AccountId destinationAccountId = AccountId.parse(destinationAccountIdStr);

    // Create the inner payload (Transfer struct contents)
    TypeAdapter<TransferAssetPayload> payloadAdapter = new TransferAssetPayloadAdapter();
    TransferAssetPayload payload =
        new TransferAssetPayload(assetId, numeric, destinationAccountId);

    // Encode with Norito framing (header + checksum)
    return NoritoCodec.encode(payload, SCHEMA_PATH, payloadAdapter);
  }

  /**
   * Parses a decimal amount string into mantissa and scale.
   *
   * <p>Example: "10.50" -> mantissa=1050, scale=2
   */
  private static NumericValue parseNumericAmount(String amount) {
    BigDecimal decimal = new BigDecimal(amount);
    int scale = Math.max(0, decimal.scale());
    if (scale > 28) {
      throw new IllegalArgumentException("Numeric scale exceeds Iroha limit of 28: " + scale);
    }
    BigInteger mantissa = decimal.movePointRight(scale).toBigIntegerExact();
    if (mantissa.bitLength() >= 512) {
      throw new IllegalArgumentException(
          "Numeric mantissa exceeds Iroha limit of 512 bits: " + mantissa.bitLength());
    }
    return new NumericValue(mantissa, scale);
  }

  /** Represents a Numeric value with mantissa and scale. */
  private static final class NumericValue {
    private final BigInteger mantissa;
    private final int scale;

    NumericValue(BigInteger mantissa, int scale) {
      this.mantissa = Objects.requireNonNull(mantissa);
      this.scale = scale;
    }

    BigInteger mantissa() {
      return mantissa;
    }

    int scale() {
      return scale;
    }
  }

  /** Transfer payload for Asset transfers. */
  private static final class TransferAssetPayload {
    private final AssetId source;
    private final NumericValue amount;
    private final AccountId destination;

    TransferAssetPayload(AssetId source, NumericValue amount, AccountId destination) {
      this.source = Objects.requireNonNull(source);
      this.amount = Objects.requireNonNull(amount);
      this.destination = Objects.requireNonNull(destination);
    }

    AssetId source() {
      return source;
    }

    NumericValue amount() {
      return amount;
    }

    AccountId destination() {
      return destination;
    }
  }

  /**
   * Rust: AssetDefinitionId is now a raw {@code [u8; 16]} payload wrapped by a canonical Base58
   * address on public surfaces.
   */
  private static final class AssetDefinitionId {
    private final byte[] definitionBytes;

    AssetDefinitionId(byte[] definitionBytes) {
      if (definitionBytes.length != 16) {
        throw new IllegalArgumentException(
            "definitionBytes must be 16 bytes, got " + definitionBytes.length);
      }
      this.definitionBytes = definitionBytes.clone();
    }

    byte[] definitionBytes() {
      return definitionBytes.clone();
    }

    /**
     * Create from a canonical unprefixed Base58 asset-definition address.
     */
    static AssetDefinitionId fromAddress(String address) {
      return new AssetDefinitionId(AssetDefinitionIdEncoder.parseAddressBytes(address));
    }
  }

  /**
   * Rust: AccountController enum { Single(PublicKey), Multisig(MultisigPolicy) }
   * Single = discriminant 0, Multisig = discriminant 1.
   * PublicKey serializes as a normalized multihash hex string.
   */
  private static final class AccountController {
    private final String publicKeyMultihash;
    private final AccountAddress.MultisigPolicyPayload multisigPolicy;

    static AccountController single(String publicKeyMultihash) {
      return new AccountController(Objects.requireNonNull(publicKeyMultihash), null);
    }

    static AccountController multisig(AccountAddress.MultisigPolicyPayload policy) {
      return new AccountController(null, Objects.requireNonNull(policy));
    }

    private AccountController(
        String publicKeyMultihash, AccountAddress.MultisigPolicyPayload multisigPolicy) {
      this.publicKeyMultihash = publicKeyMultihash;
      this.multisigPolicy = multisigPolicy;
    }

    boolean isSingle() {
      return publicKeyMultihash != null;
    }

    String publicKeyMultihash() {
      return publicKeyMultihash;
    }

    AccountAddress.MultisigPolicyPayload multisigPolicy() {
      return multisigPolicy;
    }
  }

  /**
   * Rust: AccountId { controller: AccountController }
   */
  private static final class AccountId {
    private final AccountController controller;

    AccountId(AccountController controller) {
      this.controller = Objects.requireNonNull(controller);
    }

    AccountController controller() {
      return controller;
    }

    static AccountId parse(String accountIdStr) {
      final String canonicalAccountId =
          AccountIdLiteral.requireCanonicalI105Address(accountIdStr, "accountId");
      final AccountAddress address;
      try {
        address = AccountAddress.parseEncodedIgnoringCurveSupport(canonicalAccountId, null).address;
      } catch (AccountAddress.AccountAddressException e) {
        throw new IllegalArgumentException(
            "Failed to parse canonical Katakana i105 account identifier: " + canonicalAccountId, e);
      }

      try {
        Optional<AccountAddress.SingleKeyPayload> singleKey =
            address.singleKeyPayloadIgnoringCurveSupport();
        if (singleKey.isPresent()) {
          AccountAddress.SingleKeyPayload key = singleKey.get();
          String multihash =
              PublicKeyCodec.encodePublicKeyMultihash(key.curveId(), key.publicKey());
          return new AccountId(AccountController.single(multihash));
        }

        Optional<AccountAddress.MultisigPolicyPayload> multisig =
            address.multisigPolicyPayloadIgnoringCurveSupport();
        if (multisig.isPresent()) {
          return new AccountId(AccountController.multisig(multisig.get()));
        }
      } catch (AccountAddress.AccountAddressException e) {
        throw new IllegalArgumentException(
            "Failed to extract controller from canonical Katakana i105 account id", e);
      }

      throw new IllegalArgumentException(
          "Address contains neither single-key nor multisig controller");
    }
  }

  /**
   * Rust: AssetId { account: AccountId, definition: AssetDefinitionId }
   */
  private static final class AssetId {
    private final AccountId account;
    private final AssetDefinitionId definition;
    private final byte[] encodedAccountPayload;
    private final byte[] scopePayload;

    AssetId(
        AccountId account,
        AssetDefinitionId definition,
        byte[] encodedAccountPayload,
        byte[] scopePayload) {
      if (account == null && encodedAccountPayload == null) {
        throw new IllegalArgumentException("AssetId requires either account or encodedAccountPayload");
      }
      this.account = account;
      this.definition = Objects.requireNonNull(definition);
      this.encodedAccountPayload =
          encodedAccountPayload == null ? null : encodedAccountPayload.clone();
      this.scopePayload = Objects.requireNonNull(scopePayload).clone();
    }

    AccountId account() {
      return account;
    }

    AssetDefinitionId definition() {
      return definition;
    }

    byte[] encodedAccountPayload() {
      return encodedAccountPayload == null ? null : encodedAccountPayload.clone();
    }

    byte[] scopePayload() {
      return scopePayload.clone();
    }

    /**
     * Parse from {@code <base58-asset-definition-id>#<katakana-i105-account-id>} with an optional
     * {@code #dataspace:<id>} suffix.
     */
    static AssetId parse(String assetIdStr) {
      final String trimmed = assetIdStr.trim();
      final String[] parts = trimmed.split("#", -1);
      if (parts.length < 2 || parts.length > 3) {
        throw new IllegalArgumentException(
            "Invalid AssetId format: expected <base58-asset-definition-id>#<katakana-i105-account-id> with optional #dataspace:<id>");
      }
      final String assetDefPart = parts[0];
      final String accountIdPart = parts[1];
      final AccountId accountId = AccountId.parse(accountIdPart);

      if (!AssetDefinitionIdEncoder.isCanonicalAddress(assetDefPart)) {
        throw new IllegalArgumentException(
            "Invalid AssetId format: expected canonical <base58-asset-definition-id>#<katakana-i105-account-id>");
      }
      final AssetDefinitionId assetDef = AssetDefinitionId.fromAddress(assetDefPart);
      final byte[] scopePayload;
      if (parts.length == 2) {
        scopePayload = globalScopePayload();
      } else {
        final String scopePart = parts[2];
        if (!scopePart.startsWith("dataspace:")) {
          throw new IllegalArgumentException(
              "Invalid AssetId format: scope must use dataspace:<id> when present");
        }
        final String rawDataspace = scopePart.substring("dataspace:".length());
        final long dataspaceId;
        try {
          dataspaceId = Long.parseLong(rawDataspace);
        } catch (NumberFormatException ex) {
          throw new IllegalArgumentException(
              "Invalid AssetId format: dataspace scope must be an unsigned integer", ex);
        }
        if (dataspaceId < 0) {
          throw new IllegalArgumentException(
              "Invalid AssetId format: dataspace scope must be non-negative");
        }
        final AssetBalanceScopePayload scope = AssetBalanceScopePayload.dataspace(dataspaceId);
        scopePayload = encodeAssetBalanceScopePayload(scope);
      }

      return new AssetId(accountId, assetDef, null, scopePayload);
    }
  }

  /** Adapter for encoding TransferAssetPayload in Norito format. */
  private static final class TransferAssetPayloadAdapter
      implements TypeAdapter<TransferAssetPayload> {

    private static final TypeAdapter<AssetId> ASSET_ID_ADAPTER = new AssetIdAdapter();
    private static final TypeAdapter<AccountId> ACCOUNT_ID_ADAPTER = new AccountIdAdapter();

    @Override
    public void encode(NoritoEncoder encoder, TransferAssetPayload value) {
      // TransferBox enum tag (Asset = 2)
      UINT32_ADAPTER.encode(encoder, (long) TRANSFER_BOX_ASSET_DISCRIMINANT);

      // Norito enum variants have a u64 length prefix for the variant payload.
      // Encode the Transfer struct to a child buffer first to compute length.
      NoritoEncoder child = encoder.childEncoder();
      encodeTransferStruct(child, value);
      byte[] variantPayload = child.toByteArray();

      // Write u64 length prefix (little-endian, not compact)
      encoder.writeUInt(variantPayload.length, 64);
      encoder.writeBytes(variantPayload);
    }

    /**
     * Encode Transfer<Asset, Numeric, Account> struct fields with u64 length prefixes.
     *
     * <p>In norito non-packed mode, each struct field is prefixed with a u64 (little-endian) length.
     */
    private void encodeTransferStruct(NoritoEncoder encoder, TransferAssetPayload value) {
      // Transfer struct fields in order with u64 length prefixes:
      // - source: AssetId (struct)
      // - object: Numeric
      // - destination: AccountId (struct)
      encodeFieldWithLength(encoder, ASSET_ID_ADAPTER, value.source());
      encodeFieldWithLength(encoder, new NumericAdapter(), value.amount());
      encodeFieldWithLength(encoder, ACCOUNT_ID_ADAPTER, value.destination());
    }

    /**
     * Encode a field value with a u64 (little-endian, non-compact) length prefix.
     */
    private <T> void encodeFieldWithLength(NoritoEncoder encoder, TypeAdapter<T> adapter, T value) {
      NoritoEncoder child = encoder.childEncoder();
      adapter.encode(child, value);
      byte[] payload = child.toByteArray();
      // u64 little-endian length prefix (NOT compact/varint)
      encoder.writeUInt(payload.length, 64);
      encoder.writeBytes(payload);
    }

    @Override
    public TransferAssetPayload decode(org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding transfer payloads is not supported");
    }
  }

  /**
   * Adapter for encoding AssetDefinitionId as {@code [u8; 16]}.
   *
   * <p>With {@code flags=0} (no COMPACT_LEN), Rust serializes each array element with a u64 length
   * prefix: 16 × (u64_le(1) + byte) = 144 bytes. With COMPACT_LEN, Rust uses varint lengths
   * instead. The current signing path always uses {@code flags=0}.
   */
  private static final class AssetDefinitionIdAdapter implements TypeAdapter<AssetDefinitionId> {

    @Override
    public void encode(NoritoEncoder encoder, AssetDefinitionId value) {
      encodeFixedByteArray(encoder, value.definitionBytes());
    }

    @Override
    public AssetDefinitionId decode(org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding AssetDefinitionId is not supported");
    }
  }

  /**
   * Adapter for encoding AccountId as a STRUCT.
   *
   * <p>Rust's AccountId:
   * <pre>
   *   pub struct AccountId {
   *       pub controller: AccountController,
   *   }
   * </pre>
   *
   * Each field has a u64 length prefix in norito non-packed mode.
   */
  private static final class AccountIdAdapter implements TypeAdapter<AccountId> {
    private static final TypeAdapter<AccountController> CONTROLLER_ADAPTER =
        new AccountControllerAdapter();

    @Override
    public void encode(NoritoEncoder encoder, AccountId value) {
      // AccountId has transparent NoritoSerialize — it delegates directly to
      // AccountController without any struct field wrapping.
      CONTROLLER_ADAPTER.encode(encoder, value.controller());
    }

    @Override
    public AccountId decode(org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding AccountId is not supported");
    }
  }

  /**
   * Adapter for encoding AccountController enum.
   *
   * <p>Rust AccountController is a standard norito enum:
   * <pre>
   *   #[derive(Encode, Decode)]
   *   pub enum AccountController {
   *       Single(PublicKey),         // discriminant 0
   *       Multisig(MultisigPolicy),  // discriminant 1
   *   }
   * </pre>
   *
   * Standard enum format: u32 discriminant + u64 length prefix + variant payload.
   */
  private static final class AccountControllerAdapter implements TypeAdapter<AccountController> {
    private static final int SINGLE_DISCRIMINANT = 0;
    private static final int MULTISIG_DISCRIMINANT = 1;
    private static final TypeAdapter<Long> UINT8_ADAPTER = NoritoAdapters.uint(8);
    private static final TypeAdapter<Long> UINT16_ADAPTER = NoritoAdapters.uint(16);

    @Override
    public void encode(NoritoEncoder encoder, AccountController value) {
      if (value.isSingle()) {
        encodeSingle(encoder, value.publicKeyMultihash());
      } else {
        encodeMultisig(encoder, value.multisigPolicy());
      }
    }

    private void encodeSingle(NoritoEncoder encoder, String publicKeyMultihash) {
      UINT32_ADAPTER.encode(encoder, (long) SINGLE_DISCRIMINANT);
      NoritoEncoder child = encoder.childEncoder();
      STRING_ADAPTER.encode(child, publicKeyMultihash);
      byte[] payload = child.toByteArray();
      encoder.writeUInt(payload.length, 64);
      encoder.writeBytes(payload);
    }

    private void encodeMultisig(
        NoritoEncoder encoder, AccountAddress.MultisigPolicyPayload policy) {
      validateMultisigPolicySemantics(policy.version(), policy.threshold(), policy.members());
      UINT32_ADAPTER.encode(encoder, (long) MULTISIG_DISCRIMINANT);

      NoritoEncoder policyEncoder = encoder.childEncoder();
      encodeSizedField(policyEncoder, UINT8_ADAPTER, (long) policy.version());
      encodeSizedField(policyEncoder, UINT16_ADAPTER, (long) policy.threshold());
      encodeMultisigMembers(policyEncoder, policy.members());

      byte[] policyPayload = policyEncoder.toByteArray();
      encoder.writeUInt(policyPayload.length, 64);
      encoder.writeBytes(policyPayload);
    }

    private void encodeMultisigMembers(
        NoritoEncoder encoder, List<AccountAddress.MultisigMemberPayload> members) {
      List<AccountAddress.MultisigMemberPayload> sorted = new ArrayList<>(members);
      sorted.sort((a, b) -> {
        byte[] keyA = canonicalSortKey(a);
        byte[] keyB = canonicalSortKey(b);
        return compareUnsigned(keyA, keyB);
      });
      for (int i = 1; i < sorted.size(); i++) {
        if (Arrays.equals(canonicalSortKey(sorted.get(i - 1)), canonicalSortKey(sorted.get(i)))) {
          throw new IllegalArgumentException("Duplicate multisig member");
        }
      }

      NoritoEncoder vecEncoder = encoder.childEncoder();
      vecEncoder.writeUInt(sorted.size(), 64);
      for (AccountAddress.MultisigMemberPayload member : sorted) {
        NoritoEncoder memberEncoder = vecEncoder.childEncoder();
        String memberMultihash =
            PublicKeyCodec.encodePublicKeyMultihash(member.curveId(), member.publicKey());
        encodeSizedField(memberEncoder, STRING_ADAPTER, memberMultihash);
        encodeSizedField(memberEncoder, UINT16_ADAPTER, (long) member.weight());
        byte[] memberPayload = memberEncoder.toByteArray();
        vecEncoder.writeUInt(memberPayload.length, 64);
        vecEncoder.writeBytes(memberPayload);
      }
      byte[] vecPayload = vecEncoder.toByteArray();
      encoder.writeUInt(vecPayload.length, 64);
      encoder.writeBytes(vecPayload);
    }

    private static <T> void encodeSizedField(
        NoritoEncoder encoder, TypeAdapter<T> adapter, T value) {
      NoritoEncoder child = encoder.childEncoder();
      adapter.encode(child, value);
      byte[] payload = child.toByteArray();
      encoder.writeUInt(payload.length, 64);
      encoder.writeBytes(payload);
    }

    @Override
    public AccountController decode(org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding AccountController is not supported");
    }
  }

  /**
   * Adapter for encoding AssetId: { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }
   *
   * <p>Legacy text input defaults to {@code AssetBalanceScope::Global}. Canonical Norito asset
   * identifiers preserve the account and scope payload bytes exactly as provided.
   */
  private static final class AssetIdAdapter implements TypeAdapter<AssetId> {
    private static final TypeAdapter<AccountId> ACCOUNT_ID_ADAPTER = new AccountIdAdapter();
    private static final TypeAdapter<AssetDefinitionId> ASSET_DEF_ID_ADAPTER =
        new AssetDefinitionIdAdapter();

    @Override
    public void encode(NoritoEncoder encoder, AssetId value) {
      // AssetId struct fields in order with u64 length prefixes:
      // 1. account: AccountId
      // 2. definition: AssetDefinitionId
      // 3. scope: AssetBalanceScope
      byte[] encodedAccountPayload = value.encodedAccountPayload();
      if (encodedAccountPayload != null) {
        encoder.writeUInt(encodedAccountPayload.length, 64);
        encoder.writeBytes(encodedAccountPayload);
      } else {
        encodeFieldWithLength(encoder, ACCOUNT_ID_ADAPTER, value.account());
      }
      encodeFieldWithLength(encoder, ASSET_DEF_ID_ADAPTER, value.definition());
      byte[] scopePayload = value.scopePayload();
      encoder.writeUInt(scopePayload.length, 64);
      encoder.writeBytes(scopePayload);
    }

    @Override
    public AssetId decode(org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding AssetId is not supported");
    }

    private <T> void encodeFieldWithLength(NoritoEncoder encoder, TypeAdapter<T> adapter, T value) {
      NoritoEncoder child = encoder.childEncoder();
      adapter.encode(child, value);
      byte[] payload = child.toByteArray();
      encoder.writeUInt(payload.length, 64);
      encoder.writeBytes(payload);
    }
  }

  /**
   * Adapter for encoding Numeric values (mantissa + scale).
   *
   * <p>Numeric is a struct with two fields that need u64 length prefixes:
   * - mantissa: BigInt
   * - scale: u32
   */
  private static final class NumericAdapter implements TypeAdapter<NumericValue> {

    @Override
    public void encode(NoritoEncoder encoder, NumericValue value) {
      // Numeric struct fields with u64 length prefixes:
      // 1. mantissa: BigInt
      // 2. scale: u32
      encodeFieldBigInt(encoder, value.mantissa());
      encodeFieldU32(encoder, value.scale());
    }

    @Override
    public NumericValue decode(org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding numeric values is not supported");
    }

    /**
     * Encode BigInt field with u64 length prefix.
     */
    private void encodeFieldBigInt(NoritoEncoder encoder, BigInteger value) {
      NoritoEncoder child = encoder.childEncoder();
      encodeBigInt(child, value);
      byte[] payload = child.toByteArray();
      encoder.writeUInt(payload.length, 64);
      encoder.writeBytes(payload);
    }

    /**
     * Encode u32 field with u64 length prefix.
     */
    private void encodeFieldU32(NoritoEncoder encoder, int value) {
      // u32 is always 4 bytes
      encoder.writeUInt(4, 64);  // length prefix
      UINT32_ADAPTER.encode(encoder, (long) value);
    }

    private void encodeBigInt(NoritoEncoder encoder, BigInteger value) {
      // Rust BigInt is encoded as:
      // 1. u32 length prefix (4 bytes, little-endian)
      // 2. Two's complement little-endian bytes
      byte[] twosCompBytes = toTwosComplementLittleEndian(value);
      // u32 length prefix
      encoder.writeUInt(twosCompBytes.length, 32);
      encoder.writeBytes(twosCompBytes);
    }

    /**
     * Convert BigInteger to minimal two's complement little-endian byte representation.
     *
     * <p>This matches Rust's BigInt::to_signed_bytes_le() from num-bigint.
     */
    private byte[] toTwosComplementLittleEndian(BigInteger value) {
      if (value.signum() == 0) {
        return new byte[0];
      }

      // Get two's complement representation (big-endian) from Java
      byte[] twosCompBE = value.toByteArray();

      // Reverse to little-endian
      byte[] result = new byte[twosCompBE.length];
      for (int i = 0; i < twosCompBE.length; i++) {
        result[i] = twosCompBE[twosCompBE.length - 1 - i];
      }

      // Trim trailing sign-extension bytes for minimal representation
      // For positive: trim trailing 0x00 bytes, but keep at least one byte,
      //               and keep 0x00 if next byte has MSB set
      // For negative: trim trailing 0xFF bytes, but keep at least one byte,
      //               and keep 0xFF if next byte has MSB clear
      int trimLen = result.length;
      if (value.signum() > 0) {
        while (trimLen > 1 && result[trimLen - 1] == 0
            && (result[trimLen - 2] & 0x80) == 0) {
          trimLen--;
        }
      } else {
        while (trimLen > 1 && result[trimLen - 1] == (byte) 0xFF
            && (result[trimLen - 2] & 0x80) != 0) {
          trimLen--;
        }
      }

      if (trimLen == result.length) {
        return result;
      }
      return Arrays.copyOf(result, trimLen);
    }
  }

  /**
   * Encodes a fixed-size byte array as per-element length-prefixed bytes for {@code [u8; N]}.
   * Each element is written as {@code u64_le(1) + byte}, producing 9 bytes per element.
   *
   * <p>This matches Rust's {@code [T; N]::NoritoSerialize} only when {@code COMPACT_LEN} is off
   * ({@code flags=0}). With {@code COMPACT_LEN} active, Rust uses varint lengths instead of
   * fixed u64. The current signing path always uses {@code flags=0}.
   */
  public static void encodeFixedByteArray(NoritoEncoder encoder, byte[] bytes) {
    boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    for (byte b : bytes) {
      encoder.writeLength(1, compact);
      encoder.writeByte(b);
    }
  }

  private static byte[] globalScopePayload() {
    NoritoEncoder encoder = new NoritoEncoder(0);
    UINT32_ADAPTER.encode(encoder, 0L);
    return encoder.toByteArray();
  }

  private static void validateMultisigPolicySemantics(
      int version, int threshold, List<AccountAddress.MultisigMemberPayload> members) {
    if (version != MULTISIG_POLICY_VERSION_V1) {
      throw new IllegalArgumentException(
          "Invalid multisig policy: unsupported version " + version);
    }
    if (members.isEmpty()) {
      throw new IllegalArgumentException("Invalid multisig policy: zero members");
    }
    long totalWeight = 0L;
    final List<byte[]> sortKeys = new ArrayList<>(members.size());
    for (AccountAddress.MultisigMemberPayload member : members) {
      if (member.weight() <= 0) {
        throw new IllegalArgumentException("Invalid multisig policy: non-positive weight");
      }
      if (member.publicKey().length == 0) {
        throw new IllegalArgumentException("Invalid multisig policy: empty public key");
      }
      totalWeight += member.weight();
      sortKeys.add(canonicalSortKey(member));
    }
    if (threshold <= 0) {
      throw new IllegalArgumentException("Invalid multisig policy: zero threshold");
    }
    if (totalWeight < threshold) {
      throw new IllegalArgumentException("Invalid multisig policy: threshold exceeds total weight");
    }
    sortKeys.sort(TransferWirePayloadEncoder::compareUnsigned);
    for (int i = 1; i < sortKeys.size(); i++) {
      if (Arrays.equals(sortKeys.get(i - 1), sortKeys.get(i))) {
        throw new IllegalArgumentException("Invalid multisig policy: duplicate member");
      }
    }
  }

  private static byte[] canonicalSortKey(AccountAddress.MultisigMemberPayload member) {
    String algorithm = PublicKeyCodec.algorithmForCurveId(member.curveId());
    if (algorithm == null) {
      throw new IllegalArgumentException("Invalid multisig policy: unknown curve id");
    }
    byte[] algorithmBytes = algorithm.getBytes(java.nio.charset.StandardCharsets.UTF_8);
    byte[] keyBytes = member.publicKey();
    byte[] sortKey = new byte[algorithmBytes.length + 1 + keyBytes.length];
    System.arraycopy(algorithmBytes, 0, sortKey, 0, algorithmBytes.length);
    sortKey[algorithmBytes.length] = 0;
    System.arraycopy(keyBytes, 0, sortKey, algorithmBytes.length + 1, keyBytes.length);
    return sortKey;
  }

  private static int compareUnsigned(byte[] a, byte[] b) {
    int len = Math.min(a.length, b.length);
    for (int i = 0; i < len; i++) {
      int cmp = (a[i] & 0xFF) - (b[i] & 0xFF);
      if (cmp != 0) {
        return cmp;
      }
    }
    return Integer.compare(a.length, b.length);
  }

  private static byte[] encodeAssetBalanceScopePayload(AssetBalanceScopePayload scope) {
    if (scope.isGlobal()) {
      return globalScopePayload();
    }
    final NoritoEncoder encoder = new NoritoEncoder(0);
    UINT32_ADAPTER.encode(encoder, 1L);
    encoder.writeUInt(8, 64);
    encoder.writeUInt(scope.dataspaceId(), 64);
    return encoder.toByteArray();
  }

  private static <T> T decodeSizedTypedField(
      NoritoDecoder decoder, TypeAdapter<T> adapter, String fieldName) {
    final int payloadLength =
        checkedLength(
            decoder.readLength((decoder.flags() & NoritoHeader.COMPACT_LEN) != 0),
            fieldName + " payload");
    final byte[] payload = decoder.readBytes(payloadLength);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName + " payload");
    }
    return value;
  }

  private static int checkedLength(long length, String fieldName) {
    if (length < 0L) {
      throw new IllegalArgumentException(fieldName + " must be non-negative");
    }
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " too large");
    }
    return (int) length;
  }

  private static final class AssetBalanceScopePayload {
    private final boolean global;
    private final long dataspaceId;

    private AssetBalanceScopePayload(boolean global, long dataspaceId) {
      this.global = global;
      this.dataspaceId = dataspaceId;
    }

    static AssetBalanceScopePayload global() {
      return new AssetBalanceScopePayload(true, 0L);
    }

    static AssetBalanceScopePayload dataspace(long dataspaceId) {
      return new AssetBalanceScopePayload(false, dataspaceId);
    }

    boolean isGlobal() {
      return global;
    }

    long dataspaceId() {
      return dataspaceId;
    }
  }
}
