package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Immutable view over `/v1/offline/summaries` responses. */
public final class OfflineSummaryList {

  private final List<OfflineSummaryItem> items;
  private final long total;

  public OfflineSummaryList(final List<OfflineSummaryItem> items, final long total) {
    this.items = List.copyOf(Objects.requireNonNull(items, "items"));
    this.total = total;
  }

  public List<OfflineSummaryItem> items() {
    return items;
  }

  public long total() {
    return total;
  }

  public static final class OfflineSummaryItem {
    private final String certificateIdHex;
    private final String controllerId;
    private final String controllerDisplay;
    private final String summaryHashHex;
    private final Map<String, Long> appleKeyCounters;
    private final Map<String, Long> androidSeriesCounters;

    public OfflineSummaryItem(
        final String certificateIdHex,
        final String controllerId,
        final String controllerDisplay,
        final String summaryHashHex,
        final Map<String, Long> appleKeyCounters,
        final Map<String, Long> androidSeriesCounters) {
      this.certificateIdHex = Objects.requireNonNull(certificateIdHex, "certificateIdHex");
      this.controllerId = Objects.requireNonNull(controllerId, "controllerId");
      this.controllerDisplay = Objects.requireNonNull(controllerDisplay, "controllerDisplay");
      this.summaryHashHex = Objects.requireNonNull(summaryHashHex, "summaryHashHex");
      Objects.requireNonNull(appleKeyCounters, "appleKeyCounters");
      Objects.requireNonNull(androidSeriesCounters, "androidSeriesCounters");
      this.appleKeyCounters = Map.copyOf(new LinkedHashMap<>(appleKeyCounters));
      this.androidSeriesCounters = Map.copyOf(new LinkedHashMap<>(androidSeriesCounters));
    }

    public String certificateIdHex() {
      return certificateIdHex;
    }

    public String controllerId() {
      return controllerId;
    }

    public String controllerDisplay() {
      return controllerDisplay;
    }

    public String summaryHashHex() {
      return summaryHashHex;
    }

    public Map<String, Long> appleKeyCounters() {
      return appleKeyCounters;
    }

    public Map<String, Long> androidSeriesCounters() {
      return androidSeriesCounters;
    }
  }
}
