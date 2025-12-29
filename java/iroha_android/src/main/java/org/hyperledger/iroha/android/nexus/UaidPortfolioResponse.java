package org.hyperledger.iroha.android.nexus;

import java.util.List;
import java.util.Objects;

/** Immutable view over `/v1/accounts/{uaid}/portfolio` responses. */
public final class UaidPortfolioResponse {
  private final String uaid;
  private final UaidPortfolioTotals totals;
  private final List<UaidPortfolioDataspace> dataspaces;

  public UaidPortfolioResponse(
      final String uaid,
      final UaidPortfolioTotals totals,
      final List<UaidPortfolioDataspace> dataspaces) {
    this.uaid = Objects.requireNonNull(uaid, "uaid");
    this.totals = Objects.requireNonNull(totals, "totals");
    this.dataspaces = List.copyOf(Objects.requireNonNull(dataspaces, "dataspaces"));
  }

  public String uaid() {
    return uaid;
  }

  public UaidPortfolioTotals totals() {
    return totals;
  }

  public List<UaidPortfolioDataspace> dataspaces() {
    return dataspaces;
  }

  /** Aggregated account/position totals for a UAID. */
  public static final class UaidPortfolioTotals {
    private final long accounts;
    private final long positions;

    public UaidPortfolioTotals(final long accounts, final long positions) {
      this.accounts = Math.max(0L, accounts);
      this.positions = Math.max(0L, positions);
    }

    public long accounts() {
      return accounts;
    }

    public long positions() {
      return positions;
    }
  }

  /** Dataspace entry within the aggregated portfolio response. */
  public static final class UaidPortfolioDataspace {
    private final long dataspaceId;
    private final String dataspaceAlias;
    private final List<UaidPortfolioAccount> accounts;

    public UaidPortfolioDataspace(
        final long dataspaceId,
        final String dataspaceAlias,
        final List<UaidPortfolioAccount> accounts) {
      this.dataspaceId = dataspaceId;
      this.dataspaceAlias = dataspaceAlias;
      this.accounts = List.copyOf(Objects.requireNonNull(accounts, "accounts"));
    }

    public long dataspaceId() {
      return dataspaceId;
    }

    public String dataspaceAlias() {
      return dataspaceAlias;
    }

    public List<UaidPortfolioAccount> accounts() {
      return accounts;
    }
  }

  /** Account entry underneath a dataspace portfolio record. */
  public static final class UaidPortfolioAccount {
    private final String accountId;
    private final String label;
    private final List<UaidPortfolioAsset> assets;

    public UaidPortfolioAccount(
        final String accountId, final String label, final List<UaidPortfolioAsset> assets) {
      this.accountId = Objects.requireNonNull(accountId, "accountId");
      this.label = label;
      this.assets = List.copyOf(Objects.requireNonNull(assets, "assets"));
    }

    public String accountId() {
      return accountId;
    }

    public String label() {
      return label;
    }

    public List<UaidPortfolioAsset> assets() {
      return assets;
    }
  }

  /** Asset balance entry associated with an account. */
  public static final class UaidPortfolioAsset {
    private final String assetId;
    private final String assetDefinitionId;
    private final String quantity;

    public UaidPortfolioAsset(
        final String assetId, final String assetDefinitionId, final String quantity) {
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      this.assetDefinitionId = Objects.requireNonNull(assetDefinitionId, "assetDefinitionId");
      this.quantity = Objects.requireNonNull(quantity, "quantity");
    }

    public String assetId() {
      return assetId;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public String quantity() {
      return quantity;
    }
  }
}
