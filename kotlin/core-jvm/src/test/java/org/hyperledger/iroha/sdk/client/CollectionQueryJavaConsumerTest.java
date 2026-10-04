// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.math.BigDecimal;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.sdk.client.collections.AccountAssetRow;
import org.hyperledger.iroha.sdk.client.collections.DomainRow;
import org.hyperledger.iroha.sdk.client.collections.PagedResults;
import org.hyperledger.iroha.sdk.client.collections.TransactionRow;
import org.hyperledger.iroha.sdk.client.stream.EventFields;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.hyperledger.iroha.sdk.json.Json;
import org.hyperledger.iroha.sdk.query.Filter;
import org.hyperledger.iroha.sdk.query.ListQuery;
import org.hyperledger.iroha.sdk.query.ListQueryException;
import org.hyperledger.iroha.sdk.query.Page;
import org.hyperledger.iroha.sdk.query.SortKey;
import org.junit.jupiter.api.Test;

/** Java applications build filters and read Torii collections through the Kotlin API. */
final class CollectionQueryJavaConsumerTest {
  @Test
  void javaBuildsTheSameCanonicalQuery() {
    final Filter filter = Filter.field("owned_by").eq("alice")
        .and(Filter.field("quantity").gte(new BigDecimal("10.5")))
        .and(Filter.field("status").isIn("active", "paused").or(Filter.field("metadata.tier").exists().not()));
    final ListQuery query = ListQuery.builder()
        .filter(filter)
        .sort(SortKey.desc("quantity"), Filter.field("id").asc())
        .limit(50)
        .includeTotal()
        .build();

    assertEquals(
        "owned_by = \"alice\" and quantity >= \"10.5\" and (status in [\"active\", \"paused\"] or not exists(metadata.tier))",
        filter.toString());
    assertEquals(filter, Filter.parse(filter.toString()));
    assertEquals(
        "{\"filter\":" + filter.toJson().toJsonString() + ",\"sort\":[\"-quantity\",\"id\"],\"limit\":50,\"include_total\":true}",
        query.toJson().toJsonString());
    assertEquals("-quantity,id", query.toQueryPairs().get(1).getValue());
    final ListQueryException invalid = assertThrows(ListQueryException.class, () -> ListQuery.builder().limit(0).build());
    assertEquals("invalid_limit", invalid.getCode());
    assertEquals(
        "tx_hash = \"ab\" and block_height > 10",
        EventFields.TX_HASH.eq("ab").and(EventFields.BLOCK_HEIGHT.gt(10)).toString());
  }

  @Test
  void javaPagesAndIteratesCollections() throws Exception {
    final Scripted executor = new Scripted(Arrays.asList(
        "{\"items\":[{\"id\":\"a\",\"owned_by\":\"o@w\"}],\"next_cursor\":\"c1\",\"total\":2}",
        "{\"items\":[{\"id\":\"b\",\"owned_by\":\"o@w\"}],\"next_cursor\":null}",
        "{\"items\":[{\"asset\":\"xor\",\"scope\":\"global\",\"account_id\":\"a@w\",\"quantity\":\"0.000000001\"}],\"next_cursor\":null}"));
    final HttpClientTransport client = new HttpClientTransport(
        executor, ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final Page<DomainRow> first = client.domains().page(ListQuery.builder().limit(1).includeTotal().build()).get();
    assertEquals(Long.valueOf(2L), first.total);
    assertTrue(first.hasMore());
    final List<String> ids = new ArrayList<>();
    try (PagedResults<DomainRow> rows = client.domains().iterate(ListQuery.EMPTY.withCursor(first.nextCursor))) {
      for (DomainRow row : rows) {
        ids.add(row.id);
      }
    }
    assertEquals(Arrays.asList("b"), ids);

    final AccountAssetRow balance = client.accountAssets("a@w").fetchAll().get().get(0);
    assertEquals(new BigDecimal("0.000000001"), balance.quantity);
  }

  @Test
  void javaReadsTransactionHistory() throws Exception {
    final Scripted executor = new Scripted(Arrays.asList(
        "{\"items\":[],\"next_cursor\":\"h1\"}",
        "{\"items\":[{\"entrypoint_hash\":\"t1\",\"block_height\":9,\"block_index\":2,"
            + "\"asset_definition_ids\":[\"xor\"]}],\"next_cursor\":null}"));
    final HttpClientTransport client = new HttpClientTransport(
        executor, ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final List<TransactionRow> rows = client.transactions()
        .fetchAll(ListQuery.builder().filter(Filter.field("block_height").gte(5)).build())
        .get();
    assertEquals(1, rows.size());
    assertEquals(9L, rows.get(0).blockHeight);
    assertEquals(Arrays.asList("xor"), rows.get(0).assetDefinitionIds);
    final ListQueryException sort = assertThrows(
        ListQueryException.class,
        () -> client.accountTransactions("a@w").page(ListQuery.builder().sort("-block_height").build()));
    assertEquals("invalid_sort", sort.getCode());
  }

  @Test
  void javaSeesTypedApiErrors() {
    final Scripted executor = new Scripted(new ArrayList<>());
    executor.error = "{\"code\":\"invalid_sort\",\"message\":\"invalid `sort`: unknown field\","
        + "\"details\":{\"field\":\"sort\",\"actual\":\"colour\",\"expected\":\"id, owned_by\"}}";
    final HttpClientTransport client = new HttpClientTransport(
        executor, ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build());

    final CompletionException failure = assertThrows(
        CompletionException.class,
        () -> client.nfts().page(ListQuery.builder().sort("colour").build()).join());
    final ToriiApiException error = (ToriiApiException) failure.getCause();
    assertEquals(400, error.status);
    assertEquals("invalid_sort", error.code);
    assertEquals("sort", error.getField());
    assertEquals("colour", error.detail("actual"));
    assertFalse(error.getMessage().isEmpty());
    assertEquals(Json.parse("\"colour\""), error.details.get("actual"));
  }

  private static final class Scripted implements HttpTransportExecutor {
    private final ArrayDeque<String> bodies;
    String error;

    Scripted(List<String> bodies) {
      this.bodies = new ArrayDeque<>(bodies);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(TransportRequest request) {
      final String body = error != null ? error : bodies.removeFirst();
      return CompletableFuture.completedFuture(TransportResponse.builder()
          .setStatusCode(error != null ? 400 : 200)
          .setBody(body.getBytes(StandardCharsets.UTF_8))
          .build());
    }
  }
}
