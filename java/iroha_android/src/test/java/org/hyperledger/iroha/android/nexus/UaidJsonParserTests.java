package org.hyperledger.iroha.android.nexus;

import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.function.Consumer;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery.UaidManifestStatusFilter;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestRecord;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse.UaidManifestStatus;

/** Focused exact-contract tests for UAID response parsing and manifest queries. */
public final class UaidJsonParserTests {

  private static final String UAID =
      "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";
  private static final String HASH = "ab".repeat(32);

  private UaidJsonParserTests() {}

  public static void main(final String[] args) {
    parsesPortfolioPayload();
    rejectsRetiredPortfolioPayload();
    rejectsNoncanonicalPortfolioQuantities();
    rejectsInvalidUaidLsb();
    rejectsNoncanonicalUaidSpellings();
    rejectsNegativeRustUnsignedFields();
    rejectsMalformedPresentStringFields();
    rejectsMissingUnknownAndPaddedFields();
    parsesCurrentManifestContract();
    rejectsRetiredManifestEncodings();
    manifestQueryUsesOnlyCurrentParameters();
    System.out.println("[IrohaAndroid] UaidJsonParserTests passed.");
  }

  private static void parsesPortfolioPayload() {
    final UaidPortfolioResponse response =
        UaidJsonParser.parsePortfolio(portfolio("\"15\"").getBytes(StandardCharsets.UTF_8));
    assert UAID.equals(response.uaid()) : "uaid mismatch";
    assert response.totals().accounts() == 1L : "accounts total mismatch";
    assert response.totals().positions() == 1L : "positions total mismatch";
    final UaidPortfolioResponse.UaidPortfolioDataspace dataspace =
        response.dataspaces().get(0);
    assert dataspace.dataspaceId() == 7L : "dataspace id mismatch";
    assert dataspace.dataspaceAlias() == null : "dataspace alias mismatch";
    final UaidPortfolioResponse.UaidPortfolioAccount account =
        dataspace.accounts().get(0);
    assert "account".equals(account.accountId()) : "account id mismatch";
    assert account.label() == null : "account label mismatch";
    final UaidPortfolioResponse.UaidPortfolioAsset asset = account.assets().get(0);
    assert "asset".equals(asset.assetId()) : "asset id mismatch";
    assert "definition".equals(asset.assetDefinitionId()) : "definition id mismatch";
    assert "15".equals(asset.quantity()) : "quantity mismatch";
  }

  private static void rejectsRetiredPortfolioPayload() {
    expectInvalidResponse(
        """
        {
          "uaid":"%s",
          "totals":{"accounts":1,"positions":1},
          "dataspaces":[{
            "dataspace_id":7,
            "dataspace_alias":null,
            "accounts":[{
              "account_id":"account",
              "label":null,
              "assets":[{"asset":"definition","scope":"global","quantity":"15"}]
            }]
          }]
        }
        """.formatted(UAID),
        UaidJsonParser::parsePortfolio);
  }

  private static void rejectsNoncanonicalPortfolioQuantities() {
    for (final String quantity :
        new String[] {"1", "\"01\"", "\"1e0\"", "\"-1\"", "\"1.0\"", "\"1.2300\""}) {
      expectInvalidResponse(portfolio(quantity), UaidJsonParser::parsePortfolio);
    }
  }

  private static void rejectsInvalidUaidLsb() {
    final String invalid = "uaid:" + "10".repeat(32);
    expectInvalidResponse(
        "{\"uaid\":\""
            + invalid
            + "\",\"totals\":{\"accounts\":0,\"positions\":0},\"dataspaces\":[]}",
        UaidJsonParser::parsePortfolio);
  }

  private static void rejectsNoncanonicalUaidSpellings() {
    final String digest = UAID.substring("uaid:".length());
    for (final String uaid :
        new String[] {digest, "UAID:" + digest, "uaid:" + digest.toUpperCase()}) {
      expectInvalidResponse(
          "{\"uaid\":\""
              + uaid
              + "\",\"totals\":{\"accounts\":0,\"positions\":0},\"dataspaces\":[]}",
          UaidJsonParser::parsePortfolio);
    }
  }

  private static void rejectsNegativeRustUnsignedFields() {
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"totals\":{\"accounts\":-1,\"positions\":0},\"dataspaces\":[]}",
        UaidJsonParser::parsePortfolio);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"dataspaces\":[{\"dataspace_id\":-1,\"dataspace_alias\":null,\"accounts\":[]}]}",
        UaidJsonParser::parseBindings);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"total\":-1,\"has_more\":false,\"count_mode\":\"exact\",\"manifests\":[]}",
        UaidJsonParser::parseManifests);
  }

  private static void rejectsMalformedPresentStringFields() {
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"totals\":{\"accounts\":0,\"positions\":0},\"dataspaces\":[{\"dataspace_id\":7,\"dataspace_alias\":9,\"accounts\":[]}]}",
        UaidJsonParser::parsePortfolio);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"dataspaces\":[{\"dataspace_id\":7,\"dataspace_alias\":null,\"accounts\":[null]}]}",
        UaidJsonParser::parseBindings);
  }

  private static void rejectsMissingUnknownAndPaddedFields() {
    expectInvalidResponse(
        "{\"uaid\":\"" + UAID + "\",\"dataspaces\":[]}",
        UaidJsonParser::parsePortfolio);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"totals\":{\"accounts\":0,\"positions\":0},\"dataspaces\":[],\"legacy\":true}",
        UaidJsonParser::parsePortfolio);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"dataspaces\":[{\"dataspace_id\":7,\"accounts\":[]}]}",
        UaidJsonParser::parseBindings);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"dataspaces\":[{\"dataspace_id\":7,\"dataspace_alias\":null,\"accounts\":[\" account\"]}]}",
        UaidJsonParser::parseBindings);
    expectInvalidResponse(
        "{\"uaid\":\"" + UAID + "\",\"total\":0,\"manifests\":[]}",
        UaidJsonParser::parseManifests);
    expectInvalidResponse(
        "{\"uaid\":\""
            + UAID
            + "\",\"total\":0,\"has_more\":false,\"count_mode\":\"EXACT\",\"manifests\":[]}",
        UaidJsonParser::parseManifests);
  }

  private static void parsesCurrentManifestContract() {
    final UaidManifestsResponse response =
        UaidJsonParser.parseManifests(manifestResponse().getBytes(StandardCharsets.UTF_8));
    assert response.total() == 1L : "total mismatch";
    assert !response.hasMore() : "has_more mismatch";
    assert response.countMode() == UaidManifestCountMode.EXACT : "count_mode mismatch";
    final UaidManifestRecord record = response.manifests().get(0);
    assert HASH.equals(record.manifestHash()) : "hash mismatch";
    assert record.status() == UaidManifestStatus.REVOKED : "status mismatch";
    assert record.lifecycle().revocation() != null : "revocation missing";
    assert record.lifecycle().revocation().epoch() == 15L : "revocation epoch mismatch";
    assert record.lifecycle().revocation().reason() == null : "revocation reason mismatch";
    assert ((Number) record.manifestAsMap().get("version")).longValue() == 1L
        : "manifest version mismatch";
  }

  private static void rejectsRetiredManifestEncodings() {
    final String current = manifestResponse();
    for (final String payload :
        new String[] {
          current.replace("\"version\":1", "\"version\":\"1\""),
          current.replace("\"issued_ms\":100,", ""),
          current.replace(
              "\"issued_ms\":100,", "\"issued_ms\":100,\"expiry_epoch\":null,"),
          current.replace("\"notes\":\"test\"", "\"notes\":null"),
          current.replace("\"asset\":\"asset\"", "\"asset\":null"),
          current.replace("\"max_amount\":\"10\"", "\"max_amount\":null"),
          current.replace("\"manifest_hash\":\"" + HASH + "\"", "\"manifest_hash\":\"deadbeef\""),
          current.replace("\"status\":\"Revoked\"", "\"status\":\"revoked\"")
        }) {
      expectInvalidResponse(payload, UaidJsonParser::parseManifests);
    }
  }

  private static void manifestQueryUsesOnlyCurrentParameters() {
    final Map<String, String> parameters =
        UaidManifestQuery.builder()
            .setDataspaceId(7L)
            .setStatus(UaidManifestStatusFilter.INACTIVE)
            .setLimit(25L)
            .setOffset(5L)
            .setCountMode(UaidManifestCountMode.EXACT)
            .build()
            .toQueryParameters();
    assert parameters.equals(
        Map.of(
            "dataspace", "7",
            "status", "inactive",
            "limit", "25",
            "offset", "5",
            "count_mode", "exact"))
        : "manifest query mismatch";
    boolean rejected = false;
    try {
      UaidManifestQuery.builder().setLimit(0L);
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "zero manifest limit must be rejected before dispatch";
  }

  private static String portfolio(final String quantityJson) {
    return """
        {
          "uaid":"%s",
          "totals":{"accounts":1,"positions":1},
          "dataspaces":[{
            "dataspace_id":7,
            "dataspace_alias":null,
            "accounts":[{
              "account_id":"account",
              "label":null,
              "assets":[{
                "asset_id":"asset",
                "asset_definition_id":"definition",
                "quantity":%s
              }]
            }]
          }]
        }
        """.formatted(UAID, quantityJson);
  }

  private static String manifestResponse() {
    return """
        {
          "uaid":"%s",
          "total":1,
          "has_more":false,
          "count_mode":"exact",
          "manifests":[{
            "dataspace_id":7,
            "dataspace_alias":"primary",
            "manifest_hash":"%s",
            "status":"Revoked",
            "lifecycle":{
              "activated_epoch":10,
              "expired_epoch":null,
              "revocation":{"epoch":15,"reason":null}
            },
            "accounts":["account"],
            "manifest":{
              "version":1,
              "uaid":"%s",
              "dataspace":7,
              "issued_ms":100,
              "activation_epoch":10,
              "entries":[{
                "scope":{
                  "dataspace":7,
                  "program":"cash",
                  "method":"transfer",
                  "asset":"asset",
                  "role":"Initiator"
                },
                "effect":{"Allow":{"max_amount":"10","window":"PerDay"}},
                "notes":"test"
              }]
            }
          }]
        }
        """.formatted(UAID, HASH, UAID);
  }

  private static void expectInvalidResponse(
      final String json, final Consumer<byte[]> parser) {
    try {
      parser.accept(json.getBytes(StandardCharsets.UTF_8));
    } catch (final IllegalStateException | IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError("invalid UAID response field was accepted");
  }
}
