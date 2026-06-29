using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Offline;

public sealed class OfflineNoteReceiveRequest
{
    private readonly byte[] keyCertificateNorito;
    private readonly byte[] outputCommitment;

    public OfflineNoteReceiveRequest(
        string chainId,
        string paymentRequestId,
        string accountId,
        string assetDefinitionId,
        string assetId,
        string amount,
        byte[] keyCertificateNorito,
        byte[] outputCommitment)
    {
        ChainId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(
            chainId,
            "chain_id",
            nameof(chainId));
        PaymentRequestId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(
            paymentRequestId,
            "payment_request_id",
            nameof(paymentRequestId));
        AccountId = OfflineNoteCanonicalPayloadCodec.CanonicalAccountId(accountId);
        AssetId = OfflineNoteCanonicalPayloadCodec.CanonicalAssetId(assetId);
        AssetDefinitionId = CanonicalAssetDefinitionId(assetDefinitionId, AccountId);
        var parsedAssetId = AssetId.Split('#', StringSplitOptions.None);
        if (!string.Equals(parsedAssetId[0], AssetDefinitionId, StringComparison.Ordinal)
            || !string.Equals(parsedAssetId[1], AccountId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "asset_id must match asset_definition_id and account_id.",
                nameof(assetId));
        }

        Amount = OfflineNoteCanonicalPayloadCodec.ParseCanonicalNumeric(amount);
        ArgumentNullException.ThrowIfNull(keyCertificateNorito);
        var certificateAccountId = OfflineNotePaymentTokenCodec.DecodeKeyCertificateAccountId(keyCertificateNorito);
        if (!string.Equals(certificateAccountId, AccountId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "key_certificate account_id must match receive request account_id.",
                nameof(keyCertificateNorito));
        }

        this.keyCertificateNorito = keyCertificateNorito.ToArray();
        this.outputCommitment = OfflineNoteCanonicalPayloadCodec.RequireHash(
            outputCommitment,
            "output_commitment",
            nameof(outputCommitment));
    }

    public string ChainId { get; }

    public string PaymentRequestId { get; }

    public string AccountId { get; }

    public string AssetDefinitionId { get; }

    public string AssetId { get; }

    public string Amount { get; }

    public byte[] KeyCertificateNorito => keyCertificateNorito.ToArray();

    public byte[] OutputCommitment => outputCommitment.ToArray();

    public string OutputCommitmentHex => Convert.ToHexString(outputCommitment).ToLowerInvariant();

    private static string CanonicalAssetDefinitionId(string assetDefinitionId, string accountId)
    {
        var exact = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(
            assetDefinitionId,
            "asset_definition_id",
            nameof(assetDefinitionId));
        if (exact.Contains('#', StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "asset_definition_id must not include an account or scope separator.",
                nameof(assetDefinitionId));
        }

        return OfflineNoteCanonicalPayloadCodec.CanonicalAssetId(exact + "#" + accountId)
            .Split('#', StringSplitOptions.None)[0];
    }
}

public static class OfflineNoteReceiveRequestCodec
{
    public const string Type = "offline_receive_request";
    public const string TextPrefix = "wallet-offline-bearer-cash-receive:";
    public const string EnvelopeTypeName =
        "iroha_data_model::offline::model::OfflineNoteReceiveRequestEnvelope";

    private const byte CompactLenFlag = 0x02;

    public static byte[] EncodeNorito(OfflineNoteReceiveRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        using var writer = new MemoryStream();
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, request.ChainId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, request.PaymentRequestId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, request.AccountId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, request.AssetDefinitionId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, request.AssetId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, request.Amount));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteBytesVec(child, request.KeyCertificateNorito));
        OfflineNoteCanonicalPayloadCodec.WriteField(writer, child => child.Write(request.OutputCommitment));
        return NoritoCodec.Encode(EnvelopeTypeName, writer.ToArray(), CompactLenFlag);
    }

    public static OfflineNoteReceiveRequest DecodeNorito(byte[] payload)
    {
        ArgumentNullException.ThrowIfNull(payload);

        var framePayload = OfflineNoteCanonicalPayloadCodec.DecodeArchivePayload(payload, EnvelopeTypeName);
        var offset = 0;
        var chainId = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "chain_id",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        var paymentRequestId = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "payment_request_id",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        var accountId = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "account_id",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        var assetDefinitionId = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "asset_definition_id",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        var assetId = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "asset_id",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        var amount = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "amount",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        var keyCertificateNorito = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "key_certificate",
            OfflineNoteCanonicalPayloadCodec.ReadBytesVec);
        var outputCommitment = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "output_commitment",
            (byte[] fieldPayload, ref int fieldOffset) =>
                OfflineNoteCanonicalPayloadCodec.ReadHash(fieldPayload, ref fieldOffset, "output_commitment"));
        OfflineNoteCanonicalPayloadCodec.RequireNoTrailing(framePayload, offset, "receive_request");

        return new OfflineNoteReceiveRequest(
            chainId,
            paymentRequestId,
            accountId,
            assetDefinitionId,
            assetId,
            amount,
            keyCertificateNorito,
            outputCommitment);
    }

    public static byte[] EncodeJson(OfflineNoteReceiveRequest request)
    {
        return EncodeNorito(request);
    }

    public static OfflineNoteReceiveRequest DecodeJson(byte[] payload)
    {
        return DecodeNorito(payload);
    }

    public static string EncodeText(OfflineNoteReceiveRequest request)
    {
        return TextPrefix + Base64UrlEncode(EncodeNorito(request));
    }

    public static OfflineNoteReceiveRequest DecodeText(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (!string.Equals(text.Trim(), text, StringComparison.Ordinal) || text.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Offline Note receive request text must be exact.", nameof(text));
        }

        if (!text.StartsWith(TextPrefix, StringComparison.Ordinal))
        {
            throw new ArgumentException("Offline Note receive request prefix missing.", nameof(text));
        }

        return DecodeNorito(Base64UrlDecode(text[TextPrefix.Length..]));
    }

    public static OfflineNoteReceiveRequest DecodeQrPayload(byte[] payload)
    {
        return DecodeNorito(payload);
    }

    private static string Base64UrlEncode(byte[] payload)
    {
        return Convert.ToBase64String(payload).TrimEnd('=').Replace('+', '-').Replace('/', '_');
    }

    private static byte[] Base64UrlDecode(string value)
    {
        if (value.Trim().Length == 0
            || value.Contains('=')
            || value.Any(static ch => !((ch is >= 'A' and <= 'Z')
                || (ch is >= 'a' and <= 'z')
                || (ch is >= '0' and <= '9')
                || ch == '-'
                || ch == '_')))
        {
            throw new ArgumentException("Offline Note receive request payload is invalid.", nameof(value));
        }

        var normalized = value.Replace('-', '+').Replace('_', '/');
        normalized = normalized.PadRight(normalized.Length + ((4 - (normalized.Length % 4)) % 4), '=');
        byte[] decoded;
        try
        {
            decoded = Convert.FromBase64String(normalized);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException(
                "Offline Note receive request payload is invalid.",
                nameof(value),
                exception);
        }

        if (!string.Equals(Base64UrlEncode(decoded), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Offline Note receive request payload is invalid.", nameof(value));
        }

        return decoded;
    }
}
