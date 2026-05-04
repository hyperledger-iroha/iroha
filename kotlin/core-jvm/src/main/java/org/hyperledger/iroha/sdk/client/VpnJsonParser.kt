package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** Minimal JSON parser for Sora VPN Torii responses. */
object VpnJsonParser {

    @JvmStatic
    fun parseProfile(payload: ByteArray): VpnProfile {
        val root = expectObject(parse(payload, "vpn profile response"), "vpn profile response")
        return VpnProfile(
            available = root["available"] == true,
            relayEndpoint = requiredString(root["relay_endpoint"], "vpn profile response.relay_endpoint"),
            supportedExitClasses = stringList(root["supported_exit_classes"], "vpn profile response.supported_exit_classes"),
            defaultExitClass = requiredString(root["default_exit_class"], "vpn profile response.default_exit_class"),
            leaseSecs = asLong(root["lease_secs"], "vpn profile response.lease_secs"),
            dnsPushIntervalSecs = asLong(root["dns_push_interval_secs"], "vpn profile response.dns_push_interval_secs"),
            meterFamily = requiredString(root["meter_family"], "vpn profile response.meter_family"),
            routePushes = stringList(root["route_pushes"], "vpn profile response.route_pushes"),
            excludedRoutes = stringList(root["excluded_routes"], "vpn profile response.excluded_routes"),
            dnsServers = stringList(root["dns_servers"], "vpn profile response.dns_servers"),
            tunnelAddresses = stringList(root["tunnel_addresses"], "vpn profile response.tunnel_addresses"),
            mtuBytes = asLong(root["mtu_bytes"], "vpn profile response.mtu_bytes"),
            displayBillingLabel = requiredString(root["display_billing_label"], "vpn profile response.display_billing_label"),
            feeAssetId = requiredString(root["fee_asset_id"], "vpn profile response.fee_asset_id"),
            escrowAccountId = requiredString(root["escrow_account_id"], "vpn profile response.escrow_account_id"),
            operatorAccountId = requiredString(root["operator_account_id"], "vpn profile response.operator_account_id"),
            leaseFeeNanos = asLong(root["lease_fee_nanos"], "vpn profile response.lease_fee_nanos"),
            settlementGraceSecs = asLong(root["settlement_grace_secs"], "vpn profile response.settlement_grace_secs"),
            flowLabelBits = asInt(root["flow_label_bits"], "vpn profile response.flow_label_bits"),
            paddingBudgetMs = asInt(root["padding_budget_ms"], "vpn profile response.padding_budget_ms"),
            relayTlsSpkiSha256Hex = optionalHex32(root["relay_tls_spki_sha256_hex"], "relayTlsSpkiSha256Hex"),
        )
    }

    @JvmStatic
    fun parseQuote(payload: ByteArray): VpnQuote {
        val root = expectObject(parse(payload, "vpn quote response"), "vpn quote response")
        return VpnQuote(
            quoteId = hex32(root["quote_id"], "quoteId"),
            leaseIdHex = hex32(root["lease_id_hex"], "leaseIdHex"),
            sessionIdHex = evenHex(root["session_id_hex"], "sessionIdHex"),
            paymentReference = requiredString(root["payment_reference"], "vpn quote response.payment_reference"),
            accountId = requiredString(root["account_id"], "vpn quote response.account_id"),
            exitClass = requiredString(root["exit_class"], "vpn quote response.exit_class"),
            relayEndpoint = requiredString(root["relay_endpoint"], "vpn quote response.relay_endpoint"),
            leaseSecs = asLong(root["lease_secs"], "vpn quote response.lease_secs"),
            quoteExpiresAtMs = asLong(root["quote_expires_at_ms"], "vpn quote response.quote_expires_at_ms"),
            feeAssetId = requiredString(root["fee_asset_id"], "vpn quote response.fee_asset_id"),
            escrowAccountId = requiredString(root["escrow_account_id"], "vpn quote response.escrow_account_id"),
            operatorAccountId = requiredString(root["operator_account_id"], "vpn quote response.operator_account_id"),
            leaseFeeNanos = asLong(root["lease_fee_nanos"], "vpn quote response.lease_fee_nanos"),
            routePushes = stringList(root["route_pushes"], "vpn quote response.route_pushes"),
            excludedRoutes = stringList(root["excluded_routes"], "vpn quote response.excluded_routes"),
            dnsServers = stringList(root["dns_servers"], "vpn quote response.dns_servers"),
            tunnelAddresses = stringList(root["tunnel_addresses"], "vpn quote response.tunnel_addresses"),
            mtuBytes = asLong(root["mtu_bytes"], "vpn quote response.mtu_bytes"),
            meterFamily = requiredString(root["meter_family"], "vpn quote response.meter_family"),
            flowLabelBits = asInt(root["flow_label_bits"], "vpn quote response.flow_label_bits"),
            paddingBudgetMs = asInt(root["padding_budget_ms"], "vpn quote response.padding_budget_ms"),
            relayTlsSpkiSha256Hex = optionalHex32(root["relay_tls_spki_sha256_hex"], "relayTlsSpkiSha256Hex"),
            meteringPublicKeyHex = hex32(root["metering_public_key_hex"], "meteringPublicKeyHex"),
            openLeaseInstruction = optionalTxInstruction(root["open_lease_instruction"], "vpn quote response.open_lease_instruction"),
            txInstructions = txInstructionList(root["tx_instructions"], "vpn quote response.tx_instructions"),
        )
    }

    @JvmStatic
    fun parseSession(payload: ByteArray): VpnSession {
        val root = expectObject(parse(payload, "vpn session response"), "vpn session response")
        return VpnSession(
            sessionId = hex32(root["session_id"], "sessionId"),
            accountId = requiredString(root["account_id"], "vpn session response.account_id"),
            exitClass = requiredString(root["exit_class"], "vpn session response.exit_class"),
            relayEndpoint = requiredString(root["relay_endpoint"], "vpn session response.relay_endpoint"),
            leaseSecs = asLong(root["lease_secs"], "vpn session response.lease_secs"),
            expiresAtMs = asLong(root["expires_at_ms"], "vpn session response.expires_at_ms"),
            connectedAtMs = asLong(root["connected_at_ms"], "vpn session response.connected_at_ms"),
            meterFamily = requiredString(root["meter_family"], "vpn session response.meter_family"),
            quoteId = hex32(root["quote_id"], "quoteId"),
            paymentReference = requiredString(root["payment_reference"], "vpn session response.payment_reference"),
            paymentTxHash = hex32(root["payment_tx_hash"], "paymentTxHash"),
            feeAssetId = requiredString(root["fee_asset_id"], "vpn session response.fee_asset_id"),
            escrowAccountId = requiredString(root["escrow_account_id"], "vpn session response.escrow_account_id"),
            operatorAccountId = requiredString(root["operator_account_id"], "vpn session response.operator_account_id"),
            leaseFeeNanos = asLong(root["lease_fee_nanos"], "vpn session response.lease_fee_nanos"),
            flowLabelBits = asInt(root["flow_label_bits"], "vpn session response.flow_label_bits"),
            paddingBudgetMs = asInt(root["padding_budget_ms"], "vpn session response.padding_budget_ms"),
            relayTlsSpkiSha256Hex = optionalHex32(root["relay_tls_spki_sha256_hex"], "relayTlsSpkiSha256Hex"),
            routePushes = stringList(root["route_pushes"], "vpn session response.route_pushes"),
            excludedRoutes = stringList(root["excluded_routes"], "vpn session response.excluded_routes"),
            dnsServers = stringList(root["dns_servers"], "vpn session response.dns_servers"),
            tunnelAddresses = stringList(root["tunnel_addresses"], "vpn session response.tunnel_addresses"),
            mtuBytes = asLong(root["mtu_bytes"], "vpn session response.mtu_bytes"),
            helperTicketHex = evenHex(root["helper_ticket_hex"], "helperTicketHex"),
            bytesIn = asLong(root["bytes_in"], "vpn session response.bytes_in"),
            bytesOut = asLong(root["bytes_out"], "vpn session response.bytes_out"),
            status = requiredString(root["status"], "vpn session response.status"),
        )
    }

    @JvmStatic
    fun parseReceipt(payload: ByteArray): VpnReceipt {
        val root = expectObject(parse(payload, "vpn receipt response"), "vpn receipt response")
        return parseReceiptObject(root, "vpn receipt response")
    }

    @JvmStatic
    fun parseReceiptList(payload: ByteArray): VpnReceiptListResponse {
        val root = expectObject(parse(payload, "vpn receipt list response"), "vpn receipt list response")
        val items = requiredList(root["items"], "vpn receipt list response.items")
            .mapIndexed { index, item ->
                parseReceiptObject(expectObject(item, "vpn receipt list response.items[$index]"), "vpn receipt list response.items[$index]")
            }
        return VpnReceiptListResponse(
            items = items,
            total = asLong(root["total"], "vpn receipt list response.total"),
        )
    }

    private fun parseReceiptObject(root: Map<String, Any?>, path: String): VpnReceipt =
        VpnReceipt(
            sessionId = hex32(root["session_id"], "sessionId"),
            accountId = requiredString(root["account_id"], "$path.account_id"),
            exitClass = requiredString(root["exit_class"], "$path.exit_class"),
            relayEndpoint = requiredString(root["relay_endpoint"], "$path.relay_endpoint"),
            meterFamily = requiredString(root["meter_family"], "$path.meter_family"),
            connectedAtMs = asLong(root["connected_at_ms"], "$path.connected_at_ms"),
            disconnectedAtMs = asLong(root["disconnected_at_ms"], "$path.disconnected_at_ms"),
            durationMs = asLong(root["duration_ms"], "$path.duration_ms"),
            bytesIn = asLong(root["bytes_in"], "$path.bytes_in"),
            bytesOut = asLong(root["bytes_out"], "$path.bytes_out"),
            status = requiredString(root["status"], "$path.status"),
            receiptSource = requiredString(root["receipt_source"], "$path.receipt_source"),
            quoteId = hex32(root["quote_id"], "quoteId"),
            paymentTxHash = hex32(root["payment_tx_hash"], "paymentTxHash"),
            feeAssetId = requiredString(root["fee_asset_id"], "$path.fee_asset_id"),
            escrowAccountId = requiredString(root["escrow_account_id"], "$path.escrow_account_id"),
            operatorAccountId = requiredString(root["operator_account_id"], "$path.operator_account_id"),
            leaseFeeNanos = asLong(root["lease_fee_nanos"], "$path.lease_fee_nanos"),
            earnedFeeNanos = asLong(root["earned_fee_nanos"], "$path.earned_fee_nanos"),
            refundedFeeNanos = asLong(root["refunded_fee_nanos"], "$path.refunded_fee_nanos"),
            leaseIdHex = if (root.containsKey("lease_id_hex") && root["lease_id_hex"] != null) hex32(root["lease_id_hex"], "leaseIdHex") else "",
            settleLeaseInstruction = optionalTxInstruction(root["settle_lease_instruction"], "$path.settle_lease_instruction"),
            txInstructions = txInstructionList(root["tx_instructions"], "$path.tx_instructions"),
        )

    private fun parse(payload: ByteArray?, context: String): Any? {
        check(payload != null && payload.isNotEmpty()) { "$context returned an empty payload" }
        val json = String(payload, StandardCharsets.UTF_8).trim()
        check(json.isNotEmpty()) { "$context returned a blank payload" }
        return JsonParser.parse(json)
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any?> {
        check(value is Map<*, *>) { "$path must be a JSON object" }
        return value as Map<String, Any?>
    }

    @Suppress("UNCHECKED_CAST")
    private fun requiredList(value: Any?, path: String): List<Any?> {
        check(value is List<*>) { "$path must be an array" }
        return value as List<Any?>
    }

    private fun stringList(value: Any?, path: String): List<String> {
        if (value == null) return emptyList()
        return requiredList(value, path).mapIndexed { index, item ->
            requiredString(item, "$path[$index]")
        }
    }

    private fun txInstructionList(value: Any?, path: String): List<VpnTxInstruction> {
        if (value == null) return emptyList()
        return requiredList(value, path).mapIndexed { index, item ->
            parseTxInstruction(expectObject(item, "$path[$index]"), "$path[$index]")
        }
    }

    private fun optionalTxInstruction(value: Any?, path: String): VpnTxInstruction? {
        if (value == null) return null
        return parseTxInstruction(expectObject(value, path), path)
    }

    private fun parseTxInstruction(root: Map<String, Any?>, path: String): VpnTxInstruction =
        VpnTxInstruction(
            wireId = requiredString(root["wire_id"], "$path.wire_id"),
            payloadHex = evenHex(root["payload_hex"], "payloadHex"),
        )

    private fun requiredString(value: Any?, path: String): String {
        val string = optionalString(value)
        check(!string.isNullOrBlank()) { "$path must be a non-empty string" }
        return string.trim()
    }

    private fun optionalString(value: Any?): String? {
        if (value == null) return null
        val string = if (value is String) value else value.toString()
        return string.trim().ifEmpty { null }
    }

    private fun asLong(value: Any?, path: String): Long = JsonNumbers.asLong(value, path)
    private fun asInt(value: Any?, path: String): Int = JsonNumbers.asInt(value, path)

    private fun hex32(value: Any?, field: String): String =
        HttpClientTransport.normalizeHex32(requiredString(value, field), field)

    private fun optionalHex32(value: Any?, field: String): String? {
        val literal = optionalString(value) ?: return null
        return HttpClientTransport.normalizeHex32(literal, field)
    }

    private fun evenHex(value: Any?, field: String): String =
        HttpClientTransport.normalizeEvenLengthHex(requiredString(value, field), field)
}
