package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.model.ContractAddressValidator;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Canonical Norito encoders for the account-owner contract lifecycle instructions.
 *
 * <p>Every mutation carries the exact non-zero lifecycle revision observed by the caller.
 * Ownership offers keep account and Parliament targets distinct so a textual value cannot
 * silently change the native enum variant.
 */
public final class ContractLifecycleWirePayloadEncoder {
  public static final String SET_PARLIAMENT_DELEGATION_WIRE_NAME =
      "iroha.instruction.v1::smart_contract_code::SetContractParliamentDelegation";
  public static final String OFFER_OWNERSHIP_WIRE_NAME =
      "iroha.instruction.v1::smart_contract_code::OfferContractOwnership";
  public static final String ACCEPT_OWNERSHIP_WIRE_NAME =
      "iroha.instruction.v1::smart_contract_code::AcceptContractOwnership";
  public static final String CANCEL_OWNERSHIP_OFFER_WIRE_NAME =
      "iroha.instruction.v1::smart_contract_code::CancelContractOwnershipOffer";

  /** Closed first-release lifecycle instruction catalog in registry order. */
  public static final List<String> WIRE_NAMES =
      Collections.unmodifiableList(
          Arrays.asList(
              SET_PARLIAMENT_DELEGATION_WIRE_NAME,
              OFFER_OWNERSHIP_WIRE_NAME,
              ACCEPT_OWNERSHIP_WIRE_NAME,
              CANCEL_OWNERSHIP_OFFER_WIRE_NAME));

  private static final String SET_PARLIAMENT_DELEGATION_SCHEMA =
      "iroha_data_model::isi::smart_contract_code::SetContractParliamentDelegation";
  private static final String OFFER_OWNERSHIP_SCHEMA =
      "iroha_data_model::isi::smart_contract_code::OfferContractOwnership";
  private static final String ACCEPT_OWNERSHIP_SCHEMA =
      "iroha_data_model::isi::smart_contract_code::AcceptContractOwnership";
  private static final String CANCEL_OWNERSHIP_OFFER_SCHEMA =
      "iroha_data_model::isi::smart_contract_code::CancelContractOwnershipOffer";
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final TypeAdapter<String> STRING = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Boolean> BOOL = NoritoAdapters.boolAdapter();
  private static final TypeAdapter<Long> UINT32 = NoritoAdapters.uint(32);

  private ContractLifecycleWirePayloadEncoder() {}

  /** Encode {@code SetContractParliamentDelegation} with an exact revision guard. */
  public static InstructionBox encodeSetContractParliamentDelegation(
      final String contractAddress,
      final BigInteger expectedRevision,
      final boolean delegated) {
    final DelegationPayload payload =
        new DelegationPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            delegated);
    return wire(
        SET_PARLIAMENT_DELEGATION_WIRE_NAME,
        NoritoCodec.encode(
            payload,
            SET_PARLIAMENT_DELEGATION_SCHEMA,
            DelegationAdapter.INSTANCE));
  }

  /** Encode an account-targeted {@code OfferContractOwnership}. */
  public static InstructionBox encodeOfferContractOwnershipToAccount(
      final String contractAddress,
      final BigInteger expectedRevision,
      final String newOwnerAccountId) {
    final OfferPayload payload =
        new OfferPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            OwnerValue.account(newOwnerAccountId));
    return wire(
        OFFER_OWNERSHIP_WIRE_NAME,
        NoritoCodec.encode(
            payload,
            OFFER_OWNERSHIP_SCHEMA,
            new OfferAdapter(null)));
  }

  /** Encode a Parliament-targeted {@code OfferContractOwnership}. */
  public static InstructionBox encodeOfferContractOwnershipToParliament(
      final String contractAddress, final BigInteger expectedRevision) {
    final OfferPayload payload =
        new OfferPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            OwnerValue.parliament());
    return wire(
        OFFER_OWNERSHIP_WIRE_NAME,
        NoritoCodec.encode(
            payload,
            OFFER_OWNERSHIP_SCHEMA,
            new OfferAdapter(null)));
  }

  /** Encode {@code AcceptContractOwnership} with an exact revision guard. */
  public static InstructionBox encodeAcceptContractOwnership(
      final String contractAddress, final BigInteger expectedRevision) {
    return encodeRevisionGuard(
        ACCEPT_OWNERSHIP_WIRE_NAME,
        ACCEPT_OWNERSHIP_SCHEMA,
        contractAddress,
        expectedRevision);
  }

  /** Encode {@code CancelContractOwnershipOffer} with an exact revision guard. */
  public static InstructionBox encodeCancelContractOwnershipOffer(
      final String contractAddress, final BigInteger expectedRevision) {
    return encodeRevisionGuard(
        CANCEL_OWNERSHIP_OFFER_WIRE_NAME,
        CANCEL_OWNERSHIP_OFFER_SCHEMA,
        contractAddress,
        expectedRevision);
  }

  static DecodedDelegation decodeSetContractParliamentDelegation(final byte[] payload) {
    final DelegationPayload decoded =
        NoritoCodec.decode(
            Objects.requireNonNull(payload, "payload"),
            DelegationAdapter.INSTANCE,
            SET_PARLIAMENT_DELEGATION_SCHEMA);
    return new DecodedDelegation(decoded.address, decoded.revision, decoded.delegated);
  }

  static DecodedOwnershipOffer decodeOfferContractOwnership(
      final byte[] payload, final int chainDiscriminant) {
    final OfferPayload decoded =
        NoritoCodec.decode(
            Objects.requireNonNull(payload, "payload"),
            new OfferAdapter(chainDiscriminant),
            OFFER_OWNERSHIP_SCHEMA);
    return new DecodedOwnershipOffer(
        decoded.address, decoded.revision, decoded.owner.accountId);
  }

  static DecodedRevisionGuard decodeAcceptContractOwnership(final byte[] payload) {
    return decodeRevisionGuard(payload, ACCEPT_OWNERSHIP_SCHEMA);
  }

  static DecodedRevisionGuard decodeCancelContractOwnershipOffer(final byte[] payload) {
    return decodeRevisionGuard(payload, CANCEL_OWNERSHIP_OFFER_SCHEMA);
  }

  static final class DecodedDelegation {
    private final String contractAddress;
    private final BigInteger expectedRevision;
    private final boolean delegated;

    DecodedDelegation(
        final String contractAddress,
        final BigInteger expectedRevision,
        final boolean delegated) {
      this.contractAddress = contractAddress;
      this.expectedRevision = expectedRevision;
      this.delegated = delegated;
    }

    String contractAddress() {
      return contractAddress;
    }

    BigInteger expectedRevision() {
      return expectedRevision;
    }

    boolean delegated() {
      return delegated;
    }
  }

  static final class DecodedOwnershipOffer {
    private final String contractAddress;
    private final BigInteger expectedRevision;
    private final String newOwnerAccountId;

    DecodedOwnershipOffer(
        final String contractAddress,
        final BigInteger expectedRevision,
        final String newOwnerAccountId) {
      this.contractAddress = contractAddress;
      this.expectedRevision = expectedRevision;
      this.newOwnerAccountId = newOwnerAccountId;
    }

    String contractAddress() {
      return contractAddress;
    }

    BigInteger expectedRevision() {
      return expectedRevision;
    }

    /** Null denotes the native {@code Parliament} owner variant. */
    String newOwnerAccountId() {
      return newOwnerAccountId;
    }
  }

  static final class DecodedRevisionGuard {
    private final String contractAddress;
    private final BigInteger expectedRevision;

    DecodedRevisionGuard(
        final String contractAddress, final BigInteger expectedRevision) {
      this.contractAddress = contractAddress;
      this.expectedRevision = expectedRevision;
    }

    String contractAddress() {
      return contractAddress;
    }

    BigInteger expectedRevision() {
      return expectedRevision;
    }
  }

  private static final class RevisionGuardPayload {
    private final String address;
    private final BigInteger revision;

    private RevisionGuardPayload(final String address, final BigInteger revision) {
      this.address = address;
      this.revision = revision;
    }
  }

  private static final class DelegationPayload {
    private final String address;
    private final BigInteger revision;
    private final boolean delegated;

    private DelegationPayload(
        final String address, final BigInteger revision, final boolean delegated) {
      this.address = address;
      this.revision = revision;
      this.delegated = delegated;
    }
  }

  private static final class OfferPayload {
    private final String address;
    private final BigInteger revision;
    private final OwnerValue owner;

    private OfferPayload(
        final String address, final BigInteger revision, final OwnerValue owner) {
      this.address = address;
      this.revision = revision;
      this.owner = owner;
    }
  }

  private static final class OwnerValue {
    private final String accountId;

    private OwnerValue(final String accountId) {
      this.accountId = accountId;
    }

    private static OwnerValue account(final String accountId) {
      TransferWirePayloadEncoder.encodeAccountIdPayload(accountId);
      return new OwnerValue(accountId);
    }

    private static OwnerValue parliament() {
      return new OwnerValue(null);
    }
  }

  private static final class U64Adapter implements TypeAdapter<BigInteger> {
    private static final U64Adapter INSTANCE = new U64Adapter();

    @Override
    public void encode(final NoritoEncoder encoder, final BigInteger value) {
      encoder.writeUInt(requireU64(value).longValue(), 64);
    }

    @Override
    public BigInteger decode(final NoritoDecoder decoder) {
      final long value = decoder.readUInt(64);
      return value >= 0
          ? BigInteger.valueOf(value)
          : BigInteger.valueOf(value & Long.MAX_VALUE).setBit(63);
    }
  }

  private static final class RevisionGuardAdapter
      implements TypeAdapter<RevisionGuardPayload> {
    private static final RevisionGuardAdapter INSTANCE = new RevisionGuardAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final RevisionGuardPayload value) {
      encodeField(encoder, STRING, value.address);
      encodeField(encoder, U64Adapter.INSTANCE, value.revision);
    }

    @Override
    public RevisionGuardPayload decode(final NoritoDecoder decoder) {
      return new RevisionGuardPayload(
          canonicalAddress(decodeField(decoder, STRING, "contract_address")),
          positiveU64(
              decodeField(decoder, U64Adapter.INSTANCE, "expected_revision")));
    }
  }

  private static final class DelegationAdapter implements TypeAdapter<DelegationPayload> {
    private static final DelegationAdapter INSTANCE = new DelegationAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final DelegationPayload value) {
      encodeField(encoder, STRING, value.address);
      encodeField(encoder, U64Adapter.INSTANCE, value.revision);
      encodeField(encoder, BOOL, value.delegated);
    }

    @Override
    public DelegationPayload decode(final NoritoDecoder decoder) {
      return new DelegationPayload(
          canonicalAddress(decodeField(decoder, STRING, "contract_address")),
          positiveU64(
              decodeField(decoder, U64Adapter.INSTANCE, "expected_revision")),
          decodeField(decoder, BOOL, "delegated"));
    }
  }

  private static final class OfferAdapter implements TypeAdapter<OfferPayload> {
    private final Integer chainDiscriminant;

    private OfferAdapter(final Integer chainDiscriminant) {
      this.chainDiscriminant = chainDiscriminant;
    }

    @Override
    public void encode(final NoritoEncoder encoder, final OfferPayload value) {
      encodeField(encoder, STRING, value.address);
      encodeField(encoder, U64Adapter.INSTANCE, value.revision);
      encodeField(encoder, new OwnerAdapter(chainDiscriminant), value.owner);
    }

    @Override
    public OfferPayload decode(final NoritoDecoder decoder) {
      return new OfferPayload(
          canonicalAddress(decodeField(decoder, STRING, "contract_address")),
          positiveU64(
              decodeField(decoder, U64Adapter.INSTANCE, "expected_revision")),
          decodeField(
              decoder, new OwnerAdapter(chainDiscriminant), "new_owner"));
    }
  }

  private static final class OwnerAdapter implements TypeAdapter<OwnerValue> {
    private final Integer chainDiscriminant;

    private OwnerAdapter(final Integer chainDiscriminant) {
      this.chainDiscriminant = chainDiscriminant;
    }

    @Override
    public void encode(final NoritoEncoder encoder, final OwnerValue value) {
      final boolean parliament = value.accountId == null;
      UINT32.encode(encoder, parliament ? 1L : 0L);
      if (!parliament) {
        final byte[] payload =
            TransferWirePayloadEncoder.encodeAccountIdPayload(value.accountId);
        writeLength(encoder, payload.length);
        encoder.writeBytes(payload);
      }
    }

    @Override
    public OwnerValue decode(final NoritoDecoder decoder) {
      final long discriminant = UINT32.decode(decoder);
      if (discriminant == 0L) {
        if (chainDiscriminant == null) {
          throw new IllegalArgumentException(
              "chainDiscriminant is required to decode an account owner");
        }
        final byte[] payload = readSizedBytes(decoder, "new_owner account variant");
        return OwnerValue.account(
            TransferWirePayloadEncoder.decodeAccountIdPayload(
                payload, chainDiscriminant, decoder.flags()));
      }
      if (discriminant == 1L) {
        return OwnerValue.parliament();
      }
      throw new IllegalArgumentException(
          "unsupported ContractLifecycleOwnerV1 discriminant: " + discriminant);
    }
  }

  private static InstructionBox encodeRevisionGuard(
      final String wireName,
      final String schema,
      final String contractAddress,
      final BigInteger expectedRevision) {
    final RevisionGuardPayload payload =
        new RevisionGuardPayload(
            canonicalAddress(contractAddress), positiveU64(expectedRevision));
    return wire(
        wireName,
        NoritoCodec.encode(payload, schema, RevisionGuardAdapter.INSTANCE));
  }

  private static DecodedRevisionGuard decodeRevisionGuard(
      final byte[] payload, final String schema) {
    final RevisionGuardPayload decoded =
        NoritoCodec.decode(
            Objects.requireNonNull(payload, "payload"),
            RevisionGuardAdapter.INSTANCE,
            schema);
    return new DecodedRevisionGuard(decoded.address, decoded.revision);
  }

  private static InstructionBox wire(final String wireName, final byte[] payload) {
    return InstructionBox.fromWirePayload(wireName, payload);
  }

  private static String canonicalAddress(final String value) {
    return ContractAddressValidator.requireCanonicalV1(value);
  }

  private static BigInteger requireU64(final BigInteger value) {
    final BigInteger nonNull = Objects.requireNonNull(value, "expectedRevision");
    if (nonNull.signum() < 0 || nonNull.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException("expectedRevision must fit u64");
    }
    return nonNull;
  }

  private static BigInteger positiveU64(final BigInteger value) {
    final BigInteger checked = requireU64(value);
    if (checked.signum() == 0) {
      throw new IllegalArgumentException("expectedRevision must be non-zero");
    }
    return checked;
  }

  private static <T> void encodeField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    writeLength(encoder, payload.length);
    encoder.writeBytes(payload);
  }

  private static <T> T decodeField(
      final NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final String field) {
    final byte[] payload = readSizedBytes(decoder, field);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("trailing bytes after " + field);
    }
    return value;
  }

  private static byte[] readSizedBytes(
      final NoritoDecoder decoder, final String field) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " exceeds the supported JVM size");
    }
    return decoder.readBytes((int) length);
  }

  private static void writeLength(final NoritoEncoder encoder, final int size) {
    encoder.writeLength(
        size,
        (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
  }
}
