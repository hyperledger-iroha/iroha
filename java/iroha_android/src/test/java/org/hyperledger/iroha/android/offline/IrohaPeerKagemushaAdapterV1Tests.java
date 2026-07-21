package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.junit.Test;

public final class IrohaPeerKagemushaAdapterV1Tests {
  @Test
  public void emitsOnlyFixedNativeArchiveSchemaAndPreservesExactBytes() {
    final byte[] archive = archive(
        "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2");
    final KagemushaPeerTransport.Payload typed =
        KagemushaPeerTransport.Payload.decode(
            archive, KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final IrohaPeerWireMessageV1 wrapped = IrohaPeerKagemushaAdapterV1.wrap(typed);
    assertEquals(
        IrohaPeerKagemushaAdapterV1.NATIVE_ARCHIVE_SCHEMA_VERSION,
        wrapped.canonicalPayload().schemaVersion());
    assertEquals(
        IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        wrapped.canonicalPayload().profile());
    assertEquals(IrohaPeerPayloadKind.RECEIVE_REQUEST, wrapped.canonicalPayload().kind());
    assertArrayEquals(archive, wrapped.canonicalPayload().bytes());
    assertArrayEquals(archive, IrohaPeerKagemushaAdapterV1.decode(wrapped).archive());

    final IrohaPeerWireLimitsV1 tooSmall =
        new IrohaPeerWireLimitsV1(32 * 1024, archive.length - 1, archive.length - 1);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerKagemushaAdapterV1.wrap(
            typed, IrohaPeerWireCompressionPolicyV1.DISABLED, tooSmall));
  }

  @Test
  public void rejectsUnexpectedProfileBeforeTypedDecode() {
    final IrohaPeerWireMessageV1 offline =
        new IrohaPeerWireMessageV1(
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.PAYMENT,
                1,
                new byte[] {1}));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerKagemushaAdapterV1.decode(offline));
  }

  private static byte[] archive(final String schema) {
    final byte[] payload = {0x51};
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(schema),
            payload.length,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final int padding = schema.endsWith("KagemushaReceiverAcknowledgementV2") ? 0 : 8;
    final byte[] archive = new byte[NoritoHeader.HEADER_LENGTH + padding + payload.length];
    System.arraycopy(header.encode(), 0, archive, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(payload, 0, archive, NoritoHeader.HEADER_LENGTH + padding, payload.length);
    return archive;
  }
}
