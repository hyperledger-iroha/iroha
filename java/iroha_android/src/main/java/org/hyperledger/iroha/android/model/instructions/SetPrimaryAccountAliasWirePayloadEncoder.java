package org.hyperledger.iroha.android.model.instructions;

import java.util.Optional;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Encodes {@code identity::SetPrimaryAccountAlias} instructions in wire-framed Norito format. */
public final class SetPrimaryAccountAliasWirePayloadEncoder {

  public static final String WIRE_NAME = "identity::SetPrimaryAccountAlias";

  private static final String SCHEMA_PATH =
      "iroha_data_model::isi::domain_link::SetPrimaryAccountAlias";

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<AccountAliasDomainPayload> ACCOUNT_ALIAS_DOMAIN_ADAPTER =
      NoritoAdapters.transparent(
          STRING_ADAPTER, value -> value.value, AccountAliasDomainPayload::new);
  private static final TypeAdapter<Optional<AccountAliasPayload>> OPTIONAL_ALIAS_ADAPTER =
      NoritoAdapters.option(new AccountAliasPayloadAdapter());
  private static final TypeAdapter<Optional<Long>> OPTIONAL_U64_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(64));

  private SetPrimaryAccountAliasWirePayloadEncoder() {}

  public static InstructionBox encode(
      final String accountId, final String alias, final String aliasDomain) {
    return encode(accountId, alias, aliasDomain, 0L);
  }

  public static InstructionBox encode(
      final String accountId, final String alias, final String aliasDomain, final long dataspace) {
    if (dataspace < 0L) {
      throw new IllegalArgumentException("dataspace must be non-negative");
    }
    final String normalizedAccountId = requireNonBlank(accountId, "accountId");
    final String normalizedAlias = requireUsername(alias, "alias");
    final Optional<AccountAliasDomainPayload> normalizedAliasDomain =
        Optional.ofNullable(aliasDomain)
            .map(value -> requireAliasDomain(value, "aliasDomain"))
            .map(AccountAliasDomainPayload::new);
    final byte[] accountPayload =
        TransferWirePayloadEncoder.encodeAccountIdPayload(normalizedAccountId);
    final byte[] wirePayload =
        NoritoCodec.encode(
            new SetPrimaryAccountAliasPayload(
                accountPayload,
                Optional.of(
                    new AccountAliasPayload(normalizedAlias, normalizedAliasDomain, dataspace)),
                Optional.empty()),
            SCHEMA_PATH,
            new SetPrimaryAccountAliasPayloadAdapter());
    return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload);
  }

  /** Decodes a Norito-framed {@code SetPrimaryAccountAlias} payload. */
  static DecodedSetPrimaryAccountAliasPayload decodePayload(final byte[] wirePayload) {
    Objects.requireNonNull(wirePayload, "wirePayload");
    final SetPrimaryAccountAliasPayload payload =
        NoritoCodec.decode(wirePayload, new SetPrimaryAccountAliasPayloadAdapter(), SCHEMA_PATH);
    return new DecodedSetPrimaryAccountAliasPayload(
        TransferWirePayloadEncoder.decodeAccountIdPayload(payload.accountPayload),
        payload.alias.map(
            alias ->
                new DecodedAccountAlias(
                    alias.alias, alias.aliasDomain.map(domain -> domain.value), alias.dataspace)),
        payload.leaseExpiryMs);
  }

  /** Decoded set-primary-account-alias instruction payload. */
  static final class DecodedSetPrimaryAccountAliasPayload {
    private final String accountId;
    private final Optional<DecodedAccountAlias> alias;
    private final Optional<Long> leaseExpiryMs;

    private DecodedSetPrimaryAccountAliasPayload(
        final String accountId,
        final Optional<DecodedAccountAlias> alias,
        final Optional<Long> leaseExpiryMs) {
      this.accountId = accountId;
      this.alias = Objects.requireNonNull(alias, "alias");
      this.leaseExpiryMs = Objects.requireNonNull(leaseExpiryMs, "leaseExpiryMs");
    }

    String accountId() {
      return accountId;
    }

    Optional<DecodedAccountAlias> alias() {
      return alias;
    }

    Optional<Long> leaseExpiryMs() {
      return leaseExpiryMs;
    }
  }

  /** Decoded account alias value. */
  static final class DecodedAccountAlias {
    private final String alias;
    private final Optional<String> aliasDomain;
    private final long dataspace;

    private DecodedAccountAlias(
        final String alias, final Optional<String> aliasDomain, final long dataspace) {
      this.alias = alias;
      this.aliasDomain = Objects.requireNonNull(aliasDomain, "aliasDomain");
      this.dataspace = dataspace;
    }

    String alias() {
      return alias;
    }

    Optional<String> aliasDomain() {
      return aliasDomain;
    }

    long dataspace() {
      return dataspace;
    }
  }

  private static final class SetPrimaryAccountAliasPayload {
    private final byte[] accountPayload;
    private final Optional<AccountAliasPayload> alias;
    private final Optional<Long> leaseExpiryMs;

    private SetPrimaryAccountAliasPayload(
        final byte[] accountPayload,
        final Optional<AccountAliasPayload> alias,
        final Optional<Long> leaseExpiryMs) {
      this.accountPayload = accountPayload.clone();
      this.alias = Objects.requireNonNull(alias, "alias");
      this.leaseExpiryMs = Objects.requireNonNull(leaseExpiryMs, "leaseExpiryMs");
    }
  }

  private static final class SetPrimaryAccountAliasPayloadAdapter
      implements TypeAdapter<SetPrimaryAccountAliasPayload> {
    private static final TypeAdapter<byte[]> PASSTHROUGH_ADAPTER = new PassthroughBytesAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final SetPrimaryAccountAliasPayload value) {
      encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.accountPayload);
      encodeSizedField(encoder, OPTIONAL_ALIAS_ADAPTER, value.alias);
      encodeSizedField(encoder, OPTIONAL_U64_ADAPTER, value.leaseExpiryMs);
    }

    @Override
    public SetPrimaryAccountAliasPayload decode(final NoritoDecoder decoder) {
      final byte[] accountPayload =
          decodeSizedField(decoder, PASSTHROUGH_ADAPTER, "SetPrimaryAccountAlias.account_id");
      final Optional<AccountAliasPayload> alias =
          decodeSizedField(decoder, OPTIONAL_ALIAS_ADAPTER, "SetPrimaryAccountAlias.alias");
      final Optional<Long> leaseExpiryMs =
          decodeSizedField(
              decoder, OPTIONAL_U64_ADAPTER, "SetPrimaryAccountAlias.lease_expiry_ms");
      return new SetPrimaryAccountAliasPayload(accountPayload, alias, leaseExpiryMs);
    }
  }

  private static final class AccountAliasPayload {
    private final String alias;
    private final Optional<AccountAliasDomainPayload> aliasDomain;
    private final long dataspace;

    private AccountAliasPayload(
        final String alias,
        final Optional<AccountAliasDomainPayload> aliasDomain,
        final long dataspace) {
      if (dataspace < 0L) {
        throw new IllegalArgumentException("dataspace must be non-negative");
      }
      this.alias = alias;
      this.aliasDomain = Objects.requireNonNull(aliasDomain, "aliasDomain");
      this.dataspace = dataspace;
    }
  }

  private static final class AccountAliasPayloadAdapter
      implements TypeAdapter<AccountAliasPayload> {
    private static final TypeAdapter<Optional<AccountAliasDomainPayload>> OPTIONAL_ALIAS_DOMAIN_ADAPTER =
        NoritoAdapters.option(ACCOUNT_ALIAS_DOMAIN_ADAPTER);
    private static final TypeAdapter<Long> U64_ADAPTER = NoritoAdapters.uint(64);

    @Override
    public void encode(final NoritoEncoder encoder, final AccountAliasPayload value) {
      encodeSizedField(encoder, STRING_ADAPTER, value.alias);
      encodeSizedField(encoder, OPTIONAL_ALIAS_DOMAIN_ADAPTER, value.aliasDomain);
      encodeSizedField(encoder, U64_ADAPTER, value.dataspace);
    }

    @Override
    public AccountAliasPayload decode(final NoritoDecoder decoder) {
      final String alias = decodeSizedField(decoder, STRING_ADAPTER, "alias.label");
      final Optional<AccountAliasDomainPayload> aliasDomain =
          decodeSizedField(decoder, OPTIONAL_ALIAS_DOMAIN_ADAPTER, "alias.domain");
      final long dataspace = decodeSizedField(decoder, U64_ADAPTER, "alias.dataspace");
      return new AccountAliasPayload(
          requireUsername(alias, "alias"),
          aliasDomain.map(
              domain -> new AccountAliasDomainPayload(requireAliasDomain(domain.value, "aliasDomain"))),
          dataspace);
    }
  }

  private static final class AccountAliasDomainPayload {
    private final String value;

    private AccountAliasDomainPayload(final String value) {
      this.value = value;
    }
  }

  private static final class PassthroughBytesAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      if (value == null || value.length == 0) {
        throw new IllegalArgumentException("payload bytes must not be empty");
      }
      encoder.writeBytes(value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      if (payload.length == 0) {
        throw new IllegalArgumentException("payload bytes must not be empty");
      }
      return payload;
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

  private static <T> T decodeSizedField(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter, final String fieldName) {
    final long length = decoder.readLength((decoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName + " payload");
    }
    return value;
  }

  private static String requireNonBlank(final String value, final String field) {
    final String trimmed = value == null ? "" : value.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return trimmed;
  }

  private static String requireUsername(final String value, final String field) {
    final String trimmed = requireNonBlank(value, field);
    if (trimmed.length() > 32) {
      throw new IllegalArgumentException(field + " must be 32 characters or fewer");
    }
    for (int i = 0; i < trimmed.length(); i++) {
      final char ch = trimmed.charAt(i);
      final boolean allowed =
          (ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '_' || ch == '-';
      if (!allowed) {
        throw new IllegalArgumentException(field + " contains unsupported characters");
      }
    }
    return trimmed;
  }

  private static String requireAliasDomain(final String value, final String field) {
    final String trimmed = requireNonBlank(value, field);
    if (trimmed.length() > 128) {
      throw new IllegalArgumentException(field + " must be 128 characters or fewer");
    }
    int segmentLength = 0;
    for (int i = 0; i < trimmed.length(); i++) {
      final char ch = trimmed.charAt(i);
      if (ch == '.') {
        if (segmentLength == 0) {
          throw new IllegalArgumentException(field + " contains an empty segment");
        }
        segmentLength = 0;
        continue;
      }
      final boolean allowed =
          (ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '_' || ch == '-';
      if (!allowed) {
        throw new IllegalArgumentException(field + " contains unsupported characters");
      }
      segmentLength++;
      if (segmentLength > 32) {
        throw new IllegalArgumentException(field + " segment must be 32 characters or fewer");
      }
    }
    if (segmentLength == 0) {
      throw new IllegalArgumentException(field + " contains an empty segment");
    }
    return trimmed;
  }
}
