using System.Buffers.Binary;
using System.Numerics;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Offline;

public static class OfflineNoteCanonicalPayloadDomains
{
    public const string KeyCertificatePayload = "iroha:offline-note:key-certificate-payload";
    public const string IssuedClaim = "iroha:offline-note:issued-claim";
    public const string RedeemPublicInputs = "iroha:offline-note:redeem-public-inputs";
    public const string AuditPublicInputs = "iroha:offline-note:audit-public-inputs";
    public const string NoteCommitment = "iroha:offline-note:note-commitment";
    public const string InputNullifier = "iroha:offline-note:input-nullifier";
    public const string PaymentTokenId = "iroha:offline-note:payment-token-id";
}

public sealed class OfflineNoteKeyCertificatePayload
{
    private readonly byte[] publicKey;
    private readonly byte[] assertionPublicKey;

    public OfflineNoteKeyCertificatePayload(
        ushort version,
        string platform,
        string keyId,
        string deviceId,
        string accountId,
        byte[] publicKey,
        string assertionScheme,
        string assertionKeyAlgorithm,
        byte[] assertionPublicKey,
        uint? assertionUsageCountLimit,
        bool oneUse)
        : this(
            OfflineNoteCanonicalPayloadDomains.KeyCertificatePayload,
            version,
            platform,
            keyId,
            deviceId,
            accountId,
            publicKey,
            assertionScheme,
            assertionKeyAlgorithm,
            assertionPublicKey,
            assertionUsageCountLimit,
            oneUse)
    {
    }

    public OfflineNoteKeyCertificatePayload(
        string domain,
        ushort version,
        string platform,
        string keyId,
        string deviceId,
        string accountId,
        byte[] publicKey,
        string assertionScheme,
        string assertionKeyAlgorithm,
        byte[] assertionPublicKey,
        uint? assertionUsageCountLimit,
        bool oneUse)
    {
        Domain = OfflineNoteCanonicalPayloadCodec.RequireDomain(
            domain,
            OfflineNoteCanonicalPayloadDomains.KeyCertificatePayload,
            nameof(domain));
        if (version != OfflineNoteCanonicalPayloadCodec.KeyCertificateVersion)
        {
            throw new ArgumentException("Offline Note key certificate format is unsupported.", nameof(version));
        }

        if (!oneUse || (assertionUsageCountLimit.HasValue && assertionUsageCountLimit.Value != 1))
        {
            throw new ArgumentException(
                "Offline Note key certificate must be one-use with usage limit 1 when present.",
                nameof(oneUse));
        }

        Platform = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(platform, "platform", nameof(platform));
        KeyId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(keyId, "key_id", nameof(keyId));
        DeviceId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(deviceId, "device_id", nameof(deviceId));
        AccountId = OfflineNoteCanonicalPayloadCodec.CanonicalAccountId(accountId);
        ArgumentNullException.ThrowIfNull(publicKey);
        if (publicKey.Length != OfflineNoteCanonicalPayloadCodec.HashLength)
        {
            throw new ArgumentException("public_key must be 32 bytes.", nameof(publicKey));
        }

        this.publicKey = publicKey.ToArray();
        AssertionScheme = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(
            assertionScheme,
            "assertion_scheme",
            nameof(assertionScheme));
        AssertionKeyAlgorithm = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(
            assertionKeyAlgorithm,
            "assertion_key_algorithm",
            nameof(assertionKeyAlgorithm));
        ArgumentNullException.ThrowIfNull(assertionPublicKey);
        this.assertionPublicKey = assertionPublicKey.ToArray();
        AssertionUsageCountLimit = assertionUsageCountLimit;
        OneUse = oneUse;
        Version = version;
    }

    public string Domain { get; }

    public ushort Version { get; }

    public string Platform { get; }

    public string KeyId { get; }

    public string DeviceId { get; }

    public string AccountId { get; }

    public byte[] PublicKey => publicKey.ToArray();

    public string AssertionScheme { get; }

    public string AssertionKeyAlgorithm { get; }

    public byte[] AssertionPublicKey => assertionPublicKey.ToArray();

    public uint? AssertionUsageCountLimit { get; }

    public bool OneUse { get; }
}

public sealed class OfflineNoteIssuedClaim
{
    private readonly byte[] noteCommitment;
    private readonly byte[] keyCertificatePayloadHash;

    public OfflineNoteIssuedClaim(
        byte[] noteCommitment,
        byte[] keyCertificatePayloadHash,
        string assetId,
        string amount)
        : this(
            OfflineNoteCanonicalPayloadDomains.IssuedClaim,
            noteCommitment,
            keyCertificatePayloadHash,
            assetId,
            amount)
    {
    }

    public OfflineNoteIssuedClaim(
        string domain,
        byte[] noteCommitment,
        byte[] keyCertificatePayloadHash,
        string assetId,
        string amount)
    {
        Domain = OfflineNoteCanonicalPayloadCodec.RequireDomain(
            domain,
            OfflineNoteCanonicalPayloadDomains.IssuedClaim,
            nameof(domain));
        this.noteCommitment = OfflineNoteCanonicalPayloadCodec.RequireHash(
            noteCommitment,
            "note_commitment",
            nameof(noteCommitment));
        this.keyCertificatePayloadHash = OfflineNoteCanonicalPayloadCodec.RequireHash(
            keyCertificatePayloadHash,
            "key_certificate_payload_hash",
            nameof(keyCertificatePayloadHash));
        AssetId = OfflineNoteCanonicalPayloadCodec.CanonicalAssetId(assetId);
        Amount = OfflineNoteCanonicalPayloadCodec.ParseCanonicalNumeric(amount);
    }

    public string Domain { get; }

    public byte[] NoteCommitment => noteCommitment.ToArray();

    public byte[] KeyCertificatePayloadHash => keyCertificatePayloadHash.ToArray();

    public string AssetId { get; }

    public string Amount { get; }
}

public sealed class OfflineNoteRedeemPublicInputs
{
    private readonly byte[] sourceNoteCommitment;
    private readonly List<byte[]> inputNullifiers;
    private readonly byte[] keyCertificatePayloadHash;

    public OfflineNoteRedeemPublicInputs(
        byte[] sourceNoteCommitment,
        IReadOnlyList<byte[]> inputNullifiers,
        byte[] keyCertificatePayloadHash,
        string recipient,
        string assetId,
        string amount)
        : this(
            OfflineNoteCanonicalPayloadDomains.RedeemPublicInputs,
            sourceNoteCommitment,
            inputNullifiers,
            keyCertificatePayloadHash,
            recipient,
            assetId,
            amount)
    {
    }

    public OfflineNoteRedeemPublicInputs(
        string domain,
        byte[] sourceNoteCommitment,
        IReadOnlyList<byte[]> inputNullifiers,
        byte[] keyCertificatePayloadHash,
        string recipient,
        string assetId,
        string amount)
    {
        Domain = OfflineNoteCanonicalPayloadCodec.RequireDomain(
            domain,
            OfflineNoteCanonicalPayloadDomains.RedeemPublicInputs,
            nameof(domain));
        this.sourceNoteCommitment = OfflineNoteCanonicalPayloadCodec.RequireHash(
            sourceNoteCommitment,
            "source_note_commitment",
            nameof(sourceNoteCommitment));
        this.inputNullifiers = OfflineNoteCanonicalPayloadCodec.CopyHashList(
            inputNullifiers,
            "input_nullifiers",
            nameof(inputNullifiers));
        this.keyCertificatePayloadHash = OfflineNoteCanonicalPayloadCodec.RequireHash(
            keyCertificatePayloadHash,
            "key_certificate_payload_hash",
            nameof(keyCertificatePayloadHash));
        Recipient = OfflineNoteCanonicalPayloadCodec.CanonicalAccountId(recipient);
        AssetId = OfflineNoteCanonicalPayloadCodec.CanonicalAssetId(assetId);
        Amount = OfflineNoteCanonicalPayloadCodec.ParseCanonicalNumeric(amount);
    }

    public string Domain { get; }

    public byte[] SourceNoteCommitment => sourceNoteCommitment.ToArray();

    public IReadOnlyList<byte[]> InputNullifiers =>
        inputNullifiers.Select(static value => value.ToArray()).ToArray();

    public byte[] KeyCertificatePayloadHash => keyCertificatePayloadHash.ToArray();

    public string Recipient { get; }

    public string AssetId { get; }

    public string Amount { get; }
}

public sealed class OfflineNoteAuditPublicInputs
{
    private readonly byte[] tokenId;
    private readonly byte[] keyCertificatePayloadHash;
    private readonly List<byte[]> inputNullifiers;
    private readonly List<OfflineNoteIssuedClaim> inputClaims;
    private readonly List<byte[]> outputCommitments;
    private readonly List<OfflineNoteIssuedClaim> outputClaims;

    public OfflineNoteAuditPublicInputs(
        byte[] tokenId,
        byte[] keyCertificatePayloadHash,
        IReadOnlyList<byte[]> inputNullifiers,
        IReadOnlyList<OfflineNoteIssuedClaim> inputClaims,
        IReadOnlyList<byte[]> outputCommitments,
        IReadOnlyList<OfflineNoteIssuedClaim> outputClaims)
        : this(
            OfflineNoteCanonicalPayloadDomains.AuditPublicInputs,
            tokenId,
            keyCertificatePayloadHash,
            inputNullifiers,
            inputClaims,
            outputCommitments,
            outputClaims)
    {
    }

    public OfflineNoteAuditPublicInputs(
        string domain,
        byte[] tokenId,
        byte[] keyCertificatePayloadHash,
        IReadOnlyList<byte[]> inputNullifiers,
        IReadOnlyList<OfflineNoteIssuedClaim> inputClaims,
        IReadOnlyList<byte[]> outputCommitments,
        IReadOnlyList<OfflineNoteIssuedClaim> outputClaims)
    {
        Domain = OfflineNoteCanonicalPayloadCodec.RequireDomain(
            domain,
            OfflineNoteCanonicalPayloadDomains.AuditPublicInputs,
            nameof(domain));
        this.tokenId = OfflineNoteCanonicalPayloadCodec.RequireHash(tokenId, "token_id", nameof(tokenId));
        this.keyCertificatePayloadHash = OfflineNoteCanonicalPayloadCodec.RequireHash(
            keyCertificatePayloadHash,
            "key_certificate_payload_hash",
            nameof(keyCertificatePayloadHash));
        this.inputNullifiers = OfflineNoteCanonicalPayloadCodec.CopyHashList(
            inputNullifiers,
            "input_nullifiers",
            nameof(inputNullifiers));
        ArgumentNullException.ThrowIfNull(inputClaims);
        if (inputClaims.Count == 0)
        {
            throw new ArgumentException("input_claims must not be empty.", nameof(inputClaims));
        }

        if (inputClaims.Count != this.inputNullifiers.Count)
        {
            throw new ArgumentException(
                "input_nullifiers count must match input_claims count.",
                nameof(inputClaims));
        }

        this.inputClaims = inputClaims.Select(static claim =>
            claim ?? throw new ArgumentException("input_claims must not contain null values.")).ToList();
        this.outputCommitments = OfflineNoteCanonicalPayloadCodec.CopyHashList(
            outputCommitments,
            "output_commitments",
            nameof(outputCommitments));
        ArgumentNullException.ThrowIfNull(outputClaims);
        if (outputClaims.Count == 0)
        {
            throw new ArgumentException("output_claims must not be empty.", nameof(outputClaims));
        }

        if (outputClaims.Count != this.outputCommitments.Count)
        {
            throw new ArgumentException(
                "output_commitments count must match output_claims count.",
                nameof(outputClaims));
        }

        this.outputClaims = outputClaims.Select(static claim =>
            claim ?? throw new ArgumentException("output_claims must not contain null values.")).ToList();
        for (var index = 0; index < this.outputCommitments.Count; index++)
        {
            if (!this.outputCommitments[index].SequenceEqual(this.outputClaims[index].NoteCommitment))
            {
                throw new ArgumentException(
                    "audit output claims must be ordered one-to-one with output commitments.",
                    nameof(outputClaims));
            }
        }
    }

    public string Domain { get; }

    public byte[] TokenId => tokenId.ToArray();

    public byte[] KeyCertificatePayloadHash => keyCertificatePayloadHash.ToArray();

    public IReadOnlyList<byte[]> InputNullifiers =>
        inputNullifiers.Select(static value => value.ToArray()).ToArray();

    public IReadOnlyList<OfflineNoteIssuedClaim> InputClaims => inputClaims.ToArray();

    public IReadOnlyList<byte[]> OutputCommitments =>
        outputCommitments.Select(static value => value.ToArray()).ToArray();

    public IReadOnlyList<OfflineNoteIssuedClaim> OutputClaims => outputClaims.ToArray();
}

public sealed class OfflineNoteCommitmentPreimage
{
    private readonly byte[] ownerKeyCertificatePayloadHash;
    private readonly byte[] noteSecret;

    public OfflineNoteCommitmentPreimage(
        string chainId,
        byte[] ownerKeyCertificatePayloadHash,
        string assetId,
        string amount,
        byte[] noteSecret,
        OfflineNoteCommitmentOrigin origin)
        : this(
            OfflineNoteCanonicalPayloadDomains.NoteCommitment,
            chainId,
            ownerKeyCertificatePayloadHash,
            assetId,
            amount,
            noteSecret,
            origin)
    {
    }

    public OfflineNoteCommitmentPreimage(
        string domain,
        string chainId,
        byte[] ownerKeyCertificatePayloadHash,
        string assetId,
        string amount,
        byte[] noteSecret,
        OfflineNoteCommitmentOrigin origin)
    {
        Domain = OfflineNoteCanonicalPayloadCodec.RequireDomain(
            domain,
            OfflineNoteCanonicalPayloadDomains.NoteCommitment,
            nameof(domain));
        ChainId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(chainId, "chain_id", nameof(chainId));
        this.ownerKeyCertificatePayloadHash = OfflineNoteCanonicalPayloadCodec.RequireHash(
            ownerKeyCertificatePayloadHash,
            "owner_key_certificate_payload_hash",
            nameof(ownerKeyCertificatePayloadHash));
        AssetId = OfflineNoteCanonicalPayloadCodec.CanonicalAssetId(assetId);
        Amount = OfflineNoteCanonicalPayloadCodec.ParseCanonicalNumeric(amount);
        this.noteSecret = OfflineNoteCanonicalPayloadCodec.RequireRandomBytes(
            noteSecret,
            "note_secret",
            nameof(noteSecret));
        Origin = origin ?? throw new ArgumentNullException(nameof(origin));
    }

    public string Domain { get; }

    public string ChainId { get; }

    public byte[] OwnerKeyCertificatePayloadHash => ownerKeyCertificatePayloadHash.ToArray();

    public string AssetId { get; }

    public string Amount { get; }

    public byte[] NoteSecret => noteSecret.ToArray();

    public OfflineNoteCommitmentOrigin Origin { get; }

    public byte[] DeriveNoteCommitment()
    {
        return IrohaHash.Hash(OfflineNoteCanonicalPayloadCodec.EncodeNoteCommitmentPreimage(this));
    }
}

public sealed class OfflineNoteInputNullifierPreimage
{
    private readonly byte[] sourceNoteCommitment;
    private readonly byte[] ownerKeyCertificatePayloadHash;
    private readonly byte[] noteSecret;

    public OfflineNoteInputNullifierPreimage(
        string chainId,
        byte[] sourceNoteCommitment,
        byte[] ownerKeyCertificatePayloadHash,
        byte[] noteSecret)
        : this(
            OfflineNoteCanonicalPayloadDomains.InputNullifier,
            chainId,
            sourceNoteCommitment,
            ownerKeyCertificatePayloadHash,
            noteSecret)
    {
    }

    public OfflineNoteInputNullifierPreimage(
        string domain,
        string chainId,
        byte[] sourceNoteCommitment,
        byte[] ownerKeyCertificatePayloadHash,
        byte[] noteSecret)
    {
        Domain = OfflineNoteCanonicalPayloadCodec.RequireDomain(
            domain,
            OfflineNoteCanonicalPayloadDomains.InputNullifier,
            nameof(domain));
        ChainId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(chainId, "chain_id", nameof(chainId));
        this.sourceNoteCommitment = OfflineNoteCanonicalPayloadCodec.RequireHash(
            sourceNoteCommitment,
            "source_note_commitment",
            nameof(sourceNoteCommitment));
        this.ownerKeyCertificatePayloadHash = OfflineNoteCanonicalPayloadCodec.RequireHash(
            ownerKeyCertificatePayloadHash,
            "owner_key_certificate_payload_hash",
            nameof(ownerKeyCertificatePayloadHash));
        this.noteSecret = OfflineNoteCanonicalPayloadCodec.RequireRandomBytes(
            noteSecret,
            "note_secret",
            nameof(noteSecret));
    }

    public string Domain { get; }

    public string ChainId { get; }

    public byte[] SourceNoteCommitment => sourceNoteCommitment.ToArray();

    public byte[] OwnerKeyCertificatePayloadHash => ownerKeyCertificatePayloadHash.ToArray();

    public byte[] NoteSecret => noteSecret.ToArray();

    public byte[] DeriveInputNullifier()
    {
        return IrohaHash.Hash(OfflineNoteCanonicalPayloadCodec.EncodeInputNullifierPreimage(this));
    }
}

public sealed class OfflineNotePaymentTokenIdPreimage
{
    private readonly byte[] tokenNonce;
    private readonly byte[] senderKeyCertificatePayloadHash;
    private readonly List<byte[]> inputNullifiers;
    private readonly List<byte[]> outputCommitments;

    public OfflineNotePaymentTokenIdPreimage(
        string chainId,
        string paymentRequestId,
        ulong createdAtMs,
        byte[] tokenNonce,
        byte[] senderKeyCertificatePayloadHash,
        IReadOnlyList<byte[]> inputNullifiers,
        IReadOnlyList<byte[]> outputCommitments)
        : this(
            OfflineNoteCanonicalPayloadDomains.PaymentTokenId,
            chainId,
            paymentRequestId,
            createdAtMs,
            tokenNonce,
            senderKeyCertificatePayloadHash,
            inputNullifiers,
            outputCommitments)
    {
    }

    public OfflineNotePaymentTokenIdPreimage(
        string domain,
        string chainId,
        string paymentRequestId,
        ulong createdAtMs,
        byte[] tokenNonce,
        byte[] senderKeyCertificatePayloadHash,
        IReadOnlyList<byte[]> inputNullifiers,
        IReadOnlyList<byte[]> outputCommitments)
    {
        Domain = OfflineNoteCanonicalPayloadCodec.RequireDomain(
            domain,
            OfflineNoteCanonicalPayloadDomains.PaymentTokenId,
            nameof(domain));
        ChainId = OfflineNoteCanonicalPayloadCodec.RequireExactNonBlankText(chainId, "chain_id", nameof(chainId));
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
        this.senderKeyCertificatePayloadHash = OfflineNoteCanonicalPayloadCodec.RequireHash(
            senderKeyCertificatePayloadHash,
            "sender_key_certificate_payload_hash",
            nameof(senderKeyCertificatePayloadHash));
        this.inputNullifiers = OfflineNoteCanonicalPayloadCodec.CopyHashList(
            inputNullifiers,
            "input_nullifiers",
            nameof(inputNullifiers));
        this.outputCommitments = OfflineNoteCanonicalPayloadCodec.CopyHashList(
            outputCommitments,
            "output_commitments",
            nameof(outputCommitments));
    }

    public string Domain { get; }

    public string ChainId { get; }

    public string PaymentRequestId { get; }

    public ulong CreatedAtMs { get; }

    public byte[] TokenNonce => tokenNonce.ToArray();

    public byte[] SenderKeyCertificatePayloadHash => senderKeyCertificatePayloadHash.ToArray();

    public IReadOnlyList<byte[]> InputNullifiers =>
        inputNullifiers.Select(static value => value.ToArray()).ToArray();

    public IReadOnlyList<byte[]> OutputCommitments =>
        outputCommitments.Select(static value => value.ToArray()).ToArray();

    public byte[] DerivePaymentTokenId()
    {
        return IrohaHash.Hash(OfflineNoteCanonicalPayloadCodec.EncodePaymentTokenIdPreimage(this));
    }
}

public static class OfflineNoteCanonicalPayloadCodec
{
    public const ushort KeyCertificateVersion = 1;
    public const int HashLength = 32;
    public const string KeyCertificatePayloadTypeName =
        "iroha_data_model::offline::model::OfflineNoteKeyCertificatePayload";
    public const string IssuedClaimTypeName =
        "iroha_data_model::offline::model::OfflineNoteIssuedClaim";
    public const string RedeemPublicInputsTypeName =
        "iroha_data_model::offline::model::OfflineNoteRedeemPublicInputs";
    public const string AuditPublicInputsTypeName =
        "iroha_data_model::offline::model::OfflineNoteAuditPublicInputs";
    public const string NoteCommitmentPreimageTypeName =
        "iroha_data_model::offline::model::OfflineNoteCommitmentPreimage";
    public const string InputNullifierPreimageTypeName =
        "iroha_data_model::offline::model::OfflineNoteInputNullifierPreimage";
    public const string PaymentTokenIdPreimageTypeName =
        "iroha_data_model::offline::model::OfflineNotePaymentTokenIdPreimage";

    private const byte CompactLenFlag = 0x02;
    private const int MaxNoritoHeaderPaddingBytes = 64;
    private const int MaxNumericScale = 28;
    private const int MaxBigIntBytes = 64;
    private const int AssetDefinitionBytesLength = 16;
    private const int AssetDefinitionAddressPayloadLength = 21;
    private const byte AssetDefinitionAddressVersion = 1;

    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);
    private static readonly Dictionary<char, int> Base58Alphabet =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
            .Select(static (character, index) => new KeyValuePair<char, int>(character, index))
            .ToDictionary();
    private static readonly char[] Base58Symbols =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".ToCharArray();

    public static byte[] EncodeKeyCertificatePayload(OfflineNoteKeyCertificatePayload payload)
    {
        ArgumentNullException.ThrowIfNull(payload);
        return Wrap(KeyCertificatePayloadTypeName, EncodeKeyCertificatePayloadFields(payload));
    }

    public static OfflineNoteKeyCertificatePayload DecodeKeyCertificatePayload(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = DecodeArchivePayload(archive, KeyCertificatePayloadTypeName);
        var offset = 0;
        var value = DecodeKeyCertificatePayloadFields(payload, ref offset);
        RequireNoTrailing(payload, offset, "key_certificate_payload");
        return value;
    }

    public static byte[] EncodeIssuedClaim(OfflineNoteIssuedClaim claim)
    {
        ArgumentNullException.ThrowIfNull(claim);
        return Wrap(IssuedClaimTypeName, EncodeIssuedClaimFields(claim));
    }

    public static OfflineNoteIssuedClaim DecodeIssuedClaim(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = DecodeArchivePayload(archive, IssuedClaimTypeName);
        var offset = 0;
        var value = DecodeIssuedClaimFields(payload, ref offset);
        RequireNoTrailing(payload, offset, "issued_claim");
        return value;
    }

    public static byte[] EncodeRedeemPublicInputs(OfflineNoteRedeemPublicInputs inputs)
    {
        ArgumentNullException.ThrowIfNull(inputs);
        return Wrap(RedeemPublicInputsTypeName, EncodeRedeemPublicInputsFields(inputs));
    }

    public static OfflineNoteRedeemPublicInputs DecodeRedeemPublicInputs(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = DecodeArchivePayload(archive, RedeemPublicInputsTypeName);
        var offset = 0;
        var value = DecodeRedeemPublicInputsFields(payload, ref offset);
        RequireNoTrailing(payload, offset, "redeem_public_inputs");
        return value;
    }

    public static byte[] EncodeAuditPublicInputs(OfflineNoteAuditPublicInputs inputs)
    {
        ArgumentNullException.ThrowIfNull(inputs);
        return Wrap(AuditPublicInputsTypeName, EncodeAuditPublicInputsFields(inputs));
    }

    public static OfflineNoteAuditPublicInputs DecodeAuditPublicInputs(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = DecodeArchivePayload(archive, AuditPublicInputsTypeName);
        var offset = 0;
        var value = DecodeAuditPublicInputsFields(payload, ref offset);
        RequireNoTrailing(payload, offset, "audit_public_inputs");
        return value;
    }

    public static byte[] EncodeNoteCommitmentPreimage(OfflineNoteCommitmentPreimage preimage)
    {
        ArgumentNullException.ThrowIfNull(preimage);
        return Wrap(NoteCommitmentPreimageTypeName, EncodeNoteCommitmentPreimageFields(preimage));
    }

    public static OfflineNoteCommitmentPreimage DecodeNoteCommitmentPreimage(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = DecodeArchivePayload(archive, NoteCommitmentPreimageTypeName);
        var offset = 0;
        var value = DecodeNoteCommitmentPreimageFields(payload, ref offset);
        RequireNoTrailing(payload, offset, "note_commitment_preimage");
        return value;
    }

    public static byte[] EncodeInputNullifierPreimage(OfflineNoteInputNullifierPreimage preimage)
    {
        ArgumentNullException.ThrowIfNull(preimage);
        return Wrap(InputNullifierPreimageTypeName, EncodeInputNullifierPreimageFields(preimage));
    }

    public static OfflineNoteInputNullifierPreimage DecodeInputNullifierPreimage(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = DecodeArchivePayload(archive, InputNullifierPreimageTypeName);
        var offset = 0;
        var value = DecodeInputNullifierPreimageFields(payload, ref offset);
        RequireNoTrailing(payload, offset, "input_nullifier_preimage");
        return value;
    }

    public static byte[] EncodePaymentTokenIdPreimage(OfflineNotePaymentTokenIdPreimage preimage)
    {
        ArgumentNullException.ThrowIfNull(preimage);
        return Wrap(PaymentTokenIdPreimageTypeName, EncodePaymentTokenIdPreimageFields(preimage));
    }

    public static OfflineNotePaymentTokenIdPreimage DecodePaymentTokenIdPreimage(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        var payload = DecodeArchivePayload(archive, PaymentTokenIdPreimageTypeName);
        var offset = 0;
        var value = DecodePaymentTokenIdPreimageFields(payload, ref offset);
        RequireNoTrailing(payload, offset, "payment_token_id_preimage");
        return value;
    }

    internal static string RequireDomain(string value, string expected, string paramName)
    {
        ArgumentNullException.ThrowIfNull(value, paramName);
        if (!string.Equals(value, expected, StringComparison.Ordinal))
        {
            throw new ArgumentException($"domain must be {expected}.", paramName);
        }

        return value;
    }

    internal static string RequireExactNonBlankText(string value, string field, string paramName)
    {
        ArgumentNullException.ThrowIfNull(value, paramName);
        if (value.Trim().Length == 0)
        {
            throw new ArgumentException($"{field} must not be blank.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{field} must not contain whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException($"{field} must not contain control characters.", paramName);
        }

        try
        {
            StrictUtf8.GetByteCount(value);
        }
        catch (EncoderFallbackException exception)
        {
            throw new ArgumentException($"{field} must be valid UTF-8 text.", paramName, exception);
        }

        return value;
    }

    internal static byte[] RequireHash(byte[] value, string field, string paramName)
    {
        ArgumentNullException.ThrowIfNull(value, paramName);
        if (value.Length != HashLength)
        {
            throw new ArgumentException($"{field} must be 32 bytes.", paramName);
        }

        if ((value[^1] & 1) != 1)
        {
            throw new ArgumentException($"{field} must carry the Iroha prehash marker.", paramName);
        }

        return value.ToArray();
    }

    internal static byte[] RequireRandomBytes(byte[] value, string field, string paramName)
    {
        ArgumentNullException.ThrowIfNull(value, paramName);
        if (value.Length != HashLength)
        {
            throw new ArgumentException($"{field} must be exactly 32 bytes.", paramName);
        }

        return value.ToArray();
    }

    internal static List<byte[]> CopyHashList(IReadOnlyList<byte[]> values, string field, string paramName)
    {
        ArgumentNullException.ThrowIfNull(values, paramName);
        if (values.Count == 0)
        {
            throw new ArgumentException($"{field} must not be empty.", paramName);
        }

        var output = new List<byte[]>(values.Count);
        for (var index = 0; index < values.Count; index++)
        {
            output.Add(RequireHash(values[index], $"{field}[{index}]", paramName));
        }

        return output;
    }

    internal static string CanonicalAccountId(string value)
    {
        return CanonicalAccountId(value, "account_id", nameof(value));
    }

    internal static string CanonicalAccountId(string value, string field, string paramName)
    {
        var exact = RequireExactNonBlankText(value, field, paramName);
        try
        {
            return AccountAddress.Parse(exact, AccountAddress.DefaultChainDiscriminant)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        }
        catch (AccountAddressException exception)
        {
            throw new ArgumentException($"{field} must be a canonical I105 account id.", paramName, exception);
        }
    }

    internal static string CanonicalAssetId(string value)
    {
        return CanonicalAssetId(value, "asset_id", nameof(value));
    }

    internal static string CanonicalAssetId(string value, string field, string paramName)
    {
        var exact = RequireExactNonBlankText(value, field, paramName);
        var parts = exact.Split('#', StringSplitOptions.None);
        if (parts.Length != 2 && parts.Length != 3)
        {
            throw new ArgumentException(
                "asset_id must be '<asset-definition>#<account>' with optional '#dataspace:<id>'.",
                paramName);
        }

        var definitionId = EncodeAssetDefinitionAddress(ParseAssetDefinitionAddress(parts[0]));
        var accountId = CanonicalAccountId(parts[1], $"{field}.account_id", paramName);
        if (parts.Length == 2)
        {
            return definitionId + "#" + accountId;
        }

        var dataspaceId = ParseCanonicalDataspaceScope(parts[2], paramName);
        return definitionId + "#" + accountId + "#dataspace:" + dataspaceId;
    }

    internal static string ParseCanonicalNumeric(string value)
    {
        return ParseNumeric(value).CanonicalString;
    }

    private static byte[] Wrap(string typeName, byte[] payload)
    {
        return NoritoCodec.Encode(typeName, payload, CompactLenFlag);
    }

    private static byte[] EncodeKeyCertificatePayloadFields(OfflineNoteKeyCertificatePayload payload)
    {
        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, payload.Domain));
        WriteField(writer, child => WriteUInt16LittleEndian(child, payload.Version));
        WriteField(writer, child => WriteString(child, payload.Platform));
        WriteField(writer, child => WriteString(child, payload.KeyId));
        WriteField(writer, child => WriteString(child, payload.DeviceId));
        WriteField(writer, child => WriteAccountId(child, payload.AccountId));
        WriteField(writer, child => WriteBytesVec(child, payload.PublicKey));
        WriteField(writer, child => WriteString(child, payload.AssertionScheme));
        WriteField(writer, child => WriteString(child, payload.AssertionKeyAlgorithm));
        WriteField(writer, child => WriteBytesVec(child, payload.AssertionPublicKey));
        WriteField(writer, child => WriteOptionUInt32(child, payload.AssertionUsageCountLimit));
        WriteField(writer, child => child.WriteByte(payload.OneUse ? (byte)1 : (byte)0));
        return writer.ToArray();
    }

    private static OfflineNoteKeyCertificatePayload DecodeKeyCertificatePayloadFields(
        byte[] payload,
        ref int offset)
    {
        return new OfflineNoteKeyCertificatePayload(
            ReadField(payload, ref offset, "domain", ReadString),
            ReadField(payload, ref offset, "version", ReadUInt16LittleEndian),
            ReadField(payload, ref offset, "platform", ReadString),
            ReadField(payload, ref offset, "key_id", ReadString),
            ReadField(payload, ref offset, "device_id", ReadString),
            ReadField(payload, ref offset, "account_id", ReadAccountId),
            ReadField(payload, ref offset, "public_key", ReadBytesVec),
            ReadField(payload, ref offset, "assertion_scheme", ReadString),
            ReadField(payload, ref offset, "assertion_key_algorithm", ReadString),
            ReadField(payload, ref offset, "assertion_public_key", ReadBytesVec),
            ReadField(payload, ref offset, "assertion_usage_count_limit", ReadOptionUInt32),
            ReadField(payload, ref offset, "one_use", ReadBool));
    }

    private static byte[] EncodeIssuedClaimFields(OfflineNoteIssuedClaim claim)
    {
        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, claim.Domain));
        WriteField(writer, child => child.Write(claim.NoteCommitment));
        WriteField(writer, child => child.Write(claim.KeyCertificatePayloadHash));
        WriteField(writer, child => WriteAssetId(child, claim.AssetId));
        WriteField(writer, child => WriteNumeric(child, claim.Amount));
        return writer.ToArray();
    }

    private static OfflineNoteIssuedClaim DecodeIssuedClaimFields(byte[] payload, ref int offset)
    {
        return new OfflineNoteIssuedClaim(
            ReadField(payload, ref offset, "domain", ReadString),
            ReadField(payload, ref offset, "note_commitment", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "note_commitment")),
            ReadField(payload, ref offset, "key_certificate_payload_hash", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "key_certificate_payload_hash")),
            ReadField(payload, ref offset, "asset_id", ReadAssetId),
            ReadField(payload, ref offset, "amount", ReadNumeric));
    }

    private static byte[] EncodeRedeemPublicInputsFields(OfflineNoteRedeemPublicInputs inputs)
    {
        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, inputs.Domain));
        WriteField(writer, child => child.Write(inputs.SourceNoteCommitment));
        WriteField(writer, child => WriteVec(child, inputs.InputNullifiers, (element, value) => element.Write(value)));
        WriteField(writer, child => child.Write(inputs.KeyCertificatePayloadHash));
        WriteField(writer, child => WriteAccountId(child, inputs.Recipient));
        WriteField(writer, child => WriteAssetId(child, inputs.AssetId));
        WriteField(writer, child => WriteNumeric(child, inputs.Amount));
        return writer.ToArray();
    }

    private static OfflineNoteRedeemPublicInputs DecodeRedeemPublicInputsFields(byte[] payload, ref int offset)
    {
        return new OfflineNoteRedeemPublicInputs(
            ReadField(payload, ref offset, "domain", ReadString),
            ReadField(payload, ref offset, "source_note_commitment", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "source_note_commitment")),
            ReadField(payload, ref offset, "input_nullifiers", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadVec(fieldPayload, ref fieldOffset, "input_nullifiers", (byte[] child, ref int childOffset) =>
                    ReadHash(child, ref childOffset, "input_nullifier"))),
            ReadField(payload, ref offset, "key_certificate_payload_hash", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "key_certificate_payload_hash")),
            ReadField(payload, ref offset, "recipient", ReadAccountId),
            ReadField(payload, ref offset, "asset_id", ReadAssetId),
            ReadField(payload, ref offset, "amount", ReadNumeric));
    }

    private static byte[] EncodeAuditPublicInputsFields(OfflineNoteAuditPublicInputs inputs)
    {
        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, inputs.Domain));
        WriteField(writer, child => child.Write(inputs.TokenId));
        WriteField(writer, child => child.Write(inputs.KeyCertificatePayloadHash));
        WriteField(writer, child => WriteVec(child, inputs.InputNullifiers, (element, value) => element.Write(value)));
        WriteField(writer, child => WriteVec(child, inputs.InputClaims, (element, value) =>
            element.Write(EncodeIssuedClaimFields(value))));
        WriteField(writer, child => WriteVec(child, inputs.OutputCommitments, (element, value) => element.Write(value)));
        WriteField(writer, child => WriteVec(child, inputs.OutputClaims, (element, value) =>
            element.Write(EncodeIssuedClaimFields(value))));
        return writer.ToArray();
    }

    private static OfflineNoteAuditPublicInputs DecodeAuditPublicInputsFields(byte[] payload, ref int offset)
    {
        return new OfflineNoteAuditPublicInputs(
            ReadField(payload, ref offset, "domain", ReadString),
            ReadField(payload, ref offset, "token_id", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "token_id")),
            ReadField(payload, ref offset, "key_certificate_payload_hash", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "key_certificate_payload_hash")),
            ReadField(payload, ref offset, "input_nullifiers", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadVec(fieldPayload, ref fieldOffset, "input_nullifiers", (byte[] child, ref int childOffset) =>
                    ReadHash(child, ref childOffset, "input_nullifier"))),
            ReadField(payload, ref offset, "input_claims", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadVec(fieldPayload, ref fieldOffset, "input_claims", DecodeIssuedClaimFields)),
            ReadField(payload, ref offset, "output_commitments", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadVec(fieldPayload, ref fieldOffset, "output_commitments", (byte[] child, ref int childOffset) =>
                    ReadHash(child, ref childOffset, "output_commitment"))),
            ReadField(payload, ref offset, "output_claims", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadVec(fieldPayload, ref fieldOffset, "output_claims", DecodeIssuedClaimFields)));
    }

    private static byte[] EncodeNoteCommitmentPreimageFields(OfflineNoteCommitmentPreimage preimage)
    {
        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, preimage.Domain));
        WriteField(writer, child => WriteChainId(child, preimage.ChainId));
        WriteField(writer, child => child.Write(preimage.OwnerKeyCertificatePayloadHash));
        WriteField(writer, child => WriteAssetId(child, preimage.AssetId));
        WriteField(writer, child => WriteNumeric(child, preimage.Amount));
        WriteField(writer, child => WriteBytesVec(child, preimage.NoteSecret));
        WriteField(writer, child => WriteCommitmentOrigin(child, preimage.Origin));
        return writer.ToArray();
    }

    private static OfflineNoteCommitmentPreimage DecodeNoteCommitmentPreimageFields(
        byte[] payload,
        ref int offset)
    {
        return new OfflineNoteCommitmentPreimage(
            ReadField(payload, ref offset, "domain", ReadString),
            ReadField(payload, ref offset, "chain_id", ReadChainId),
            ReadField(payload, ref offset, "owner_key_certificate_payload_hash", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "owner_key_certificate_payload_hash")),
            ReadField(payload, ref offset, "asset_id", ReadAssetId),
            ReadField(payload, ref offset, "amount", ReadNumeric),
            ReadField(payload, ref offset, "note_secret", ReadBytesVec),
            ReadField(payload, ref offset, "origin", ReadCommitmentOrigin));
    }

    private static byte[] EncodeInputNullifierPreimageFields(OfflineNoteInputNullifierPreimage preimage)
    {
        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, preimage.Domain));
        WriteField(writer, child => WriteChainId(child, preimage.ChainId));
        WriteField(writer, child => child.Write(preimage.SourceNoteCommitment));
        WriteField(writer, child => child.Write(preimage.OwnerKeyCertificatePayloadHash));
        WriteField(writer, child => WriteBytesVec(child, preimage.NoteSecret));
        return writer.ToArray();
    }

    private static OfflineNoteInputNullifierPreimage DecodeInputNullifierPreimageFields(
        byte[] payload,
        ref int offset)
    {
        return new OfflineNoteInputNullifierPreimage(
            ReadField(payload, ref offset, "domain", ReadString),
            ReadField(payload, ref offset, "chain_id", ReadChainId),
            ReadField(payload, ref offset, "source_note_commitment", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "source_note_commitment")),
            ReadField(payload, ref offset, "owner_key_certificate_payload_hash", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "owner_key_certificate_payload_hash")),
            ReadField(payload, ref offset, "note_secret", ReadBytesVec));
    }

    private static byte[] EncodePaymentTokenIdPreimageFields(OfflineNotePaymentTokenIdPreimage preimage)
    {
        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, preimage.Domain));
        WriteField(writer, child => WriteChainId(child, preimage.ChainId));
        WriteField(writer, child => WriteString(child, preimage.PaymentRequestId));
        WriteField(writer, child => WriteUInt64LittleEndian(child, preimage.CreatedAtMs));
        WriteField(writer, child => WriteBytesVec(child, preimage.TokenNonce));
        WriteField(writer, child => child.Write(preimage.SenderKeyCertificatePayloadHash));
        WriteField(writer, child => WriteVec(child, preimage.InputNullifiers, (element, value) => element.Write(value)));
        WriteField(writer, child => WriteVec(child, preimage.OutputCommitments, (element, value) => element.Write(value)));
        return writer.ToArray();
    }

    private static OfflineNotePaymentTokenIdPreimage DecodePaymentTokenIdPreimageFields(
        byte[] payload,
        ref int offset)
    {
        return new OfflineNotePaymentTokenIdPreimage(
            ReadField(payload, ref offset, "domain", ReadString),
            ReadField(payload, ref offset, "chain_id", ReadChainId),
            ReadField(payload, ref offset, "payment_request_id", ReadString),
            ReadField(payload, ref offset, "created_at_ms", ReadUInt64LittleEndian),
            ReadField(payload, ref offset, "token_nonce", ReadBytesVec),
            ReadField(payload, ref offset, "sender_key_certificate_payload_hash", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadHash(fieldPayload, ref fieldOffset, "sender_key_certificate_payload_hash")),
            ReadField(payload, ref offset, "input_nullifiers", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadVec(fieldPayload, ref fieldOffset, "input_nullifiers", (byte[] child, ref int childOffset) =>
                    ReadHash(child, ref childOffset, "input_nullifier"))),
            ReadField(payload, ref offset, "output_commitments", (byte[] fieldPayload, ref int fieldOffset) =>
                ReadVec(fieldPayload, ref fieldOffset, "output_commitments", (byte[] child, ref int childOffset) =>
                    ReadHash(child, ref childOffset, "output_commitment"))));
    }

    private static void WriteChainId(MemoryStream writer, string chainId)
    {
        WriteField(writer, child => WriteString(child, chainId));
    }

    private static string ReadChainId(byte[] payload, ref int offset)
    {
        return ReadField(payload, ref offset, "chain_id.value", ReadString);
    }

    private static void WriteCommitmentOrigin(MemoryStream writer, OfflineNoteCommitmentOrigin origin)
    {
        switch (origin)
        {
            case OfflineNoteCommitmentOrigin.IssuerLoad issuerLoad:
                WriteUInt32LittleEndian(writer, 0);
                WriteField(writer, child =>
                {
                    WriteField(child, grandchild => WriteString(grandchild, issuerLoad.OperationId));
                    WriteField(child, grandchild => WriteString(grandchild, issuerLoad.LineageId));
                    WriteField(child, grandchild => WriteUInt64LittleEndian(grandchild, issuerLoad.LocalRevision));
                });
                return;
            case OfflineNoteCommitmentOrigin.P2pOutput p2pOutput:
                WriteUInt32LittleEndian(writer, 1);
                WriteField(writer, child =>
                {
                    WriteField(child, grandchild => WriteString(grandchild, p2pOutput.PaymentRequestId));
                    WriteField(child, grandchild => WriteUInt32LittleEndian(grandchild, p2pOutput.OutputIndex));
                });
                return;
            default:
                throw InvalidField("origin");
        }
    }

    private static OfflineNoteCommitmentOrigin ReadCommitmentOrigin(byte[] payload, ref int offset)
    {
        var tag = ReadUInt32LittleEndian(payload, ref offset);
        return tag switch
        {
            0 => ReadField(payload, ref offset, "origin.issuer_load", ReadIssuerLoadOrigin),
            1 => ReadField(payload, ref offset, "origin.p2p_output", ReadP2pOutputOrigin),
            _ => throw InvalidField("origin"),
        };
    }

    private static OfflineNoteCommitmentOrigin.IssuerLoad ReadIssuerLoadOrigin(byte[] payload, ref int offset)
    {
        return new OfflineNoteCommitmentOrigin.IssuerLoad(
            ReadField(payload, ref offset, "origin.operation_id", ReadString),
            ReadField(payload, ref offset, "origin.lineage_id", ReadString),
            ReadField(payload, ref offset, "origin.local_revision", ReadUInt64LittleEndian));
    }

    private static OfflineNoteCommitmentOrigin.P2pOutput ReadP2pOutputOrigin(byte[] payload, ref int offset)
    {
        return new OfflineNoteCommitmentOrigin.P2pOutput(
            ReadField(payload, ref offset, "origin.payment_request_id", ReadString),
            ReadField(payload, ref offset, "origin.output_index", ReadUInt32LittleEndian));
    }

    internal delegate T FieldReader<T>(byte[] payload, ref int offset);

    internal delegate void ElementWriter<in T>(MemoryStream writer, T value);

    internal static void WriteField(MemoryStream writer, Action<MemoryStream> write)
    {
        using var child = new MemoryStream();
        write(child);
        var payload = child.ToArray();
        WriteCompactLength(writer, (ulong)payload.Length);
        writer.Write(payload);
    }

    internal static T ReadField<T>(byte[] payload, ref int offset, string field, FieldReader<T> read)
    {
        var length = ReadCompactLength(payload, ref offset, field);
        if (length > int.MaxValue || length > (ulong)(payload.Length - offset))
        {
            throw InvalidField(field);
        }

        var child = payload.AsSpan(offset, (int)length).ToArray();
        offset += (int)length;
        var childOffset = 0;
        var value = read(child, ref childOffset);
        RequireNoTrailing(child, childOffset, field);
        return value;
    }

    internal static void WriteString(MemoryStream writer, string value)
    {
        var bytes = StrictUtf8.GetBytes(value);
        WriteCompactLength(writer, (ulong)bytes.Length);
        writer.Write(bytes);
    }

    internal static string ReadString(byte[] payload, ref int offset)
    {
        var length = ReadCompactLength(payload, ref offset, "string");
        if (length > int.MaxValue || length > (ulong)(payload.Length - offset))
        {
            throw InvalidField("string");
        }

        try
        {
            var value = StrictUtf8.GetString(payload, offset, (int)length);
            offset += (int)length;
            return value;
        }
        catch (DecoderFallbackException exception)
        {
            throw new ArgumentException("Offline Note canonical payload string is invalid.", nameof(payload), exception);
        }
    }

    internal static void WriteBytesVec(MemoryStream writer, byte[] value)
    {
        WriteUInt64LittleEndian(writer, (ulong)value.Length);
        writer.Write(value);
    }

    internal static byte[] ReadBytesVec(byte[] payload, ref int offset)
    {
        var length = ReadUInt64LittleEndian(payload, ref offset);
        if (length > int.MaxValue || length > (ulong)(payload.Length - offset))
        {
            throw InvalidField("bytes_vec");
        }

        var bytes = payload.AsSpan(offset, (int)length).ToArray();
        offset += (int)length;
        return bytes;
    }

    internal static void WriteOptionUInt32(MemoryStream writer, uint? value)
    {
        if (!value.HasValue)
        {
            writer.WriteByte(0);
            return;
        }

        writer.WriteByte(1);
        WriteField(writer, child => WriteUInt32LittleEndian(child, value.Value));
    }

    internal static uint? ReadOptionUInt32(byte[] payload, ref int offset)
    {
        var tag = ReadByte(payload, ref offset, "option");
        return tag switch
        {
            0 => null,
            1 => ReadField(payload, ref offset, "option.value", ReadUInt32LittleEndian),
            _ => throw InvalidField("option"),
        };
    }

    internal static void WriteVec<T>(
        MemoryStream writer,
        IReadOnlyList<T> values,
        ElementWriter<T> writeElement)
    {
        WriteUInt64LittleEndian(writer, (ulong)values.Count);
        foreach (var value in values)
        {
            WriteField(writer, child => writeElement(child, value));
        }
    }

    internal static List<T> ReadVec<T>(
        byte[] payload,
        ref int offset,
        string field,
        FieldReader<T> readElement)
    {
        var count = ReadUInt64LittleEndian(payload, ref offset);
        if (count > int.MaxValue)
        {
            throw InvalidField(field);
        }

        var values = new List<T>((int)count);
        for (var index = 0; index < (int)count; index++)
        {
            values.Add(ReadField(payload, ref offset, $"{field}[{index}]", readElement));
        }

        return values;
    }

    internal static void WriteAccountId(MemoryStream writer, string accountId)
    {
        var address = AccountAddress.Parse(
            RequireExactNonBlankText(accountId, "account_id", nameof(accountId)),
            AccountAddress.DefaultChainDiscriminant);
        if (address.CurveIdentifier is null || address.PublicKey.Length == 0)
        {
            throw new ArgumentException("Multisig account controllers are not yet supported by this codec.", nameof(accountId));
        }

        WriteUInt32LittleEndian(writer, 0);
        WriteField(writer, child => WritePublicKey(child, address.CurveIdentifier.Value, address.PublicKey));
    }

    internal static string ReadAccountId(byte[] payload, ref int offset)
    {
        var tag = ReadUInt32LittleEndian(payload, ref offset);
        if (tag != 0)
        {
            throw InvalidField("account_id");
        }

        return ReadField(payload, ref offset, "account_id.controller", (byte[] child, ref int childOffset) =>
        {
            var (curve, publicKey) = ReadPublicKey(child, ref childOffset);
            return AccountAddress.FromPublicKey(publicKey, curve)
                .ToI105(AccountAddress.DefaultChainDiscriminant);
        });
    }

    private static void WritePublicKey(MemoryStream writer, CurveId curve, byte[] publicKey)
    {
        var tag = curve switch
        {
            CurveId.Ed25519 => (byte)0,
            CurveId.MlDsa => (byte)4,
            CurveId.Gost256A => (byte)5,
            CurveId.Gost256B => (byte)6,
            CurveId.Gost256C => (byte)7,
            CurveId.Gost512A => (byte)8,
            CurveId.Gost512B => (byte)9,
            CurveId.Sm2 => (byte)10,
            _ => throw InvalidField("account_id"),
        };
        var compactPayload = new byte[1 + publicKey.Length];
        compactPayload[0] = tag;
        publicKey.CopyTo(compactPayload.AsSpan(1));
        WriteConstVec(writer, compactPayload);
    }

    private static (string Algorithm, byte[] PublicKey) ReadPublicKey(byte[] payload, ref int offset)
    {
        var compactPayload = ReadConstVec(payload, ref offset);
        if (compactPayload.Length < 2)
        {
            throw InvalidField("account_id");
        }

        var algorithm = compactPayload[0] switch
        {
            0 => "ed25519",
            4 => "ml-dsa",
            5 => "gost-256-a",
            6 => "gost-256-b",
            7 => "gost-256-c",
            8 => "gost-512-a",
            9 => "gost-512-b",
            10 => "sm2",
            _ => throw InvalidField("account_id"),
        };
        return (algorithm, compactPayload[1..]);
    }

    internal static void WriteConstVec(MemoryStream writer, byte[] bytes)
    {
        WriteUInt64LittleEndian(writer, (ulong)bytes.Length);
        foreach (var value in bytes)
        {
            WriteCompactLength(writer, 1);
            writer.WriteByte(value);
        }
    }

    internal static byte[] ReadConstVec(byte[] payload, ref int offset)
    {
        var length = ReadUInt64LittleEndian(payload, ref offset);
        if (length > int.MaxValue)
        {
            throw InvalidField("const_vec");
        }

        var output = new byte[(int)length];
        for (var index = 0; index < output.Length; index++)
        {
            var elementLength = ReadCompactLength(payload, ref offset, "const_vec.element");
            if (elementLength != 1)
            {
                throw InvalidField("const_vec");
            }

            output[index] = ReadByte(payload, ref offset, "const_vec.element");
        }

        return output;
    }

    internal static void WriteAssetId(MemoryStream writer, string assetId)
    {
        var parsed = ParseAssetId(assetId);
        WriteField(writer, child => WriteAccountId(child, parsed.AccountId));
        WriteField(writer, child => WriteAssetDefinitionAddress(child, parsed.DefinitionBytes));
        WriteField(writer, child => WriteAssetBalanceScope(child, parsed.DataspaceId));
    }

    internal static string ReadAssetId(byte[] payload, ref int offset)
    {
        var accountId = ReadField(payload, ref offset, "asset_id.account_id", ReadAccountId);
        var definitionBytes = ReadField(
            payload,
            ref offset,
            "asset_id.definition",
            ReadAssetDefinitionAddress);
        var dataspaceId = ReadField(payload, ref offset, "asset_id.scope", ReadAssetBalanceScope);
        var baseId = EncodeAssetDefinitionAddress(definitionBytes) + "#" + accountId;
        return dataspaceId.HasValue ? baseId + "#dataspace:" + dataspaceId.Value : baseId;
    }

    private static ParsedAssetId ParseAssetId(string assetId)
    {
        var exact = RequireExactNonBlankText(assetId, "asset_id", nameof(assetId));
        var parts = exact.Split('#', StringSplitOptions.None);
        if (parts.Length != 2 && parts.Length != 3)
        {
            throw new ArgumentException(
                "asset_id must be '<asset-definition>#<account>' with optional '#dataspace:<id>'.",
                nameof(assetId));
        }

        var definitionBytes = ParseAssetDefinitionAddress(parts[0]);
        var accountId = CanonicalAccountId(parts[1]);
        ulong? dataspaceId = null;
        if (parts.Length == 3)
        {
            dataspaceId = ParseCanonicalDataspaceScope(parts[2], nameof(assetId));
        }

        return new ParsedAssetId(accountId, definitionBytes, dataspaceId);
    }

    private static ulong ParseCanonicalDataspaceScope(string scope, string paramName)
    {
        const string Prefix = "dataspace:";
        if (!scope.StartsWith(Prefix, StringComparison.Ordinal))
        {
            throw new ArgumentException("asset_id dataspace scope must use dataspace:<id>.", paramName);
        }

        var idText = scope[Prefix.Length..];
        if (!IsCanonicalUnsignedDecimalText(idText)
            || !ulong.TryParse(
                idText,
                System.Globalization.NumberStyles.None,
                System.Globalization.CultureInfo.InvariantCulture,
                out var dataspaceId))
        {
            throw new ArgumentException("asset_id dataspace scope must use canonical dataspace:<id>.", paramName);
        }

        return dataspaceId;
    }

    private static bool IsCanonicalUnsignedDecimalText(string value)
    {
        if (value.Length == 0)
        {
            return false;
        }

        if (value.Length > 1 && value[0] == '0')
        {
            return false;
        }

        foreach (var character in value)
        {
            if (character is < '0' or > '9')
            {
                return false;
            }
        }

        return true;
    }

    private static void WriteAssetDefinitionAddress(MemoryStream writer, byte[] value)
    {
        if (value.Length != AssetDefinitionBytesLength)
        {
            throw InvalidField("asset_definition");
        }

        foreach (var b in value)
        {
            WriteCompactLength(writer, 1);
            writer.WriteByte(b);
        }
    }

    private static byte[] ReadAssetDefinitionAddress(byte[] payload, ref int offset)
    {
        using var writer = new MemoryStream();
        while (offset < payload.Length)
        {
            var length = ReadCompactLength(payload, ref offset, "asset_definition");
            if (length != 1)
            {
                throw InvalidField("asset_definition");
            }

            writer.WriteByte(ReadByte(payload, ref offset, "asset_definition"));
        }

        var bytes = writer.ToArray();
        _ = EncodeAssetDefinitionAddress(bytes);
        return bytes;
    }

    private static void WriteAssetBalanceScope(MemoryStream writer, ulong? dataspaceId)
    {
        if (!dataspaceId.HasValue)
        {
            WriteUInt32LittleEndian(writer, 0);
            return;
        }

        WriteUInt32LittleEndian(writer, 1);
        WriteField(writer, child => WriteUInt64LittleEndian(child, dataspaceId.Value));
    }

    private static ulong? ReadAssetBalanceScope(byte[] payload, ref int offset)
    {
        var tag = ReadUInt32LittleEndian(payload, ref offset);
        return tag switch
        {
            0 => null,
            1 => ReadField(payload, ref offset, "asset_id.scope.dataspace", ReadUInt64LittleEndian),
            _ => throw InvalidField("asset_id.scope"),
        };
    }

    internal static void WriteNumeric(MemoryStream writer, string value)
    {
        var numeric = ParseNumeric(value);
        WriteField(writer, child =>
        {
            WriteUInt32LittleEndian(child, (uint)numeric.MantissaBytes.Length);
            child.Write(numeric.MantissaBytes);
        });
        WriteField(writer, child => WriteUInt32LittleEndian(child, numeric.Scale));
    }

    internal static string ReadNumeric(byte[] payload, ref int offset)
    {
        var mantissaBytes = ReadField(payload, ref offset, "amount.mantissa", (byte[] child, ref int childOffset) =>
        {
            var length = ReadUInt32LittleEndian(child, ref childOffset);
            if (length > MaxBigIntBytes || length > child.Length - childOffset)
            {
                throw InvalidField("amount.mantissa");
            }

            var bytes = child.AsSpan(childOffset, (int)length).ToArray();
            childOffset += (int)length;
            return bytes;
        });
        var scale = ReadField(payload, ref offset, "amount.scale", ReadUInt32LittleEndian);
        if (scale > MaxNumericScale)
        {
            throw InvalidField("amount.scale");
        }

        var mantissa = mantissaBytes.Length == 0
            ? BigInteger.Zero
            : new BigInteger(mantissaBytes, isUnsigned: false, isBigEndian: false);
        return CanonicalNumericString(mantissa, scale);
    }

    private static NumericValue ParseNumeric(string value)
    {
        var exact = RequireExactNonBlankText(value, "amount", nameof(value));
        var sign = 1;
        var body = exact;
        if (body[0] == '-')
        {
            sign = -1;
            body = body[1..];
        }
        else if (body[0] == '+')
        {
            body = body[1..];
        }

        var parts = body.Split('.', StringSplitOptions.None);
        if (parts.Length > 2 || parts.All(static part => part.Length == 0))
        {
            throw new ArgumentException($"Invalid numeric literal `{value}`.", nameof(value));
        }

        if (parts.Any(static part => part.Any(static ch => ch is < '0' or > '9')))
        {
            throw new ArgumentException($"Invalid numeric literal `{value}`.", nameof(value));
        }

        var digits = string.Concat(parts);
        if (digits.Length == 0)
        {
            throw new ArgumentException($"Invalid numeric literal `{value}`.", nameof(value));
        }

        var scale = parts.Length == 2 ? parts[1].Length : 0;
        if (scale > MaxNumericScale)
        {
            throw new ArgumentOutOfRangeException(nameof(value), $"Iroha numerics support at most {MaxNumericScale} fractional digits.");
        }

        var mantissa = BigInteger.Parse(digits, System.Globalization.CultureInfo.InvariantCulture);
        if (sign < 0)
        {
            mantissa = BigInteger.Negate(mantissa);
        }

        var mantissaBytes = ToTwosComplementLittleEndian(mantissa);
        if (mantissaBytes.Length > MaxBigIntBytes)
        {
            throw new ArgumentException($"numeric mantissa exceeds {MaxBigIntBytes} bytes.", nameof(value));
        }

        return new NumericValue(mantissaBytes, (uint)scale, CanonicalNumericString(mantissa, (uint)scale));
    }

    private static string CanonicalNumericString(BigInteger mantissa, uint scale)
    {
        var negative = mantissa.Sign < 0;
        var digits = BigInteger.Abs(mantissa).ToString(System.Globalization.CultureInfo.InvariantCulture);
        while (digits.Length > 1 && digits[0] == '0')
        {
            digits = digits[1..];
        }

        if (scale == 0)
        {
            return negative && digits != "0" ? "-" + digits : digits;
        }

        while (digits.Length <= scale)
        {
            digits = "0" + digits;
        }

        var splitAt = digits.Length - (int)scale;
        var body = digits[..splitAt] + "." + digits[splitAt..];
        return negative && mantissa.Sign != 0 ? "-" + body : body;
    }

    private static byte[] ToTwosComplementLittleEndian(BigInteger value)
    {
        if (value.IsZero)
        {
            return [];
        }

        return value.ToByteArray(isUnsigned: false, isBigEndian: false);
    }

    internal static void WriteUInt16LittleEndian(MemoryStream writer, ushort value)
    {
        Span<byte> scratch = stackalloc byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16LittleEndian(scratch, value);
        writer.Write(scratch);
    }

    internal static ushort ReadUInt16LittleEndian(byte[] payload, ref int offset)
    {
        if (payload.Length - offset < sizeof(ushort))
        {
            throw InvalidField("u16");
        }

        var value = BinaryPrimitives.ReadUInt16LittleEndian(payload.AsSpan(offset, sizeof(ushort)));
        offset += sizeof(ushort);
        return value;
    }

    internal static void WriteUInt32LittleEndian(MemoryStream writer, uint value)
    {
        Span<byte> scratch = stackalloc byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(scratch, value);
        writer.Write(scratch);
    }

    internal static uint ReadUInt32LittleEndian(byte[] payload, ref int offset)
    {
        if (payload.Length - offset < sizeof(uint))
        {
            throw InvalidField("u32");
        }

        var value = BinaryPrimitives.ReadUInt32LittleEndian(payload.AsSpan(offset, sizeof(uint)));
        offset += sizeof(uint);
        return value;
    }

    internal static void WriteUInt64LittleEndian(MemoryStream writer, ulong value)
    {
        Span<byte> scratch = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(scratch, value);
        writer.Write(scratch);
    }

    internal static ulong ReadUInt64LittleEndian(byte[] payload, ref int offset)
    {
        if (payload.Length - offset < sizeof(ulong))
        {
            throw InvalidField("u64");
        }

        var value = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(offset, sizeof(ulong)));
        offset += sizeof(ulong);
        return value;
    }

    internal static bool ReadBool(byte[] payload, ref int offset)
    {
        var tag = ReadByte(payload, ref offset, "bool");
        return tag switch
        {
            0 => false,
            1 => true,
            _ => throw InvalidField("bool"),
        };
    }

    internal static byte[] ReadHash(byte[] payload, ref int offset, string field)
    {
        if (payload.Length - offset < HashLength)
        {
            throw InvalidField(field);
        }

        var bytes = payload.AsSpan(offset, HashLength).ToArray();
        offset += HashLength;
        return RequireHash(bytes, field, nameof(payload));
    }

    private static byte ReadByte(byte[] payload, ref int offset, string field)
    {
        if (offset >= payload.Length)
        {
            throw InvalidField(field);
        }

        return payload[offset++];
    }

    internal static void WriteCompactLength(MemoryStream writer, ulong value)
    {
        while (value >= 0x80)
        {
            writer.WriteByte((byte)((value & 0x7f) | 0x80));
            value >>= 7;
        }

        writer.WriteByte((byte)value);
    }

    internal static ulong ReadCompactLength(byte[] payload, ref int offset, string field)
    {
        var startOffset = offset;
        ulong value = 0;
        var shift = 0;
        while (offset < payload.Length && offset - startOffset < 10)
        {
            var current = payload[offset++];
            var currentValue = current & 0x7f;
            if (shift >= 63 && currentValue > 1)
            {
                throw InvalidField(field);
            }

            value |= (ulong)currentValue << shift;
            if ((current & 0x80) == 0)
            {
                var encodedLength = offset - startOffset;
                if (encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))))
                {
                    throw InvalidField(field);
                }

                return value;
            }

            shift += 7;
        }

        throw InvalidField(field);
    }

    internal static byte[] DecodeArchivePayload(byte[] archive, string typeName)
    {
        if (archive.Length < NoritoHeader.EncodedLength)
        {
            throw InvalidField("payload");
        }

        if (archive[0] != (byte)'N'
            || archive[1] != (byte)'R'
            || archive[2] != (byte)'T'
            || archive[3] != (byte)'0'
            || archive[4] != 0
            || archive[5] != 0)
        {
            throw InvalidField("payload");
        }

        var expectedSchema = NoritoCodec.SchemaHash(typeName);
        if (!archive.AsSpan(6, expectedSchema.Length).SequenceEqual(expectedSchema))
        {
            throw InvalidField("schema");
        }

        if (archive[22] != (byte)NoritoCompression.None)
        {
            throw InvalidField("layout");
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, sizeof(ulong)));
        if (payloadLength > int.MaxValue
            || payloadLength > (ulong)(archive.Length - NoritoHeader.EncodedLength))
        {
            throw InvalidField("payload");
        }

        var flags = archive[39];
        if (flags != CompactLenFlag)
        {
            throw InvalidField("layout");
        }

        var payloadLengthInt = (int)payloadLength;
        var minimumLength = NoritoHeader.EncodedLength + payloadLengthInt;
        if (minimumLength > archive.Length)
        {
            throw InvalidField("payload");
        }

        var paddingLength = archive.Length - minimumLength;
        if (paddingLength > MaxNoritoHeaderPaddingBytes)
        {
            throw InvalidField("payload");
        }

        for (var index = 0; index < paddingLength; index++)
        {
            if (archive[NoritoHeader.EncodedLength + index] != 0)
            {
                throw InvalidField("payload");
            }
        }

        var payloadOffset = NoritoHeader.EncodedLength + paddingLength;
        var decodedPayload = archive.AsSpan(payloadOffset, payloadLengthInt).ToArray();
        var expectedChecksum = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(31, sizeof(ulong)));
        if (Crc64Ecma.Compute(decodedPayload) != expectedChecksum)
        {
            throw InvalidField("checksum");
        }

        return decodedPayload;
    }

    internal static void RequireNoTrailing(byte[] payload, int offset, string field)
    {
        if (offset != payload.Length)
        {
            throw InvalidField(field);
        }
    }

    private static byte[] ParseAssetDefinitionAddress(string value)
    {
        var exact = RequireExactNonBlankText(value, "asset_definition_id", nameof(value));
        if (exact.IndexOfAny([':', '#', '@', '$']) >= 0)
        {
            throw new ArgumentException($"Invalid asset definition id `{value}`.", nameof(value));
        }

        var payload = DecodeBase58(exact);
        if (payload.Length != AssetDefinitionAddressPayloadLength || payload[0] != AssetDefinitionAddressVersion)
        {
            throw new ArgumentException($"Invalid asset definition id `{value}`.", nameof(value));
        }

        var definitionBytes = payload.AsSpan(1, AssetDefinitionBytesLength).ToArray();
        var expectedChecksum = Blake3Hash32(payload.AsSpan(0, 1 + AssetDefinitionBytesLength).ToArray())[..4];
        var actualChecksum = payload.AsSpan(1 + AssetDefinitionBytesLength, 4);
        if (!actualChecksum.SequenceEqual(expectedChecksum))
        {
            throw new ArgumentException($"Invalid asset definition id checksum `{value}`.", nameof(value));
        }

        ValidateAssetDefinitionBytes(definitionBytes);
        if (!string.Equals(EncodeAssetDefinitionAddress(definitionBytes), exact, StringComparison.Ordinal))
        {
            throw new ArgumentException($"Invalid asset definition id `{value}`.", nameof(value));
        }

        return definitionBytes;
    }

    private static string EncodeAssetDefinitionAddress(byte[] definitionBytes)
    {
        ValidateAssetDefinitionBytes(definitionBytes);
        var payload = new byte[AssetDefinitionAddressPayloadLength];
        payload[0] = AssetDefinitionAddressVersion;
        definitionBytes.CopyTo(payload.AsSpan(1));
        var checksum = Blake3Hash32(payload.AsSpan(0, 1 + AssetDefinitionBytesLength).ToArray());
        checksum.AsSpan(0, 4).CopyTo(payload.AsSpan(1 + AssetDefinitionBytesLength));
        return EncodeBase58(payload);
    }

    private static void ValidateAssetDefinitionBytes(byte[] definitionBytes)
    {
        if (definitionBytes.Length != AssetDefinitionBytesLength)
        {
            throw InvalidField("asset_definition");
        }

        if ((definitionBytes[6] & 0xF0) != 0x40)
        {
            throw InvalidField("asset_definition");
        }

        if ((definitionBytes[8] & 0xC0) != 0x80)
        {
            throw InvalidField("asset_definition");
        }
    }

    private static byte[] DecodeBase58(string literal)
    {
        var zeroCount = literal.TakeWhile(static character => character == '1').Count();
        var bytes = new List<byte> { 0 };
        foreach (var character in literal)
        {
            if (!Base58Alphabet.TryGetValue(character, out var value))
            {
                throw new ArgumentException($"Invalid asset definition id `{literal}`.", nameof(literal));
            }

            var carry = value;
            for (var index = 0; index < bytes.Count; index++)
            {
                var total = (bytes[index] * 58) + carry;
                bytes[index] = (byte)(total & 0xFF);
                carry = total >> 8;
            }

            while (carry > 0)
            {
                bytes.Add((byte)(carry & 0xFF));
                carry >>= 8;
            }
        }

        var decoded = new byte[zeroCount + bytes.Count];
        for (var index = 0; index < zeroCount; index++)
        {
            decoded[index] = 0;
        }

        for (var index = 0; index < bytes.Count; index++)
        {
            decoded[decoded.Length - 1 - index] = bytes[index];
        }

        return decoded;
    }

    private static string EncodeBase58(byte[] input)
    {
        if (input.Length == 0)
        {
            return Base58Symbols[0].ToString();
        }

        var value = new BigInteger(input, isUnsigned: true, isBigEndian: true);
        var builder = new StringBuilder();
        while (value > BigInteger.Zero)
        {
            value = BigInteger.DivRem(value, 58, out var remainder);
            builder.Append(Base58Symbols[(int)remainder]);
        }

        foreach (var b in input)
        {
            if (b != 0)
            {
                break;
            }

            builder.Append(Base58Symbols[0]);
        }

        if (builder.Length == 0)
        {
            builder.Append(Base58Symbols[0]);
        }

        var chars = builder.ToString().ToCharArray();
        Array.Reverse(chars);
        return new string(chars);
    }

    private static byte[] Blake3Hash32(byte[] input)
    {
        if (input.Length > 64)
        {
            throw new ArgumentException("single-block BLAKE3 helper only supports payloads up to 64 bytes.", nameof(input));
        }

        var blockBytes = new byte[64];
        input.CopyTo(blockBytes.AsSpan());
        Span<uint> block = stackalloc uint[16];
        for (var index = 0; index < block.Length; index++)
        {
            block[index] = BinaryPrimitives.ReadUInt32LittleEndian(blockBytes.AsSpan(index * 4, 4));
        }

        var outputWords = Blake3Compress(
            Blake3Iv,
            block.ToArray(),
            0,
            (uint)input.Length,
            Blake3ChunkStart | Blake3ChunkEnd | Blake3Root);
        var output = new byte[HashLength];
        for (var index = 0; index < 8; index++)
        {
            BinaryPrimitives.WriteUInt32LittleEndian(output.AsSpan(index * 4, 4), outputWords[index]);
        }

        return output;
    }

    private const uint Blake3ChunkStart = 1;
    private const uint Blake3ChunkEnd = 2;
    private const uint Blake3Root = 8;

    private static readonly uint[] Blake3Iv =
    [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
        0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
    ];

    private static readonly byte[] Blake3MsgPermutation =
    [
        2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8,
    ];

    private static uint[] Blake3Compress(
        IReadOnlyList<uint> chainingValue,
        IReadOnlyList<uint> blockWords,
        ulong counter,
        uint blockLength,
        uint flags)
    {
        var state = new uint[16];
        for (var index = 0; index < 8; index++)
        {
            state[index] = chainingValue[index];
            state[index + 8] = Blake3Iv[index];
        }

        state[12] = (uint)counter;
        state[13] = (uint)(counter >> 32);
        state[14] = blockLength;
        state[15] = flags;

        var message = blockWords.ToArray();
        for (var round = 0; round < 7; round++)
        {
            Blake3Round(state, message);
            if (round == 6)
            {
                continue;
            }

            var permuted = new uint[16];
            for (var index = 0; index < permuted.Length; index++)
            {
                permuted[index] = message[Blake3MsgPermutation[index]];
            }

            message = permuted;
        }

        var output = new uint[16];
        for (var index = 0; index < 8; index++)
        {
            output[index] = state[index] ^ state[index + 8];
            output[index + 8] = state[index + 8] ^ chainingValue[index];
        }

        return output;
    }

    private static void Blake3Round(uint[] state, IReadOnlyList<uint> message)
    {
        Blake3G(state, 0, 4, 8, 12, message[0], message[1]);
        Blake3G(state, 1, 5, 9, 13, message[2], message[3]);
        Blake3G(state, 2, 6, 10, 14, message[4], message[5]);
        Blake3G(state, 3, 7, 11, 15, message[6], message[7]);
        Blake3G(state, 0, 5, 10, 15, message[8], message[9]);
        Blake3G(state, 1, 6, 11, 12, message[10], message[11]);
        Blake3G(state, 2, 7, 8, 13, message[12], message[13]);
        Blake3G(state, 3, 4, 9, 14, message[14], message[15]);
    }

    private static void Blake3G(uint[] state, int a, int b, int c, int d, uint mx, uint my)
    {
        state[a] = unchecked(state[a] + state[b] + mx);
        state[d] = BitOperations.RotateRight(state[d] ^ state[a], 16);
        state[c] = unchecked(state[c] + state[d]);
        state[b] = BitOperations.RotateRight(state[b] ^ state[c], 12);
        state[a] = unchecked(state[a] + state[b] + my);
        state[d] = BitOperations.RotateRight(state[d] ^ state[a], 8);
        state[c] = unchecked(state[c] + state[d]);
        state[b] = BitOperations.RotateRight(state[b] ^ state[c], 7);
    }

    internal static ArgumentException InvalidField(string field)
    {
        return new ArgumentException($"Offline Note canonical payload field {field} is invalid.");
    }

    private sealed record ParsedAssetId(string AccountId, byte[] DefinitionBytes, ulong? DataspaceId);

    private sealed record NumericValue(byte[] MantissaBytes, uint Scale, string CanonicalString);
}
