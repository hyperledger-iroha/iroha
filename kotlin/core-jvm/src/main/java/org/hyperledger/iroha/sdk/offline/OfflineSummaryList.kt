package org.hyperledger.iroha.sdk.offline

/** Immutable view over `/v1/offline/summaries` responses. */
class OfflineSummaryList(
    items: List<OfflineSummaryItem>,
    val total: Long,
) {
    val items: List<OfflineSummaryItem> = items.toList()

    class OfflineSummaryItem(
        val certificateIdHex: String,
        val controllerId: String,
        val controllerDisplay: String,
        val summaryHashHex: String,
        appleKeyCounters: Map<String, Long>,
        androidSeriesCounters: Map<String, Long>,
    ) {
        val appleKeyCounters: Map<String, Long> = appleKeyCounters.toMap()
        val androidSeriesCounters: Map<String, Long> = androidSeriesCounters.toMap()
    }
}
