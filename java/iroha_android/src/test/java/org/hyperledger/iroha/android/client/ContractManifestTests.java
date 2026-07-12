package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Exact-shape checks for the complete Kotodama V1 contract manifest. */
public final class ContractManifestTests {
  private static final String HASH_B = repeat('b', 64);
  private static final String HASH_D = repeat('d', 64);
  private static final String FILTER =
      "TlJUMAAAl9+YQQ4oJZjALRf6FAto0QAKAAAAAAAAANzCjydU9+jNAgIAAAAFBAAAAAA=";

  private ContractManifestTests() {}

  public static void main(final String[] args) {
    fullManifestPreservesExactKotodamaV1Interface();
    manifestRejectsUnknownEnglishAndNoncanonicalShapes();
    manifestEndpointValidatesPathAndParsesFullRecord();
    flatListTapeEnforcesTheExactV1DepthBoundary();
    everyReservedProjectionAndPageHasAnExactNominalName();
    everyReservedProjectionAndPageRejectsForgedStructure();
    System.out.println("[IrohaAndroid] contract manifest tests passed.");
  }

  private static void fullManifestPreservesExactKotodamaV1Interface() {
    final ContractManifestRecord record =
        ContractJsonParser.parseManifestRecord(fullResponse().getBytes(StandardCharsets.UTF_8));
    final ContractManifest manifest = record.manifest();
    require("Ledger".equals(manifest.seiyakuName()), "seiyaku_name");
    require(HASH_B.equals(manifest.codeHashHex()), "manifest code hash");
    require(HASH_D.equals(manifest.abiHashHex()), "manifest ABI hash");
    require(
        manifest.accessSetHints().dynamicReads().get(0).maxKeys() == 64,
        "dynamic read bound");
    final ContractManifest.EntrypointDescriptor entrypoint = manifest.entrypoints().get(0);
    require(
        entrypoint.kind() == ContractManifest.EntrypointKind.KOTOAGE,
        "branded kotoage kind");
    require(entrypoint.argumentSchema().fields().get(0).valueType().wordCount() == 2, "arg words");
    require(
        "struct Transfer"
            .equals(entrypoint.argumentSchema().fields().get(0).valueType().canonicalTypeName()),
        "argument type identity");
    final ContractManifest.ValueTypeV1 tagsType =
        entrypoint.argumentSchema().fields().get(1).valueType();
    require(tagsType.nodes().size() == 2, "flat list node count");
    require(tagsType.nodes().get(0).listValue().capacity() == 64, "flat list capacity");
    require("List<Name, 64>".equals(tagsType.canonicalTypeName()), "flat list type identity");
    require(entrypoint.returnSchema().wordCount() == 1, "result handle word");
    require(
        "Result<(bool, decimal), string>".equals(entrypoint.returnSchema().canonicalTypeName()),
        "return type identity");
    require(
        entrypoint.triggers().get(0).repeats().kind()
            == ContractManifest.TriggerRepeatsKind.INDEFINITELY,
        "trigger repetition");
    require(
        "transfer".equals(entrypoint.triggers().get(0).callback().entrypoint()),
        "trigger callback");
    require(
        "StateMap<AccountId, quantity>".equals(manifest.states().get(0).typeName()),
        "state type");
    require(manifest.errorCodes().get(0).code() == 1001, "error code");
    require(
        "ja".equals(manifest.kotoba().get(0).translations().get(1).language()),
        "kotoba language");
    require("ed25519:fixture".equals(manifest.provenance().signer()), "provenance signer");
  }

  private static void manifestRejectsUnknownEnglishAndNoncanonicalShapes() {
    final String response = fullResponse();
    final String[] invalid = {
      replaceFirst(response, "\"seiyaku_name\"", "\"contract_name\""),
      replaceFirst(response, "\"Kotoage\"", "\"Public\""),
      replaceFirst(response, "\"Kotoage\"", "\"View\""),
      replaceFirst(response, "\"capacity\":64", "\"capacity\":65"),
      replaceFirst(
          response,
          "\"name\":\"request\",\"ty\"",
          "\"name\":\"different\",\"ty\""),
      replaceFirst(response, "#ABA2", "#0000"),
      replaceFirst(
          response,
          "\"seiyaku_name\":\"Ledger\"",
          "\"seiyaku_name\":\"match\""),
      replaceFirst(
          response,
          "\"seiyaku_name\":\"Ledger\"",
          "\"seiyaku_name\":\"Option\""),
      replaceFirst(
          response,
          "\"seiyaku_name\":\"Ledger\"",
          "\"seiyaku_name\":\"__kotodama_link_private\""),
      replaceFirst(
          response,
          "\"seiyaku_name\":\"Ledger\"",
          "\"seiyaku_name\":\"state_map_get\""),
      replaceFirst(
          response,
          "\"seiyaku_name\":\"Ledger\"",
          "\"seiyaku_name\":\"__kotodama_quantity_ratio_round\""),
      replaceFirst(
          response,
          "\"seiyaku_name\":\"Ledger\"",
          "\"seiyaku_name\":\"__kotodama_decimal_to_int_trunc\""),
      replaceFirst(
          response,
          "\"seiyaku_name\":\"Ledger\"",
          "\"seiyaku_name\":\"__kotodama_decimal_to_int_round\""),
      replaceFirst(response, "\"kind\":\"Quantity\"", "\"kind\":\"Amount\""),
      replaceFirst(response, "\"kind\":\"Decimal\"", "\"kind\":\"U128\""),
      replaceFirst(
          response,
          "\"namespace\":\"TransferError\"",
          "\"namespace\":\"Option\""),
      replaceFirst(response, "\"features_bitmap\":0", "\"features_bitmap\":4"),
      replaceFirst(
          response,
          "\"dynamic_writes\":[]",
          "\"dynamic_writes\":[],\"unknown\":true"),
      replaceFirst(
          response,
          "\"repeats\":{\"Indefinitely\":null}",
          "\"repeats\":{\"kind\":\"Indefinitely\",\"value\":null}"),
      replaceFirst(
          response,
          "\"code_hash\":\"" + HASH_B + "\"",
          "\"code_hash\":\"" + repeat('f', 64) + "\"")
    };
    for (final String payload : invalid) {
      expectFailure(
          () ->
              ContractJsonParser.parseManifestRecord(
                  payload.getBytes(StandardCharsets.UTF_8)));
    }
  }

  private static void manifestEndpointValidatesPathAndParsesFullRecord() {
    final ManifestExecutor executor =
        new ManifestExecutor(fullResponse().getBytes(StandardCharsets.UTF_8));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .build());
    final ContractManifestRecord record = transport.getContractManifest(HASH_B).join();
    require("Ledger".equals(record.manifest().seiyakuName()), "endpoint manifest");
    require(
        ("https://torii.example/api/v1/contracts/code/" + HASH_B)
            .equals(executor.lastRequest.uri().toString()),
        "endpoint path");
    final int requests = executor.requestCount;
    expectFailure(() -> transport.getContractManifest("abc"));
    expectFailure(() -> transport.getContractManifest("0x" + HASH_B));
    require(requests == executor.requestCount, "invalid hash dispatched");
  }

  private static void flatListTapeEnforcesTheExactV1DepthBoundary() {
    final String list = listNode(1);
    final String leaf = leafNode("Int");
    final List<String> validNodes = new ArrayList<>();
    final StringBuilder validName = new StringBuilder();
    for (int index = 0; index < 255; index++) {
      validNodes.add(list);
      validName.append("List<");
    }
    validNodes.add(leaf);
    validName.append("int");
    for (int index = 0; index < 255; index++) {
      validName.append(", 1>");
    }
    final ContractManifest.ValueTypeV1 schema =
        parseBoundarySchema(validNodes, validName.toString());
    require(schema.nodes().size() == 256, "maximum flat depth node count");
    require(schema.wordCount() == 1, "maximum flat depth word count");
    require(validName.toString().equals(schema.canonicalTypeName()), "maximum flat depth name");

    final List<List<String>> malformed = new ArrayList<>();
    malformed.add(Arrays.asList(list));
    malformed.add(Arrays.asList(leaf, leaf));
    malformed.add(
        Arrays.asList(
            "{\"kind\":\"List\",\"value\":{\"capacity\":1,\"element\":{\"nodes\":["
                + leaf
                + "]}}}",
            leaf));
    malformed.add(Arrays.asList(listNode(0), leaf));
    malformed.add(Arrays.asList(listNode(65), leaf));
    malformed.add(Arrays.asList(optionNode()));
    malformed.add(Arrays.asList("{\"kind\":\"Result\",\"value\":null}", leaf));
    malformed.add(Arrays.asList("{\"kind\":\"Tuple\",\"value\":2}", leaf));
    final List<String> tooDeep = new ArrayList<>();
    for (int index = 0; index < 256; index++) {
      tooDeep.add(list);
    }
    tooDeep.add(leaf);
    malformed.add(tooDeep);
    for (final List<String> nodes : malformed) {
      expectFailure(() -> parseBoundarySchema(nodes, "int"));
    }
  }

  private static void everyReservedProjectionAndPageHasAnExactNominalName() {
    final List<String> pair =
        Arrays.asList(structNode("Pair", "left", "right"), leafNode("Int"), leafNode("Bool"));
    require(
        "struct Pair".equals(parseBoundarySchema(pair, "struct Pair").canonicalTypeName()),
        "ordinary struct canonical name");

    for (final String viewName : coreViewNames()) {
      final List<String> view = coreViewNodes(viewName);
      require(
          viewName.equals(parseBoundarySchema(view, viewName).canonicalTypeName()),
          viewName + " canonical name");
      final String pageName = "QueryPage<" + viewName + ">";
      require(
          pageName.equals(parseBoundarySchema(queryPageNodes(view), pageName).canonicalTypeName()),
          pageName + " canonical name");
    }
  }

  private static void everyReservedProjectionAndPageRejectsForgedStructure() {
    assertCanonicalSchemaFailure(
        "AccountView",
        Arrays.asList(
            structNode("AccountView", "id", "metadata"),
            leafNode("AccountId"),
            leafNode("Bool")));
    assertCanonicalSchemaFailure(
        "AssetView",
        Arrays.asList(
            structNode("AssetView", "id", "amount"),
            leafNode("AssetId"),
            leafNode("Decimal")));
    assertCanonicalSchemaFailure(
        "AssetDefinitionView",
        Arrays.asList(
            structNode(
                "AssetDefinitionView",
                "id",
                "name",
                "description",
                "owned_by",
                "total_quantity",
                "metadata"),
            leafNode("AssetDefinitionId"),
            leafNode("String"),
            optionNode(),
            leafNode("Bool"),
            leafNode("AccountId"),
            leafNode("Quantity"),
            leafNode("Json")));
    assertCanonicalSchemaFailure(
        "DomainView",
        Arrays.asList(
            structNode("DomainView", "id", "owned_by", "metadata"),
            leafNode("DomainId"),
            leafNode("DomainId"),
            leafNode("Json")));
    assertCanonicalSchemaFailure(
        "NftView",
        Arrays.asList(
            structNode("NftView", "id", "owned_by", "content"),
            leafNode("NftId"),
            leafNode("AccountId"),
            leafNode("String")));

    final List<String> page = queryPageNodes(coreViewNodes("AccountView"));
    final List<String> wrongCapacity = new ArrayList<>(page);
    wrongCapacity.set(1, listNode(63));
    assertCanonicalSchemaFailure("QueryPage<AccountView>", wrongCapacity);
    final List<String> wrongOffset = new ArrayList<>(page);
    wrongOffset.set(wrongOffset.size() - 1, leafNode("Bool"));
    assertCanonicalSchemaFailure("QueryPage<AccountView>", wrongOffset);
    final List<String> wrongFields = new ArrayList<>(page);
    wrongFields.set(0, structNode("QueryPage", "next_offset", "items"));
    assertCanonicalSchemaFailure("QueryPage<AccountView>", wrongFields);
    assertCanonicalSchemaFailure(
        "struct QueryPage",
        Arrays.asList(
            structNode("QueryPage", "items", "next_offset"),
            listNode(64),
            structNode("Pair", "left", "right"),
            leafNode("Int"),
            leafNode("Bool"),
            optionNode(),
            leafNode("Int")));
  }

  private static ContractManifest.ValueTypeV1 parseBoundarySchema(
      final String nodes, final String typeName) {
    final String payload =
        "{\"manifest\":{\"entrypoints\":[{\"name\":\"inspect\","
            + "\"kind\":{\"kind\":\"View\",\"value\":null},"
            + "\"params\":[{\"name\":\"value\",\"type_name\":\""
            + typeName
            + "\"}],\"argument_schema\":{\"fields\":[{\"name\":\"value\","
            + "\"ty\":{\"nodes\":["
            + nodes
            + "]}}]}}]}}";
    return ContractJsonParser.parseManifestRecord(payload.getBytes(StandardCharsets.UTF_8))
        .manifest()
        .entrypoints()
        .get(0)
        .argumentSchema()
        .fields()
        .get(0)
        .valueType();
  }

  private static ContractManifest.ValueTypeV1 parseBoundarySchema(
      final List<String> nodes, final String typeName) {
    return parseBoundarySchema(join(nodes, ","), typeName);
  }

  private static String optionNode() {
    return "{\"kind\":\"Option\",\"value\":null}";
  }

  private static String listNode(final int capacity) {
    return "{\"kind\":\"List\",\"value\":{\"capacity\":" + capacity + "}}";
  }

  private static String leafNode(final String kind) {
    return "{\"kind\":\"Leaf\",\"value\":{\"kind\":\""
        + kind
        + "\",\"value\":null}}";
  }

  private static String structNode(final String name, final String... fields) {
    final List<String> quoted = new ArrayList<>();
    for (final String field : fields) {
      quoted.add("\"" + field + "\"");
    }
    return "{\"kind\":\"Struct\",\"value\":{\"name\":\""
        + name
        + "\",\"fields\":["
        + join(quoted, ",")
        + "]}}";
  }

  private static List<String> coreViewNames() {
    return Arrays.asList(
        "AccountView", "AssetView", "AssetDefinitionView", "DomainView", "NftView");
  }

  private static List<String> coreViewNodes(final String name) {
    if ("AccountView".equals(name)) {
      return Arrays.asList(
          structNode(name, "id", "metadata"), leafNode("AccountId"), leafNode("Json"));
    }
    if ("AssetView".equals(name)) {
      return Arrays.asList(
          structNode(name, "id", "amount"), leafNode("AssetId"), leafNode("Quantity"));
    }
    if ("AssetDefinitionView".equals(name)) {
      return Arrays.asList(
          structNode(
              name, "id", "name", "description", "owned_by", "total_quantity", "metadata"),
          leafNode("AssetDefinitionId"),
          leafNode("String"),
          optionNode(),
          leafNode("String"),
          leafNode("AccountId"),
          leafNode("Quantity"),
          leafNode("Json"));
    }
    if ("DomainView".equals(name)) {
      return Arrays.asList(
          structNode(name, "id", "owned_by", "metadata"),
          leafNode("DomainId"),
          leafNode("AccountId"),
          leafNode("Json"));
    }
    if ("NftView".equals(name)) {
      return Arrays.asList(
          structNode(name, "id", "owned_by", "content"),
          leafNode("NftId"),
          leafNode("AccountId"),
          leafNode("Json"));
    }
    throw new IllegalArgumentException("unsupported test view " + name);
  }

  private static List<String> queryPageNodes(final List<String> view) {
    final List<String> nodes = new ArrayList<>();
    nodes.add(structNode("QueryPage", "items", "next_offset"));
    nodes.add(listNode(64));
    nodes.addAll(view);
    nodes.add(optionNode());
    nodes.add(leafNode("Int"));
    return nodes;
  }

  private static void assertCanonicalSchemaFailure(
      final String typeName, final List<String> nodes) {
    try {
      parseBoundarySchema(nodes, typeName);
    } catch (final IllegalStateException expected) {
      require(
          expected.getMessage() != null && expected.getMessage().contains("canonical flat preorder"),
          "forged schema failed for an unrelated reason: " + expected.getMessage());
      return;
    }
    throw new AssertionError("forged reserved schema was accepted: " + typeName);
  }

  private static String join(final List<String> values, final String delimiter) {
    final StringBuilder result = new StringBuilder();
    for (int index = 0; index < values.size(); index++) {
      if (index != 0) {
        result.append(delimiter);
      }
      result.append(values.get(index));
    }
    return result.toString();
  }

  private static String fullResponse() {
    return "{"
        + "\"manifest\":{"
        + "\"seiyaku_name\":\"Ledger\","
        + "\"code_hash\":\"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2\","
        + "\"abi_hash\":\"hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071\","
        + "\"compiler_fingerprint\":\"kotodama_lang\",\"features_bitmap\":0,"
        + "\"access_set_hints\":{"
        + "\"read_keys\":[\"state:Balances\"],\"write_keys\":[\"state:Balances\"],"
        + "\"dynamic_reads\":[{\"base_key\":\"state:Balances\",\"key_type\":\"AccountId\","
        + "\"bound_kind\":\"take\",\"max_keys\":64}],\"dynamic_writes\":[]},"
        + "\"entrypoints\":[{\"name\":\"transfer\","
        + "\"kind\":{\"kind\":\"Kotoage\",\"value\":null},"
        + "\"params\":[{\"name\":\"request\",\"type_name\":\"struct Transfer\"},"
        + "{\"name\":\"tags\",\"type_name\":\"List<Name, 64>\"}],"
        + "\"argument_schema\":{\"fields\":["
        + "{\"name\":\"request\",\"ty\":{\"nodes\":["
        + "{\"kind\":\"Struct\",\"value\":{\"name\":\"Transfer\",\"fields\":[\"amount\",\"memo\"]}},"
        + "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"Quantity\",\"value\":null}},"
        + "{\"kind\":\"Option\",\"value\":null},"
        + "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"String\",\"value\":null}}]}},"
        + "{\"name\":\"tags\",\"ty\":{\"nodes\":["
        + "{\"kind\":\"List\",\"value\":{\"capacity\":64}},"
        + "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"Name\",\"value\":null}}]}}]},"
        + "\"return_type\":\"Result<(bool, decimal), string>\","
        + "\"return_schema\":{\"nodes\":[{\"kind\":\"Result\",\"value\":null},"
        + "{\"kind\":\"Tuple\",\"value\":2},"
        + "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"Bool\",\"value\":null}},"
        + "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"Decimal\",\"value\":null}},"
        + "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"String\",\"value\":null}}]},"
        + "\"permission\":\"TransferAsset\",\"read_keys\":[\"state:Balances\"],"
        + "\"write_keys\":[\"state:Balances\"],\"access_hints_complete\":true,"
        + "\"access_hints_skipped\":[],\"triggers\":[{\"id\":\"settle\","
        + "\"repeats\":{\"Indefinitely\":null},\"filter\":\""
        + FILTER
        + "\",\"authority\":null,\"metadata\":{\"purpose\":\"daily-settlement\",\"round\":7},"
        + "\"callback\":{\"namespace\":null,\"entrypoint\":\"transfer\"}}]}],"
        + "\"states\":[{\"name\":\"Balances\",\"type_name\":\"StateMap<AccountId, quantity>\"}],"
        + "\"error_codes\":[{\"namespace\":\"TransferError\",\"name\":\"InsufficientFunds\",\"code\":1001}],"
        + "\"kotoba\":[{\"msg_id\":\"transfer.denied\",\"translations\":["
        + "{\"lang\":\"en\",\"text\":\"Transfer denied\"},{\"lang\":\"ja\",\"text\":\"送金は拒否されました\"}]}],"
        + "\"provenance\":{\"signer\":\"ed25519:fixture\",\"signature\":\"fixture-signature\"}},"
        + "\"code_hash\":\""
        + HASH_B
        + "\",\"abi_hash\":\""
        + HASH_D
        + "\"}";
  }

  private static String replaceFirst(
      final String source, final String target, final String replacement) {
    final int index = source.indexOf(target);
    require(index >= 0, "test replacement target missing: " + target);
    return source.substring(0, index) + replacement + source.substring(index + target.length());
  }

  private static String repeat(final char value, final int count) {
    final char[] chars = new char[count];
    java.util.Arrays.fill(chars, value);
    return new String(chars);
  }

  private static void expectFailure(final Runnable action) {
    boolean failed = false;
    try {
      action.run();
    } catch (final IllegalArgumentException | IllegalStateException expected) {
      failed = true;
    }
    require(failed, "invalid manifest was accepted");
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }

  private static final class ManifestExecutor implements HttpTransportExecutor {
    private final byte[] payload;
    private TransportRequest lastRequest;
    private int requestCount;

    private ManifestExecutor(final byte[] payload) {
      this.payload = payload.clone();
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requestCount++;
      lastRequest = request;
      return CompletableFuture.completedFuture(
          new TransportResponse(200, payload, "ok", Collections.emptyMap()));
    }
  }
}
