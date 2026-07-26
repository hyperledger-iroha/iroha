package org.hyperledger.iroha.android.nexus;

import java.util.List;
import java.util.Objects;

/** Immutable view over `/v1/space-directory/uaids/{uaid}` responses. */
public final class UaidBindingsResponse {
  private final String uaid;
  private final List<UaidBindingsDataspace> dataspaces;

  public UaidBindingsResponse(final String uaid, final List<UaidBindingsDataspace> dataspaces) {
    this.uaid = Objects.requireNonNull(uaid, "uaid");
    this.dataspaces = List.copyOf(Objects.requireNonNull(dataspaces, "dataspaces"));
  }

  public String uaid() {
    return uaid;
  }

  public List<UaidBindingsDataspace> dataspaces() {
    return dataspaces;
  }

  /** Dataspace binding entry returned by Torii. */
  public static final class UaidBindingsDataspace {
    private final long dataspaceId;
    private final String dataspaceAlias;
    private final List<String> accounts;

    public UaidBindingsDataspace(
        final long dataspaceId, final String dataspaceAlias, final List<String> accounts) {
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

    public List<String> accounts() {
      return accounts;
    }
  }
}
