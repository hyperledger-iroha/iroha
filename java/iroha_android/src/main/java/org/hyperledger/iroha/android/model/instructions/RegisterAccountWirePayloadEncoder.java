// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Encodes and decodes account registration instructions in wire-framed Norito format. */
public final class RegisterAccountWirePayloadEncoder {
  public static final String WIRE_NAME = "iroha.register";

  private static final String SCHEMA_PATH =
      "iroha_data_model::isi::register::RegisterBox";
  private static final long REGISTER_BOX_ACCOUNT_DISCRIMINANT = 2L;

  private RegisterAccountWirePayloadEncoder() {}

  /** Encodes one exact self-registration instruction. */
  public static InstructionBox encodeRegisterAccount(final String accountId) {
    return InstructionBox.fromWirePayload(WIRE_NAME, encodeRegisterBox(accountId));
  }

  /** Decodes one exact {@code RegisterBox::Account} payload. */
  public static String decodeRegisterAccountPayload(
      final byte[] wirePayload, final int chainDiscriminant) {
    return NoritoCodec.decode(
        Objects.requireNonNull(wirePayload, "wirePayload"),
        new RegisterBoxAccountAdapter(chainDiscriminant),
        SCHEMA_PATH);
  }

  private static byte[] encodeRegisterBox(final String accountId) {
    final Integer chainDiscriminant = AccountAddress.detectI105Discriminant(accountId);
    if (chainDiscriminant == null) {
      throw new IllegalArgumentException(
          "accountId must carry an explicit I105 chain discriminant");
    }
    return NoritoCodec.encode(
        accountId,
        SCHEMA_PATH,
        new RegisterBoxAccountAdapter(chainDiscriminant.intValue()));
  }

  private static final class RegisterBoxAccountAdapter implements TypeAdapter<String> {
    private final int chainDiscriminant;

    private RegisterBoxAccountAdapter(final int chainDiscriminant) {
      this.chainDiscriminant = chainDiscriminant;
    }

    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      encoder.writeUInt(REGISTER_BOX_ACCOUNT_DISCRIMINANT, 32);
      final NoritoEncoder variantChild = encoder.childEncoder();
      encodeRegisterAccountStruct(variantChild, value);
      final byte[] variantPayload = variantChild.toByteArray();
      writeLength(encoder, variantPayload.length);
      encoder.writeBytes(variantPayload);
    }

    private static void encodeRegisterAccountStruct(
        final NoritoEncoder encoder, final String accountId) {
      final NoritoEncoder objectChild = encoder.childEncoder();
      encodeNewAccount(objectChild, accountId);
      final byte[] objectPayload = objectChild.toByteArray();
      writeLength(encoder, objectPayload.length);
      encoder.writeBytes(objectPayload);
    }

    private static void encodeNewAccount(
        final NoritoEncoder encoder, final String accountId) {
      writeFieldWithLength(
          encoder, TransferWirePayloadEncoder.encodeAccountIdPayload(accountId));
      final byte[] emptySequence = encodeEmptySequence();
      writeFieldWithLength(encoder, emptySequence);
      final byte[] none = encodeNone();
      writeFieldWithLength(encoder, none);
      writeFieldWithLength(encoder, none);
      writeFieldWithLength(encoder, emptySequence);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final long discriminant = decoder.readUInt(32);
      if (discriminant != REGISTER_BOX_ACCOUNT_DISCRIMINANT) {
        throw new IllegalArgumentException(
            "Unsupported RegisterBox discriminant: " + discriminant);
      }
      final int payloadLength =
          checkedLength(
              decoder.readLength(compactLength(decoder)),
              "RegisterBox::Account payload");
      final NoritoDecoder child = new NoritoDecoder(decoder.readBytes(payloadLength), decoder.flags());
      final String accountId = decodeRegisterAccountStruct(child);
      if (child.remaining() != 0) {
        throw new IllegalArgumentException(
            "Trailing bytes after RegisterBox::Account payload");
      }
      return accountId;
    }

    private String decodeRegisterAccountStruct(final NoritoDecoder decoder) {
      final NoritoDecoder child =
          new NoritoDecoder(
              decodeSizedRawField(decoder, "Register<Account>.object"), decoder.flags());
      final String accountId = decodeNewAccount(child);
      if (child.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after NewAccount payload");
      }
      return accountId;
    }

    private String decodeNewAccount(final NoritoDecoder decoder) {
      final String accountId =
          TransferWirePayloadEncoder.decodeAccountIdPayload(
              decodeSizedRawField(decoder, "NewAccount.id"),
              chainDiscriminant);
      requireEmptySequence(
          decodeSizedRawField(decoder, "NewAccount.metadata"), "NewAccount.metadata");
      requireNone(decodeSizedRawField(decoder, "NewAccount.label"), "NewAccount.label");
      requireNone(decodeSizedRawField(decoder, "NewAccount.uaid"), "NewAccount.uaid");
      requireEmptySequence(
          decodeSizedRawField(decoder, "NewAccount.opaque_ids"), "NewAccount.opaque_ids");
      return accountId;
    }
  }

  private static void writeFieldWithLength(
      final NoritoEncoder encoder, final byte[] payload) {
    writeLength(encoder, payload.length);
    encoder.writeBytes(payload);
  }

  private static void writeLength(final NoritoEncoder encoder, final int size) {
    encoder.writeLength(size, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
  }

  private static byte[] encodeEmptySequence() {
    final NoritoEncoder encoder = new NoritoEncoder(0);
    encoder.writeUInt(0L, 64);
    return encoder.toByteArray();
  }

  private static byte[] encodeNone() {
    final NoritoEncoder encoder = new NoritoEncoder(0);
    encoder.writeByte(0);
    return encoder.toByteArray();
  }

  private static byte[] decodeSizedRawField(
      final NoritoDecoder decoder, final String fieldName) {
    final int payloadLength =
        checkedLength(
            decoder.readLength(compactLength(decoder)), fieldName + " payload");
    return decoder.readBytes(payloadLength);
  }

  private static int checkedLength(final long length, final String fieldName) {
    if (length < 0L || length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " is outside the supported range");
    }
    return (int) length;
  }

  private static boolean compactLength(final NoritoDecoder decoder) {
    return (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static void requireEmptySequence(
      final byte[] payload, final String fieldName) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, 0);
    if (decoder.readUInt(64) != 0L || decoder.remaining() != 0) {
      throw new IllegalArgumentException(fieldName + " must be empty");
    }
  }

  private static void requireNone(final byte[] payload, final String fieldName) {
    if (payload.length != 1 || payload[0] != 0) {
      throw new IllegalArgumentException(fieldName + " must be Option::None");
    }
  }
}
