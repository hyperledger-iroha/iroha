package org.hyperledger.iroha.android.norito;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.client.MultisigProposeRequest;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Norito adapter that mirrors the {@link TransactionPayload} structure used by the Android library.
 * IVM bytecode payloads are encoded directly. Instruction payloads must be provided as wire-framed
 * Norito blobs (wire id + Norito header). Metadata values are encoded as raw JSON literals to match
 * the Rust `Json` wrapper.
 */
final class TransactionPayloadAdapter implements TypeAdapter<TransactionPayload> {

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<String> ACCOUNT_ID_ADAPTER = new AccountIdAdapter();
  private static final TypeAdapter<String> CHAIN_ID_ADAPTER = new ChainIdAdapter();
  private static final TypeAdapter<String> JSON_VALUE_ADAPTER = new JsonAdapter();
  private static final TypeAdapter<Long> UINT64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> UINT32_AS_LONG_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> UINT16_ADAPTER = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT8_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> RAW_BYTE_VEC_ADAPTER = NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<byte[]> IVM_BYTECODE_ADAPTER = new IvmBytecodeAdapter();
  private static final TypeAdapter<Optional<String>> OPTIONAL_STRING_ADAPTER =
      NoritoAdapters.option(STRING_ADAPTER);
  private static final TypeAdapter<Optional<String>> OPTIONAL_ACCOUNT_ID_ADAPTER =
      NoritoAdapters.option(ACCOUNT_ID_ADAPTER);
  private static final TypeAdapter<List<InstructionBox>> INSTRUCTION_LIST_ADAPTER =
      NoritoAdapters.sequence(new InstructionAdapter());
  private static final TypeAdapter<List<byte[]>> ENCODED_INSTRUCTION_LIST_ADAPTER =
      NoritoAdapters.sequence(new EncodedInstructionAdapter());
  private static final TypeAdapter<Long> ENUM_TAG_ADAPTER = NoritoAdapters.uint(32);
  private static final long EXECUTABLE_INSTRUCTIONS_TAG = 0L;
  private static final long EXECUTABLE_CONTRACT_CALL_TAG = 1L;
  private static final long EXECUTABLE_IVM_TAG = 2L;
  private static final long EXECUTABLE_IVM_PROVED_TAG = 3L;
  private static final TypeAdapter<Optional<Long>> TTL_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(64));
  private static final TypeAdapter<Optional<Long>> NONCE_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(32));
  private static final TypeAdapter<Executable> EXECUTABLE_ADAPTER = new ExecutableAdapter();
  private static final TypeAdapter<Map<String, JsonValue>> METADATA_ADAPTER = new MetadataAdapter();
  private static final String INSTRUCTION_BOX_SCHEMA = "iroha.data_model.isi.InstructionBox.v1";
  private static final String MULTISIG_PROPOSE_DTO_SCHEMA =
      "iroha_torii::routing::MultisigProposeDto";

  @Override
  public void encode(final NoritoEncoder encoder, final TransactionPayload value) {
    encodeSizedField(encoder, CHAIN_ID_ADAPTER, value.chainId());
    encodeSizedField(encoder, ACCOUNT_ID_ADAPTER, value.authority());
    encodeSizedField(encoder, UINT64_ADAPTER, value.creationTimeMs());
    encodeSizedField(encoder, EXECUTABLE_ADAPTER, value.executable());
    encodeSizedField(encoder, TTL_ADAPTER, value.timeToLiveMs());
    encodeSizedField(encoder, NONCE_ADAPTER, value.nonce().map(Integer::longValue));
    encodeSizedField(encoder, METADATA_ADAPTER, value.metadata());
  }

  @Override
  public TransactionPayload decode(final NoritoDecoder decoder) {
    final String chainId = decodeSizedField(decoder, CHAIN_ID_ADAPTER);
    final String authority = decodeAuthorityField(decoder);
    final long creationTimeMs = decodeSizedField(decoder, UINT64_ADAPTER);
    final Executable executable = decodeSizedField(decoder, EXECUTABLE_ADAPTER);
    final Optional<Long> ttl = decodeSizedField(decoder, TTL_ADAPTER);
    final Optional<Long> nonceRaw = decodeSizedField(decoder, NONCE_ADAPTER);
    final Map<String, JsonValue> metadata =
        new LinkedHashMap<>(decodeSizedField(decoder, METADATA_ADAPTER));

    final TransactionPayload.Builder builder =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority(authority)
            .setCreationTimeMs(creationTimeMs)
            .setExecutable(executable)
            .setMetadata(metadata);
    ttl.ifPresent(builder::setTimeToLiveMs);
    nonceRaw.ifPresent(value -> builder.setNonce(Math.toIntExact(value)));
    return builder.build();
  }

  static byte[] encodeInstructionBox(final InstructionBox instruction) {
    return NoritoCodec.encode(instruction, INSTRUCTION_BOX_SCHEMA, new InstructionAdapter());
  }

  static byte[] encodeMultisigProposeRequest(final MultisigProposeRequest request) {
    return NoritoCodec.encode(
        request,
        MULTISIG_PROPOSE_DTO_SCHEMA,
        new MultisigProposeRequestAdapter(),
        NoritoHeader.COMPACT_LEN);
  }

  static InstructionBox decodeInstructionBox(final byte[] encoded) {
    return NoritoCodec.decode(encoded, new InstructionAdapter(), INSTRUCTION_BOX_SCHEMA);
  }

  private static void encodeExecutable(final NoritoEncoder encoder, final Executable executable) {
    if (executable.isIvm()) {
      ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_IVM_TAG);
      encodeSizedField(encoder, IVM_BYTECODE_ADAPTER, executable.ivmBytes());
      return;
    }
    ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_INSTRUCTIONS_TAG);
    encodeSizedField(encoder, INSTRUCTION_LIST_ADAPTER, executable.instructions());
  }

  private static Executable decodeExecutable(final NoritoDecoder decoder) {
    final long tag = ENUM_TAG_ADAPTER.decode(decoder);
    if (tag == EXECUTABLE_IVM_TAG) {
      final byte[] bytes = decodeSizedField(decoder, IVM_BYTECODE_ADAPTER);
      return Executable.ivm(bytes);
    }
    if (tag == EXECUTABLE_INSTRUCTIONS_TAG) {
      final List<InstructionBox> instructions = decodeSizedField(decoder, INSTRUCTION_LIST_ADAPTER);
      return Executable.instructions(instructions);
    }
    if (tag == EXECUTABLE_CONTRACT_CALL_TAG || tag == EXECUTABLE_IVM_PROVED_TAG) {
      throw new IllegalArgumentException("Unsupported Executable discriminant: " + tag);
    }
    throw new IllegalArgumentException("Unknown Executable discriminant: " + tag);
  }

  private static final class InstructionAdapter implements TypeAdapter<InstructionBox> {
    @Override
    public void encode(final NoritoEncoder encoder, final InstructionBox value) {
      final InstructionBox.InstructionPayload payload = value.payload();
      if (payload instanceof InstructionBox.WirePayload wire) {
        if (!isWirePayloadCandidate(wire.wireName(), wire.payloadBytes())) {
          throw new IllegalArgumentException("Wire payload must include a valid Norito header");
        }
        encodeSizedField(encoder, STRING_ADAPTER, wire.wireName());
        encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, wire.payloadBytes());
        return;
      }
      throw new IllegalArgumentException("Instruction payload must be wire-framed");
    }

    @Override
    public InstructionBox decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      if (payload.length == 0) {
        throw new IllegalArgumentException("Instruction payload must not be empty");
      }
      final InstructionBox wire = tryDecodeWireInstruction(payload, decoder.flags(), decoder.flagsHint());
      if (wire != null) {
        return wire;
      }
      throw new IllegalArgumentException("Instruction payload must be wire-framed");
    }
  }

  private static final class EncodedInstructionAdapter implements TypeAdapter<byte[]> {
    private static final InstructionAdapter INSTRUCTION_ADAPTER = new InstructionAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      if (value == null || value.length == 0) {
        throw new IllegalArgumentException("instruction bytes must not be empty");
      }
      INSTRUCTION_ADAPTER.encode(encoder, decodeInstructionBox(value));
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Multisig instruction byte decoding is not supported");
    }
  }

  private static final class MultisigProposeRequestAdapter
      implements TypeAdapter<MultisigProposeRequest> {
    @Override
    public void encode(final NoritoEncoder encoder, final MultisigProposeRequest value) {
      validateMultisigProposeRequest(value);
      encodeSizedField(
          encoder,
          OPTIONAL_ACCOUNT_ID_ADAPTER,
          optionalString(value.multisigAccountId()));
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalString(value.multisigAccountAlias()));
      encodeSizedField(
          encoder,
          ACCOUNT_ID_ADAPTER,
          requireNonBlank(value.signerAccountId(), "signerAccountId"));
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, Optional.empty());
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, optionalString(value.publicKeyHex()));
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, optionalString(value.signatureB64()));
      encodeSizedField(
          encoder,
          NoritoAdapters.option(UINT64_ADAPTER),
          Optional.ofNullable(value.creationTimeMs()));
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, optionalString(value.feeSponsor()));
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, optionalString(value.memo()));
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalValidationFeePolicyVersion(value.validationFeePolicyVersion()));
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalValidationFeePolicyHash(value.validationFeePolicyHash()));
      encodeSizedField(encoder, ENCODED_INSTRUCTION_LIST_ADAPTER, value.instructions());
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalValidationFeeInstructionIndex(value.validationFeeInstructionIndex()));
    }

    @Override
    public MultisigProposeRequest decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("MultisigProposeRequest decoding is not supported");
    }
  }

  private static final class ExecutableAdapter implements TypeAdapter<Executable> {
    @Override
    public void encode(final NoritoEncoder encoder, final Executable value) {
      encodeExecutable(encoder, value);
    }

    @Override
    public Executable decode(final NoritoDecoder decoder) {
      return decodeExecutable(decoder);
    }
  }

  private static final class AccountIdAdapter implements TypeAdapter<String> {
    private static final long SINGLE_CONTROLLER_TAG = 0L;
    private static final long MULTISIG_CONTROLLER_TAG = 1L;
    private static final TypeAdapter<ControllerPayload> CONTROLLER_ADAPTER = new AccountControllerAdapter();
    private static final TypeAdapter<AccountAddress.MultisigPolicyPayload> MULTISIG_POLICY_ADAPTER =
        new MultisigPolicyAdapter();
    private static final TypeAdapter<AccountAddress.MultisigMemberPayload> MULTISIG_MEMBER_ADAPTER =
        new MultisigMemberAdapter();
    private static final TypeAdapter<List<AccountAddress.MultisigMemberPayload>> MULTISIG_MEMBER_LIST_ADAPTER =
        NoritoAdapters.sequence(MULTISIG_MEMBER_ADAPTER);

    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      CONTROLLER_ADAPTER.encode(encoder, parseAuthority(value));
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static String decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      final NoritoDecoder controllerDecoder = new NoritoDecoder(payload, flags, flagsHint);
      final ControllerPayload controller = decodeControllerPayload(controllerDecoder);
      if (controllerDecoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after authority payload");
      }
      return renderAuthority(controller);
    }

    private static ControllerPayload decodeControllerPayload(final NoritoDecoder decoder) {
      final long controllerTag = ENUM_TAG_ADAPTER.decode(decoder);
      if (controllerTag == SINGLE_CONTROLLER_TAG) {
        return ControllerPayload.single(decodeSizedField(decoder, BYTE_VECTOR_ADAPTER));
      }
      if (controllerTag == MULTISIG_CONTROLLER_TAG) {
        return ControllerPayload.multisig(decodeSizedField(decoder, MULTISIG_POLICY_ADAPTER));
      }
      throw new IllegalArgumentException("Unsupported AccountController tag: " + controllerTag);
    }

    private static ControllerPayload parseAuthority(final String authority) {
      final String canonicalAuthority =
          AccountIdLiteral.requireCanonicalI105Address(authority, "authority");
      final AccountAddress.ParseResult parsed;
      try {
        parsed = AccountAddress.parseEncodedIgnoringCurveSupport(canonicalAuthority, null);
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException("authority must use canonical I105 encoding", ex);
      }
      return parseAddressToController(parsed.address);
    }

    private static ControllerPayload parseAddressToController(final AccountAddress address) {
      try {
        final java.util.Optional<AccountAddress.SingleKeyPayload> singlePayload =
            address.singleKeyPayloadIgnoringCurveSupport();
        if (singlePayload.isPresent()) {
          final AccountAddress.SingleKeyPayload payload = singlePayload.get();
          final byte[] publicKeyPayload =
              PublicKeyCodec.compactPublicKeyPayload(payload.curveId(), payload.publicKey());
          return ControllerPayload.single(publicKeyPayload);
        }
        final java.util.Optional<AccountAddress.MultisigPolicyPayload> multisigPayload =
            address.multisigPolicyPayloadIgnoringCurveSupport();
        if (multisigPayload.isPresent()) {
          return ControllerPayload.multisig(multisigPayload.get());
        }
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException(
            "Failed to extract controller from canonical I105 account id", ex);
      }
      throw new IllegalArgumentException(
          "Address contains neither single-key nor multisig controller");
    }

    private static String renderAuthority(final ControllerPayload controller) {
      if (controller.isSingle()) {
        final PublicKeyCodec.PublicKeyPayload payload =
            PublicKeyCodec.decodeCompactPublicKeyPayload(controller.publicKeyPayload());
        if (payload == null) {
          throw new IllegalArgumentException("Invalid single-key AccountController payload");
        }
        return renderSingleAuthority(payload);
      }
      return renderMultisigAuthority(controller.multisigPolicy());
    }

    private static final class ControllerPayload {
      private final byte[] publicKeyPayload;
      private final AccountAddress.MultisigPolicyPayload multisigPolicy;

      private ControllerPayload(
          final byte[] publicKeyPayload,
          final AccountAddress.MultisigPolicyPayload multisigPolicy) {
        this.publicKeyPayload =
            publicKeyPayload == null ? null : Arrays.copyOf(publicKeyPayload, publicKeyPayload.length);
        this.multisigPolicy = multisigPolicy;
      }

      private static ControllerPayload single(final byte[] publicKeyPayload) {
        if (publicKeyPayload == null || publicKeyPayload.length == 0) {
          throw new IllegalArgumentException("public key payload must not be empty");
        }
        return new ControllerPayload(publicKeyPayload, null);
      }

      private static ControllerPayload multisig(
          final AccountAddress.MultisigPolicyPayload multisigPolicy) {
        if (multisigPolicy == null) {
          throw new IllegalArgumentException("multisig policy must not be null");
        }
        return new ControllerPayload(null, multisigPolicy);
      }

      private boolean isSingle() {
        return multisigPolicy == null;
      }

      private byte[] publicKeyPayload() {
        return Arrays.copyOf(publicKeyPayload, publicKeyPayload.length);
      }

      private AccountAddress.MultisigPolicyPayload multisigPolicy() {
        return multisigPolicy;
      }
    }

    private static final class AccountControllerAdapter implements TypeAdapter<ControllerPayload> {
      @Override
      public void encode(final NoritoEncoder encoder, final ControllerPayload value) {
        if (value == null) {
          throw new IllegalArgumentException("AccountController payload must not be null");
        }
        if (value.isSingle()) {
          ENUM_TAG_ADAPTER.encode(encoder, SINGLE_CONTROLLER_TAG);
          encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, value.publicKeyPayload());
          return;
        }
        ENUM_TAG_ADAPTER.encode(encoder, MULTISIG_CONTROLLER_TAG);
        encodeSizedField(encoder, MULTISIG_POLICY_ADAPTER, value.multisigPolicy());
      }

      @Override
      public ControllerPayload decode(final NoritoDecoder decoder) {
        final long controllerTag = ENUM_TAG_ADAPTER.decode(decoder);
        final ControllerPayload controller;
        if (controllerTag == SINGLE_CONTROLLER_TAG) {
          final byte[] publicKeyPayload = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER);
          controller = ControllerPayload.single(publicKeyPayload);
        } else if (controllerTag == MULTISIG_CONTROLLER_TAG) {
          final AccountAddress.MultisigPolicyPayload policy =
              decodeSizedField(decoder, MULTISIG_POLICY_ADAPTER);
          controller = ControllerPayload.multisig(policy);
        } else {
          throw new IllegalArgumentException("Unsupported AccountController tag: " + controllerTag);
        }
        if (decoder.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after AccountController payload");
        }
        return controller;
      }
    }

    private static final class MultisigPolicyAdapter
        implements TypeAdapter<AccountAddress.MultisigPolicyPayload> {
      @Override
      public void encode(
          final NoritoEncoder encoder, final AccountAddress.MultisigPolicyPayload value) {
        encodeSizedField(encoder, UINT8_ADAPTER, (long) value.version());
        encodeSizedField(encoder, UINT16_ADAPTER, (long) value.threshold());
        encodeSizedField(encoder, MULTISIG_MEMBER_LIST_ADAPTER, value.members());
      }

      @Override
      public AccountAddress.MultisigPolicyPayload decode(final NoritoDecoder decoder) {
        final int version = Math.toIntExact(decodeSizedField(decoder, UINT8_ADAPTER));
        final int threshold = Math.toIntExact(decodeSizedField(decoder, UINT16_ADAPTER));
        final List<AccountAddress.MultisigMemberPayload> members =
            decodeSizedField(decoder, MULTISIG_MEMBER_LIST_ADAPTER);
        return AccountAddress.MultisigPolicyPayload.of(version, threshold, members);
      }
    }

    private static final class MultisigMemberAdapter
        implements TypeAdapter<AccountAddress.MultisigMemberPayload> {
      @Override
      public void encode(
          final NoritoEncoder encoder, final AccountAddress.MultisigMemberPayload value) {
        final byte[] publicKeyPayload =
            PublicKeyCodec.compactPublicKeyPayload(value.curveId(), value.publicKey());
        encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, publicKeyPayload);
        encodeSizedField(encoder, UINT16_ADAPTER, (long) value.weight());
      }

      @Override
      public AccountAddress.MultisigMemberPayload decode(final NoritoDecoder decoder) {
        final byte[] publicKeyPayload = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER);
        final int weight = Math.toIntExact(decodeSizedField(decoder, UINT16_ADAPTER));
        final PublicKeyCodec.PublicKeyPayload payload =
            PublicKeyCodec.decodeCompactPublicKeyPayload(publicKeyPayload);
        if (payload == null) {
          throw new IllegalArgumentException("Invalid multisig member public key");
        }
        return AccountAddress.MultisigMemberPayload.of(
            payload.curveId(), weight, payload.keyBytes());
      }
    }

    private static String renderSingleAuthority(
        final PublicKeyCodec.PublicKeyPayload payload) {
      final String algorithm = PublicKeyCodec.algorithmForCurveId(payload.curveId());
      if (algorithm == null) {
        throw new IllegalArgumentException(
            "Unsupported curve id in AccountController payload: " + payload.curveId());
      }
      try {
        final AccountAddress address = AccountAddress.fromAccount(payload.keyBytes(), algorithm);
        return address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException("Invalid single-key AccountController payload", ex);
      }
    }

    private static String renderMultisigAuthority(
        final AccountAddress.MultisigPolicyPayload policy) {
      try {
        final AccountAddress address = AccountAddress.fromMultisigPolicy(policy);
        return address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException("Invalid multisig policy for AccountId", ex);
      }
    }
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private static <T> T decodeSizedField(final NoritoDecoder decoder, final TypeAdapter<T> adapter) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Field payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after field payload");
    }
    return value;
  }

  private static String decodeAuthorityField(final NoritoDecoder decoder) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Field payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    return AccountIdAdapter.decodePayload(payload, decoder.flags(), decoder.flagsHint());
  }

  private static Optional<String> optionalString(final String value) {
    if (value == null) {
      return Optional.empty();
    }
    final String normalized = value.trim();
    if (normalized.isEmpty()) {
      return Optional.empty();
    }
    return Optional.of(normalized);
  }

  private static Optional<String> optionalValidationFeePolicyVersion(final Long value) {
    if (value == null) {
      return Optional.empty();
    }
    if (value.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeePolicyVersion must be non-negative");
    }
    return Optional.of(value.toString());
  }

  private static Optional<String> optionalValidationFeePolicyHash(final String value) {
    if (value == null) {
      return Optional.empty();
    }
    return Optional.of(normalizeValidationFeePolicyHash(value));
  }

  private static Optional<String> optionalValidationFeeInstructionIndex(final Long value) {
    if (value == null) {
      return Optional.empty();
    }
    if (value.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeeInstructionIndex must be non-negative");
    }
    return Optional.of(value.toString());
  }

  private static String normalizeValidationFeePolicyHash(final String value) {
    final String normalized =
        requireNonBlank(value, "validationFeePolicyHash").toLowerCase(Locale.ROOT);
    if (normalized.length() != 64) {
      throw new IllegalArgumentException("validationFeePolicyHash must contain 64 hex characters");
    }
    for (int index = 0; index < normalized.length(); index++) {
      final char character = normalized.charAt(index);
      final boolean isHex =
          (character >= '0' && character <= '9')
              || (character >= 'a' && character <= 'f');
      if (!isHex) {
        throw new IllegalArgumentException("validationFeePolicyHash must contain 64 hex characters");
      }
    }
    return normalized;
  }

  private static String requireNonBlank(final String value, final String fieldName) {
    if (value == null) {
      throw new IllegalArgumentException(fieldName + " must not be null");
    }
    final String normalized = value.trim();
    if (normalized.isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    return normalized;
  }

  private static void validateMultisigProposeRequest(final MultisigProposeRequest request) {
    if (request == null) {
      throw new IllegalArgumentException("request must not be null");
    }
    final boolean hasAccountId = optionalString(request.multisigAccountId()).isPresent();
    final boolean hasAlias = optionalString(request.multisigAccountAlias()).isPresent();
    if (hasAccountId == hasAlias) {
      throw new IllegalArgumentException(
          "Exactly one of multisigAccountId or multisigAccountAlias must be provided");
    }
    requireNonBlank(request.signerAccountId(), "signerAccountId");
    if (request.instructions().isEmpty()) {
      throw new IllegalArgumentException("instructions must not be empty");
    }
    if (request.creationTimeMs() != null && request.creationTimeMs().longValue() < 0L) {
      throw new IllegalArgumentException("creationTimeMs must be non-negative");
    }
    final boolean hasPolicyVersion = request.validationFeePolicyVersion() != null;
    final boolean hasPolicyHash = request.validationFeePolicyHash() != null;
    final boolean hasInstructionIndex = request.validationFeeInstructionIndex() != null;
    if (hasPolicyVersion != hasPolicyHash) {
      throw new IllegalArgumentException(
          "validationFeePolicyVersion and validationFeePolicyHash must be provided together");
    }
    if (!hasPolicyVersion && hasInstructionIndex) {
      throw new IllegalArgumentException(
          "validationFeeInstructionIndex requires validation fee policy metadata");
    }
    optionalValidationFeePolicyVersion(request.validationFeePolicyVersion());
    optionalValidationFeePolicyHash(request.validationFeePolicyHash());
    optionalValidationFeeInstructionIndex(request.validationFeeInstructionIndex());
  }

  private static InstructionBox tryDecodeWireInstruction(
      final byte[] payload, final int flags, final int flagsHint) {
    try {
      final NoritoDecoder wireDecoder = new NoritoDecoder(payload, flags, flagsHint);
      final String wireName = decodeSizedField(wireDecoder, STRING_ADAPTER);
      final byte[] wirePayload = decodeSizedField(wireDecoder, RAW_BYTE_VEC_ADAPTER);
      if (wireDecoder.remaining() != 0) {
        return null;
      }
      if (!isWirePayloadCandidate(wireName, wirePayload)) {
        return null;
      }
      return InstructionBox.fromWirePayload(wireName, wirePayload);
    } catch (final IllegalArgumentException ex) {
      return null;
    }
  }

  private static boolean isWirePayloadCandidate(final String wireName, final byte[] payload) {
    if (wireName == null || wireName.isBlank()) {
      return false;
    }
    if (payload == null || payload.length < NoritoHeader.HEADER_LENGTH) {
      return false;
    }
    if (payload[0] != 'N' || payload[1] != 'R' || payload[2] != 'T' || payload[3] != '0') {
      return false;
    }
    try {
      final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(payload, null);
      decoded.header().validateChecksum(decoded.payload());
      return true;
    } catch (final IllegalArgumentException ex) {
      return false;
    }
  }

  private static final class ChainIdAdapter implements TypeAdapter<String> {
    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      encodeSizedField(encoder, STRING_ADAPTER, value);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static String decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      final NoritoDecoder sized = new NoritoDecoder(payload, flags, flagsHint);
      final String value = decodeSizedField(sized, STRING_ADAPTER);
      if (sized.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after ChainId payload");
      }
      return value;
    }
  }

  private static final class IvmBytecodeAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static byte[] decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      final NoritoDecoder sized = new NoritoDecoder(payload, flags, flagsHint);
      final byte[] value = decodeSizedField(sized, RAW_BYTE_VEC_ADAPTER);
      if (sized.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after IVM payload");
      }
      return value;
    }
  }

  private static final class JsonAdapter implements TypeAdapter<String> {
    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      if (value == null) {
        throw new IllegalArgumentException("Metadata values must not be null");
      }
      encodeSizedField(encoder, STRING_ADAPTER, value);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      return decodeSizedField(decoder, STRING_ADAPTER);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class MetadataAdapter implements TypeAdapter<Map<String, JsonValue>> {
    private static final TypeAdapter<List<MetadataEntry>> ENTRY_LIST_ADAPTER =
        NoritoAdapters.sequence(new MetadataEntryAdapter());

    @Override
    public void encode(final NoritoEncoder encoder, final Map<String, JsonValue> value) {
      final List<MetadataEntry> entries = new ArrayList<>(value.size());
      final List<String> keys = new ArrayList<>(value.keySet());
      Collections.sort(keys);
      for (final String key : keys) {
        final JsonValue entryValue = value.get(key);
        if (entryValue == null) {
          throw new IllegalArgumentException("Metadata values must not be null");
        }
        entries.add(new MetadataEntry(key, entryValue));
      }
      ENTRY_LIST_ADAPTER.encode(encoder, entries);
    }

    @Override
    public Map<String, JsonValue> decode(final NoritoDecoder decoder) {
      final List<MetadataEntry> entries = ENTRY_LIST_ADAPTER.decode(decoder);
      final Map<String, JsonValue> decoded = new LinkedHashMap<>(entries.size());
      for (final MetadataEntry entry : entries) {
        if (decoded.put(entry.key(), entry.value()) != null) {
          throw new IllegalArgumentException("Duplicate metadata key");
        }
      }
      return decoded;
    }
  }

  private static final class MetadataEntry {
    private final String key;
    private final JsonValue value;

    private MetadataEntry(final String key, final JsonValue value) {
      this.key = key;
      this.value = value;
    }

    private String key() {
      return key;
    }

    private JsonValue value() {
      return value;
    }
  }

  private static final class MetadataEntryAdapter implements TypeAdapter<MetadataEntry> {
    @Override
    public void encode(final NoritoEncoder encoder, final MetadataEntry entry) {
      encodeSizedField(encoder, STRING_ADAPTER, entry.key());
      encodeSizedField(encoder, JSON_VALUE_ADAPTER, entry.value().rawJson());
    }

    @Override
    public MetadataEntry decode(final NoritoDecoder decoder) {
      final String key = decodeSizedField(decoder, STRING_ADAPTER);
      final String value = decodeSizedField(decoder, JSON_VALUE_ADAPTER);
      return new MetadataEntry(key, JsonValue.raw(value));
    }
  }

}
