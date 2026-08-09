using System.Net;
using System.Net.Sockets;
using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiVpnJson
{
    private static readonly BigInteger Ed25519FieldModulus = (BigInteger.One << 255) - 19;
    private static readonly BigInteger Ed25519SubgroupOrder =
        (BigInteger.One << 252) + BigInteger.Parse("27742317777372353535851937790883648493");
    private static readonly BigInteger Ed25519D = ModEd25519(
        -121665 * BigInteger.ModPow(121666, Ed25519FieldModulus - 2, Ed25519FieldModulus));
    private static readonly BigInteger Ed25519SqrtMinusOne = BigInteger.ModPow(
        2,
        (Ed25519FieldModulus - 1) / 4,
        Ed25519FieldModulus);

    private readonly record struct Ed25519ExtendedPoint(
        BigInteger X,
        BigInteger Y,
        BigInteger Z,
        BigInteger T);

    internal static void ValidateVpnTxInstruction(ToriiVpnTxInstruction instruction, string context)
    {
        ArgumentNullException.ThrowIfNull(instruction);

        RequireExactNonEmptyText(instruction.WireId, $"{context}.wire_id");
        RequireExactEvenLengthHex(instruction.PayloadHex, $"{context}.payload_hex");
    }

    internal static void ValidateVpnProfile(ToriiVpnProfile response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        RequireVpnRelayEndpoint(
            response.RelayEndpoint,
            $"{context}.relay_endpoint",
            allowEmpty: !response.Available);
        ValidateVpnExitClassList(response.SupportedExitClasses, $"{context}.supported_exit_classes");
        RequireVpnExitClass(response.DefaultExitClass, $"{context}.default_exit_class");
        RequireUInt64Range(response.LeaseSeconds, 1, uint.MaxValue, $"{context}.lease_secs");
        RequireMinimumUInt64(response.DnsPushIntervalSeconds, 30, $"{context}.dns_push_interval_secs");
        ToriiSseEventJson.RequireExactTokenText(response.MeterFamily, $"{context}.meter_family");
        ValidateStringList(response.RoutePushes, $"{context}.route_pushes", token: true);
        ValidateStringList(response.ExcludedRoutes, $"{context}.excluded_routes", token: true);
        ValidateStringList(response.DnsServers, $"{context}.dns_servers", token: true);
        ValidateStringList(response.TunnelAddresses, $"{context}.tunnel_addresses", token: true);
        RequireExactUInt64(response.MtuBytes, 1280, $"{context}.mtu_bytes");
        RequireExactNonEmptyText(response.DisplayBillingLabel, $"{context}.display_billing_label");
        RequireCanonicalAccountId(response.OperatorAccountId, $"{context}.operator_account_id");
        _ = ToriiQuantityJson.RequireCanonicalQuantity(response.LeaseFee, $"{context}.lease_fee");
        RequirePositiveUInt64(response.SettlementGraceSeconds, $"{context}.settlement_grace_secs");
        RequireExactUInt64(response.FlowLabelBits, 24, $"{context}.flow_label_bits");
        RequireUInt64Range(response.PaddingBudgetMilliseconds, 1, ushort.MaxValue, $"{context}.padding_budget_ms");
        ValidateVpnTrust(
            response.RelayIdHex,
            response.DescriptorCommitHex,
            response.TlsServerName,
            response.RelayTlsSpkiSha256Hex,
            response.RelayCertificateSha256Hex,
            response.DirectorySnapshotDigestHex,
            context,
            allowEmpty: !response.Available);
    }

    internal static void ValidateVpnQuote(ToriiVpnQuote response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactSizedHex(response.QuoteId, $"{context}.quote_id", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.LeaseIdHex, $"{context}.lease_id_hex", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.SessionIdHex, $"{context}.session_id_hex", 16);
        ToriiSseEventJson.RequireExactTokenText(response.PaymentReference, $"{context}.payment_reference");
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        RequireVpnExitClass(response.ExitClass, $"{context}.exit_class");
        RequireVpnRelayEndpoint(response.RelayEndpoint, $"{context}.relay_endpoint");
        RequireUInt64Range(response.LeaseSeconds, 1, uint.MaxValue, $"{context}.lease_secs");
        ToriiSseEventJson.RequireExactTokenText(response.FeeAssetId, $"{context}.fee_asset_id");
        RequireCanonicalAccountId(response.EscrowAccountId, $"{context}.escrow_account_id");
        RequireCanonicalAccountId(response.OperatorAccountId, $"{context}.operator_account_id");
        _ = ToriiQuantityJson.RequireCanonicalQuantity(response.LeaseFee, $"{context}.lease_fee");
        ValidateStringList(response.RoutePushes, $"{context}.route_pushes", token: true);
        ValidateStringList(response.ExcludedRoutes, $"{context}.excluded_routes", token: true);
        ValidateStringList(response.DnsServers, $"{context}.dns_servers", token: true);
        ValidateStringList(response.TunnelAddresses, $"{context}.tunnel_addresses", token: true);
        RequireExactUInt64(response.MtuBytes, 1280, $"{context}.mtu_bytes");
        ToriiSseEventJson.RequireExactTokenText(response.MeterFamily, $"{context}.meter_family");
        RequireExactUInt64(response.FlowLabelBits, 24, $"{context}.flow_label_bits");
        RequireUInt64Range(response.PaddingBudgetMilliseconds, 1, ushort.MaxValue, $"{context}.padding_budget_ms");
        ValidateVpnTrust(
            response.RelayIdHex,
            response.DescriptorCommitHex,
            response.TlsServerName,
            response.RelayTlsSpkiSha256Hex,
            response.RelayCertificateSha256Hex,
            response.DirectorySnapshotDigestHex,
            context);
        ToriiSseEventJson.RequireExactSizedHex(response.MeteringPublicKeyHex, $"{context}.metering_public_key_hex", 32);
        ValidateVpnTxInstruction(response.OpenLeaseInstruction, $"{context}.open_lease_instruction");
    }

    internal static void ValidateVpnSession(ToriiVpnSession response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactSizedHex(response.SessionId, $"{context}.session_id", 32);
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        RequireVpnExitClass(response.ExitClass, $"{context}.exit_class");
        RequireVpnRelayEndpoint(response.RelayEndpoint, $"{context}.relay_endpoint");
        RequireUInt64Range(response.LeaseSeconds, 1, uint.MaxValue, $"{context}.lease_secs");
        if (response.ExpiresAtMilliseconds <= response.ConnectedAtMilliseconds)
        {
            throw new JsonException($"{context}.expires_at_ms must be greater than connected_at_ms.");
        }

        ToriiSseEventJson.RequireExactTokenText(response.MeterFamily, $"{context}.meter_family");
        ToriiSseEventJson.RequireExactSizedHex(response.QuoteId, $"{context}.quote_id", 32);
        ToriiSseEventJson.RequireExactTokenText(response.PaymentReference, $"{context}.payment_reference");
        ToriiSseEventJson.RequireExactSizedHex(response.PaymentTransactionHash, $"{context}.payment_tx_hash", 32);
        ToriiSseEventJson.RequireExactTokenText(response.FeeAssetId, $"{context}.fee_asset_id");
        RequireCanonicalAccountId(response.EscrowAccountId, $"{context}.escrow_account_id");
        RequireCanonicalAccountId(response.OperatorAccountId, $"{context}.operator_account_id");
        _ = ToriiQuantityJson.RequireCanonicalQuantity(response.LeaseFee, $"{context}.lease_fee");
        RequireExactUInt64(response.FlowLabelBits, 24, $"{context}.flow_label_bits");
        RequireUInt64Range(response.PaddingBudgetMilliseconds, 1, ushort.MaxValue, $"{context}.padding_budget_ms");
        ValidateVpnTrust(
            response.RelayIdHex,
            response.DescriptorCommitHex,
            response.TlsServerName,
            response.RelayTlsSpkiSha256Hex,
            response.RelayCertificateSha256Hex,
            response.DirectorySnapshotDigestHex,
            context);
        ValidateStringList(response.RoutePushes, $"{context}.route_pushes", token: true);
        ValidateStringList(response.ExcludedRoutes, $"{context}.excluded_routes", token: true);
        ValidateStringList(response.DnsServers, $"{context}.dns_servers", token: true);
        ValidateStringList(response.TunnelAddresses, $"{context}.tunnel_addresses", token: true);
        RequireExactUInt64(response.MtuBytes, 1280, $"{context}.mtu_bytes");
        ToriiSseEventJson.RequireExactSizedHex(
            response.HelperTicketHex,
            $"{context}.helper_ticket_hex",
            ToriiVpnDirectMetadata.HelperTicketByteLength);
        RequireVpnSessionStatus(response.Status, $"{context}.status");
    }

    internal static void ValidateVpnReceipt(ToriiVpnReceipt response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactSizedHex(response.SessionId, $"{context}.session_id", 32);
        RequireCanonicalAccountId(response.AccountId, $"{context}.account_id");
        RequireVpnExitClass(response.ExitClass, $"{context}.exit_class");
        ToriiSseEventJson.RequireExactTokenText(response.RelayEndpoint, $"{context}.relay_endpoint");
        ToriiSseEventJson.RequireExactTokenText(response.MeterFamily, $"{context}.meter_family");
        if (response.DisconnectedAtMilliseconds < response.ConnectedAtMilliseconds)
        {
            throw new JsonException($"{context}.disconnected_at_ms must be greater than or equal to connected_at_ms.");
        }

        RequireVpnReceiptStatus(response.Status, $"{context}.status");
        RequireVpnReceiptSource(response.ReceiptSource, $"{context}.receipt_source");
        ToriiSseEventJson.RequireExactSizedHex(response.QuoteId, $"{context}.quote_id", 32);
        ToriiSseEventJson.RequireExactSizedHex(response.PaymentTransactionHash, $"{context}.payment_tx_hash", 32);
        ToriiSseEventJson.RequireExactTokenText(response.FeeAssetId, $"{context}.fee_asset_id");
        RequireCanonicalAccountId(response.EscrowAccountId, $"{context}.escrow_account_id");
        RequireCanonicalAccountId(response.OperatorAccountId, $"{context}.operator_account_id");
        _ = ToriiQuantityJson.RequireCanonicalQuantity(response.LeaseFee, $"{context}.lease_fee");
        _ = ToriiQuantityJson.RequireCanonicalQuantity(response.EarnedFee, $"{context}.earned_fee");
        _ = ToriiQuantityJson.RequireCanonicalQuantity(response.RefundedFee, $"{context}.refunded_fee");
        ToriiSseEventJson.RequireExactSizedHex(response.LeaseIdHex, $"{context}.lease_id_hex", 32);

        ValidateOptionalVpnTxInstruction(response.SettleLeaseInstruction, $"{context}.settle_lease_instruction");
    }

    internal static void ValidateVpnReceiptListResponse(ToriiVpnReceiptListResponse response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        if (response.Items is null)
        {
            throw new JsonException($"{context}.items is required.");
        }

        for (var index = 0; index < response.Items.Count; index++)
        {
            ValidateVpnReceipt(response.Items[index], $"{context}.items[{index}]");
        }

        if ((ulong)response.Items.Count > response.Total)
        {
            throw new JsonException($"{context}.items item count must be less than or equal to total.");
        }

        if (response.Items.Count > 24)
        {
            throw new JsonException($"{context}.items must contain at most 24 items.");
        }

        if (response.Total > 24)
        {
            throw new JsonException($"{context}.total must be at most 24.");
        }
    }

    internal static ToriiVpnTxInstruction ReadVpnTxInstruction(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? wireId = null;
        string? payloadHex = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var instruction = CreateWithDirectMetadataContext(() => new ToriiVpnTxInstruction
                {
                    WireId = RequireString(wireId, $"{context}.wire_id"),
                    PayloadHex = RequireString(payloadHex, $"{context}.payload_hex"),
                }, context);
                ValidateVpnTxInstruction(instruction, context);
                RequireRequiredProperties(seen, context, ["wire_id", "payload_hex"]);
                return instruction;
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "wire_id":
                    wireId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.wire_id");
                    break;
                case "payload_hex":
                    payloadHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.payload_hex");
                    break;
                default:
                    throw new JsonException($"{context}.{propertyName} is not allowed.");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ToriiVpnTxInstruction? ReadOptionalVpnTxInstruction(ref Utf8JsonReader reader, string context)
    {
        return reader.TokenType == JsonTokenType.Null ? null : ReadVpnTxInstruction(ref reader, context);
    }

    internal static IReadOnlyList<string>? ReadStringList(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var items = new List<string>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return items;
            }

            if (reader.TokenType == JsonTokenType.Null)
            {
                throw new JsonException($"{context}[{index}] must not be null.");
            }
            else if (reader.TokenType == JsonTokenType.String)
            {
                items.Add(RequireString(reader.GetString(), $"{context}[{index}]"));
            }
            else
            {
                throw new JsonException($"{context}[{index}] must be a string.");
            }

            index++;
        }

        throw new JsonException($"{context} JSON array is incomplete.");
    }

    internal static bool ReadBool(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.True && reader.TokenType != JsonTokenType.False)
        {
            throw new JsonException($"{field} must be a boolean.");
        }

        return reader.GetBoolean();
    }

    internal static bool RequireBool(bool? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    internal static ushort ReadUInt16(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt16(out var value))
        {
            throw new JsonException($"{field} must be an unsigned 16-bit integer.");
        }

        return value;
    }

    internal static byte RequireByte(byte? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    internal static ushort RequireUInt16(ushort? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    internal static string RequireString(string? value, string field)
    {
        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        return value;
    }

    internal static IReadOnlyList<T> RequireList<T>(IReadOnlyList<T>? values, string context, string propertyName)
    {
        if (values is null)
        {
            throw new JsonException($"{context}.{propertyName} is required.");
        }

        return values;
    }

    private static void ValidateStringList(IReadOnlyList<string>? values, string context, bool token)
    {
        if (values is null)
        {
            throw new JsonException($"{context} is required.");
        }

        for (var index = 0; index < values.Count; index++)
        {
            if (token)
            {
                ToriiSseEventJson.RequireExactTokenText(values[index], $"{context}[{index}]");
            }
            else
            {
                RequireExactNonEmptyText(values[index], $"{context}[{index}]");
            }
        }
    }

    private static void ValidateVpnExitClassList(IReadOnlyList<string>? values, string context)
    {
        if (values is null)
        {
            throw new JsonException($"{context} is required.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        for (var index = 0; index < values.Count; index++)
        {
            var field = $"{context}[{index}]";
            RequireVpnExitClass(values[index], field);
            if (!seen.Add(values[index]))
            {
                throw new JsonException($"{field} must be unique within {context}.");
            }
        }

        if (values.Count != 3)
        {
            throw new JsonException($"{context} must contain exactly 3 items.");
        }
    }

    private static void ValidateOptionalVpnTxInstruction(ToriiVpnTxInstruction? instruction, string context)
    {
        if (instruction is not null)
        {
            ValidateVpnTxInstruction(instruction, context);
        }
    }

    private static void RequirePositiveUInt64(ulong value, string field)
    {
        if (value == 0)
        {
            throw new JsonException($"{field} must be positive.");
        }
    }

    private static void RequireMinimumUInt64(ulong value, ulong minimum, string field)
    {
        if (value < minimum)
        {
            throw new JsonException($"{field} must be at least {minimum}.");
        }
    }

    private static void RequireUInt64Range(ulong value, ulong minimum, ulong maximum, string field)
    {
        if (value < minimum || value > maximum)
        {
            throw new JsonException($"{field} must be between {minimum} and {maximum}.");
        }
    }

    private static void RequireExactUInt64(ulong value, ulong expected, string field)
    {
        if (value != expected)
        {
            throw new JsonException($"{field} must equal {expected}.");
        }
    }

    private static void ValidateVpnTrust(
        string relayIdHex,
        string descriptorCommitHex,
        string tlsServerName,
        string relayTlsSpkiSha256Hex,
        string relayCertificateSha256Hex,
        string directorySnapshotDigestHex,
        string context,
        bool allowEmpty = false)
    {
        RequireVpnRelayId(relayIdHex, $"{context}.relay_id_hex", allowEmpty);
        RequireVpnTrustDigest(
            descriptorCommitHex,
            $"{context}.descriptor_commit_hex",
            allowEmpty);
        RequireVpnTlsServerName(tlsServerName, $"{context}.tls_server_name", allowEmpty);
        RequireVpnTrustDigest(
            relayTlsSpkiSha256Hex,
            $"{context}.relay_tls_spki_sha256_hex",
            allowEmpty);
        RequireVpnTrustDigest(
            relayCertificateSha256Hex,
            $"{context}.relay_certificate_sha256_hex",
            allowEmpty);
        RequireVpnTrustDigest(
            directorySnapshotDigestHex,
            $"{context}.directory_snapshot_digest_hex",
            allowEmpty);
    }

    private static void RequireVpnRelayId(string value, string field, bool allowEmpty)
    {
        if (allowEmpty && value.Length == 0)
        {
            return;
        }

        ToriiSseEventJson.RequireExactSizedHex(value, field, 32);
        if (!IsCanonicalPrimeOrderEd25519PublicKey(Convert.FromHexString(value)))
        {
            throw new JsonException(
                $"{field} must encode a canonical prime-order Ed25519 public key.");
        }
    }

    private static void RequireVpnTrustDigest(string value, string field, bool allowEmpty)
    {
        if (allowEmpty && value.Length == 0)
        {
            return;
        }

        ToriiSseEventJson.RequireExactSizedHex(value, field, 32);
        if (value.AsSpan().IndexOfAnyExcept('0') < 0)
        {
            throw new JsonException($"{field} must not be the all-zero digest.");
        }
    }

    private static void RequireVpnTlsServerName(
        string value,
        string field,
        bool allowEmpty = false)
    {
        if (allowEmpty && value.Length == 0)
        {
            return;
        }

        if (value.Length is 0 or > 253
            || !string.Equals(value, value.ToLowerInvariant(), StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must be a canonical lowercase DNS name.");
        }

        foreach (var label in value.Split('.', StringSplitOptions.None))
        {
            if (label.Length is 0 or > 63
                || !IsDnsAlphaNumeric(label[0])
                || !IsDnsAlphaNumeric(label[^1])
                || label.Any(static character => !IsDnsAlphaNumeric(character) && character != '-'))
            {
                throw new JsonException($"{field} must be a canonical lowercase DNS name.");
            }
        }
    }

    private static bool IsDnsAlphaNumeric(char character)
    {
        return character is (>= 'a' and <= 'z') or (>= '0' and <= '9');
    }

    private static void RequireVpnRelayEndpoint(
        string value,
        string field,
        bool allowEmpty = false)
    {
        if (allowEmpty && value.Length == 0)
        {
            return;
        }

        var parts = value.Split('/', StringSplitOptions.None);
        if (parts.Length != 6
            || parts[0].Length != 0
            || parts[1] is not ("ip4" or "ip6" or "dns" or "dns4" or "dns6")
            || !string.Equals(parts[3], "udp", StringComparison.Ordinal)
            || !string.Equals(parts[5], "quic", StringComparison.Ordinal))
        {
            throw new JsonException(
                $"{field} must use /{{ip4|ip6|dns|dns4|dns6}}/host/udp/port/quic.");
        }

        var protocol = parts[1];
        var host = parts[2];
        if (protocol == "ip4")
        {
            if (!IPAddress.TryParse(host, out var address)
                || address.AddressFamily != AddressFamily.InterNetwork
                || !string.Equals(address.ToString(), host, StringComparison.Ordinal))
            {
                throw new JsonException($"{field} must contain a canonical IPv4 address.");
            }
        }
        else if (protocol == "ip6")
        {
            if (!IPAddress.TryParse(host, out var address)
                || address.AddressFamily != AddressFamily.InterNetworkV6
                || !string.Equals(address.ToString(), host, StringComparison.Ordinal))
            {
                throw new JsonException(
                    $"{field} must contain a canonical lowercase IPv6 address.");
            }
        }
        else
        {
            RequireVpnTlsServerName(host, $"{field} host");
        }

        if (!ushort.TryParse(parts[4], out var port)
            || port == 0
            || !string.Equals(port.ToString(), parts[4], StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must contain a canonical non-zero UDP port.");
        }
    }

    private static bool IsCanonicalPrimeOrderEd25519PublicKey(ReadOnlySpan<byte> encoded)
    {
        if (encoded.Length != 32)
        {
            return false;
        }

        Span<byte> yBytes = stackalloc byte[32];
        encoded.CopyTo(yBytes);
        var sign = yBytes[31] >> 7;
        yBytes[31] &= 0x7f;
        var y = new BigInteger(yBytes, isUnsigned: true, isBigEndian: false);
        if (y >= Ed25519FieldModulus)
        {
            return false;
        }

        var ySquared = ModEd25519(y * y);
        var denominator = ModEd25519((Ed25519D * ySquared) + 1);
        if (denominator.IsZero)
        {
            return false;
        }

        var xSquared = ModEd25519(
            (ySquared - 1)
            * BigInteger.ModPow(denominator, Ed25519FieldModulus - 2, Ed25519FieldModulus));
        var x = BigInteger.ModPow(
            xSquared,
            (Ed25519FieldModulus + 3) / 8,
            Ed25519FieldModulus);
        if (ModEd25519(x * x) != xSquared)
        {
            x = ModEd25519(x * Ed25519SqrtMinusOne);
        }

        if (ModEd25519(x * x) != xSquared || (x.IsZero && sign == 1))
        {
            return false;
        }

        if ((x.IsEven ? 0 : 1) != sign)
        {
            x = Ed25519FieldModulus - x;
        }

        if (x.IsZero && y.IsOne)
        {
            return false;
        }

        var subgroup = MultiplyEd25519(
            new Ed25519ExtendedPoint(x, y, BigInteger.One, ModEd25519(x * y)),
            Ed25519SubgroupOrder);
        return subgroup.X.IsZero && subgroup.Y == subgroup.Z;
    }

    private static Ed25519ExtendedPoint MultiplyEd25519(
        Ed25519ExtendedPoint point,
        BigInteger scalar)
    {
        var result = new Ed25519ExtendedPoint(
            BigInteger.Zero,
            BigInteger.One,
            BigInteger.One,
            BigInteger.Zero);
        var addend = point;
        while (scalar > 0)
        {
            if (!scalar.IsEven)
            {
                result = AddEd25519(result, addend);
            }

            addend = AddEd25519(addend, addend);
            scalar >>= 1;
        }

        return result;
    }

    private static Ed25519ExtendedPoint AddEd25519(
        Ed25519ExtendedPoint left,
        Ed25519ExtendedPoint right)
    {
        var a = ModEd25519((left.Y - left.X) * (right.Y - right.X));
        var b = ModEd25519((left.Y + left.X) * (right.Y + right.X));
        var c = ModEd25519(2 * Ed25519D * left.T * right.T);
        var d = ModEd25519(2 * left.Z * right.Z);
        var e = ModEd25519(b - a);
        var f = ModEd25519(d - c);
        var g = ModEd25519(d + c);
        var h = ModEd25519(b + a);
        return new Ed25519ExtendedPoint(
            ModEd25519(e * f),
            ModEd25519(g * h),
            ModEd25519(f * g),
            ModEd25519(e * h));
    }

    private static BigInteger ModEd25519(BigInteger value)
    {
        var reduced = value % Ed25519FieldModulus;
        return reduced.Sign < 0 ? reduced + Ed25519FieldModulus : reduced;
    }

    private static void RequireVpnExitClass(string value, string field)
    {
        ToriiSseEventJson.RequireExactTokenText(value, field);
        if (value is not ("standard" or "low-latency" or "high-security"))
        {
            throw new JsonException(
                $"{field} must be one of standard, low-latency, or high-security.");
        }
    }

    private static void RequireVpnSessionStatus(string value, string field)
    {
        ToriiSseEventJson.RequireExactTokenText(value, field);
        if (!string.Equals(value, "active", StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must equal active.");
        }
    }

    private static void RequireVpnReceiptStatus(string value, string field)
    {
        ToriiSseEventJson.RequireExactTokenText(value, field);
        if (value is not ("disconnected" or "expired" or "replaced" or "settled"))
        {
            throw new JsonException(
                $"{field} must be one of disconnected, expired, replaced, or settled.");
        }
    }

    private static void RequireVpnReceiptSource(string value, string field)
    {
        ToriiSseEventJson.RequireExactTokenText(value, field);
        if (value is not ("torii" or "relay" or "wsv"))
        {
            throw new JsonException($"{field} must be one of torii, relay, or wsv.");
        }
    }

    internal static void RequireRequiredProperties(
        ISet<string> seen,
        string context,
        IReadOnlyList<string> requiredPropertyNames)
    {
        foreach (var propertyName in requiredPropertyNames)
        {
            if (!seen.Contains(propertyName))
            {
                throw new JsonException($"{context}.{propertyName} is required.");
            }
        }
    }

    internal static void WriteNullableString(Utf8JsonWriter writer, string propertyName, string? value)
    {
        if (value is null)
        {
            writer.WriteNull(propertyName);
            return;
        }

        writer.WriteString(propertyName, value);
    }

    internal static void WriteStringList(Utf8JsonWriter writer, string propertyName, IReadOnlyList<string>? values)
    {
        if (values is null)
        {
            writer.WriteNull(propertyName);
            return;
        }

        writer.WriteStartArray(propertyName);
        foreach (var value in values)
        {
            if (value is null)
            {
                writer.WriteNullValue();
            }
            else
            {
                writer.WriteStringValue(value);
            }
        }

        writer.WriteEndArray();
    }

    internal static void WriteOptionalVpnTxInstruction(
        Utf8JsonWriter writer,
        string propertyName,
        ToriiVpnTxInstruction? instruction,
        string context)
    {
        if (instruction is null)
        {
            writer.WriteNull(propertyName);
            return;
        }

        writer.WritePropertyName(propertyName);
        WriteVpnTxInstruction(writer, instruction, context);
    }

    internal static void WriteVpnTxInstruction(Utf8JsonWriter writer, ToriiVpnTxInstruction instruction, string context)
    {
        ValidateVpnTxInstruction(instruction, context);

        writer.WriteStartObject();
        writer.WriteString("wire_id", instruction.WireId);
        writer.WriteString("payload_hex", instruction.PayloadHex);
        writer.WriteEndObject();
    }

    internal static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        return new JsonException($"{context}.{MapDirectMetadataField(error.ParamName ?? "metadata")}: {error.Message}", error);
    }

    internal static T CreateWithDirectMetadataContext<T>(Func<T> factory, string context)
    {
        try
        {
            return factory();
        }
        catch (ArgumentException error) when (error.ParamName is not null)
        {
            throw DirectMetadataErrorToJsonException(error, context);
        }
    }

    private static string MapDirectMetadataField(string paramName)
    {
        return paramName switch
        {
            _ when TryMapCollectionField(paramName, nameof(ToriiVpnProfile.SupportedExitClasses), "supported_exit_classes", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiVpnProfile.RoutePushes), "route_pushes", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiVpnProfile.ExcludedRoutes), "excluded_routes", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiVpnProfile.DnsServers), "dns_servers", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiVpnProfile.TunnelAddresses), "tunnel_addresses", out var mapped) => mapped,
            _ when TryMapCollectionField(paramName, nameof(ToriiVpnReceiptListResponse.Items), "items", out var mapped) => mapped,
            _ when TryMapNestedField(paramName, nameof(ToriiVpnQuote.OpenLeaseInstruction), "open_lease_instruction", out var mapped) => mapped,
            _ when TryMapNestedField(paramName, nameof(ToriiVpnReceipt.SettleLeaseInstruction), "settle_lease_instruction", out var mapped) => mapped,
            nameof(ToriiVpnProfile.RelayEndpoint) => "relay_endpoint",
            nameof(ToriiVpnProfile.DefaultExitClass) => "default_exit_class",
            nameof(ToriiVpnProfile.LeaseSeconds) => "lease_secs",
            nameof(ToriiVpnProfile.DnsPushIntervalSeconds) => "dns_push_interval_secs",
            nameof(ToriiVpnProfile.MeterFamily) => "meter_family",
            nameof(ToriiVpnProfile.MtuBytes) => "mtu_bytes",
            nameof(ToriiVpnProfile.DisplayBillingLabel) => "display_billing_label",
            nameof(ToriiVpnProfile.OperatorAccountId) => "operator_account_id",
            nameof(ToriiVpnProfile.RelayIdHex) => "relay_id_hex",
            nameof(ToriiVpnProfile.DescriptorCommitHex) => "descriptor_commit_hex",
            nameof(ToriiVpnProfile.TlsServerName) => "tls_server_name",
            nameof(ToriiVpnProfile.RelayTlsSpkiSha256Hex) => "relay_tls_spki_sha256_hex",
            nameof(ToriiVpnProfile.RelayCertificateSha256Hex) => "relay_certificate_sha256_hex",
            nameof(ToriiVpnProfile.DirectorySnapshotDigestHex) => "directory_snapshot_digest_hex",
            nameof(ToriiVpnTxInstruction.WireId) => "wire_id",
            nameof(ToriiVpnTxInstruction.PayloadHex) => "payload_hex",
            nameof(ToriiVpnQuote.QuoteId) => "quote_id",
            nameof(ToriiVpnQuote.LeaseIdHex) => "lease_id_hex",
            nameof(ToriiVpnQuote.SessionIdHex) => "session_id_hex",
            nameof(ToriiVpnQuote.PaymentReference) => "payment_reference",
            nameof(ToriiVpnQuote.AccountId) => "account_id",
            nameof(ToriiVpnQuote.ExitClass) => "exit_class",
            nameof(ToriiVpnQuote.FeeAssetId) => "fee_asset_id",
            nameof(ToriiVpnQuote.EscrowAccountId) => "escrow_account_id",
            nameof(ToriiVpnQuote.LeaseFee) => "lease_fee",
            nameof(ToriiVpnQuote.QuoteExpiresAtMilliseconds) => "quote_expires_at_ms",
            nameof(ToriiVpnQuote.MeteringPublicKeyHex) => "metering_public_key_hex",
            nameof(ToriiVpnSession.SessionId) => "session_id",
            nameof(ToriiVpnSession.ExpiresAtMilliseconds) => "expires_at_ms",
            nameof(ToriiVpnSession.ConnectedAtMilliseconds) => "connected_at_ms",
            nameof(ToriiVpnSession.PaymentTransactionHash) => "payment_tx_hash",
            nameof(ToriiVpnSession.HelperTicketHex) => "helper_ticket_hex",
            nameof(ToriiVpnSession.Status) => "status",
            nameof(ToriiVpnReceipt.DisconnectedAtMilliseconds) => "disconnected_at_ms",
            nameof(ToriiVpnReceipt.ReceiptSource) => "receipt_source",
            _ => paramName,
        };
    }

    private static bool TryMapCollectionField(
        string paramName,
        string propertyName,
        string jsonName,
        out string mapped)
    {
        var prefix = propertyName + "[";
        if (paramName.StartsWith(prefix, StringComparison.Ordinal))
        {
            mapped = jsonName + MapIndexedSuffix(paramName[propertyName.Length..]);
            return true;
        }

        mapped = string.Empty;
        return false;
    }

    private static bool TryMapNestedField(
        string paramName,
        string propertyName,
        string jsonName,
        out string mapped)
    {
        var prefix = propertyName + ".";
        if (paramName.StartsWith(prefix, StringComparison.Ordinal))
        {
            mapped = jsonName + "." + MapDirectMetadataField(paramName[prefix.Length..]);
            return true;
        }

        mapped = string.Empty;
        return false;
    }

    private static string MapIndexedSuffix(string suffix)
    {
        var dot = suffix.IndexOf('.', StringComparison.Ordinal);
        if (dot < 0)
        {
            return suffix;
        }

        return suffix[..(dot + 1)] + MapDirectMetadataField(suffix[(dot + 1)..]);
    }

    private static string RequireExactNonEmptyText(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        return value;
    }

    private static string RequireCanonicalAccountId(string? value, string field)
    {
        var text = ToriiSseEventJson.RequireExactTokenText(value, field);
        try
        {
            return AccountAddress.Parse(text, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        catch (AccountAddressException exception)
        {
            throw new JsonException($"{field} must be a canonical I105 account id.", exception);
        }
    }

    private static void RequireExactEvenLengthHex(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty even-length hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                throw new JsonException($"{field} must not contain whitespace.");
            }

            if (char.IsControl(character))
            {
                throw new JsonException($"{field} must not contain control characters.");
            }
        }

        if (value.Length % 2 != 0 || !IsLowercaseHex(value))
        {
            throw new JsonException($"{field} must be an exact lowercase even-length hex string.");
        }
    }

    private static bool ContainsControlCharacter(string value)
    {
        foreach (var character in value)
        {
            if (char.IsControl(character))
            {
                return true;
            }
        }

        return false;
    }

    private static bool IsLowercaseHex(string value)
    {
        foreach (var character in value)
        {
            if (character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f'))
            {
                return false;
            }
        }

        return true;
    }
}

internal sealed class ToriiVpnTxInstructionJsonConverter : JsonConverter<ToriiVpnTxInstruction>
{
    public override bool HandleNull => true;

    public override ToriiVpnTxInstruction Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiVpnJson.ReadVpnTxInstruction(ref reader, "vpn tx instruction");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiVpnTxInstruction value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiVpnJson.WriteVpnTxInstruction(writer, value, "vpn tx instruction");
    }
}

internal sealed class ToriiVpnProfileJsonConverter : JsonConverter<ToriiVpnProfile>
{
    private static readonly string[] RequiredPropertyNames =
    [
        "available",
        "relay_endpoint",
        "supported_exit_classes",
        "default_exit_class",
        "lease_secs",
        "dns_push_interval_secs",
        "meter_family",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "display_billing_label",
        "operator_account_id",
        "lease_fee",
        "settlement_grace_secs",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
    ];

    public override bool HandleNull => true;

    public override ToriiVpnProfile Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        const string context = "vpn profile";
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        bool? available = null;
        string? relayEndpoint = null;
        IReadOnlyList<string>? supportedExitClasses = null;
        string? defaultExitClass = null;
        ulong? leaseSeconds = null;
        ulong? dnsPushIntervalSeconds = null;
        string? meterFamily = null;
        IReadOnlyList<string>? routePushes = null;
        IReadOnlyList<string>? excludedRoutes = null;
        IReadOnlyList<string>? dnsServers = null;
        IReadOnlyList<string>? tunnelAddresses = null;
        ulong? mtuBytes = null;
        string? displayBillingLabel = null;
        string? operatorAccountId = null;
        string? leaseFee = null;
        ulong? settlementGraceSeconds = null;
        byte? flowLabelBits = null;
        ushort? paddingBudgetMilliseconds = null;
        string? relayIdHex = null;
        string? descriptorCommitHex = null;
        string? tlsServerName = null;
        string? relayTlsSpkiSha256Hex = null;
        string? relayCertificateSha256Hex = null;
        string? directorySnapshotDigestHex = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var profile = ToriiVpnJson.CreateWithDirectMetadataContext(() => new ToriiVpnProfile
                {
                    Available = ToriiVpnJson.RequireBool(available, context, "available"),
                    RelayEndpoint = ToriiVpnJson.RequireString(relayEndpoint, $"{context}.relay_endpoint"),
                    SupportedExitClasses = supportedExitClasses!,
                    DefaultExitClass = ToriiVpnJson.RequireString(defaultExitClass, $"{context}.default_exit_class"),
                    LeaseSeconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(leaseSeconds, context, "lease_secs"),
                    DnsPushIntervalSeconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        dnsPushIntervalSeconds,
                        context,
                        "dns_push_interval_secs"),
                    MeterFamily = ToriiVpnJson.RequireString(meterFamily, $"{context}.meter_family"),
                    RoutePushes = routePushes!,
                    ExcludedRoutes = excludedRoutes!,
                    DnsServers = dnsServers!,
                    TunnelAddresses = tunnelAddresses!,
                    MtuBytes = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(mtuBytes, context, "mtu_bytes"),
                    DisplayBillingLabel = ToriiVpnJson.RequireString(displayBillingLabel, $"{context}.display_billing_label"),
                    OperatorAccountId = ToriiVpnJson.RequireString(operatorAccountId, $"{context}.operator_account_id"),
                    LeaseFee = ToriiQuantityJson.RequireCanonicalQuantity(
                        ToriiVpnJson.RequireString(leaseFee, $"{context}.lease_fee"),
                        $"{context}.lease_fee"),
                    SettlementGraceSeconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        settlementGraceSeconds,
                        context,
                        "settlement_grace_secs"),
                    FlowLabelBits = ToriiVpnJson.RequireByte(flowLabelBits, context, "flow_label_bits"),
                    PaddingBudgetMilliseconds = ToriiVpnJson.RequireUInt16(
                        paddingBudgetMilliseconds,
                        context,
                        "padding_budget_ms"),
                    RelayIdHex = ToriiVpnJson.RequireString(relayIdHex, $"{context}.relay_id_hex"),
                    DescriptorCommitHex = ToriiVpnJson.RequireString(
                        descriptorCommitHex,
                        $"{context}.descriptor_commit_hex"),
                    TlsServerName = ToriiVpnJson.RequireString(tlsServerName, $"{context}.tls_server_name"),
                    RelayTlsSpkiSha256Hex = ToriiVpnJson.RequireString(
                        relayTlsSpkiSha256Hex,
                        $"{context}.relay_tls_spki_sha256_hex"),
                    RelayCertificateSha256Hex = ToriiVpnJson.RequireString(
                        relayCertificateSha256Hex,
                        $"{context}.relay_certificate_sha256_hex"),
                    DirectorySnapshotDigestHex = ToriiVpnJson.RequireString(
                        directorySnapshotDigestHex,
                        $"{context}.directory_snapshot_digest_hex"),
                }, context);
                ToriiVpnJson.ValidateVpnProfile(profile, context);
                ToriiVpnJson.RequireRequiredProperties(seen, context, RequiredPropertyNames);
                return profile;
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "available":
                    available = ToriiVpnJson.ReadBool(ref reader, $"{context}.available");
                    break;
                case "relay_endpoint":
                    relayEndpoint = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_endpoint");
                    break;
                case "supported_exit_classes":
                    supportedExitClasses = ToriiVpnJson.ReadStringList(ref reader, $"{context}.supported_exit_classes");
                    break;
                case "default_exit_class":
                    defaultExitClass = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.default_exit_class");
                    break;
                case "lease_secs":
                    leaseSeconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.lease_secs");
                    break;
                case "dns_push_interval_secs":
                    dnsPushIntervalSeconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.dns_push_interval_secs");
                    break;
                case "meter_family":
                    meterFamily = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.meter_family");
                    break;
                case "route_pushes":
                    routePushes = ToriiVpnJson.ReadStringList(ref reader, $"{context}.route_pushes");
                    break;
                case "excluded_routes":
                    excludedRoutes = ToriiVpnJson.ReadStringList(ref reader, $"{context}.excluded_routes");
                    break;
                case "dns_servers":
                    dnsServers = ToriiVpnJson.ReadStringList(ref reader, $"{context}.dns_servers");
                    break;
                case "tunnel_addresses":
                    tunnelAddresses = ToriiVpnJson.ReadStringList(ref reader, $"{context}.tunnel_addresses");
                    break;
                case "mtu_bytes":
                    mtuBytes = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.mtu_bytes");
                    break;
                case "display_billing_label":
                    displayBillingLabel = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.display_billing_label");
                    break;
                case "operator_account_id":
                    operatorAccountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.operator_account_id");
                    break;
                case "lease_fee":
                    leaseFee = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.lease_fee");
                    break;
                case "settlement_grace_secs":
                    settlementGraceSeconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.settlement_grace_secs");
                    break;
                case "flow_label_bits":
                    flowLabelBits = ToriiAccountFaucetJson.ReadByte(ref reader, $"{context}.flow_label_bits");
                    break;
                case "padding_budget_ms":
                    paddingBudgetMilliseconds = ToriiVpnJson.ReadUInt16(ref reader, $"{context}.padding_budget_ms");
                    break;
                case "relay_id_hex":
                    relayIdHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_id_hex");
                    break;
                case "descriptor_commit_hex":
                    descriptorCommitHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.descriptor_commit_hex");
                    break;
                case "tls_server_name":
                    tlsServerName = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tls_server_name");
                    break;
                case "relay_tls_spki_sha256_hex":
                    relayTlsSpkiSha256Hex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_tls_spki_sha256_hex");
                    break;
                case "relay_certificate_sha256_hex":
                    relayCertificateSha256Hex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_certificate_sha256_hex");
                    break;
                case "directory_snapshot_digest_hex":
                    directorySnapshotDigestHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.directory_snapshot_digest_hex");
                    break;
                default:
                    throw new JsonException($"{context}.{propertyName} is not allowed.");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiVpnProfile value,
        JsonSerializerOptions options)
    {
        ToriiVpnJson.ValidateVpnProfile(value, "vpn profile");

        writer.WriteStartObject();
        writer.WriteBoolean("available", value.Available);
        writer.WriteString("relay_endpoint", value.RelayEndpoint);
        ToriiVpnJson.WriteStringList(writer, "supported_exit_classes", value.SupportedExitClasses);
        writer.WriteString("default_exit_class", value.DefaultExitClass);
        writer.WriteNumber("lease_secs", value.LeaseSeconds);
        writer.WriteNumber("dns_push_interval_secs", value.DnsPushIntervalSeconds);
        writer.WriteString("meter_family", value.MeterFamily);
        ToriiVpnJson.WriteStringList(writer, "route_pushes", value.RoutePushes);
        ToriiVpnJson.WriteStringList(writer, "excluded_routes", value.ExcludedRoutes);
        ToriiVpnJson.WriteStringList(writer, "dns_servers", value.DnsServers);
        ToriiVpnJson.WriteStringList(writer, "tunnel_addresses", value.TunnelAddresses);
        writer.WriteNumber("mtu_bytes", value.MtuBytes);
        writer.WriteString("display_billing_label", value.DisplayBillingLabel);
        writer.WriteString("operator_account_id", value.OperatorAccountId);
        writer.WriteString("lease_fee", value.LeaseFee);
        writer.WriteNumber("settlement_grace_secs", value.SettlementGraceSeconds);
        writer.WriteNumber("flow_label_bits", value.FlowLabelBits);
        writer.WriteNumber("padding_budget_ms", value.PaddingBudgetMilliseconds);
        writer.WriteString("relay_id_hex", value.RelayIdHex);
        writer.WriteString("descriptor_commit_hex", value.DescriptorCommitHex);
        writer.WriteString("tls_server_name", value.TlsServerName);
        writer.WriteString("relay_tls_spki_sha256_hex", value.RelayTlsSpkiSha256Hex);
        writer.WriteString("relay_certificate_sha256_hex", value.RelayCertificateSha256Hex);
        writer.WriteString("directory_snapshot_digest_hex", value.DirectorySnapshotDigestHex);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiVpnQuoteJsonConverter : JsonConverter<ToriiVpnQuote>
{
    private static readonly string[] RequiredPropertyNames =
    [
        "quote_id",
        "lease_id_hex",
        "session_id_hex",
        "payment_reference",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "lease_secs",
        "quote_expires_at_ms",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "meter_family",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
        "metering_public_key_hex",
        "open_lease_instruction",
    ];

    public override bool HandleNull => true;

    public override ToriiVpnQuote Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        const string context = "vpn quote";
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? quoteId = null;
        string? leaseIdHex = null;
        string? sessionIdHex = null;
        string? paymentReference = null;
        string? accountId = null;
        string? exitClass = null;
        string? relayEndpoint = null;
        ulong? leaseSeconds = null;
        ulong? quoteExpiresAtMilliseconds = null;
        string? feeAssetId = null;
        string? escrowAccountId = null;
        string? operatorAccountId = null;
        string? leaseFee = null;
        IReadOnlyList<string>? routePushes = null;
        IReadOnlyList<string>? excludedRoutes = null;
        IReadOnlyList<string>? dnsServers = null;
        IReadOnlyList<string>? tunnelAddresses = null;
        ulong? mtuBytes = null;
        string? meterFamily = null;
        byte? flowLabelBits = null;
        ushort? paddingBudgetMilliseconds = null;
        string? relayIdHex = null;
        string? descriptorCommitHex = null;
        string? tlsServerName = null;
        string? relayTlsSpkiSha256Hex = null;
        string? relayCertificateSha256Hex = null;
        string? directorySnapshotDigestHex = null;
        string? meteringPublicKeyHex = null;
        ToriiVpnTxInstruction? openLeaseInstruction = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var quote = ToriiVpnJson.CreateWithDirectMetadataContext(() => new ToriiVpnQuote
                {
                    QuoteId = ToriiVpnJson.RequireString(quoteId, $"{context}.quote_id"),
                    LeaseIdHex = ToriiVpnJson.RequireString(leaseIdHex, $"{context}.lease_id_hex"),
                    SessionIdHex = ToriiVpnJson.RequireString(sessionIdHex, $"{context}.session_id_hex"),
                    PaymentReference = ToriiVpnJson.RequireString(paymentReference, $"{context}.payment_reference"),
                    AccountId = ToriiVpnJson.RequireString(accountId, $"{context}.account_id"),
                    ExitClass = ToriiVpnJson.RequireString(exitClass, $"{context}.exit_class"),
                    RelayEndpoint = ToriiVpnJson.RequireString(relayEndpoint, $"{context}.relay_endpoint"),
                    LeaseSeconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(leaseSeconds, context, "lease_secs"),
                    QuoteExpiresAtMilliseconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        quoteExpiresAtMilliseconds,
                        context,
                        "quote_expires_at_ms"),
                    FeeAssetId = ToriiVpnJson.RequireString(feeAssetId, $"{context}.fee_asset_id"),
                    EscrowAccountId = ToriiVpnJson.RequireString(escrowAccountId, $"{context}.escrow_account_id"),
                    OperatorAccountId = ToriiVpnJson.RequireString(operatorAccountId, $"{context}.operator_account_id"),
                    LeaseFee = ToriiQuantityJson.RequireCanonicalQuantity(
                        ToriiVpnJson.RequireString(leaseFee, $"{context}.lease_fee"),
                        $"{context}.lease_fee"),
                    RoutePushes = routePushes!,
                    ExcludedRoutes = excludedRoutes!,
                    DnsServers = dnsServers!,
                    TunnelAddresses = tunnelAddresses!,
                    MtuBytes = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(mtuBytes, context, "mtu_bytes"),
                    MeterFamily = ToriiVpnJson.RequireString(meterFamily, $"{context}.meter_family"),
                    FlowLabelBits = ToriiVpnJson.RequireByte(flowLabelBits, context, "flow_label_bits"),
                    PaddingBudgetMilliseconds = ToriiVpnJson.RequireUInt16(
                        paddingBudgetMilliseconds,
                        context,
                        "padding_budget_ms"),
                    RelayIdHex = ToriiVpnJson.RequireString(relayIdHex, $"{context}.relay_id_hex"),
                    DescriptorCommitHex = ToriiVpnJson.RequireString(
                        descriptorCommitHex,
                        $"{context}.descriptor_commit_hex"),
                    TlsServerName = ToriiVpnJson.RequireString(tlsServerName, $"{context}.tls_server_name"),
                    RelayTlsSpkiSha256Hex = ToriiVpnJson.RequireString(
                        relayTlsSpkiSha256Hex,
                        $"{context}.relay_tls_spki_sha256_hex"),
                    RelayCertificateSha256Hex = ToriiVpnJson.RequireString(
                        relayCertificateSha256Hex,
                        $"{context}.relay_certificate_sha256_hex"),
                    DirectorySnapshotDigestHex = ToriiVpnJson.RequireString(
                        directorySnapshotDigestHex,
                        $"{context}.directory_snapshot_digest_hex"),
                    MeteringPublicKeyHex = ToriiVpnJson.RequireString(
                        meteringPublicKeyHex,
                        $"{context}.metering_public_key_hex"),
                    OpenLeaseInstruction = openLeaseInstruction
                        ?? throw new JsonException($"{context}.open_lease_instruction must not be null."),
                }, context);
                ToriiVpnJson.ValidateVpnQuote(quote, context);
                ToriiVpnJson.RequireRequiredProperties(seen, context, RequiredPropertyNames);
                return quote;
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "quote_id":
                    quoteId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.quote_id");
                    break;
                case "lease_id_hex":
                    leaseIdHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.lease_id_hex");
                    break;
                case "session_id_hex":
                    sessionIdHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.session_id_hex");
                    break;
                case "payment_reference":
                    paymentReference = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.payment_reference");
                    break;
                case "account_id":
                    accountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "exit_class":
                    exitClass = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.exit_class");
                    break;
                case "relay_endpoint":
                    relayEndpoint = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_endpoint");
                    break;
                case "lease_secs":
                    leaseSeconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.lease_secs");
                    break;
                case "quote_expires_at_ms":
                    quoteExpiresAtMilliseconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.quote_expires_at_ms");
                    break;
                case "fee_asset_id":
                    feeAssetId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.fee_asset_id");
                    break;
                case "escrow_account_id":
                    escrowAccountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.escrow_account_id");
                    break;
                case "operator_account_id":
                    operatorAccountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.operator_account_id");
                    break;
                case "lease_fee":
                    leaseFee = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.lease_fee");
                    break;
                case "route_pushes":
                    routePushes = ToriiVpnJson.ReadStringList(ref reader, $"{context}.route_pushes");
                    break;
                case "excluded_routes":
                    excludedRoutes = ToriiVpnJson.ReadStringList(ref reader, $"{context}.excluded_routes");
                    break;
                case "dns_servers":
                    dnsServers = ToriiVpnJson.ReadStringList(ref reader, $"{context}.dns_servers");
                    break;
                case "tunnel_addresses":
                    tunnelAddresses = ToriiVpnJson.ReadStringList(ref reader, $"{context}.tunnel_addresses");
                    break;
                case "mtu_bytes":
                    mtuBytes = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.mtu_bytes");
                    break;
                case "meter_family":
                    meterFamily = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.meter_family");
                    break;
                case "flow_label_bits":
                    flowLabelBits = ToriiAccountFaucetJson.ReadByte(ref reader, $"{context}.flow_label_bits");
                    break;
                case "padding_budget_ms":
                    paddingBudgetMilliseconds = ToriiVpnJson.ReadUInt16(ref reader, $"{context}.padding_budget_ms");
                    break;
                case "relay_id_hex":
                    relayIdHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_id_hex");
                    break;
                case "descriptor_commit_hex":
                    descriptorCommitHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.descriptor_commit_hex");
                    break;
                case "tls_server_name":
                    tlsServerName = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tls_server_name");
                    break;
                case "relay_tls_spki_sha256_hex":
                    relayTlsSpkiSha256Hex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_tls_spki_sha256_hex");
                    break;
                case "relay_certificate_sha256_hex":
                    relayCertificateSha256Hex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_certificate_sha256_hex");
                    break;
                case "directory_snapshot_digest_hex":
                    directorySnapshotDigestHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.directory_snapshot_digest_hex");
                    break;
                case "metering_public_key_hex":
                    meteringPublicKeyHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.metering_public_key_hex");
                    break;
                case "open_lease_instruction":
                    openLeaseInstruction = ToriiVpnJson.ReadVpnTxInstruction(
                        ref reader,
                        $"{context}.open_lease_instruction");
                    break;
                default:
                    throw new JsonException($"{context}.{propertyName} is not allowed.");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiVpnQuote value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiVpnJson.ValidateVpnQuote(value, "vpn quote");

        writer.WriteStartObject();
        writer.WriteString("quote_id", value.QuoteId);
        writer.WriteString("lease_id_hex", value.LeaseIdHex);
        writer.WriteString("session_id_hex", value.SessionIdHex);
        writer.WriteString("payment_reference", value.PaymentReference);
        writer.WriteString("account_id", value.AccountId);
        writer.WriteString("exit_class", value.ExitClass);
        writer.WriteString("relay_endpoint", value.RelayEndpoint);
        writer.WriteNumber("lease_secs", value.LeaseSeconds);
        writer.WriteNumber("quote_expires_at_ms", value.QuoteExpiresAtMilliseconds);
        writer.WriteString("fee_asset_id", value.FeeAssetId);
        writer.WriteString("escrow_account_id", value.EscrowAccountId);
        writer.WriteString("operator_account_id", value.OperatorAccountId);
        writer.WriteString("lease_fee", value.LeaseFee);
        ToriiVpnJson.WriteStringList(writer, "route_pushes", value.RoutePushes);
        ToriiVpnJson.WriteStringList(writer, "excluded_routes", value.ExcludedRoutes);
        ToriiVpnJson.WriteStringList(writer, "dns_servers", value.DnsServers);
        ToriiVpnJson.WriteStringList(writer, "tunnel_addresses", value.TunnelAddresses);
        writer.WriteNumber("mtu_bytes", value.MtuBytes);
        writer.WriteString("meter_family", value.MeterFamily);
        writer.WriteNumber("flow_label_bits", value.FlowLabelBits);
        writer.WriteNumber("padding_budget_ms", value.PaddingBudgetMilliseconds);
        writer.WriteString("relay_id_hex", value.RelayIdHex);
        writer.WriteString("descriptor_commit_hex", value.DescriptorCommitHex);
        writer.WriteString("tls_server_name", value.TlsServerName);
        writer.WriteString("relay_tls_spki_sha256_hex", value.RelayTlsSpkiSha256Hex);
        writer.WriteString("relay_certificate_sha256_hex", value.RelayCertificateSha256Hex);
        writer.WriteString("directory_snapshot_digest_hex", value.DirectorySnapshotDigestHex);
        writer.WriteString("metering_public_key_hex", value.MeteringPublicKeyHex);
        writer.WritePropertyName("open_lease_instruction");
        ToriiVpnJson.WriteVpnTxInstruction(
            writer,
            value.OpenLeaseInstruction,
            "vpn quote.open_lease_instruction");
        writer.WriteEndObject();
    }
}

internal sealed class ToriiVpnSessionJsonConverter : JsonConverter<ToriiVpnSession>
{
    private static readonly string[] RequiredPropertyNames =
    [
        "session_id",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "lease_secs",
        "expires_at_ms",
        "connected_at_ms",
        "meter_family",
        "quote_id",
        "payment_reference",
        "payment_tx_hash",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "helper_ticket_hex",
        "bytes_in",
        "bytes_out",
        "status",
    ];

    public override bool HandleNull => true;

    public override ToriiVpnSession Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        const string context = "vpn session";
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? sessionId = null;
        string? accountId = null;
        string? exitClass = null;
        string? relayEndpoint = null;
        ulong? leaseSeconds = null;
        ulong? expiresAtMilliseconds = null;
        ulong? connectedAtMilliseconds = null;
        string? meterFamily = null;
        string? quoteId = null;
        string? paymentReference = null;
        string? paymentTransactionHash = null;
        string? feeAssetId = null;
        string? escrowAccountId = null;
        string? operatorAccountId = null;
        string? leaseFee = null;
        byte? flowLabelBits = null;
        ushort? paddingBudgetMilliseconds = null;
        string? relayIdHex = null;
        string? descriptorCommitHex = null;
        string? tlsServerName = null;
        string? relayTlsSpkiSha256Hex = null;
        string? relayCertificateSha256Hex = null;
        string? directorySnapshotDigestHex = null;
        IReadOnlyList<string>? routePushes = null;
        IReadOnlyList<string>? excludedRoutes = null;
        IReadOnlyList<string>? dnsServers = null;
        IReadOnlyList<string>? tunnelAddresses = null;
        ulong? mtuBytes = null;
        string? helperTicketHex = null;
        ulong? bytesIn = null;
        ulong? bytesOut = null;
        string? status = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var session = ToriiVpnJson.CreateWithDirectMetadataContext(() => new ToriiVpnSession
                {
                    SessionId = ToriiVpnJson.RequireString(sessionId, $"{context}.session_id"),
                    AccountId = ToriiVpnJson.RequireString(accountId, $"{context}.account_id"),
                    ExitClass = ToriiVpnJson.RequireString(exitClass, $"{context}.exit_class"),
                    RelayEndpoint = ToriiVpnJson.RequireString(relayEndpoint, $"{context}.relay_endpoint"),
                    LeaseSeconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(leaseSeconds, context, "lease_secs"),
                    ExpiresAtMilliseconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        expiresAtMilliseconds,
                        context,
                        "expires_at_ms"),
                    ConnectedAtMilliseconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        connectedAtMilliseconds,
                        context,
                        "connected_at_ms"),
                    MeterFamily = ToriiVpnJson.RequireString(meterFamily, $"{context}.meter_family"),
                    QuoteId = ToriiVpnJson.RequireString(quoteId, $"{context}.quote_id"),
                    PaymentReference = ToriiVpnJson.RequireString(paymentReference, $"{context}.payment_reference"),
                    PaymentTransactionHash = ToriiVpnJson.RequireString(
                        paymentTransactionHash,
                        $"{context}.payment_tx_hash"),
                    FeeAssetId = ToriiVpnJson.RequireString(feeAssetId, $"{context}.fee_asset_id"),
                    EscrowAccountId = ToriiVpnJson.RequireString(escrowAccountId, $"{context}.escrow_account_id"),
                    OperatorAccountId = ToriiVpnJson.RequireString(operatorAccountId, $"{context}.operator_account_id"),
                    LeaseFee = ToriiQuantityJson.RequireCanonicalQuantity(
                        ToriiVpnJson.RequireString(leaseFee, $"{context}.lease_fee"),
                        $"{context}.lease_fee"),
                    FlowLabelBits = ToriiVpnJson.RequireByte(flowLabelBits, context, "flow_label_bits"),
                    PaddingBudgetMilliseconds = ToriiVpnJson.RequireUInt16(
                        paddingBudgetMilliseconds,
                        context,
                        "padding_budget_ms"),
                    RelayIdHex = ToriiVpnJson.RequireString(relayIdHex, $"{context}.relay_id_hex"),
                    DescriptorCommitHex = ToriiVpnJson.RequireString(
                        descriptorCommitHex,
                        $"{context}.descriptor_commit_hex"),
                    TlsServerName = ToriiVpnJson.RequireString(tlsServerName, $"{context}.tls_server_name"),
                    RelayTlsSpkiSha256Hex = ToriiVpnJson.RequireString(
                        relayTlsSpkiSha256Hex,
                        $"{context}.relay_tls_spki_sha256_hex"),
                    RelayCertificateSha256Hex = ToriiVpnJson.RequireString(
                        relayCertificateSha256Hex,
                        $"{context}.relay_certificate_sha256_hex"),
                    DirectorySnapshotDigestHex = ToriiVpnJson.RequireString(
                        directorySnapshotDigestHex,
                        $"{context}.directory_snapshot_digest_hex"),
                    RoutePushes = routePushes!,
                    ExcludedRoutes = excludedRoutes!,
                    DnsServers = dnsServers!,
                    TunnelAddresses = tunnelAddresses!,
                    MtuBytes = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(mtuBytes, context, "mtu_bytes"),
                    HelperTicketHex = ToriiVpnJson.RequireString(helperTicketHex, $"{context}.helper_ticket_hex"),
                    BytesIn = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(bytesIn, context, "bytes_in"),
                    BytesOut = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(bytesOut, context, "bytes_out"),
                    Status = ToriiVpnJson.RequireString(status, $"{context}.status"),
                }, context);
                ToriiVpnJson.ValidateVpnSession(session, context);
                ToriiVpnJson.RequireRequiredProperties(seen, context, RequiredPropertyNames);
                return session;
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "session_id":
                    sessionId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.session_id");
                    break;
                case "account_id":
                    accountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "exit_class":
                    exitClass = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.exit_class");
                    break;
                case "relay_endpoint":
                    relayEndpoint = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_endpoint");
                    break;
                case "lease_secs":
                    leaseSeconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.lease_secs");
                    break;
                case "expires_at_ms":
                    expiresAtMilliseconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.expires_at_ms");
                    break;
                case "connected_at_ms":
                    connectedAtMilliseconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.connected_at_ms");
                    break;
                case "meter_family":
                    meterFamily = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.meter_family");
                    break;
                case "quote_id":
                    quoteId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.quote_id");
                    break;
                case "payment_reference":
                    paymentReference = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.payment_reference");
                    break;
                case "payment_tx_hash":
                    paymentTransactionHash = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.payment_tx_hash");
                    break;
                case "fee_asset_id":
                    feeAssetId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.fee_asset_id");
                    break;
                case "escrow_account_id":
                    escrowAccountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.escrow_account_id");
                    break;
                case "operator_account_id":
                    operatorAccountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.operator_account_id");
                    break;
                case "lease_fee":
                    leaseFee = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.lease_fee");
                    break;
                case "flow_label_bits":
                    flowLabelBits = ToriiAccountFaucetJson.ReadByte(ref reader, $"{context}.flow_label_bits");
                    break;
                case "padding_budget_ms":
                    paddingBudgetMilliseconds = ToriiVpnJson.ReadUInt16(ref reader, $"{context}.padding_budget_ms");
                    break;
                case "relay_id_hex":
                    relayIdHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_id_hex");
                    break;
                case "descriptor_commit_hex":
                    descriptorCommitHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.descriptor_commit_hex");
                    break;
                case "tls_server_name":
                    tlsServerName = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.tls_server_name");
                    break;
                case "relay_tls_spki_sha256_hex":
                    relayTlsSpkiSha256Hex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_tls_spki_sha256_hex");
                    break;
                case "relay_certificate_sha256_hex":
                    relayCertificateSha256Hex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_certificate_sha256_hex");
                    break;
                case "directory_snapshot_digest_hex":
                    directorySnapshotDigestHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.directory_snapshot_digest_hex");
                    break;
                case "route_pushes":
                    routePushes = ToriiVpnJson.ReadStringList(ref reader, $"{context}.route_pushes");
                    break;
                case "excluded_routes":
                    excludedRoutes = ToriiVpnJson.ReadStringList(ref reader, $"{context}.excluded_routes");
                    break;
                case "dns_servers":
                    dnsServers = ToriiVpnJson.ReadStringList(ref reader, $"{context}.dns_servers");
                    break;
                case "tunnel_addresses":
                    tunnelAddresses = ToriiVpnJson.ReadStringList(ref reader, $"{context}.tunnel_addresses");
                    break;
                case "mtu_bytes":
                    mtuBytes = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.mtu_bytes");
                    break;
                case "helper_ticket_hex":
                    helperTicketHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.helper_ticket_hex");
                    break;
                case "bytes_in":
                    bytesIn = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.bytes_in");
                    break;
                case "bytes_out":
                    bytesOut = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.bytes_out");
                    break;
                case "status":
                    status = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.status");
                    break;
                default:
                    throw new JsonException($"{context}.{propertyName} is not allowed.");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiVpnSession value,
        JsonSerializerOptions options)
    {
        ToriiVpnJson.ValidateVpnSession(value, "vpn session");

        writer.WriteStartObject();
        writer.WriteString("session_id", value.SessionId);
        writer.WriteString("account_id", value.AccountId);
        writer.WriteString("exit_class", value.ExitClass);
        writer.WriteString("relay_endpoint", value.RelayEndpoint);
        writer.WriteNumber("lease_secs", value.LeaseSeconds);
        writer.WriteNumber("expires_at_ms", value.ExpiresAtMilliseconds);
        writer.WriteNumber("connected_at_ms", value.ConnectedAtMilliseconds);
        writer.WriteString("meter_family", value.MeterFamily);
        writer.WriteString("quote_id", value.QuoteId);
        writer.WriteString("payment_reference", value.PaymentReference);
        writer.WriteString("payment_tx_hash", value.PaymentTransactionHash);
        writer.WriteString("fee_asset_id", value.FeeAssetId);
        writer.WriteString("escrow_account_id", value.EscrowAccountId);
        writer.WriteString("operator_account_id", value.OperatorAccountId);
        writer.WriteString("lease_fee", value.LeaseFee);
        writer.WriteNumber("flow_label_bits", value.FlowLabelBits);
        writer.WriteNumber("padding_budget_ms", value.PaddingBudgetMilliseconds);
        writer.WriteString("relay_id_hex", value.RelayIdHex);
        writer.WriteString("descriptor_commit_hex", value.DescriptorCommitHex);
        writer.WriteString("tls_server_name", value.TlsServerName);
        writer.WriteString("relay_tls_spki_sha256_hex", value.RelayTlsSpkiSha256Hex);
        writer.WriteString("relay_certificate_sha256_hex", value.RelayCertificateSha256Hex);
        writer.WriteString("directory_snapshot_digest_hex", value.DirectorySnapshotDigestHex);
        ToriiVpnJson.WriteStringList(writer, "route_pushes", value.RoutePushes);
        ToriiVpnJson.WriteStringList(writer, "excluded_routes", value.ExcludedRoutes);
        ToriiVpnJson.WriteStringList(writer, "dns_servers", value.DnsServers);
        ToriiVpnJson.WriteStringList(writer, "tunnel_addresses", value.TunnelAddresses);
        writer.WriteNumber("mtu_bytes", value.MtuBytes);
        writer.WriteString("helper_ticket_hex", value.HelperTicketHex);
        writer.WriteNumber("bytes_in", value.BytesIn);
        writer.WriteNumber("bytes_out", value.BytesOut);
        writer.WriteString("status", value.Status);
        writer.WriteEndObject();
    }
}

internal sealed class ToriiVpnReceiptJsonConverter : JsonConverter<ToriiVpnReceipt>
{
    private static readonly string[] RequiredPropertyNames =
    [
        "session_id",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "meter_family",
        "connected_at_ms",
        "disconnected_at_ms",
        "duration_ms",
        "bytes_in",
        "bytes_out",
        "status",
        "receipt_source",
        "quote_id",
        "payment_tx_hash",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "earned_fee",
        "refunded_fee",
        "lease_id_hex",
        "settle_lease_instruction",
    ];

    public override bool HandleNull => true;

    public override ToriiVpnReceipt Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ReadReceipt(ref reader, "vpn receipt");
    }

    internal static ToriiVpnReceipt ReadReceipt(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? sessionId = null;
        string? accountId = null;
        string? exitClass = null;
        string? relayEndpoint = null;
        string? meterFamily = null;
        ulong? connectedAtMilliseconds = null;
        ulong? disconnectedAtMilliseconds = null;
        ulong? durationMilliseconds = null;
        ulong? bytesIn = null;
        ulong? bytesOut = null;
        string? status = null;
        string? receiptSource = null;
        string? quoteId = null;
        string? paymentTransactionHash = null;
        string? feeAssetId = null;
        string? escrowAccountId = null;
        string? operatorAccountId = null;
        string? leaseFee = null;
        string? earnedFee = null;
        string? refundedFee = null;
        string? leaseIdHex = null;
        ToriiVpnTxInstruction? settleLeaseInstruction = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var receipt = ToriiVpnJson.CreateWithDirectMetadataContext(() => new ToriiVpnReceipt
                {
                    SessionId = ToriiVpnJson.RequireString(sessionId, $"{context}.session_id"),
                    AccountId = ToriiVpnJson.RequireString(accountId, $"{context}.account_id"),
                    ExitClass = ToriiVpnJson.RequireString(exitClass, $"{context}.exit_class"),
                    RelayEndpoint = ToriiVpnJson.RequireString(relayEndpoint, $"{context}.relay_endpoint"),
                    MeterFamily = ToriiVpnJson.RequireString(meterFamily, $"{context}.meter_family"),
                    ConnectedAtMilliseconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        connectedAtMilliseconds,
                        context,
                        "connected_at_ms"),
                    DisconnectedAtMilliseconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        disconnectedAtMilliseconds,
                        context,
                        "disconnected_at_ms"),
                    DurationMilliseconds = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(
                        durationMilliseconds,
                        context,
                        "duration_ms"),
                    BytesIn = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(bytesIn, context, "bytes_in"),
                    BytesOut = ToriiVpnReceiptListResponseJsonConverter.RequireUInt64(bytesOut, context, "bytes_out"),
                    Status = ToriiVpnJson.RequireString(status, $"{context}.status"),
                    ReceiptSource = ToriiVpnJson.RequireString(receiptSource, $"{context}.receipt_source"),
                    QuoteId = ToriiVpnJson.RequireString(quoteId, $"{context}.quote_id"),
                    PaymentTransactionHash = ToriiVpnJson.RequireString(
                        paymentTransactionHash,
                        $"{context}.payment_tx_hash"),
                    FeeAssetId = ToriiVpnJson.RequireString(feeAssetId, $"{context}.fee_asset_id"),
                    EscrowAccountId = ToriiVpnJson.RequireString(escrowAccountId, $"{context}.escrow_account_id"),
                    OperatorAccountId = ToriiVpnJson.RequireString(operatorAccountId, $"{context}.operator_account_id"),
                    LeaseFee = ToriiQuantityJson.RequireCanonicalQuantity(
                        ToriiVpnJson.RequireString(leaseFee, $"{context}.lease_fee"),
                        $"{context}.lease_fee"),
                    EarnedFee = ToriiQuantityJson.RequireCanonicalQuantity(
                        ToriiVpnJson.RequireString(earnedFee, $"{context}.earned_fee"),
                        $"{context}.earned_fee"),
                    RefundedFee = ToriiQuantityJson.RequireCanonicalQuantity(
                        ToriiVpnJson.RequireString(refundedFee, $"{context}.refunded_fee"),
                        $"{context}.refunded_fee"),
                    LeaseIdHex = ToriiVpnJson.RequireString(leaseIdHex, $"{context}.lease_id_hex"),
                    SettleLeaseInstruction = settleLeaseInstruction,
                }, context);
                ToriiVpnJson.ValidateVpnReceipt(receipt, context);
                ToriiVpnJson.RequireRequiredProperties(seen, context, RequiredPropertyNames);
                return receipt;
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "session_id":
                    sessionId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.session_id");
                    break;
                case "account_id":
                    accountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.account_id");
                    break;
                case "exit_class":
                    exitClass = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.exit_class");
                    break;
                case "relay_endpoint":
                    relayEndpoint = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.relay_endpoint");
                    break;
                case "meter_family":
                    meterFamily = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.meter_family");
                    break;
                case "connected_at_ms":
                    connectedAtMilliseconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.connected_at_ms");
                    break;
                case "disconnected_at_ms":
                    disconnectedAtMilliseconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.disconnected_at_ms");
                    break;
                case "duration_ms":
                    durationMilliseconds = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.duration_ms");
                    break;
                case "bytes_in":
                    bytesIn = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.bytes_in");
                    break;
                case "bytes_out":
                    bytesOut = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.bytes_out");
                    break;
                case "status":
                    status = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.status");
                    break;
                case "receipt_source":
                    receiptSource = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.receipt_source");
                    break;
                case "quote_id":
                    quoteId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.quote_id");
                    break;
                case "payment_tx_hash":
                    paymentTransactionHash = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.payment_tx_hash");
                    break;
                case "fee_asset_id":
                    feeAssetId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.fee_asset_id");
                    break;
                case "escrow_account_id":
                    escrowAccountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.escrow_account_id");
                    break;
                case "operator_account_id":
                    operatorAccountId = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.operator_account_id");
                    break;
                case "lease_fee":
                    leaseFee = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.lease_fee");
                    break;
                case "earned_fee":
                    earnedFee = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.earned_fee");
                    break;
                case "refunded_fee":
                    refundedFee = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.refunded_fee");
                    break;
                case "lease_id_hex":
                    leaseIdHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, $"{context}.lease_id_hex");
                    break;
                case "settle_lease_instruction":
                    settleLeaseInstruction = ToriiVpnJson.ReadOptionalVpnTxInstruction(ref reader, $"{context}.settle_lease_instruction");
                    break;
                default:
                    throw new JsonException($"{context}.{propertyName} is not allowed.");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiVpnReceipt value,
        JsonSerializerOptions options)
    {
        WriteReceipt(writer, value, "vpn receipt");
    }

    internal static void WriteReceipt(Utf8JsonWriter writer, ToriiVpnReceipt value, string context)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiVpnJson.ValidateVpnReceipt(value, context);

        writer.WriteStartObject();
        writer.WriteString("session_id", value.SessionId);
        writer.WriteString("account_id", value.AccountId);
        writer.WriteString("exit_class", value.ExitClass);
        writer.WriteString("relay_endpoint", value.RelayEndpoint);
        writer.WriteString("meter_family", value.MeterFamily);
        writer.WriteNumber("connected_at_ms", value.ConnectedAtMilliseconds);
        writer.WriteNumber("disconnected_at_ms", value.DisconnectedAtMilliseconds);
        writer.WriteNumber("duration_ms", value.DurationMilliseconds);
        writer.WriteNumber("bytes_in", value.BytesIn);
        writer.WriteNumber("bytes_out", value.BytesOut);
        writer.WriteString("status", value.Status);
        writer.WriteString("receipt_source", value.ReceiptSource);
        writer.WriteString("quote_id", value.QuoteId);
        writer.WriteString("payment_tx_hash", value.PaymentTransactionHash);
        writer.WriteString("fee_asset_id", value.FeeAssetId);
        writer.WriteString("escrow_account_id", value.EscrowAccountId);
        writer.WriteString("operator_account_id", value.OperatorAccountId);
        writer.WriteString("lease_fee", value.LeaseFee);
        writer.WriteString("earned_fee", value.EarnedFee);
        writer.WriteString("refunded_fee", value.RefundedFee);
        writer.WriteString("lease_id_hex", value.LeaseIdHex);
        ToriiVpnJson.WriteOptionalVpnTxInstruction(
            writer,
            "settle_lease_instruction",
            value.SettleLeaseInstruction,
            $"{context}.settle_lease_instruction");
        writer.WriteEndObject();
    }
}

internal sealed class ToriiVpnReceiptListResponseJsonConverter : JsonConverter<ToriiVpnReceiptListResponse>
{
    private static readonly string[] RequiredPropertyNames = ["items", "total"];

    public override bool HandleNull => true;

    public override ToriiVpnReceiptListResponse Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        const string context = "vpn receipt list response";
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException($"{context} must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{context} must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        IReadOnlyList<ToriiVpnReceipt>? items = null;
        ulong? total = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                var response = ToriiVpnJson.CreateWithDirectMetadataContext(() => new ToriiVpnReceiptListResponse
                {
                    Items = ToriiVpnJson.RequireList(items, context, "items"),
                    Total = RequireTotal(total, context),
                }, context);
                ToriiVpnJson.ValidateVpnReceiptListResponse(response, context);
                ToriiVpnJson.RequireRequiredProperties(seen, context, RequiredPropertyNames);
                return response;
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{context} property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException($"{context} property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, context);
            if (!reader.Read())
            {
                throw new JsonException($"{context}.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "items":
                    items = ReadReceiptItems(ref reader, $"{context}.items");
                    break;
                case "total":
                    total = ToriiAccountFaucetJson.ReadUInt64(ref reader, $"{context}.total");
                    break;
                default:
                    throw new JsonException($"{context}.{propertyName} is not allowed.");
            }
        }

        throw new JsonException($"{context} JSON object is incomplete.");
    }

    internal static ulong RequireUInt64(ulong? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    private static ulong RequireTotal(ulong? value, string context)
    {
        return RequireUInt64(value, context, "total");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiVpnReceiptListResponse value,
        JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        ToriiVpnJson.ValidateVpnReceiptListResponse(value, "vpn receipt list response");

        writer.WriteStartObject();
        if (value.Items is null)
        {
            writer.WriteNull("items");
        }
        else
        {
            writer.WriteStartArray("items");
            for (var index = 0; index < value.Items.Count; index++)
            {
                ToriiVpnReceiptJsonConverter.WriteReceipt(
                    writer,
                    value.Items[index],
                    $"vpn receipt list response.items[{index}]");
            }

            writer.WriteEndArray();
        }

        writer.WriteNumber("total", value.Total);
        writer.WriteEndObject();
    }

    private static IReadOnlyList<ToriiVpnReceipt>? ReadReceiptItems(ref Utf8JsonReader reader, string context)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            return null;
        }

        if (reader.TokenType != JsonTokenType.StartArray)
        {
            throw new JsonException($"{context} must be an array.");
        }

        var items = new List<ToriiVpnReceipt>();
        var index = 0;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndArray)
            {
                return items;
            }

            items.Add(ToriiVpnReceiptJsonConverter.ReadReceipt(ref reader, $"{context}[{index}]"));
            index++;
        }

        throw new JsonException($"{context} JSON array is incomplete.");
    }
}
