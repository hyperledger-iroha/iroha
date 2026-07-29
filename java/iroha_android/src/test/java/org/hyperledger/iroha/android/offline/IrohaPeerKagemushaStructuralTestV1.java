package org.hyperledger.iroha.android.offline;

import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

final class IrohaPeerKagemushaStructuralTestV1 {
  private IrohaPeerKagemushaStructuralTestV1() {}

  static IrohaPeerWireMessageV1 message(
      final IrohaPeerPayloadKind kind, final byte[] payload) {
    return new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
            kind,
            0x0102,
            archive(kind, payload)));
  }

  static byte[] archive(final IrohaPeerPayloadKind kind, final byte[] payload) {
    if (payload.length == 0) throw new IllegalArgumentException("fixture payload is empty");
    final String schema = switch (kind) {
      case RECEIVE_REQUEST ->
          "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2";
      case PAYMENT ->
          "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4";
      case ACKNOWLEDGEMENT ->
          "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2";
    };
    final int padding = kind == IrohaPeerPayloadKind.ACKNOWLEDGEMENT ? 0 : 8;
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(schema),
            payload.length,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] archive = new byte[header.encode().length + padding + payload.length];
    System.arraycopy(header.encode(), 0, archive, 0, header.encode().length);
    System.arraycopy(payload, 0, archive, header.encode().length + padding, payload.length);
    return archive;
  }
}
