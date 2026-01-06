package org.hyperledger.iroha.android.nexus;

import java.nio.charset.StandardCharsets;
import java.util.List;

public final class UaidJsonParserTests {

  private static final String UAID =
      "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

  private UaidJsonParserTests() {}

  public static void main(final String[] args) {
    parsesPortfolioPayload();
    rejectsFractionalEpoch();
    System.out.println("[IrohaAndroid] UaidJsonParserTests passed.");
  }

  private static void parsesPortfolioPayload() {
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
                  "account_id": "alice@wonderland",
                  "label": "alice",
                  "assets": [
                    {
                      "asset_id": "usd#wonderland#alice",
                      "asset_definition_id": "usd#wonderland",
                      "quantity": "15"
                    }
                  ]
                }
              ]
            }
          ]
        }
        """;
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
    assert "alice@wonderland".equals(account.accountId()) : "account id mismatch";
    assert "alice".equals(account.label()) : "account label mismatch";
    assert account.assets().size() == 1 : "asset list size mismatch";
    final UaidPortfolioResponse.UaidPortfolioAsset asset = account.assets().get(0);
    assert "usd#wonderland#alice".equals(asset.assetId()) : "asset id mismatch";
    assert "usd#wonderland".equals(asset.assetDefinitionId()) : "definition id mismatch";
    assert "15".equals(asset.quantity()) : "quantity mismatch";
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
              "accounts": ["alice@wonderland"],
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
}
