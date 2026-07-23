package org.hyperledger.iroha.android.nexus;

import java.nio.charset.StandardCharsets;
import java.util.List;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;

public final class UaidJsonParserTests {

  private static final String UAID =
      "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

  private UaidJsonParserTests() {}

  public static void main(final String[] args) {
    parsesPortfolioPayload();
    parsesLegacyPortfolioPayload();
    rejectsNoncanonicalPortfolioQuantities();
    rejectsInvalidUaidLsb();
    rejectsFractionalEpoch();
    rejectsNegativeRustUnsignedFields();
    rejectsMalformedPresentStringFields();
    System.out.println("[IrohaAndroid] UaidJsonParserTests passed.");
  }

  private static void rejectsNegativeRustUnsignedFields() {
    expectInvalidResponse(
        "{\"uaid\":\"" + UAID + "\",\"totals\":{\"accounts\":-1},\"dataspaces\":[]}",
        UaidJsonParser::parsePortfolio);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"dataspaces\":[{\"dataspace_id\":-1,\"accounts\":[]}]}",
        UaidJsonParser::parseBindings);
    expectInvalidResponse(
        "{\"uaid\":\"" + UAID + "\",\"total\":-1,\"manifests\":[]}",
        UaidJsonParser::parseManifests);
  }

  private static void rejectsMalformedPresentStringFields() {
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"dataspaces\":[{\"dataspace_id\":7,\"dataspace_alias\":9,"
            + "\"accounts\":[]}]}",
        UaidJsonParser::parsePortfolio);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"dataspaces\":[{\"dataspace_id\":7,\"accounts\":[null]}]}",
        UaidJsonParser::parseBindings);
  }

  private static void expectInvalidResponse(
      final String json, final java.util.function.Consumer<byte[]> parser) {
    try {
      parser.accept(json.getBytes(StandardCharsets.UTF_8));
    } catch (final IllegalStateException expected) {
      return;
    }
    throw new AssertionError("invalid UAID response field was accepted");
  }

  private static void parsesPortfolioPayload() {
    final String assetDefinitionId = TestAssetDefinitionIds.SECONDARY;
    final String json =
        """
        {
          "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
          "totals": { "accounts": 2, "positions": 3 },
          "dataspaces": [
            {
              "dataspace_id": 7,
              "dataspace_alias": "primary",
              "accounts": [
                {
                  "account_id": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                  "label": "alice",
                  "assets": [
                    {
                      "asset_id": "%s#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                      "asset_definition_id": "%s",
                      "quantity": "15"
                    }
                  ]
                }
              ]
            }
          ]
        }
        """
            .formatted(assetDefinitionId, assetDefinitionId);
    final UaidPortfolioResponse response =
        UaidJsonParser.parsePortfolio(json.getBytes(StandardCharsets.UTF_8));
    assert UAID.equals(response.uaid()) : "uaid mismatch";
    assert response.totals().accounts() == 2 : "accounts total mismatch";
    assert response.totals().positions() == 3 : "positions total mismatch";
    final List<UaidPortfolioResponse.UaidPortfolioDataspace> dataspaces = response.dataspaces();
    assert dataspaces.size() == 1 : "dataspaces size mismatch";
    final UaidPortfolioResponse.UaidPortfolioDataspace dataspace = dataspaces.get(0);
    assert dataspace.dataspaceId() == 7 : "dataspace id mismatch";
    assert "primary".equals(dataspace.dataspaceAlias()) : "dataspace alias mismatch";
    assert dataspace.accounts().size() == 1 : "account list size mismatch";
    final UaidPortfolioResponse.UaidPortfolioAccount account = dataspace.accounts().get(0);
    assert "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".equals(account.accountId()) : "account id mismatch";
    assert "alice".equals(account.label()) : "account label mismatch";
    assert account.assets().size() == 1 : "asset list size mismatch";
    final UaidPortfolioResponse.UaidPortfolioAsset asset = account.assets().get(0);
    assert (assetDefinitionId + "#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB").equals(asset.assetId()) : "asset id mismatch";
    assert assetDefinitionId.equals(asset.assetDefinitionId()) : "definition id mismatch";
    assert assetDefinitionId.equals(asset.asset()) : "legacy definition alias mismatch";
    assert asset.scope() == null : "modern payload should not synthesize a legacy scope";
    assert "15".equals(asset.quantity()) : "quantity mismatch";
  }

  private static void parsesLegacyPortfolioPayload() {
    final String assetDefinitionId = TestAssetDefinitionIds.SECONDARY;
    final String json =
        """
        {
          "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
          "dataspaces": [
            {
              "dataspace_id": 7,
              "accounts": [
                {
                  "account_id": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                  "assets": [
                    {
                      "asset": "%s",
                      "scope": "global",
                      "quantity": "15"
                    }
                  ]
                }
              ]
            }
          ]
        }
        """
            .formatted(assetDefinitionId);
    final UaidPortfolioResponse response =
        UaidJsonParser.parsePortfolio(json.getBytes(StandardCharsets.UTF_8));
    final UaidPortfolioResponse.UaidPortfolioAsset asset =
        response.dataspaces().get(0).accounts().get(0).assets().get(0);
    assert assetDefinitionId.equals(asset.assetId()) : "legacy asset id fallback mismatch";
    assert assetDefinitionId.equals(asset.assetDefinitionId()) : "legacy definition id mismatch";
    assert assetDefinitionId.equals(asset.asset()) : "legacy asset accessor mismatch";
    assert "global".equals(asset.scope()) : "legacy scope mismatch";
    assert "15".equals(asset.quantity()) : "legacy quantity mismatch";
  }

  private static void rejectsNoncanonicalPortfolioQuantities() {
    for (final String quantity :
        new String[] {"1", "\"01\"", "\"1e0\"", "\"-1\"", "\"1.0\"", "\"1.2300\""}) {
      final String json =
          """
          {
            "uaid":"uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
            "dataspaces":[{
              "dataspace_id":7,
              "accounts":[{
                "account_id":"account",
                "assets":[{
                  "asset_id":"asset",
                  "asset_definition_id":"definition",
                  "quantity":%s
                }]
              }]
            }]
          }
          """.formatted(quantity);
      boolean threw = false;
      try {
        UaidJsonParser.parsePortfolio(json.getBytes(StandardCharsets.UTF_8));
      } catch (final IllegalArgumentException expected) {
        threw = true;
      }
      assert threw : "noncanonical portfolio quantity was accepted: " + quantity;
    }
  }

  private static void rejectsFractionalEpoch() {
    final String json =
        """
        {
          "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
          "total": 1,
          "manifests": [
            {
              "dataspace_id": 7,
              "dataspace_alias": "primary",
              "manifest_hash": "deadbeef",
              "status": "Active",
              "lifecycle": { "activated_epoch": 1.5 },
              "accounts": ["sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"],
              "manifest": {}
            }
          ]
        }
        """;
    boolean thrown = false;
    try {
      UaidJsonParser.parseManifests(json.getBytes(StandardCharsets.UTF_8));
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected non-integer epochs to be rejected";
  }

  private static void rejectsInvalidUaidLsb() {
    final String json =
        """
        {
          "uaid": "uaid:%s",
          "totals": { "accounts": 1, "positions": 1 },
          "dataspaces": []
        }
        """
            .formatted("10".repeat(32));
    boolean thrown = false;
    try {
      UaidJsonParser.parsePortfolio(json.getBytes(StandardCharsets.UTF_8));
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected UAID LSB violations to be rejected";
  }
}
