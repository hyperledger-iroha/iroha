package org.hyperledger.iroha.android.norito;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.hyperledger.iroha.norito.Varint;

/**
 * Norito adapter that mirrors the {@link TransactionPayload} structure used by the Android library.
 * IVM bytecode payloads are encoded directly. Instruction payloads are preserved when provided as
 * wire-framed Norito blobs (wire id + Norito header), while legacy argument-map encoding remains
 * available for backwards-compatible decoding. Metadata values are encoded as JSON strings to match
 * the Rust `Json` wrapper.
 */
final class TransactionPayloadAdapter implements TypeAdapter<TransactionPayload> {

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<String> DOMAIN_ID_ADAPTER = new DomainIdAdapter();
  private static final TypeAdapter<String> ACCOUNT_ID_ADAPTER = new AccountIdAdapter();
  private static final TypeAdapter<String> CHAIN_ID_ADAPTER = new ChainIdAdapter();
  private static final JsonStringAdapter JSON_STRING_ADAPTER = new JsonStringAdapter();
  private static final TypeAdapter<String> JSON_ADAPTER = new JsonAdapter();
  private static final TypeAdapter<Long> UINT64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> UINT32_AS_LONG_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> IVM_BYTECODE_ADAPTER = new IvmBytecodeAdapter();
  private static final TypeAdapter<Map<String, String>> STRING_MAP_ADAPTER =
      NoritoAdapters.map(STRING_ADAPTER, STRING_ADAPTER);
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
        encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, wire.payloadBytes());
        return;
      }
      // TODO: Switch legacy instructions to canonical wire payloads once codegen encoders land.
      encodeSizedField(encoder, ENUM_TAG_ADAPTER, (long) value.kind().discriminant());
      encodeSizedField(encoder, STRING_MAP_ADAPTER, value.arguments());
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
      return decodeLegacyInstruction(payload, decoder.flags(), decoder.flagsHint());
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
    private static final TypeAdapter<String> CONTROLLER_ADAPTER = new AccountControllerAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      final AccountIdSingle single = parseAuthoritySingle(value);
      if (single == null) {
        STRING_ADAPTER.encode(encoder, value);
        return;
      }
      encodeSizedField(encoder, DOMAIN_ID_ADAPTER, single.domain);
      encodeSizedField(encoder, CONTROLLER_ADAPTER, single.publicKey);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static String decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      try {
        return decodeAccountIdStruct(payload, flags, flagsHint);
      } catch (final IllegalArgumentException ex) {
        final NoritoDecoder stringDecoder = new NoritoDecoder(payload, flags, flagsHint);
        final String literal = STRING_ADAPTER.decode(stringDecoder);
        if (stringDecoder.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after authority payload");
        }
        return literal;
      }
    }

    private static String decodeAccountIdStruct(
        final byte[] payload, final int flags, final int flagsHint) {
      try {
        return decodeAccountIdSized(payload, flags, flagsHint);
      } catch (final IllegalArgumentException ex) {
        return decodeAccountIdLegacy(payload, flags, flagsHint);
      }
    }

    private static String decodeAccountIdSized(
        final byte[] payload, final int flags, final int flagsHint) {
      final NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);
      final String domain = decodeSizedField(decoder, DOMAIN_ID_ADAPTER);
      final String publicKeyLiteral = decodeSizedField(decoder, CONTROLLER_ADAPTER);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after AccountId payload");
      }
      final PublicKeyPayload payloadData = decodePublicKeyLiteral(publicKeyLiteral);
      if (payloadData != null) {
        final String ih58 = toIh58(domain, payloadData);
        if (ih58 != null) {
          return ih58 + "@" + domain;
        }
      }
      return publicKeyLiteral + "@" + domain;
    }

    private static String decodeAccountIdLegacy(
        final byte[] payload, final int flags, final int flagsHint) {
      final NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);
      final String domain = STRING_ADAPTER.decode(decoder);
      final long controllerTag = ENUM_TAG_ADAPTER.decode(decoder);
      if (controllerTag != SINGLE_CONTROLLER_TAG) {
        // TODO: Support multisig controller payloads once the SDK exposes them.
        throw new IllegalArgumentException("Unsupported AccountController tag: " + controllerTag);
      }
      final String publicKeyLiteral = STRING_ADAPTER.decode(decoder);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after AccountId payload");
      }
      final PublicKeyPayload payloadData = decodePublicKeyLiteral(publicKeyLiteral);
      if (payloadData != null) {
        final String ih58 = toIh58(domain, payloadData);
        if (ih58 != null) {
          return ih58 + "@" + domain;
        }
      }
      return publicKeyLiteral + "@" + domain;
    }

    private static AccountIdSingle parseAuthoritySingle(final String authority) {
      if (authority == null || authority.isBlank()) {
        throw new IllegalArgumentException("authority must not be blank");
      }
      final String trimmed = authority.trim();
      final int atIndex = trimmed.lastIndexOf('@');
      if (atIndex <= 0 || atIndex == trimmed.length() - 1) {
        return null;
      }
      final String identifier = trimmed.substring(0, atIndex);
      final String domain = trimmed.substring(atIndex + 1);
      if (domain.isBlank()) {
        return null;
      }
      final String publicKey = parseIdentifierToPublicKey(identifier);
      if (publicKey == null) {
        return null;
      }
      return new AccountIdSingle(domain, publicKey);
    }

    private static String parseIdentifierToPublicKey(final String identifier) {
      try {
        final AccountAddress.ParseResult parsed = AccountAddress.parseAny(identifier, null);
        final java.util.Optional<AccountAddress.SingleKeyPayload> singlePayload =
            parsed.address.singleKeyPayload();
        if (singlePayload.isPresent()) {
          final AccountAddress.SingleKeyPayload payload = singlePayload.get();
          return encodePublicKeyMultihash(payload.curveId(), payload.publicKey());
        }
      } catch (final AccountAddress.AccountAddressException ignored) {
        // Fall through to multihash parsing.
      }
      final PublicKeyPayload payload = decodePublicKeyLiteral(identifier);
      if (payload == null) {
        return null;
      }
      return encodePublicKeyMultihash(payload.curveId, payload.keyBytes);
    }

    private static final class AccountControllerAdapter implements TypeAdapter<String> {
      @Override
      public void encode(final NoritoEncoder encoder, final String value) {
        ENUM_TAG_ADAPTER.encode(encoder, SINGLE_CONTROLLER_TAG);
        encodeSizedField(encoder, STRING_ADAPTER, value);
      }

      @Override
      public String decode(final NoritoDecoder decoder) {
        final long controllerTag = ENUM_TAG_ADAPTER.decode(decoder);
        if (controllerTag != SINGLE_CONTROLLER_TAG) {
          throw new IllegalArgumentException("Unsupported AccountController tag: " + controllerTag);
        }
        final String publicKeyLiteral = decodeSizedField(decoder, STRING_ADAPTER);
        if (decoder.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after AccountController payload");
        }
        return publicKeyLiteral;
      }
    }

    private static String toIh58(final String domain, final PublicKeyPayload payload) {
      final String algorithm = algorithmForCurveId(payload.curveId);
      if (algorithm == null) {
        return null;
      }
      try {
        final AccountAddress address =
            AccountAddress.fromAccount(domain, payload.keyBytes, algorithm);
        return address.toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
      } catch (final AccountAddress.AccountAddressException ex) {
        return null;
      }
    }

    private static String encodePublicKeyMultihash(final int curveId, final byte[] keyBytes) {
      final long code = multihashCodeForCurveId(curveId);
      final byte[] codeVarint = Varint.encode(code);
      final byte[] lenVarint = Varint.encode(keyBytes.length);
      final StringBuilder builder =
          new StringBuilder((codeVarint.length + lenVarint.length + keyBytes.length) * 2);
      appendHexLower(builder, codeVarint);
      appendHexLower(builder, lenVarint);
      appendHexUpper(builder, keyBytes);
      return builder.toString();
    }

    private static PublicKeyPayload decodePublicKeyLiteral(final String literal) {
      if (literal == null || literal.isBlank()) {
        return null;
      }
      String trimmed = literal.trim();
      final int colonIndex = trimmed.indexOf(':');
      if (colonIndex > 0) {
        trimmed = trimmed.substring(colonIndex + 1);
      }
      if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
        return null;
      }
      if ((trimmed.length() & 1) == 1) {
        return null;
      }
      if (!trimmed.matches("(?i)[0-9a-f]+")) {
        return null;
      }
      final byte[] bytes = hexToBytes(trimmed);
      final Varint.DecodeResult code = Varint.decode(bytes, 0);
      final Varint.DecodeResult len = Varint.decode(bytes, code.nextOffset());
      if (len.value() > Integer.MAX_VALUE) {
        return null;
      }
      final int payloadOffset = len.nextOffset();
      final int payloadLength = (int) len.value();
      if (payloadOffset + payloadLength != bytes.length) {
        return null;
      }
      final int curveId = curveIdForMultihashCode(code.value());
      if (curveId < 0) {
        return null;
      }
      final byte[] keyBytes =
          Arrays.copyOfRange(bytes, payloadOffset, payloadOffset + payloadLength);
      return new PublicKeyPayload(curveId, keyBytes);
    }

    private static int curveIdForMultihashCode(final long code) {
      if (code == 0xedL) {
        return 0x01;
      }
      if (code == 0xeeL) {
        return 0x02;
      }
      if (code == 0x1200L) {
        return 0x0A;
      }
      if (code == 0x1201L) {
        return 0x0B;
      }
      if (code == 0x1202L) {
        return 0x0C;
      }
      if (code == 0x1203L) {
        return 0x0D;
      }
      if (code == 0x1204L) {
        return 0x0E;
      }
      if (code == 0x1306L) {
        return 0x0F;
      }
      return -1;
    }

    private static long multihashCodeForCurveId(final int curveId) {
      switch (curveId) {
        case 0x01:
          return 0xedL;
        case 0x02:
          return 0xeeL;
        case 0x0A:
          return 0x1200L;
        case 0x0B:
          return 0x1201L;
        case 0x0C:
          return 0x1202L;
        case 0x0D:
          return 0x1203L;
        case 0x0E:
          return 0x1204L;
        case 0x0F:
          return 0x1306L;
        default:
          throw new IllegalArgumentException("Unsupported curve id: " + curveId);
      }
    }

    private static String algorithmForCurveId(final int curveId) {
      switch (curveId) {
        case 0x01:
          return "ed25519";
        case 0x02:
          return "ml-dsa";
        case 0x0A:
          return "gost256a";
        case 0x0B:
          return "gost256b";
        case 0x0C:
          return "gost256c";
        case 0x0D:
          return "gost512a";
        case 0x0E:
          return "gost512b";
        case 0x0F:
          return "sm2";
        default:
          return null;
      }
    }

    private static byte[] hexToBytes(final String hex) {
      final int len = hex.length();
      final byte[] out = new byte[len / 2];
      for (int i = 0; i < len; i += 2) {
        final int high = Character.digit(hex.charAt(i), 16);
        final int low = Character.digit(hex.charAt(i + 1), 16);
        if (high < 0 || low < 0) {
          throw new IllegalArgumentException("Invalid hex literal");
        }
        out[i / 2] = (byte) ((high << 4) + low);
      }
      return out;
    }

    private static void appendHexLower(final StringBuilder builder, final byte[] bytes) {
      for (final byte b : bytes) {
        builder.append(String.format(Locale.ROOT, "%02x", b & 0xFF));
      }
    }

    private static void appendHexUpper(final StringBuilder builder, final byte[] bytes) {
      for (final byte b : bytes) {
        builder.append(String.format(Locale.ROOT, "%02X", b & 0xFF));
      }
    }
  }

  private static final class AccountIdSingle {
    private final String domain;
    private final String publicKey;

    private AccountIdSingle(final String domain, final String publicKey) {
      this.domain = domain;
      this.publicKey = publicKey;
    }
  }

  private static final class DomainIdAdapter implements TypeAdapter<String> {
    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      encodeSizedField(encoder, STRING_ADAPTER, value);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodeDomainPayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static String decodeDomainPayload(
        final byte[] payload, final int flags, final int flagsHint) {
      try {
        final NoritoDecoder child = new NoritoDecoder(payload, flags, flagsHint);
        final String domain = decodeSizedField(child, STRING_ADAPTER);
        if (child.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after DomainId payload");
        }
        return domain;
      } catch (final IllegalArgumentException ex) {
        final NoritoDecoder child = new NoritoDecoder(payload, flags, flagsHint);
        final String domain = STRING_ADAPTER.decode(child);
        if (child.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after DomainId payload");
        }
        return domain;
      }
    }
  }

  private static final class PublicKeyPayload {
    private final int curveId;
    private final byte[] keyBytes;

    private PublicKeyPayload(final int curveId, final byte[] keyBytes) {
      this.curveId = curveId;
      this.keyBytes = keyBytes;
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
      final byte[] wirePayload = decodeSizedField(wireDecoder, BYTE_VECTOR_ADAPTER);
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

  private static InstructionBox decodeLegacyInstruction(
      final byte[] payload, final int flags, final int flagsHint) {
    final NoritoDecoder legacyDecoder = new NoritoDecoder(payload, flags, flagsHint);
    final long discriminant = decodeSizedField(legacyDecoder, ENUM_TAG_ADAPTER);
    final Map<String, String> arguments =
        new LinkedHashMap<>(decodeSizedField(legacyDecoder, STRING_MAP_ADAPTER));
    if (legacyDecoder.remaining() != 0) {
      throw new IllegalArgumentException("Instruction payload has trailing bytes");
    }
    final InstructionKind kind = InstructionKind.fromDiscriminant(discriminant);
    return InstructionBox.fromNorito(kind, arguments);
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
      STRING_ADAPTER.encode(encoder, value);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static String decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      try {
        final NoritoDecoder direct = new NoritoDecoder(payload, flags, flagsHint);
        final String value = STRING_ADAPTER.decode(direct);
        if (direct.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after ChainId payload");
        }
        return value;
      } catch (final IllegalArgumentException ex) {
        final NoritoDecoder legacy = new NoritoDecoder(payload, flags, flagsHint);
        final String value = decodeSizedField(legacy, STRING_ADAPTER);
        if (legacy.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after ChainId legacy payload");
        }
        return value;
      }
    }
  }

  private static final class IvmBytecodeAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      BYTE_VECTOR_ADAPTER.encode(encoder, value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static byte[] decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      try {
        final NoritoDecoder direct = new NoritoDecoder(payload, flags, flagsHint);
        final byte[] value = BYTE_VECTOR_ADAPTER.decode(direct);
        if (direct.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after IVM payload");
        }
        return value;
      } catch (final IllegalArgumentException ex) {
        final NoritoDecoder legacy = new NoritoDecoder(payload, flags, flagsHint);
        final byte[] value = decodeSizedField(legacy, BYTE_VECTOR_ADAPTER);
        if (legacy.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after IVM legacy payload");
        }
        return value;
      }
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
