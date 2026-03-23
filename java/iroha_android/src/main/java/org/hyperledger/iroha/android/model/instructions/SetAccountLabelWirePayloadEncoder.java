package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Encodes {@code identity::SetAccountLabel} instructions in wire-framed Norito format. */
public final class SetAccountLabelWirePayloadEncoder {

  public static final String WIRE_NAME = "identity::SetAccountLabel";

  private static final String SCHEMA_PATH =
      "iroha_data_model::isi::domain_link::SetAccountLabel";

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();

  private SetAccountLabelWirePayloadEncoder() {}

  public static InstructionBox encode(
      final String accountId, final String label, final String domainId) {
    final String normalizedAccountId = requireNonBlank(accountId, "accountId");
    final String normalizedLabel = requireUsername(label, "label");
    final String normalizedDomainId = requireNonBlank(domainId, "domainId");
    final byte[] accountPayload =
        TransferWirePayloadEncoder.encodeAccountIdPayload(normalizedAccountId);
    final byte[] wirePayload =
        NoritoCodec.encode(
            new SetAccountLabelPayload(accountPayload, normalizedDomainId, normalizedLabel),
            SCHEMA_PATH,
            new SetAccountLabelPayloadAdapter());
    return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload);
  }

  private static final class SetAccountLabelPayload {
    private final byte[] accountPayload;
    private final String domainId;
    private final String label;

    private SetAccountLabelPayload(
        final byte[] accountPayload, final String domainId, final String label) {
      this.accountPayload = accountPayload.clone();
      this.domainId = domainId;
      this.label = label;
    }
  }

  private static final class SetAccountLabelPayloadAdapter
      implements TypeAdapter<SetAccountLabelPayload> {
    private static final TypeAdapter<byte[]> PASSTHROUGH_ADAPTER = new PassthroughBytesAdapter();
    private static final TypeAdapter<AccountLabelPayload> ACCOUNT_LABEL_ADAPTER =
        new AccountLabelPayloadAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final SetAccountLabelPayload value) {
      encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.accountPayload);
      encodeSizedField(
          encoder,
          ACCOUNT_LABEL_ADAPTER,
          new AccountLabelPayload(value.domainId, value.label));
    }

    @Override
    public SetAccountLabelPayload decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding SetAccountLabel is not supported");
    }
  }

  private static final class AccountLabelPayload {
    private final String domainId;
    private final String label;

    private AccountLabelPayload(final String domainId, final String label) {
      this.domainId = domainId;
      this.label = label;
    }
  }

  private static final class AccountLabelPayloadAdapter
      implements TypeAdapter<AccountLabelPayload> {
    private static final TypeAdapter<DomainIdPayload> DOMAIN_ID_ADAPTER = new DomainIdPayloadAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final AccountLabelPayload value) {
      encodeSizedField(encoder, DOMAIN_ID_ADAPTER, new DomainIdPayload(value.domainId));
      encodeSizedField(encoder, STRING_ADAPTER, value.label);
    }

    @Override
    public AccountLabelPayload decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding AccountLabel is not supported");
    }
  }

  private static final class DomainIdPayload {
    private final String value;

    private DomainIdPayload(final String value) {
      this.value = value;
    }
  }

  private static final class DomainIdPayloadAdapter implements TypeAdapter<DomainIdPayload> {
    @Override
    public void encode(final NoritoEncoder encoder, final DomainIdPayload value) {
      encodeSizedField(encoder, STRING_ADAPTER, value.value);
    }

    @Override
    public DomainIdPayload decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding DomainId is not supported");
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
      throw new UnsupportedOperationException("Decoding passthrough payloads is not supported");
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
}
