package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.crypto.Ed25519PublicKeyAdmission
import org.hyperledger.iroha.sdk.numeric.NumericV1Codec

import java.net.Inet6Address
import java.net.InetAddress
import java.nio.charset.StandardCharsets

/** Minimal JSON parser for Sora VPN Torii responses. */
object VpnJsonParser {

    private const val VPN_HELPER_TICKET_HEX_LENGTH = 1_576
    private const val RELAY_MLDSA65_PUBLIC_KEY_HEX_LENGTH = 3_904
    private const val U32_MAX = 4_294_967_295L
    private val EXIT_CLASSES = setOf("standard", "low-latency", "high-security")
    private val RECEIPT_STATUSES =
        setOf("disconnected", "expired", "replaced", "settlement_pending", "settled")
    private val RECEIPT_SOURCES = setOf("torii", "relay", "wsv")
    private val PROFILE_FIELDS = setOf(
        "available", "relay_endpoint", "supported_exit_classes", "default_exit_class",
        "lease_secs", "dns_push_interval_secs", "meter_family", "route_pushes",
        "excluded_routes", "dns_servers", "tunnel_addresses", "mtu_bytes",
        "display_billing_label", "operator_account_id",
        "lease_fee", "settlement_grace_secs", "flow_label_bits", "padding_budget_ms",
        "relay_id_hex", "relay_mldsa65_public_key_hex", "descriptor_commit_hex", "tls_server_name",
        "relay_tls_spki_sha256_hex", "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
    )
    private val QUOTE_FIELDS = setOf(
        "quote_id", "lease_id_hex", "session_id_hex", "payment_reference", "account_id",
        "exit_class", "relay_endpoint", "lease_secs", "quote_expires_at_ms", "fee_asset_id",
        "escrow_account_id", "operator_account_id", "lease_fee", "route_pushes",
        "excluded_routes", "dns_servers", "tunnel_addresses", "mtu_bytes", "meter_family",
        "flow_label_bits", "padding_budget_ms", "relay_id_hex", "relay_mldsa65_public_key_hex", "descriptor_commit_hex",
        "tls_server_name", "relay_tls_spki_sha256_hex", "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
        "metering_public_key_hex", "open_lease_instruction",
    )
    private val SESSION_FIELDS = setOf(
        "session_id", "account_id", "exit_class", "relay_endpoint", "lease_secs",
        "expires_at_ms", "connected_at_ms", "meter_family", "quote_id", "payment_reference",
        "payment_tx_hash", "fee_asset_id", "escrow_account_id", "operator_account_id",
        "lease_fee", "flow_label_bits", "padding_budget_ms", "relay_id_hex", "relay_mldsa65_public_key_hex",
        "descriptor_commit_hex", "tls_server_name", "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex", "directory_snapshot_digest_hex",
        "route_pushes", "excluded_routes", "dns_servers", "tunnel_addresses", "mtu_bytes",
        "helper_ticket_hex", "bytes_in", "bytes_out", "status",
    )
    private val RECEIPT_FIELDS = setOf(
        "session_id", "account_id", "exit_class", "relay_endpoint", "meter_family",
        "connected_at_ms", "disconnected_at_ms", "duration_ms", "bytes_in", "bytes_out",
        "status", "receipt_source", "quote_id", "payment_tx_hash", "fee_asset_id",
        "escrow_account_id", "operator_account_id", "lease_fee", "earned_fee", "refunded_fee",
        "lease_id_hex", "settle_lease_instruction",
    )
    private val RECEIPT_LIST_FIELDS = setOf("items", "total")
    private val TX_INSTRUCTION_FIELDS = setOf("wire_id", "payload_hex")

    @JvmStatic
    fun parseProfile(payload: ByteArray): VpnProfile {
        val root = expectObject(parse(payload, "vpn profile response"), "vpn profile response")
        requireExactFields(root, PROFILE_FIELDS, "vpn profile response")
        val available = requiredBoolean(root["available"], "vpn profile response.available")
        val trust = vpnTrustTuple(root, "vpn profile response", allowEmpty = !available)
        return VpnProfile(
            available = available,
            relayEndpoint = relayEndpoint(
                root["relay_endpoint"],
                "vpn profile response.relay_endpoint",
                allowEmpty = !available,
            ),
            supportedExitClasses = exitClassList(root["supported_exit_classes"], "vpn profile response.supported_exit_classes"),
            defaultExitClass = exitClass(root["default_exit_class"], "vpn profile response.default_exit_class"),
            leaseSecs = boundedLong(root["lease_secs"], "vpn profile response.lease_secs", 1, U32_MAX),
            dnsPushIntervalSecs = atLeastLong(
                root["dns_push_interval_secs"],
                "vpn profile response.dns_push_interval_secs",
                30,
            ),
            meterFamily = requiredString(root["meter_family"], "vpn profile response.meter_family"),
            routePushes = stringList(root["route_pushes"], "vpn profile response.route_pushes"),
            excludedRoutes = stringList(root["excluded_routes"], "vpn profile response.excluded_routes"),
            dnsServers = stringList(root["dns_servers"], "vpn profile response.dns_servers"),
            tunnelAddresses = stringList(root["tunnel_addresses"], "vpn profile response.tunnel_addresses"),
            mtuBytes = exactLong(root["mtu_bytes"], "vpn profile response.mtu_bytes", 1_280),
            displayBillingLabel = requiredString(root["display_billing_label"], "vpn profile response.display_billing_label"),
            operatorAccountId = requiredString(root["operator_account_id"], "vpn profile response.operator_account_id"),
            leaseFee = quantity(root["lease_fee"], "vpn profile response.lease_fee"),
            settlementGraceSecs = atLeastLong(root["settlement_grace_secs"], "vpn profile response.settlement_grace_secs", 1),
            flowLabelBits = exactInt(root["flow_label_bits"], "vpn profile response.flow_label_bits", 24),
            paddingBudgetMs = boundedInt(root["padding_budget_ms"], "vpn profile response.padding_budget_ms", 1, 65_535),
            relayIdHex = trust.relayIdHex,
            relayMldsa65PublicKeyHex = trust.relayMldsa65PublicKeyHex,
            descriptorCommitHex = trust.descriptorCommitHex,
            tlsServerName = trust.tlsServerName,
            relayTlsSpkiSha256Hex = trust.relayTlsSpkiSha256Hex,
            relayCertificateSha256Hex = trust.relayCertificateSha256Hex,
            directorySnapshotDigestHex = trust.directorySnapshotDigestHex,
        )
    }

    @JvmStatic
    fun parseQuote(payload: ByteArray): VpnQuote {
        val root = expectObject(parse(payload, "vpn quote response"), "vpn quote response")
        requireExactFields(root, QUOTE_FIELDS, "vpn quote response")
        val trust = vpnTrustTuple(root, "vpn quote response", allowEmpty = false)
        return VpnQuote(
            quoteId = hex32(root["quote_id"], "quoteId"),
            leaseIdHex = hex32(root["lease_id_hex"], "leaseIdHex"),
            sessionIdHex = hex16(root["session_id_hex"], "sessionIdHex"),
            paymentReference = requiredString(root["payment_reference"], "vpn quote response.payment_reference"),
            accountId = requiredString(root["account_id"], "vpn quote response.account_id"),
            exitClass = exitClass(root["exit_class"], "vpn quote response.exit_class"),
            relayEndpoint = relayEndpoint(root["relay_endpoint"], "vpn quote response.relay_endpoint"),
            leaseSecs = boundedLong(root["lease_secs"], "vpn quote response.lease_secs", 1, U32_MAX),
            quoteExpiresAtMs = atLeastLong(root["quote_expires_at_ms"], "vpn quote response.quote_expires_at_ms", 0),
            feeAssetId = requiredString(root["fee_asset_id"], "vpn quote response.fee_asset_id"),
            escrowAccountId = requiredString(root["escrow_account_id"], "vpn quote response.escrow_account_id"),
            operatorAccountId = requiredString(root["operator_account_id"], "vpn quote response.operator_account_id"),
            leaseFee = quantity(root["lease_fee"], "vpn quote response.lease_fee"),
            routePushes = stringList(root["route_pushes"], "vpn quote response.route_pushes"),
            excludedRoutes = stringList(root["excluded_routes"], "vpn quote response.excluded_routes"),
            dnsServers = stringList(root["dns_servers"], "vpn quote response.dns_servers"),
            tunnelAddresses = stringList(root["tunnel_addresses"], "vpn quote response.tunnel_addresses"),
            mtuBytes = exactLong(root["mtu_bytes"], "vpn quote response.mtu_bytes", 1_280),
            meterFamily = requiredString(root["meter_family"], "vpn quote response.meter_family"),
            flowLabelBits = exactInt(root["flow_label_bits"], "vpn quote response.flow_label_bits", 24),
            paddingBudgetMs = boundedInt(root["padding_budget_ms"], "vpn quote response.padding_budget_ms", 1, 65_535),
            relayIdHex = trust.relayIdHex,
            relayMldsa65PublicKeyHex = trust.relayMldsa65PublicKeyHex,
            descriptorCommitHex = trust.descriptorCommitHex,
            tlsServerName = trust.tlsServerName,
            relayTlsSpkiSha256Hex = trust.relayTlsSpkiSha256Hex,
            relayCertificateSha256Hex = trust.relayCertificateSha256Hex,
            directorySnapshotDigestHex = trust.directorySnapshotDigestHex,
            meteringPublicKeyHex =
                ed25519PublicKeyHex(root["metering_public_key_hex"], "meteringPublicKeyHex"),
            openLeaseInstruction = parseTxInstruction(
                expectObject(root["open_lease_instruction"], "vpn quote response.open_lease_instruction"),
                "vpn quote response.open_lease_instruction",
            ),
        )
    }

    @JvmStatic
    fun parseSession(payload: ByteArray): VpnSession {
        val root = expectObject(parse(payload, "vpn session response"), "vpn session response")
        requireExactFields(root, SESSION_FIELDS, "vpn session response")
        val trust = vpnTrustTuple(root, "vpn session response", allowEmpty = false)
        return VpnSession(
            sessionId = hex16(root["session_id"], "sessionId"),
            accountId = requiredString(root["account_id"], "vpn session response.account_id"),
            exitClass = exitClass(root["exit_class"], "vpn session response.exit_class"),
            relayEndpoint = relayEndpoint(root["relay_endpoint"], "vpn session response.relay_endpoint"),
            leaseSecs = boundedLong(root["lease_secs"], "vpn session response.lease_secs", 1, U32_MAX),
            expiresAtMs = atLeastLong(root["expires_at_ms"], "vpn session response.expires_at_ms", 0),
            connectedAtMs = atLeastLong(root["connected_at_ms"], "vpn session response.connected_at_ms", 0),
            meterFamily = requiredString(root["meter_family"], "vpn session response.meter_family"),
            quoteId = hex32(root["quote_id"], "quoteId"),
            paymentReference = requiredString(root["payment_reference"], "vpn session response.payment_reference"),
            paymentTxHash = hex32(root["payment_tx_hash"], "paymentTxHash"),
            feeAssetId = requiredString(root["fee_asset_id"], "vpn session response.fee_asset_id"),
            escrowAccountId = requiredString(root["escrow_account_id"], "vpn session response.escrow_account_id"),
            operatorAccountId = requiredString(root["operator_account_id"], "vpn session response.operator_account_id"),
            leaseFee = quantity(root["lease_fee"], "vpn session response.lease_fee"),
            flowLabelBits = exactInt(root["flow_label_bits"], "vpn session response.flow_label_bits", 24),
            paddingBudgetMs = boundedInt(root["padding_budget_ms"], "vpn session response.padding_budget_ms", 1, 65_535),
            relayIdHex = trust.relayIdHex,
            relayMldsa65PublicKeyHex = trust.relayMldsa65PublicKeyHex,
            descriptorCommitHex = trust.descriptorCommitHex,
            tlsServerName = trust.tlsServerName,
            relayTlsSpkiSha256Hex = trust.relayTlsSpkiSha256Hex,
            relayCertificateSha256Hex = trust.relayCertificateSha256Hex,
            directorySnapshotDigestHex = trust.directorySnapshotDigestHex,
            routePushes = stringList(root["route_pushes"], "vpn session response.route_pushes"),
            excludedRoutes = stringList(root["excluded_routes"], "vpn session response.excluded_routes"),
            dnsServers = stringList(root["dns_servers"], "vpn session response.dns_servers"),
            tunnelAddresses = stringList(root["tunnel_addresses"], "vpn session response.tunnel_addresses"),
            mtuBytes = exactLong(root["mtu_bytes"], "vpn session response.mtu_bytes", 1_280),
            helperTicketHex = helperTicketHex(root["helper_ticket_hex"], "helperTicketHex"),
            bytesIn = atLeastLong(root["bytes_in"], "vpn session response.bytes_in", 0),
            bytesOut = atLeastLong(root["bytes_out"], "vpn session response.bytes_out", 0),
            status = exactString(root["status"], "vpn session response.status", setOf("active")),
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
        requireExactFields(root, RECEIPT_LIST_FIELDS, "vpn receipt list response")
        val items = requiredList(root["items"], "vpn receipt list response.items")
            .mapIndexed { index, item ->
                parseReceiptObject(expectObject(item, "vpn receipt list response.items[$index]"), "vpn receipt list response.items[$index]")
            }
        check(items.size <= 24) { "vpn receipt list response.items must contain at most 24 entries" }
        return VpnReceiptListResponse(
            items = items,
            total = boundedLong(root["total"], "vpn receipt list response.total", 0, 24),
        )
    }

    private fun parseReceiptObject(root: Map<String, Any?>, path: String): VpnReceipt {
        requireExactFields(root, RECEIPT_FIELDS, path)
        return VpnReceipt(
            sessionId = hex16(root["session_id"], "sessionId"),
            accountId = requiredString(root["account_id"], "$path.account_id"),
            exitClass = exitClass(root["exit_class"], "$path.exit_class"),
            relayEndpoint = requiredString(root["relay_endpoint"], "$path.relay_endpoint"),
            meterFamily = requiredString(root["meter_family"], "$path.meter_family"),
            connectedAtMs = atLeastLong(root["connected_at_ms"], "$path.connected_at_ms", 0),
            disconnectedAtMs = atLeastLong(root["disconnected_at_ms"], "$path.disconnected_at_ms", 0),
            durationMs = atLeastLong(root["duration_ms"], "$path.duration_ms", 0),
            bytesIn = atLeastLong(root["bytes_in"], "$path.bytes_in", 0),
            bytesOut = atLeastLong(root["bytes_out"], "$path.bytes_out", 0),
            status = exactString(root["status"], "$path.status", RECEIPT_STATUSES),
            receiptSource = exactString(root["receipt_source"], "$path.receipt_source", RECEIPT_SOURCES),
            quoteId = hex32(root["quote_id"], "quoteId"),
            paymentTxHash = hex32(root["payment_tx_hash"], "paymentTxHash"),
            feeAssetId = requiredString(root["fee_asset_id"], "$path.fee_asset_id"),
            escrowAccountId = requiredString(root["escrow_account_id"], "$path.escrow_account_id"),
            operatorAccountId = requiredString(root["operator_account_id"], "$path.operator_account_id"),
            leaseFee = quantity(root["lease_fee"], "$path.lease_fee"),
            earnedFee = quantity(root["earned_fee"], "$path.earned_fee"),
            refundedFee = quantity(root["refunded_fee"], "$path.refunded_fee"),
            leaseIdHex = hex32(root["lease_id_hex"], "leaseIdHex"),
            settleLeaseInstruction = optionalTxInstruction(root["settle_lease_instruction"], "$path.settle_lease_instruction"),
        )
    }

    private fun parse(payload: ByteArray?, context: String): Any? {
        check(payload != null && payload.isNotEmpty()) { "$context returned an empty payload" }
        val json = String(payload, StandardCharsets.UTF_8).trim()
        check(json.isNotEmpty()) { "$context returned a blank payload" }
        return JsonParser.parse(json)
    }

    private fun quantity(value: Any?, path: String): String =
        try {
            NumericV1Codec.decodeQuantityJsonValue(value).toString()
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("$path must be a canonical non-negative quantity string", error)
        }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any?> {
        check(value is Map<*, *>) { "$path must be a JSON object" }
        return value as Map<String, Any?>
    }

    private fun requiredList(value: Any?, path: String): List<Any?> {
        check(value is List<*>) { "$path must be an array" }
        return value
    }

    private fun stringList(value: Any?, path: String): List<String> {
        return requiredList(value, path).mapIndexed { index, item ->
            requiredString(item, "$path[$index]")
        }
    }

    private fun optionalTxInstruction(value: Any?, path: String): VpnTxInstruction? {
        if (value == null) return null
        return parseTxInstruction(expectObject(value, path), path)
    }

    private fun parseTxInstruction(root: Map<String, Any?>, path: String): VpnTxInstruction {
        requireExactFields(root, TX_INSTRUCTION_FIELDS, path)
        return VpnTxInstruction(
            wireId = requiredString(root["wire_id"], "$path.wire_id"),
            payloadHex = canonicalEvenHex(root["payload_hex"], "payloadHex"),
        )
    }

    private fun requiredString(value: Any?, path: String): String {
        check(value is String && value.isNotEmpty() && value.trim() == value) {
            "$path must be a non-empty string without surrounding whitespace"
        }
        return value
    }

    private fun requiredBoolean(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun exitClass(value: Any?, path: String): String =
        exactString(value, path, EXIT_CLASSES)

    private fun exitClassList(value: Any?, path: String): List<String> {
        val parsed = requiredList(value, path).mapIndexed { index, item ->
            exitClass(item, "$path[$index]")
        }
        check(parsed.size == 3 && parsed.toSet().size == 3) {
            "$path must contain each of the three supported exit classes exactly once"
        }
        return parsed
    }

    private fun exactString(value: Any?, path: String, allowed: Set<String>): String {
        val parsed = requiredString(value, path)
        check(parsed in allowed) { "$path must be one of ${allowed.joinToString()}" }
        return parsed
    }

    private fun asLong(value: Any?, path: String): Long = JsonNumbers.asLong(value, path)
    private fun asInt(value: Any?, path: String): Int = JsonNumbers.asInt(value, path)

    private fun atLeastLong(value: Any?, path: String, minimum: Long): Long {
        val parsed = asLong(value, path)
        check(parsed >= minimum) { "$path must be at least $minimum" }
        return parsed
    }

    private fun boundedLong(value: Any?, path: String, minimum: Long, maximum: Long): Long {
        val parsed = asLong(value, path)
        check(parsed in minimum..maximum) { "$path must be between $minimum and $maximum" }
        return parsed
    }

    private fun exactLong(value: Any?, path: String, expected: Long): Long {
        val parsed = asLong(value, path)
        check(parsed == expected) { "$path must equal $expected" }
        return parsed
    }

    private fun boundedInt(value: Any?, path: String, minimum: Int, maximum: Int): Int {
        val parsed = asInt(value, path)
        check(parsed in minimum..maximum) { "$path must be between $minimum and $maximum" }
        return parsed
    }

    private fun exactInt(value: Any?, path: String, expected: Int): Int {
        val parsed = asInt(value, path)
        check(parsed == expected) { "$path must equal $expected" }
        return parsed
    }

    private fun helperTicketHex(value: Any?, field: String): String {
        check(value is String &&
            value.length == VPN_HELPER_TICKET_HEX_LENGTH &&
            value.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be exactly $VPN_HELPER_TICKET_HEX_LENGTH lowercase hexadecimal characters"
        }
        return value
    }

    private fun requireExactFields(root: Map<String, Any?>, allowed: Set<String>, path: String) {
        val unknown = root.keys.firstOrNull { it !in allowed }
        check(unknown == null) { "$path contains unknown field `$unknown`" }
        val missing = allowed.firstOrNull { !root.containsKey(it) }
        check(missing == null) { "$path is missing required field `$missing`" }
    }

    private fun canonicalHex(value: Any?, field: String, length: Int): String {
        check(value is String &&
            value.length == length &&
            value.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be exactly $length lowercase hexadecimal characters"
        }
        return value
    }

    private fun hex32(value: Any?, field: String): String = canonicalHex(value, field, 64)

    private class VpnTrustTuple(
        val relayIdHex: String,
        val relayMldsa65PublicKeyHex: String,
        val descriptorCommitHex: String,
        val tlsServerName: String,
        val relayTlsSpkiSha256Hex: String,
        val relayCertificateSha256Hex: String,
        val directorySnapshotDigestHex: String,
    )

    private fun vpnTrustTuple(
        root: Map<String, Any?>,
        context: String,
        allowEmpty: Boolean,
    ): VpnTrustTuple = VpnTrustTuple(
        relayIdHex = trustRelayId(root["relay_id_hex"], "$context.relay_id_hex", allowEmpty),
        relayMldsa65PublicKeyHex = trustMldsa65PublicKey(
            root["relay_mldsa65_public_key_hex"],
            "$context.relay_mldsa65_public_key_hex",
            allowEmpty,
        ),
        descriptorCommitHex = trustDigest(
            root["descriptor_commit_hex"],
            "$context.descriptor_commit_hex",
            allowEmpty,
        ),
        tlsServerName = tlsServerName(
            root["tls_server_name"],
            "$context.tls_server_name",
            allowEmpty,
        ),
        relayTlsSpkiSha256Hex = trustDigest(
            root["relay_tls_spki_sha256_hex"],
            "$context.relay_tls_spki_sha256_hex",
            allowEmpty,
        ),
        relayCertificateSha256Hex = trustDigest(
            root["relay_certificate_sha256_hex"],
            "$context.relay_certificate_sha256_hex",
            allowEmpty,
        ),
        directorySnapshotDigestHex = trustDigest(
            root["directory_snapshot_digest_hex"],
            "$context.directory_snapshot_digest_hex",
            allowEmpty,
        ),
    )

    private fun trustRelayId(value: Any?, field: String, allowEmpty: Boolean): String {
        if (allowEmpty && value == "") return ""
        return ed25519PublicKeyHex(value, field)
    }

    private fun trustMldsa65PublicKey(value: Any?, field: String, allowEmpty: Boolean): String {
        if (allowEmpty && value == "") return ""
        val canonical = canonicalHex(value, field, RELAY_MLDSA65_PUBLIC_KEY_HEX_LENGTH)
        check(canonical.any { it != '0' }) { "$field must not be the all-zero ML-DSA-65 key" }
        return canonical
    }

    private fun trustDigest(value: Any?, field: String, allowEmpty: Boolean): String {
        if (allowEmpty && value == "") return ""
        val canonical = hex32(value, field)
        check(canonical.any { it != '0' }) { "$field must not be the all-zero digest" }
        return canonical
    }

    private fun tlsServerName(value: Any?, field: String, allowEmpty: Boolean): String {
        if (allowEmpty && value == "") return ""
        val name = requiredString(value, field)
        check(name.length <= 253 && name == name.lowercase()) {
            "$field must be a canonical lowercase DNS name"
        }
        val labels = name.split('.')
        check(labels.all { label ->
            label.isNotEmpty() &&
                label.length <= 63 &&
                (label.first() in 'a'..'z' || label.first() in '0'..'9') &&
                (label.last() in 'a'..'z' || label.last() in '0'..'9') &&
                label.all { it in 'a'..'z' || it in '0'..'9' || it == '-' }
        }) { "$field must be a canonical lowercase DNS name" }
        return name
    }

    private fun relayEndpoint(value: Any?, field: String, allowEmpty: Boolean = false): String {
        if (allowEmpty && value == "") return ""
        val endpoint = requiredString(value, field)
        val parts = endpoint.split('/')
        check(parts.size == 6 && parts[0].isEmpty() && parts[3] == "udp" && parts[5] == "quic") {
            "$field must use /{ip4|ip6|dns|dns4|dns6}/host/udp/port/quic"
        }
        val protocol = parts[1]
        val host = parts[2]
        check(protocol in setOf("ip4", "ip6", "dns", "dns4", "dns6")) {
            "$field has an unsupported host protocol"
        }
        when (protocol) {
            "ip4" -> requireCanonicalIpv4(host, field)
            "ip6" -> requireCanonicalIpv6(host, field)
            else -> tlsServerName(host, "$field host", allowEmpty = false)
        }
        val port = parts[4].toIntOrNull()
        check(port != null && port in 1..65_535 && port.toString() == parts[4]) {
            "$field must contain a canonical non-zero UDP port"
        }
        return endpoint
    }

    private fun requireCanonicalIpv4(host: String, field: String) {
        val octets = host.split('.')
        check(octets.size == 4 && octets.all { octet ->
            val parsed = octet.toIntOrNull()
            parsed != null && parsed in 0..255 && parsed.toString() == octet
        }) { "$field must contain a canonical IPv4 address" }
    }

    private fun requireCanonicalIpv6(host: String, field: String) {
        val parsed = runCatching { InetAddress.getByName(host) }.getOrNull()
        check(parsed is Inet6Address && host == canonicalIpv6(parsed.address)) {
            "$field must contain a canonical lowercase IPv6 address"
        }
    }

    private fun canonicalIpv6(bytes: ByteArray): String {
        val groups = IntArray(8) { index ->
            ((bytes[index * 2].toInt() and 0xff) shl 8) or
                (bytes[index * 2 + 1].toInt() and 0xff)
        }
        var bestStart = -1
        var bestLength = 0
        var index = 0
        while (index < groups.size) {
            if (groups[index] != 0) {
                index += 1
                continue
            }
            val start = index
            while (index < groups.size && groups[index] == 0) index += 1
            val length = index - start
            if (length >= 2 && length > bestLength) {
                bestStart = start
                bestLength = length
            }
        }
        if (bestStart < 0) return groups.joinToString(":") { it.toString(16) }
        val before = groups.take(bestStart).joinToString(":") { it.toString(16) }
        val after = groups.drop(bestStart + bestLength).joinToString(":") { it.toString(16) }
        return when {
            before.isEmpty() && after.isEmpty() -> "::"
            before.isEmpty() -> "::$after"
            after.isEmpty() -> "$before::"
            else -> "$before::$after"
        }
    }

    private fun ed25519PublicKeyHex(value: Any?, field: String): String {
        val canonical = hex32(value, field)
        val publicKey = ByteArray(Ed25519PublicKeyAdmission.PUBLIC_KEY_LENGTH) { index ->
            val offset = index * 2
            ((Character.digit(canonical[offset], 16) shl 4) or
                Character.digit(canonical[offset + 1], 16)).toByte()
        }
        check(Ed25519PublicKeyAdmission.isValid(publicKey)) {
            "$field must encode a canonical prime-order Ed25519 public key"
        }
        return canonical
    }

    private fun hex16(value: Any?, field: String): String = canonicalHex(value, field, 32)

    private fun canonicalEvenHex(value: Any?, field: String): String {
        check(value is String &&
            value.isNotEmpty() &&
            value.length % 2 == 0 &&
            value.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be non-empty even-length lowercase hexadecimal"
        }
        return value
    }
}
