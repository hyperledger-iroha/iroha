package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;

public final class OfflineQueryEnvelopeTest {

  private OfflineQueryEnvelopeTest() {}

  public static void main(final String[] args) {
    encodesEnvelopeAsJson();
    fromListParamsParsesJson();
    System.out.println("[IrohaAndroid] OfflineQueryEnvelopeTest passed.");
  }

  private static void encodesEnvelopeAsJson() {
    final OfflineQueryEnvelope envelope =
        OfflineQueryEnvelope.builder()
            .filterJson("{\"op\":\"eq\",\"args\":[\"receiver_id\",\"soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ\"]}")
            .sortJson("[{\"key\":\"bundle_id_hex\",\"order\":\"asc\"}]")
            .setLimit(25L)
            .setOffset(10L)
            .build();
    final String json = new String(envelope.toJsonBytes(), StandardCharsets.UTF_8);
    assert json.contains("\"receiver_id\"") : "filter missing";
    assert json.contains("\"bundle_id_hex\"") : "sort missing";
    assert json.contains("\"limit\":25") : "limit missing";
    assert json.contains("\"offset\":10") : "offset missing";
  }

  private static void fromListParamsParsesJson() {
    final OfflineListParams params =
        OfflineListParams.builder()
            .filter("{\"op\":\"eq\",\"args\":[\"controller_id\",\"soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ\"]}")
            .sort("[{\"key\":\"certificate_id_hex\",\"order\":\"desc\"}]")
            .limit(5L)
            .build();
    final OfflineQueryEnvelope envelope = OfflineQueryEnvelope.fromListParams(params);
    final String json = new String(envelope.toJsonBytes(), StandardCharsets.UTF_8);
    assert json.contains("\"controller_id\"") : "controller filter missing";
    assert json.contains("\"certificate_id_hex\"") : "sort missing";
    assert json.contains("\"limit\":5") : "limit mismatch";
  }
}
