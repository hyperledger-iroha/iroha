using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Offline;

public sealed class OfflineNotePaymentToken
{
    private readonly byte[] tokenNonce;
    private readonly byte[] tokenId;
    private readonly byte[] auditNorito;
    private readonly List<byte[]> bearerAuditTrailNorito;
    private readonly HashSet<string> outputRecipientAccountIds;

    public OfflineNotePaymentToken(
        string chainId,
        string paymentRequestId,
        ulong createdAtMs,
        byte[] tokenNonce,
        byte[] tokenId,
        byte[] auditNorito,
        IReadOnlyList<byte[]>? bearerAuditTrailNorito = null)
    {
        ChainId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(
            chainId,
            "chain_id",
            nameof(chainId));
        PaymentRequestId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(
            paymentRequestId,
            "payment_request_id",
            nameof(paymentRequestId));
        if (createdAtMs == 0)
        {
            throw new ArgumentException("created_at_ms must be positive.", nameof(createdAtMs));
        }

        CreatedAtMs = createdAtMs;
        this.tokenNonce = OfflineNoteCanonicalPayloadCodec.RequireRandomBytes(
            tokenNonce,
            "token_nonce",
            nameof(tokenNonce));
        this.tokenId = OfflineNoteCanonicalPayloadCodec.RequireHash(tokenId, "token_id", nameof(tokenId));
        ArgumentNullException.ThrowIfNull(auditNorito);
        var auditSummary = OfflineNotePaymentTokenCodec.DecodeAuditSummary(auditNorito);
        if (!auditSummary.TokenId.SequenceEqual(this.tokenId))
        {
            throw new ArgumentException(
                "Offline Note payment token id does not match audit bundle.",
                nameof(auditNorito));
        }

        this.auditNorito = auditNorito.ToArray();
        var trailInput = bearerAuditTrailNorito ?? new[] { auditNorito };
        if (trailInput.Count == 0)
        {
            throw new ArgumentException("bearer_audit_trail must not be empty.", nameof(bearerAuditTrailNorito));
        }

        this.bearerAuditTrailNorito = new List<byte[]>(trailInput.Count);
        for (var index = 0; index < trailInput.Count; index++)
        {
            ArgumentNullException.ThrowIfNull(trailInput[index], nameof(bearerAuditTrailNorito));
            _ = OfflineNotePaymentTokenCodec.DecodeAuditSummary(trailInput[index]);
            this.bearerAuditTrailNorito.Add(trailInput[index].ToArray());
        }

        if (!this.bearerAuditTrailNorito[^1].SequenceEqual(this.auditNorito))
        {
            throw new ArgumentException(
                "bearer_audit_trail must end with the payment token audit bundle.",
                nameof(bearerAuditTrailNorito));
        }

        outputRecipientAccountIds = auditSummary.OutputRecipientAccountIds.ToHashSet(StringComparer.Ordinal);
    }

    public string ChainId { get; }

    public string PaymentRequestId { get; }

    public ulong CreatedAtMs { get; }

    public byte[] TokenNonce => tokenNonce.ToArray();

    public byte[] TokenId => tokenId.ToArray();

    public string TokenIdHex => Convert.ToHexString(tokenId).ToLowerInvariant();

    public byte[] AuditNorito => auditNorito.ToArray();

    public IReadOnlyList<byte[]> BearerAuditTrailNorito =>
        bearerAuditTrailNorito.Select(static value => value.ToArray()).ToArray();

    public bool ContainsRecipientAccountId(string recipientAccountId)
    {
        var exact = OfflineNoteCanonicalPayloadCodec.CanonicalAccountId(
            recipientAccountId,
            "recipient_account_id",
            nameof(recipientAccountId));
        return outputRecipientAccountIds.Contains(exact);
    }
}

public static class OfflineNotePaymentTokenCodec
{
    public const string Type = "offline_payment_token";
    public const string TextPrefix = "wallet-offline-bearer-cash-payment:";
    public const ulong EnvelopeVersion = 2;
    public const string EnvelopeTypeName =
        "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelope";
    public const string AuditBundleTypeName =
        "iroha_data_model::offline::model::OfflineNoteAuditBundle";
    public const string KeyCertificateTypeName =
        "iroha_data_model::offline::model::OfflineNoteKeyCertificate";

    private const byte CompactLenFlag = 0x02;
    private const string RecursiveBackend = "halo2/ipa";
    private const string RecursiveVerifierName = "offline-note-recursive";

    public static byte[] EncodeNorito(OfflineNotePaymentToken token)
    {
        ArgumentNullException.ThrowIfNull(token);

        using var writer = new MemoryStream();
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteUInt64LittleEndian(child, EnvelopeVersion));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, token.ChainId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteString(child, token.PaymentRequestId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteUInt64LittleEndian(child, token.CreatedAtMs));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteBytesVec(child, token.TokenNonce));
        OfflineNoteCanonicalPayloadCodec.WriteField(writer, child => child.Write(token.TokenId));
        OfflineNoteCanonicalPayloadCodec.WriteField(
            writer,
            child => OfflineNoteCanonicalPayloadCodec.WriteBytesVec(child, token.AuditNorito));
        OfflineNoteCanonicalPayloadCodec.WriteField(writer, child =>
        {
            OfflineNoteCanonicalPayloadCodec.WriteVec(
                child,
                token.BearerAuditTrailNorito,
                (element, value) => OfflineNoteCanonicalPayloadCodec.WriteBytesVec(element, value));
        });
        return NoritoCodec.Encode(EnvelopeTypeName, writer.ToArray(), CompactLenFlag);
    }

    public static OfflineNotePaymentToken DecodeNorito(byte[] payload)
    {
        ArgumentNullException.ThrowIfNull(payload);

        var framePayload = OfflineNoteCanonicalPayloadCodec.DecodeArchivePayload(payload, EnvelopeTypeName);
        var offset = 0;
        var version = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "version",
            OfflineNoteCanonicalPayloadCodec.ReadUInt64LittleEndian);
        if (version != EnvelopeVersion)
        {
            throw OfflineNoteCanonicalPayloadCodec.InvalidField("version");
        }

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
        var createdAtMs = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "created_at_ms",
            OfflineNoteCanonicalPayloadCodec.ReadUInt64LittleEndian);
        var tokenNonce = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "token_nonce",
            OfflineNoteCanonicalPayloadCodec.ReadBytesVec);
        var tokenId = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "token_id",
            (byte[] fieldPayload, ref int fieldOffset) =>
                OfflineNoteCanonicalPayloadCodec.ReadHash(fieldPayload, ref fieldOffset, "token_id"));
        var auditNorito = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "audit",
            OfflineNoteCanonicalPayloadCodec.ReadBytesVec);
        var bearerAuditTrailNorito = OfflineNoteCanonicalPayloadCodec.ReadField(
            framePayload,
            ref offset,
            "bearer_audit_trail",
            ReadAuditTrail);
        OfflineNoteCanonicalPayloadCodec.RequireNoTrailing(framePayload, offset, "payment_token");

        return new OfflineNotePaymentToken(
            chainId,
            paymentRequestId,
            createdAtMs,
            tokenNonce,
            tokenId,
            auditNorito,
            bearerAuditTrailNorito);
    }

    public static byte[] EncodeJson(OfflineNotePaymentToken token)
    {
        return EncodeNorito(token);
    }

    public static OfflineNotePaymentToken DecodeJson(byte[] payload)
    {
        return DecodeNorito(payload);
    }

    public static string EncodeText(OfflineNotePaymentToken token)
    {
        return TextPrefix + Base64UrlEncode(EncodeNorito(token));
    }

    public static OfflineNotePaymentToken DecodeText(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (!string.Equals(text.Trim(), text, StringComparison.Ordinal) || text.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Offline Note payment token text must be exact.", nameof(text));
        }

        if (!text.StartsWith(TextPrefix, StringComparison.Ordinal))
        {
            throw new ArgumentException("Offline Note payment token prefix missing.", nameof(text));
        }

        return DecodeNorito(Base64UrlDecode(text[TextPrefix.Length..]));
    }

    public static OfflineNotePaymentToken DecodeQrPayload(byte[] payload)
    {
        return DecodeNorito(payload);
    }

    internal static AuditBundleSummary DecodeAuditSummary(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = OfflineNoteCanonicalPayloadCodec.DecodeArchivePayload(archive, AuditBundleTypeName);
        var offset = 0;
        var tokenId = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "token_id",
            (byte[] fieldPayload, ref int fieldOffset) =>
                OfflineNoteCanonicalPayloadCodec.ReadHash(fieldPayload, ref fieldOffset, "token_id"));
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "sender_key_certificate", ReadKeyCertificate);
        var inputNullifiers = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "input_nullifiers",
            (byte[] fieldPayload, ref int fieldOffset) => OfflineNoteCanonicalPayloadCodec.ReadVec(
                fieldPayload,
                ref fieldOffset,
                "input_nullifiers",
                (byte[] child, ref int childOffset) =>
                    OfflineNoteCanonicalPayloadCodec.ReadHash(child, ref childOffset, "input_nullifier")));
        var inputClaims = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "input_claims",
            (byte[] fieldPayload, ref int fieldOffset) => OfflineNoteCanonicalPayloadCodec.ReadVec(
                fieldPayload,
                ref fieldOffset,
                "input_claims",
                ReadIssuedClaim));
        var outputCommitments = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "output_commitments",
            (byte[] fieldPayload, ref int fieldOffset) => OfflineNoteCanonicalPayloadCodec.ReadVec(
                fieldPayload,
                ref fieldOffset,
                "output_commitments",
                (byte[] child, ref int childOffset) =>
                    OfflineNoteCanonicalPayloadCodec.ReadHash(child, ref childOffset, "output_commitment")));
        var outputClaims = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "output_claims",
            (byte[] fieldPayload, ref int fieldOffset) => OfflineNoteCanonicalPayloadCodec.ReadVec(
                fieldPayload,
                ref fieldOffset,
                "output_claims",
                ReadAuditOutputClaim));
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "recursive_proof", ReadRecursiveProof);
        OfflineNoteCanonicalPayloadCodec.RequireNoTrailing(payload, offset, "audit_bundle");

        if (inputNullifiers.Count == 0 || inputClaims.Count == 0 || inputNullifiers.Count != inputClaims.Count)
        {
            throw OfflineNoteCanonicalPayloadCodec.InvalidField("input_claims");
        }

        if (outputCommitments.Count == 0 || outputClaims.Count == 0 || outputCommitments.Count != outputClaims.Count)
        {
            throw OfflineNoteCanonicalPayloadCodec.InvalidField("output_claims");
        }

        for (var index = 0; index < outputCommitments.Count; index++)
        {
            if (!outputCommitments[index].SequenceEqual(outputClaims[index].NoteCommitment))
            {
                throw OfflineNoteCanonicalPayloadCodec.InvalidField("output_claims");
            }
        }

        return new AuditBundleSummary(
            tokenId,
            outputClaims.Select(static claim => claim.AccountId).ToArray());
    }

    internal static string DecodeKeyCertificateAccountId(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = OfflineNoteCanonicalPayloadCodec.DecodeArchivePayload(archive, KeyCertificateTypeName);
        var offset = 0;
        var accountId = ReadKeyCertificate(payload, ref offset);
        OfflineNoteCanonicalPayloadCodec.RequireNoTrailing(payload, offset, "key_certificate");
        return accountId;
    }

    private static List<byte[]> ReadAuditTrail(byte[] payload, ref int offset)
    {
        return OfflineNoteCanonicalPayloadCodec.ReadVec(
            payload,
            ref offset,
            "bearer_audit_trail",
            (byte[] child, ref int childOffset) =>
            {
                var audit = OfflineNoteCanonicalPayloadCodec.ReadBytesVec(child, ref childOffset);
                _ = DecodeAuditSummary(audit);
                return audit;
            });
    }

    private static string ReadKeyCertificate(byte[] payload, ref int offset)
    {
        var version = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.version",
            OfflineNoteCanonicalPayloadCodec.ReadUInt16LittleEndian);
        if (version != OfflineNoteCanonicalPayloadCodec.KeyCertificateVersion)
        {
            throw OfflineNoteCanonicalPayloadCodec.InvalidField("certificate.version");
        }

        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.platform",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.key_id",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.device_id",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        var accountId = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.account_id",
            OfflineNoteCanonicalPayloadCodec.ReadAccountId);
        var publicKey = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.public_key",
            OfflineNoteCanonicalPayloadCodec.ReadBytesVec);
        if (publicKey.Length != OfflineNoteCanonicalPayloadCodec.HashLength)
        {
            throw OfflineNoteCanonicalPayloadCodec.InvalidField("certificate.public_key");
        }

        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.assertion_scheme",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.assertion_key_algorithm",
            OfflineNoteCanonicalPayloadCodec.ReadString);
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.assertion_public_key",
            OfflineNoteCanonicalPayloadCodec.ReadBytesVec);
        var usageLimit = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.assertion_usage_count_limit",
            OfflineNoteCanonicalPayloadCodec.ReadOptionUInt32);
        var oneUse = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.one_use",
            OfflineNoteCanonicalPayloadCodec.ReadBool);
        var issuerSignature = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "certificate.issuer_signature",
            OfflineNoteCanonicalPayloadCodec.ReadConstVec);
        OfflineNoteCanonicalPayloadCodec.RequireNoTrailing(payload, offset, "certificate");
        if (!oneUse || (usageLimit.HasValue && usageLimit.Value != 1) || issuerSignature.Length != 64)
        {
            throw OfflineNoteCanonicalPayloadCodec.InvalidField("certificate");
        }

        return accountId;
    }

    private static OfflineNoteIssuedClaim ReadIssuedClaim(byte[] payload, ref int offset)
    {
        return new OfflineNoteIssuedClaim(
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "claim.domain", OfflineNoteCanonicalPayloadCodec.ReadString),
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "claim.note_commitment", (byte[] fieldPayload, ref int fieldOffset) =>
                OfflineNoteCanonicalPayloadCodec.ReadHash(fieldPayload, ref fieldOffset, "claim.note_commitment")),
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "claim.key_certificate_payload_hash", (byte[] fieldPayload, ref int fieldOffset) =>
                OfflineNoteCanonicalPayloadCodec.ReadHash(fieldPayload, ref fieldOffset, "claim.key_certificate_payload_hash")),
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "claim.asset_id", OfflineNoteCanonicalPayloadCodec.ReadAssetId),
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "claim.amount", OfflineNoteCanonicalPayloadCodec.ReadNumeric));
    }

    private static AuditOutputClaimSummary ReadAuditOutputClaim(byte[] payload, ref int offset)
    {
        var noteCommitment = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "audit_output_claim.note_commitment",
            (byte[] fieldPayload, ref int fieldOffset) =>
                OfflineNoteCanonicalPayloadCodec.ReadHash(fieldPayload, ref fieldOffset, "audit_output_claim.note_commitment"));
        var accountId = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "audit_output_claim.key_certificate",
            ReadKeyCertificate);
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "audit_output_claim.asset_id",
            OfflineNoteCanonicalPayloadCodec.ReadAssetId);
        _ = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "audit_output_claim.amount",
            OfflineNoteCanonicalPayloadCodec.ReadNumeric);
        return new AuditOutputClaimSummary(noteCommitment, accountId);
    }

    private static RecursiveProofSummary ReadRecursiveProof(byte[] payload, ref int offset)
    {
        var verifier = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "recursive_proof.verifier_key_id",
            ReadVerifyingKeyId);
        var publicInputsHash = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "recursive_proof.public_inputs_hash",
            (byte[] fieldPayload, ref int fieldOffset) =>
                OfflineNoteCanonicalPayloadCodec.ReadHash(fieldPayload, ref fieldOffset, "recursive_proof.public_inputs_hash"));
        var proof = OfflineNoteCanonicalPayloadCodec.ReadField(
            payload,
            ref offset,
            "recursive_proof.proof",
            ReadProofBox);
        if (!string.Equals(verifier.Backend, RecursiveBackend, StringComparison.Ordinal)
            || !string.Equals(verifier.Name, RecursiveVerifierName, StringComparison.Ordinal)
            || !string.Equals(proof.Backend, RecursiveBackend, StringComparison.Ordinal)
            || proof.Bytes.Length == 0)
        {
            throw OfflineNoteCanonicalPayloadCodec.InvalidField("recursive_proof");
        }

        return new RecursiveProofSummary(publicInputsHash);
    }

    private static VerifyingKeyIdSummary ReadVerifyingKeyId(byte[] payload, ref int offset)
    {
        return new VerifyingKeyIdSummary(
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "verifier_key_id.backend", OfflineNoteCanonicalPayloadCodec.ReadString),
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "verifier_key_id.name", OfflineNoteCanonicalPayloadCodec.ReadString));
    }

    private static ProofBoxSummary ReadProofBox(byte[] payload, ref int offset)
    {
        return new ProofBoxSummary(
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "proof.backend", OfflineNoteCanonicalPayloadCodec.ReadString),
            OfflineNoteCanonicalPayloadCodec.ReadField(payload, ref offset, "proof.bytes", OfflineNoteCanonicalPayloadCodec.ReadBytesVec));
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
            throw new ArgumentException("Offline Note payment token payload is invalid.", nameof(value));
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
            throw new ArgumentException("Offline Note payment token payload is invalid.", nameof(value), exception);
        }

        if (!string.Equals(Base64UrlEncode(decoded), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Offline Note payment token payload is invalid.", nameof(value));
        }

        return decoded;
    }

    internal sealed record AuditBundleSummary(
        byte[] TokenId,
        IReadOnlyList<string> OutputRecipientAccountIds);

    private sealed record AuditOutputClaimSummary(byte[] NoteCommitment, string AccountId);

    private sealed record VerifyingKeyIdSummary(string Backend, string Name);

    private sealed record ProofBoxSummary(string Backend, byte[] Bytes);

    private sealed record RecursiveProofSummary(byte[] PublicInputsHash);
}
