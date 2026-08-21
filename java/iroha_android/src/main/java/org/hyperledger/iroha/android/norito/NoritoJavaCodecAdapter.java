package org.hyperledger.iroha.android.norito;

import org.hyperledger.iroha.android.client.MultisigProposeRequest;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Norito codec adapter that delegates to the shared JVM Norito implementation bundled with the
 * workspace. This ensures Android tooling stays in lockstep with the canonical Rust codecs and
 * schema hashes.
 */
public final class NoritoJavaCodecAdapter implements NoritoCodecAdapter {

  private static final String DEFAULT_SCHEMA = "iroha.android.transaction.Payload.v1";

  private final int chainDiscriminant;
  private final String schemaName;
  private final TypeAdapter<TransactionPayload> adapter;

  public NoritoJavaCodecAdapter(final int chainDiscriminant) {
    this(chainDiscriminant, DEFAULT_SCHEMA);
  }

  public NoritoJavaCodecAdapter(final int chainDiscriminant, final String schemaName) {
    if (chainDiscriminant < 0 || chainDiscriminant > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    this.chainDiscriminant = chainDiscriminant;
    this.schemaName = schemaName;
    this.adapter = TransactionPayloadAdapter.forChain(chainDiscriminant);
  }

  @Override
  public byte[] encodeTransaction(final TransactionPayload payload) throws NoritoException {
    try {
      return NoritoCodec.encodeAdaptive(payload, adapter).payload();
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode Norito transaction payload", ex);
    }
  }

  @Override
  public TransactionPayload decodeTransaction(final byte[] encoded) throws NoritoException {
    try {
      if (hasHeader(encoded)) {
        return NoritoCodec.decode(encoded, adapter, schemaName);
      }
      return NoritoCodec.decodeAdaptive(encoded, adapter);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to decode Norito transaction payload", ex);
    }
  }

  /** Returns the target chain's required I105 discriminant. */
  public int chainDiscriminant() {
    return chainDiscriminant;
  }

  @Override
  public String schemaName() {
    return schemaName;
  }

  public static byte[] encodeInstructionBox(final InstructionBox instruction) throws NoritoException {
    try {
      return TransactionPayloadAdapter.encodeInstructionBox(instruction);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode Norito instruction box", ex);
    }
  }

  /** Rejects transaction payload bytes that are not the exact canonical Norito encoding. */
  public static void validateCanonicalTransactionPayload(final byte[] encoded)
      throws NoritoException {
    try {
      TransactionPayloadAdapter.validateCanonicalPayloadBytes(encoded);
    } catch (final Exception ex) {
      throw new NoritoException("Invalid canonical Norito transaction payload", ex);
    }
  }

  /** Rejects non-canonical payloads and payloads with a different admission intent. */
  public static void validateCanonicalTransactionPayload(
      final byte[] encoded, final TransactionAdmissionIntent expectedAdmissionIntent)
      throws NoritoException {
    try {
      final TransactionPayload payload =
          TransactionPayloadAdapter.validateCanonicalPayloadBytes(encoded);
      if (payload.admissionIntent() != expectedAdmissionIntent) {
        throw new IllegalArgumentException(
            "transaction payload admission intent must be " + expectedAdmissionIntent);
      }
    } catch (final Exception ex) {
      throw new NoritoException("Invalid canonical Norito transaction payload", ex);
    }
  }

  public static byte[] encodeMultisigProposeRequest(
      final MultisigProposeRequest request, final int chainDiscriminant) throws NoritoException {
    try {
      return TransactionPayloadAdapter.encodeMultisigProposeRequest(
          request, chainDiscriminant);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode Norito multisig propose request", ex);
    }
  }

  private static boolean hasHeader(final byte[] encoded) {
    if (encoded == null || encoded.length < NoritoHeader.HEADER_LENGTH) {
      return false;
    }
    return encoded[0] == 'N'
        && encoded[1] == 'R'
        && encoded[2] == 'T'
        && encoded[3] == '0';
  }
}
