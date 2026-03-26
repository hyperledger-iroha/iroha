package org.hyperledger.iroha.android.norito;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Norito adapter that mirrors the {@link TransactionPayload} structure used by the Android library.
 * IVM bytecode payloads are encoded directly. Instruction payloads must be provided as wire-framed
 * Norito blobs (wire id + Norito header). Metadata values are encoded as JSON strings to match the
 * Rust `Json` wrapper.
 */
final class TransactionPayloadAdapter implements TypeAdapter<TransactionPayload> {

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<String> ACCOUNT_ID_ADAPTER = new AccountIdAdapter();
  private static final TypeAdapter<String> CHAIN_ID_ADAPTER = new ChainIdAdapter();
  private static final JsonStringAdapter JSON_STRING_ADAPTER = new JsonStringAdapter();
  private static final TypeAdapter<String> JSON_ADAPTER = new JsonAdapter();
  private static final TypeAdapter<Long> UINT64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> UINT32_AS_LONG_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> UINT16_ADAPTER = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT8_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<byte[]> RAW_BYTE_VEC_ADAPTER = NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<byte[]> IVM_BYTECODE_ADAPTER = new IvmBytecodeAdapter();
  private static final TypeAdapter<List<InstructionBox>> INSTRUCTION_LIST_ADAPTER =
      NoritoAdapters.sequence(new InstructionAdapter());
  private static final TypeAdapter<Long> ENUM_TAG_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<Optional<Long>> TTL_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(64));
  private static final TypeAdapter<Optional<Long>> NONCE_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(32));
  private static final TypeAdapter<Executable> EXECUTABLE_ADAPTER = new ExecutableAdapter();
  private static final TypeAdapter<Map<String, String>> METADATA_ADAPTER = new MetadataAdapter();

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
    final Map<String, String> metadata =
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

  private static void encodeExecutable(final NoritoEncoder encoder, final Executable executable) {
    if (executable.isIvm()) {
      ENUM_TAG_ADAPTER.encode(encoder, 1L);
      encodeSizedField(encoder, IVM_BYTECODE_ADAPTER, executable.ivmBytes());
      return;
    }
    ENUM_TAG_ADAPTER.encode(encoder, 0L);
    encodeSizedField(encoder, INSTRUCTION_LIST_ADAPTER, executable.instructions());
  }

  private static Executable decodeExecutable(final NoritoDecoder decoder) {
    final long tag = ENUM_TAG_ADAPTER.decode(decoder);
    if (tag == 1L) {
      final byte[] bytes = decodeSizedField(decoder, IVM_BYTECODE_ADAPTER);
      return Executable.ivm(bytes);
    }
    if (tag == 0L) {
      final List<InstructionBox> instructions = decodeSizedField(decoder, INSTRUCTION_LIST_ADAPTER);
      return Executable.instructions(instructions);
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
        return ControllerPayload.single(decodeSizedField(decoder, STRING_ADAPTER));
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
        throw new IllegalArgumentException("authority must use canonical i105 encoding", ex);
      }
      return parseAddressToController(parsed.address);
    }

    private static ControllerPayload parseAddressToController(final AccountAddress address) {
      try {
        final java.util.Optional<AccountAddress.SingleKeyPayload> singlePayload =
            address.singleKeyPayloadIgnoringCurveSupport();
        if (singlePayload.isPresent()) {
          final AccountAddress.SingleKeyPayload payload = singlePayload.get();
          final String publicKey =
              PublicKeyCodec.encodePublicKeyMultihash(payload.curveId(), payload.publicKey());
          return ControllerPayload.single(publicKey);
        }
        final java.util.Optional<AccountAddress.MultisigPolicyPayload> multisigPayload =
            address.multisigPolicyPayloadIgnoringCurveSupport();
        if (multisigPayload.isPresent()) {
          return ControllerPayload.multisig(multisigPayload.get());
        }
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException(
            "Failed to extract controller from canonical i105 account id", ex);
      }
      throw new IllegalArgumentException(
          "Address contains neither single-key nor multisig controller");
    }

    private static String renderAuthority(final ControllerPayload controller) {
      if (controller.isSingle()) {
        final String publicKeyLiteral = controller.publicKeyLiteral();
        final PublicKeyCodec.PublicKeyPayload payload =
            PublicKeyCodec.decodePublicKeyLiteral(publicKeyLiteral);
        if (payload == null) {
          throw new IllegalArgumentException("Invalid single-key AccountController payload");
        }
        return renderSingleAuthority(payload);
      }
      return renderMultisigAuthority(controller.multisigPolicy());
    }

    private static final class ControllerPayload {
      private final String publicKeyLiteral;
      private final AccountAddress.MultisigPolicyPayload multisigPolicy;

      private ControllerPayload(
          final String publicKeyLiteral,
          final AccountAddress.MultisigPolicyPayload multisigPolicy) {
        this.publicKeyLiteral = publicKeyLiteral;
        this.multisigPolicy = multisigPolicy;
      }

      private static ControllerPayload single(final String publicKeyLiteral) {
        if (publicKeyLiteral == null || publicKeyLiteral.isBlank()) {
          throw new IllegalArgumentException("public key literal must not be blank");
        }
        return new ControllerPayload(publicKeyLiteral.trim(), null);
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

      private String publicKeyLiteral() {
        return publicKeyLiteral;
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
          encodeSizedField(encoder, STRING_ADAPTER, value.publicKeyLiteral());
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
          final String publicKeyLiteral = decodeSizedField(decoder, STRING_ADAPTER);
          controller = ControllerPayload.single(publicKeyLiteral);
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
        UINT8_ADAPTER.encode(encoder, (long) value.version());
        UINT16_ADAPTER.encode(encoder, (long) value.threshold());
        MULTISIG_MEMBER_LIST_ADAPTER.encode(encoder, value.members());
      }

      @Override
      public AccountAddress.MultisigPolicyPayload decode(final NoritoDecoder decoder) {
        final int version = Math.toIntExact(UINT8_ADAPTER.decode(decoder));
        final int threshold = Math.toIntExact(UINT16_ADAPTER.decode(decoder));
        final List<AccountAddress.MultisigMemberPayload> members =
            MULTISIG_MEMBER_LIST_ADAPTER.decode(decoder);
        return AccountAddress.MultisigPolicyPayload.of(version, threshold, members);
      }
    }

    private static final class MultisigMemberAdapter
        implements TypeAdapter<AccountAddress.MultisigMemberPayload> {
      @Override
      public void encode(
          final NoritoEncoder encoder, final AccountAddress.MultisigMemberPayload value) {
        final String publicKey =
            PublicKeyCodec.encodePublicKeyMultihash(value.curveId(), value.publicKey());
        STRING_ADAPTER.encode(encoder, publicKey);
        UINT16_ADAPTER.encode(encoder, (long) value.weight());
      }

      @Override
      public AccountAddress.MultisigMemberPayload decode(final NoritoDecoder decoder) {
        final String publicKeyLiteral = STRING_ADAPTER.decode(decoder);
        final int weight = Math.toIntExact(UINT16_ADAPTER.decode(decoder));
        final PublicKeyCodec.PublicKeyPayload payload =
            PublicKeyCodec.decodePublicKeyLiteral(publicKeyLiteral);
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
      encodeSizedField(encoder, JSON_STRING_ADAPTER, value);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      return decodeSizedField(decoder, JSON_STRING_ADAPTER);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class MetadataAdapter implements TypeAdapter<Map<String, String>> {
    private static final TypeAdapter<List<MetadataEntry>> ENTRY_LIST_ADAPTER =
        NoritoAdapters.sequence(new MetadataEntryAdapter());

    @Override
    public void encode(final NoritoEncoder encoder, final Map<String, String> value) {
      final List<MetadataEntry> entries = new ArrayList<>(value.size());
      final List<String> keys = new ArrayList<>(value.keySet());
      Collections.sort(keys);
      for (final String key : keys) {
        final String entryValue = value.get(key);
        if (entryValue == null) {
          throw new IllegalArgumentException("Metadata values must not be null");
        }
        entries.add(new MetadataEntry(key, entryValue));
      }
      ENTRY_LIST_ADAPTER.encode(encoder, entries);
    }

    @Override
    public Map<String, String> decode(final NoritoDecoder decoder) {
      final List<MetadataEntry> entries = ENTRY_LIST_ADAPTER.decode(decoder);
      final Map<String, String> decoded = new LinkedHashMap<>(entries.size());
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
    private final String value;

    private MetadataEntry(final String key, final String value) {
      this.key = key;
      this.value = value;
    }

    private String key() {
      return key;
    }

    private String value() {
      return value;
    }
  }

  private static final class MetadataEntryAdapter implements TypeAdapter<MetadataEntry> {
    @Override
    public void encode(final NoritoEncoder encoder, final MetadataEntry entry) {
      encodeSizedField(encoder, STRING_ADAPTER, entry.key());
      encodeSizedField(encoder, JSON_ADAPTER, entry.value());
    }

    @Override
    public MetadataEntry decode(final NoritoDecoder decoder) {
      final String key = decodeSizedField(decoder, STRING_ADAPTER);
      final String value = decodeSizedField(decoder, JSON_ADAPTER);
      return new MetadataEntry(key, value);
    }
  }

  private static final class JsonStringAdapter implements TypeAdapter<String> {
    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      if (value == null) {
        throw new IllegalArgumentException("Metadata values must not be null");
      }
      STRING_ADAPTER.encode(encoder, encodeJsonString(value));
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final String raw = STRING_ADAPTER.decode(decoder);
      return decodeJsonString(raw);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static String encodeJsonString(final String value) {
    final StringBuilder builder = new StringBuilder(value.length() + 2);
    builder.append('"');
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      switch (c) {
        case '"' -> builder.append("\\\"");
        case '\\' -> builder.append("\\\\");
        case '\b' -> builder.append("\\b");
        case '\f' -> builder.append("\\f");
        case '\n' -> builder.append("\\n");
        case '\r' -> builder.append("\\r");
        case '\t' -> builder.append("\\t");
        default -> {
          if (c < 0x20) {
            builder.append("\\u00");
            builder.append(HEX_DIGITS[(c >> 4) & 0xF]);
            builder.append(HEX_DIGITS[c & 0xF]);
          } else {
            builder.append(c);
          }
        }
      }
    }
    builder.append('"');
    return builder.toString();
  }

  private static String decodeJsonString(final String raw) {
    if (raw == null) {
      return null;
    }
    final String trimmed = raw.trim();
    if (trimmed.length() < 2 || trimmed.charAt(0) != '"' || trimmed.charAt(trimmed.length() - 1) != '"') {
      return raw;
    }
    try {
      return parseJsonString(trimmed);
    } catch (final IllegalArgumentException ex) {
      return raw;
    }
  }

  private static String parseJsonString(final String input) {
    final StringBuilder builder = new StringBuilder();
    for (int i = 1; i < input.length() - 1; ) {
      final char c = input.charAt(i++);
      if (c == '\\') {
        if (i >= input.length() - 1) {
          throw new IllegalArgumentException("Invalid JSON escape");
        }
        final char esc = input.charAt(i++);
        switch (esc) {
          case '"' -> builder.append('"');
          case '\\' -> builder.append('\\');
          case '/' -> builder.append('/');
          case 'b' -> builder.append('\b');
          case 'f' -> builder.append('\f');
          case 'n' -> builder.append('\n');
          case 'r' -> builder.append('\r');
          case 't' -> builder.append('\t');
          case 'u' -> {
            if (i + 4 > input.length() - 1) {
              throw new IllegalArgumentException("Invalid unicode escape");
            }
            int codePoint = 0;
            for (int j = 0; j < 4; j++) {
              codePoint = (codePoint << 4) | hexNibble(input.charAt(i + j));
            }
            builder.append((char) codePoint);
            i += 4;
          }
          default -> throw new IllegalArgumentException("Unsupported escape: \\" + esc);
        }
      } else {
        builder.append(c);
      }
    }
    return builder.toString();
  }

  private static int hexNibble(final char c) {
    if (c >= '0' && c <= '9') {
      return c - '0';
    }
    if (c >= 'a' && c <= 'f') {
      return 10 + (c - 'a');
    }
    if (c >= 'A' && c <= 'F') {
      return 10 + (c - 'A');
    }
    throw new IllegalArgumentException("Invalid hex digit: " + c);
  }

  private static final char[] HEX_DIGITS = "0123456789ABCDEF".toCharArray();
}
