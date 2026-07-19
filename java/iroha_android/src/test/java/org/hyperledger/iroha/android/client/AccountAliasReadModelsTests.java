package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.junit.Test;

/** Tests typed visibility-aware alias read DTOs. */
public final class AccountAliasReadModelsTests {
  @Test
  public void parsesIndexAndVisibilityFilteredAccountResults() throws Exception {
    final String account = account();
    final AccountAliasIndexResolution index =
        AccountAliasReadJsonParser.parseIndexResolution(
            ("{\"index\":7,\"alias\":\"merchant@banka.paynet\",\"account_id\":\""
                    + account
                    + "\",\"source\":\"on_chain\"}")
                .getBytes(StandardCharsets.UTF_8));
    assert BigInteger.valueOf(7).equals(index.index());
    assert "merchant@banka.paynet".equals(index.alias());

    final AccountAliasesByAccount result =
        AccountAliasReadJsonParser.parseByAccount(
            ("{\"account_id\":\""
                    + account
                    + "\",\"total\":2,\"items\":["
                    + "{\"alias\":\"alpha@paynet\",\"dataspace\":\"paynet\",\"is_primary\":false},"
                    + "{\"alias\":\"merchant@banka.paynet\",\"dataspace\":\"paynet\","
                    + "\"domain\":\"banka\",\"is_primary\":true}],\"source\":\"on_chain\"}")
                .getBytes(StandardCharsets.UTF_8));
    assert BigInteger.valueOf(2).equals(result.total());
    assert "alpha@paynet".equals(result.items().get(0).alias());
    assert result.items().get(1).isPrimary();
  }

  @Test
  public void preservesTheFullUnsigned64BitRangeAndRejectsInvalidNumbers() throws Exception {
    final String max = "18446744073709551615";
    final AccountAliasIndexResolution index =
        AccountAliasReadJsonParser.parseIndexResolution(
            ("{\"index\":"
                    + max
                    + ",\"alias\":\"merchant@paynet\",\"account_id\":\""
                    + account()
                    + "\"}")
                .getBytes(StandardCharsets.UTF_8));
    assert new BigInteger(max).equals(index.index());

    final AccountAliasResolution resolution =
        AccountAliasJsonParser.parseResolution(
            ("{\"alias\":\"merchant@paynet\",\"account_id\":\""
                    + account()
                    + "\",\"index\":"
                    + max
                    + "}")
                .getBytes(StandardCharsets.UTF_8));
    assert new BigInteger(max).equals(resolution.index());

    assertInvalidIndex("18446744073709551616");
    assertInvalidIndex("-1");
    assertInvalidIndex("7.0");
  }

  @Test
  public void requestRejectsAmbiguousScopeAndResponsesRejectDrift() throws Exception {
    final AccountAliasesByAccountRequest request =
        new AccountAliasesByAccountRequest(account(), "Paynet", "Banka");
    assert "paynet".equals(request.dataspace());
    assert "banka".equals(request.domain());
    try {
      new AccountAliasesByAccountRequest(account(), null, "banka");
      throw new AssertionError("domain without dataspace must fail");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }

    try {
      AccountAliasReadJsonParser.parseByAccount(
          ("{\"account_id\":\""
                  + account()
                  + "\",\"total\":1,\"items\":[{\"alias\":\"merchant@banka.paynet\","
                  + "\"dataspace\":\"other\",\"domain\":\"banka\",\"is_primary\":true}]}")
              .getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("inconsistent alias scope must fail");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static String account() throws Exception {
    final byte[] key = new byte[32];
    Arrays.fill(key, (byte) 0x22);
    return AccountAddress.fromAccount(key, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }

  private static void assertInvalidIndex(final String value) throws Exception {
    try {
      AccountAliasReadJsonParser.parseIndexResolution(
          ("{\"index\":"
                  + value
                  + ",\"alias\":\"merchant@paynet\",\"account_id\":\""
                  + account()
                  + "\"}")
              .getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("invalid uint64 alias index must fail: " + value);
    } catch (final IllegalStateException expected) {
      // Expected.
    }
  }
}
