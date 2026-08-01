using System.Buffers.Binary;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sccp;

/// <summary>Closed bridge payload kinds admitted in SCCP V1.</summary>
public enum SccpPayloadKindV1
{
    Transfer,
}

public static class SccpPayloadKindV1Extensions
{
    public static string WireKey(this SccpPayloadKindV1 kind) => kind switch
    {
        SccpPayloadKindV1.Transfer => "transfer",
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };

    public static SccpPayloadKindV1 ParseWireKey(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        foreach (var candidate in Enum.GetValues<SccpPayloadKindV1>())
        {
            if (string.Equals(candidate.WireKey(), value, StringComparison.Ordinal))
            {
                return candidate;
            }
        }

        throw new ArgumentException("SCCP payload kind is unknown or retired.", nameof(value));
    }
}

/// <summary>Request payload for <c>POST /v1/bridge/proofs/submit</c>.</summary>
public sealed class SccpBridgeProofSubmitRequest
{
    public SccpBridgeProofSubmitRequest(
        string authority,
        string destinationProofBase64,
        FeePaymentIntent feePayment,
        string? signatureBase64 = null,
        string? transactionPayloadBase64 = null,
        ulong? creationTimeMs = null)
    {
        Authority = SccpSubmitValidation.Authority(authority);
        ArgumentNullException.ThrowIfNull(feePayment);
        FeePayment = feePayment;
        var destinationProof = SccpSubmitValidation.CanonicalNoritoBase64(
            destinationProofBase64,
            "destination_proof_b64",
            SccpSubmitValidation.MaximumDestinationArtifactBytes,
            SccpSubmitValidation.DestinationArtifactSchemaName);
        DestinationProofBase64 = destinationProofBase64;
        if (creationTimeMs == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(creationTimeMs), "creation_time_ms must be positive.");
        }

        CreationTimeMs = creationTimeMs;
        (SignatureBase64, TransactionPayloadBase64) =
            SccpSubmitValidation.DetachedSubmissionState(
                Authority,
                signatureBase64,
                transactionPayloadBase64,
                creationTimeMs,
                destinationProof,
                expectedDestinationProof: true,
                expectedFeePayment: feePayment);
    }

    [JsonPropertyName("authority")]
    public string Authority { get; }

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; }

    [JsonPropertyName("signature_b64")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? SignatureBase64 { get; }

    [JsonPropertyName("transaction_payload_b64")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? TransactionPayloadBase64 { get; }

    [JsonPropertyName("destination_proof_b64")]
    public string DestinationProofBase64 { get; }

    [JsonPropertyName("creation_time_ms")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? CreationTimeMs { get; }
}

/// <summary>Native-proof-only request for <c>POST /v1/bridge/messages</c>.</summary>
public sealed class SccpBridgeMessageSubmitRequest
{
    public SccpBridgeMessageSubmitRequest(
        string authority,
        string nativeProofBase64,
        FeePaymentIntent feePayment,
        string? signatureBase64 = null,
        string? transactionPayloadBase64 = null,
        ulong? creationTimeMs = null)
    {
        Authority = SccpSubmitValidation.Authority(authority);
        ArgumentNullException.ThrowIfNull(feePayment);
        FeePayment = feePayment;
        var nativeProof = SccpSubmitValidation.CanonicalNoritoBase64(
            nativeProofBase64,
            "native_proof_b64",
            SccpSubmitValidation.MaximumNativeArtifactBytes,
            SccpSubmitValidation.NativeInboundProofSchemaName);
        NativeProofBase64 = nativeProofBase64;
        if (creationTimeMs == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(creationTimeMs), "creation_time_ms must be positive.");
        }

        CreationTimeMs = creationTimeMs;
        (SignatureBase64, TransactionPayloadBase64) =
            SccpSubmitValidation.DetachedSubmissionState(
                Authority,
                signatureBase64,
                transactionPayloadBase64,
                creationTimeMs,
                nativeProof,
                expectedDestinationProof: false,
                expectedFeePayment: feePayment);
    }

    [JsonPropertyName("authority")]
    public string Authority { get; }

    [JsonPropertyName("fee_payment")]
    public FeePaymentIntent FeePayment { get; }

    [JsonPropertyName("signature_b64")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? SignatureBase64 { get; }

    [JsonPropertyName("transaction_payload_b64")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? TransactionPayloadBase64 { get; }

    [JsonPropertyName("native_proof_b64")]
    public string NativeProofBase64 { get; }

    [JsonPropertyName("creation_time_ms")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? CreationTimeMs { get; }
}

/// <summary>Optional request-bound checks for a bridge response.</summary>
public sealed record SccpBridgeResponseExpectation(
    SccpPayloadKindV1? PayloadKind = null,
    string? MessageIdHex = null,
    uint? CounterpartyDomain = null,
    SccpNetworkV1? CounterpartyChain = null,
    ulong? CreationTimeMs = null,
    string? Backend = null,
    string? RouteConfigurationHashHex = null,
    ulong? RangeStartHeight = null,
    ulong? RangeEndHeight = null,
    FeePaymentIntent? FeePayment = null)
{
    public void Validate()
    {
        if (PayloadKind is { } payloadKind)
        {
            _ = payloadKind.WireKey();
        }

        if (MessageIdHex is not null)
        {
            SccpSubmitValidation.ResponseHash(MessageIdHex, nameof(MessageIdHex));
        }

        if (CounterpartyDomain is not null and not (1 or 2 or 5))
        {
            throw new ArgumentOutOfRangeException(nameof(CounterpartyDomain));
        }

        if (CounterpartyChain is { } chain && !chain.IsExternal())
        {
            throw new ArgumentException("Expected counterparty chain must be external.", nameof(CounterpartyChain));
        }

        if (CounterpartyDomain is { } domain && CounterpartyChain is { } profile
            && domain != profile.DomainId())
        {
            throw new ArgumentException("Expected SCCP counterparty profile/domain mismatch.");
        }

        if (CreationTimeMs == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(CreationTimeMs));
        }

        if (Backend is not null)
        {
            SccpSubmitValidation.RequireClosedBackend(Backend, CounterpartyChain);
        }

        if (RouteConfigurationHashHex is not null)
        {
            SccpSubmitValidation.ResponseHash(
                RouteConfigurationHashHex,
                nameof(RouteConfigurationHashHex));
        }

        if (RangeStartHeight == 0
            || RangeEndHeight == 0
            || RangeStartHeight is { } start && RangeEndHeight is { } end && end < start)
        {
            throw new ArgumentOutOfRangeException(nameof(RangeEndHeight));
        }
    }
}

/// <summary>Exact unified two-phase response from either SCCP submit endpoint.</summary>
public sealed record SccpBridgeSubmitResponse(
    bool Submitted,
    SccpPayloadKindV1 PayloadKind,
    string MessageIdHex,
    string Backend,
    uint CounterpartyDomain,
    SccpNetworkV1 CounterpartyChain,
    string RouteConfigurationHashHex,
    ulong RangeStartHeight,
    ulong RangeEndHeight,
    ulong CreationTimeMs,
    string? TxHashHex,
    string? TransactionPayloadBase64,
    string? SigningMessageBase64)
{
    private static readonly HashSet<string> Fields =
    [
        "submitted", "payload_kind", "message_id_hex", "backend", "counterparty_domain",
        "counterparty_chain", "route_configuration_hash_hex", "range_start_height", "range_end_height",
        "creation_time_ms", "tx_hash_hex", "transaction_payload_b64", "signing_message_b64",
    ];

    public static SccpBridgeSubmitResponse Parse(
        ReadOnlyMemory<byte> json,
        SccpBridgeResponseExpectation? expectation = null) =>
        ParseCore(json, expectation, expectedAuthority: null, expectedProof: null);

    internal static SccpBridgeSubmitResponse ParseForRequest(
        ReadOnlyMemory<byte> json,
        SccpBridgeResponseExpectation? expectation,
        string expectedAuthority,
        byte[] expectedProof,
        string? expectedTransactionPayloadBase64 = null,
        string? expectedSignatureBase64 = null,
        FeePaymentIntent? expectedFeePayment = null) =>
        ParseCore(
            json,
            expectation,
            expectedAuthority,
            expectedProof,
            expectedTransactionPayloadBase64,
            expectedSignatureBase64,
            expectedFeePayment);

    private static SccpBridgeSubmitResponse ParseCore(
        ReadOnlyMemory<byte> json,
        SccpBridgeResponseExpectation? expectation,
        string? expectedAuthority,
        byte[]? expectedProof,
        string? expectedTransactionPayloadBase64 = null,
        string? expectedSignatureBase64 = null,
        FeePaymentIntent? requestFeePayment = null)
    {
        using var document = SccpJson.Parse(json, "bridge submit response");
        var root = document.RootElement;
        SccpJson.ExactFields(root, Fields, "bridge submit response");
        var submitted = SccpJson.Boolean(root, "submitted");
        var payloadKind = SccpPayloadKindV1Extensions.ParseWireKey(SccpJson.Text(root, "payload_kind"));
        var messageId = SccpSubmitValidation.ResponseHash(SccpJson.Text(root, "message_id_hex"), "message_id_hex");
        var backend = SccpJson.Text(root, "backend");
        var domain = SccpJson.UInt32(root, "counterparty_domain", 1, 5);
        if (domain is not (1 or 2 or 5))
        {
            throw new ArgumentException("counterparty_domain is unsupported or retired.");
        }

        var chain = SccpNetworkV1Extensions.ParseProfileKey(SccpJson.Text(root, "counterparty_chain"));
        if (!chain.IsExternal() || chain.DomainId() != domain)
        {
            throw new ArgumentException(
                "counterparty_chain and counterparty_domain must identify one exact external network.");
        }

        SccpSubmitValidation.RequireClosedBackend(backend, chain);

        var routeConfigurationHash = SccpSubmitValidation.ResponseHash(
            SccpJson.Text(root, "route_configuration_hash_hex"),
            "route_configuration_hash_hex");
        if (routeConfigurationHash == messageId)
        {
            throw new ArgumentException(
                "SCCP message and route-configuration hash roles must be distinct.");
        }

        var start = SccpJson.UInt64(root, "range_start_height", 1);
        var end = SccpJson.UInt64(root, "range_end_height", start);
        var creation = SccpJson.UInt64(root, "creation_time_ms", 1);
        var txHash = SccpJson.OptionalText(root, "tx_hash_hex");
        if (txHash is not null)
        {
            txHash = SccpSubmitValidation.ResponseHash(txHash, "tx_hash_hex");
            if (txHash == messageId || txHash == routeConfigurationHash)
            {
                throw new ArgumentException(
                    "SCCP transaction, message, and route-configuration hash roles must be distinct.");
            }
        }

        var payloadBase64 = SccpJson.OptionalText(root, "transaction_payload_b64");
        var signingBase64 = SccpJson.OptionalText(root, "signing_message_b64");
        var requestStateKnown = expectedAuthority is not null;
        var requestWasDirect = expectedTransactionPayloadBase64 is not null
            && expectedSignatureBase64 is not null;
        if (requestFeePayment is not null
            && expectation?.FeePayment is not null
            && !requestFeePayment.HasSamePayerAndGasBound(expectation.FeePayment))
        {
            throw new ArgumentException(
                "SCCP request fee_payment contradicts the response expectation.");
        }
        var expectedFeePayment = requestFeePayment ?? expectation?.FeePayment;
        if ((expectedTransactionPayloadBase64 is null) != (expectedSignatureBase64 is null)
            || requestStateKnown && submitted != requestWasDirect)
        {
            throw new ArgumentException(
                "SCCP response submission state contradicts the exact request signing state.");
        }
        if (submitted && requestFeePayment is null
            && expectation?.FeePayment is not null && !requestWasDirect)
        {
            throw new ArgumentException(
                "A fee-payment expectation requires the exact submitted transaction payload.");
        }

        if (submitted)
        {
            if (txHash is null || payloadBase64 is not null || signingBase64 is not null)
            {
                throw new ArgumentException(
                    "Submitted SCCP response must contain tx_hash_hex and no signing payload.");
            }

            if (requestWasDirect)
            {
                var expectedTransactionHash =
                    SccpSubmitValidation.RequireCanonicalDirectSubmission(
                        expectedTransactionPayloadBase64!,
                        expectedSignatureBase64!,
                        creation,
                        backend,
                        Convert.FromHexString(routeConfigurationHash),
                        start,
                        end,
                        expectedAuthority!,
                        expectedProof,
                        expectedFeePayment);
                if (!string.Equals(txHash, expectedTransactionHash, StringComparison.Ordinal))
                {
                    throw new ArgumentException(
                        "tx_hash_hex does not match the exact detached transaction intent.");
                }
            }
        }
        else
        {
            if (txHash is not null || payloadBase64 is null || signingBase64 is null)
            {
                throw new ArgumentException(
                    "Prepared SCCP response requires transaction_payload_b64 and signing_message_b64.");
            }

            var payload = SccpSubmitValidation.CanonicalBase64(
                payloadBase64,
                "transaction_payload_b64",
                maximumBytes: SccpSubmitValidation.MaximumTransactionPayloadBytes);
            var signing = SccpSubmitValidation.CanonicalBase64(
                signingBase64,
                "signing_message_b64",
                exactBytes: IrohaHash.Length);
            SccpSubmitValidation.RequireCanonicalTransactionPayload(
                payload,
                creation,
                backend,
                Convert.FromHexString(routeConfigurationHash),
                start,
                end,
                expectedAuthority,
                expectedProof,
                expectedFeePayment);
            if (!signing.AsSpan().SequenceEqual(IrohaHash.Hash(payload)))
            {
                throw new ArgumentException(
                    "signing_message_b64 must be the exact transaction-payload prehash.");
            }
        }

        var response = new SccpBridgeSubmitResponse(
            submitted,
            payloadKind,
            messageId,
            backend,
            domain,
            chain,
            routeConfigurationHash,
            start,
            end,
            creation,
            txHash,
            payloadBase64,
            signingBase64);
        response.RequireExpectation(expectation);
        return response;
    }

    private void RequireExpectation(SccpBridgeResponseExpectation? expectation)
    {
        if (expectation is null)
        {
            return;
        }

        expectation.Validate();
        if (expectation.PayloadKind is { } payloadKind && payloadKind != PayloadKind
            || expectation.MessageIdHex is { } messageId && messageId != MessageIdHex
            || expectation.CounterpartyDomain is { } domain && domain != CounterpartyDomain
            || expectation.CounterpartyChain is { } chain && chain != CounterpartyChain
            || expectation.CreationTimeMs is { } creation && creation != CreationTimeMs
            || expectation.Backend is { } backend && backend != Backend
            || expectation.RouteConfigurationHashHex is { } routeHash
                && routeHash != RouteConfigurationHashHex
            || expectation.RangeStartHeight is { } start && start != RangeStartHeight
            || expectation.RangeEndHeight is { } end && end != RangeEndHeight)
        {
            throw new ArgumentException("Bridge submit response does not match its request expectation.");
        }
    }
}

internal static class SccpSubmitValidation
{
    internal const string DestinationArtifactSchemaName =
        "iroha_sccp::SccpGroth16Bn254ProofArtifactV1";
    internal const string NativeInboundProofSchemaName =
        "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1";
    private const string SubmitBridgeProofWireName =
        "iroha_data_model::isi::bridge::SubmitBridgeProof";
    private const string TairaChainId = "fc56984b-2be7-431d-840e-21514d1883f0";
    internal const int MaximumNativeArtifactBytes = 16 * 1024 * 1024;
    internal const int MaximumDestinationArtifactBytes = MaximumNativeArtifactBytes + 64 * 1024;
    internal const int MaximumArtifactBytes = MaximumDestinationArtifactBytes;
    internal const int MaximumTransactionPayloadBytes = MaximumArtifactBytes + 1024 * 1024;
    internal const int MaximumJsonBytes = MaximumArtifactBytes * 2 + 1024 * 1024;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    internal static void RequireClosedBackend(string backend, SccpNetworkV1? chain = null)
    {
        ArgumentNullException.ThrowIfNull(backend);
        var valid = chain is null
            ? backend is
                "evm-groth16-bn254-v1"
                    or "tron-groth16-bn254-v1"
                    or "bridge/sccp/native/ethereum-beacon-v1"
                    or "bridge/sccp/native/bsc-parlia-v1"
                    or "bridge/sccp/native/tron-dpos-v1"
            : BackendSupports(backend, chain.Value);
        if (!valid)
        {
            throw new ArgumentException("backend must match one closed SCCP V1 counterparty family.", nameof(backend));
        }
    }

    private static bool BackendSupports(string backend, SccpNetworkV1 chain) => backend switch
    {
        "evm-groth16-bn254-v1" => chain.DomainId() is 1 or 2,
        "tron-groth16-bn254-v1" => chain.DomainId() == 5,
        "bridge/sccp/native/ethereum-beacon-v1" => SccpNativeBackendV1.EthereumBeacon.Supports(chain),
        "bridge/sccp/native/bsc-parlia-v1" => SccpNativeBackendV1.BscParlia.Supports(chain),
        "bridge/sccp/native/tron-dpos-v1" => SccpNativeBackendV1.TronDpos.Supports(chain),
        _ => false,
    };

    internal static string Authority(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        AccountAddress address;
        try
        {
            address = AccountAddress.Parse(value, AccountAddress.TestChainDiscriminant);
        }
        catch (Exception error) when (error is ArgumentException or FormatException)
        {
            throw new ArgumentException(
                "authority must be an exact canonical Taira/test AccountId.",
                nameof(value),
                error);
        }

        if (!string.Equals(
                address.ToI105(AccountAddress.TestChainDiscriminant),
                value,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "authority must be an exact canonical Taira/test AccountId.",
                nameof(value));
        }

        return value;
    }

    internal static (string? SignatureBase64, string? TransactionPayloadBase64)
        DetachedSubmissionState(
            string authority,
            string? signatureBase64,
            string? transactionPayloadBase64,
            ulong? creationTimeMs,
            byte[] expectedProof,
            bool expectedDestinationProof,
            FeePaymentIntent expectedFeePayment)
    {
        if ((signatureBase64 is null) != (transactionPayloadBase64 is null))
        {
            throw new ArgumentException(
                "Preparation requires neither signature_b64 nor transaction_payload_b64; direct submission requires both.");
        }

        if (signatureBase64 is null)
        {
            return (null, null);
        }

        if (creationTimeMs is null or 0)
        {
            throw new ArgumentException(
                "Direct SCCP submission requires an explicit positive creation_time_ms.",
                nameof(creationTimeMs));
        }

        var address = AccountAddress.Parse(authority, AccountAddress.TestChainDiscriminant);
        if (address.AddressClass != AddressClass.SingleKey
            || !string.Equals(address.Algorithm, "ed25519", StringComparison.Ordinal)
            || address.PublicKey.Length != Ed25519Signer.PublicKeyLength)
        {
            throw new ArgumentException(
                "Direct SCCP submission requires a single-key Ed25519 authority.",
                nameof(authority));
        }

        var signature = OptionalSignature(signatureBase64)!;
        var payload = CanonicalBase64(
            transactionPayloadBase64!,
            "transaction_payload_b64",
            maximumBytes: MaximumTransactionPayloadBytes);
        InspectCanonicalDetachedPayload(
            payload,
            creationTimeMs.Value,
            authority,
            expectedProof,
            expectedDestinationProof,
            expectedFeePayment);
        if (!Ed25519Signer.Verify(
                IrohaHash.Hash(payload),
                Convert.FromBase64String(signature),
                address.PublicKey))
        {
            throw new ArgumentException(
                "signature_b64 does not verify the exact transaction payload for authority.");
        }

        return (signature, transactionPayloadBase64);
    }

    private static string? OptionalSignature(string? signatureBase64)
    {
        if (signatureBase64 is null)
        {
            return null;
        }

        var signature = CanonicalBase64(signatureBase64, "signature_b64", exactBytes: 64);
        if (!CanonicalEd25519Point(signature.AsSpan(0, 32))
            || !CanonicalEd25519Scalar(signature.AsSpan(32, 32)))
        {
            throw new ArgumentException("signature_b64 must contain one canonical Ed25519 signature.");
        }

        return signatureBase64;
    }

    internal static string RequireCanonicalDirectSubmission(
        string transactionPayloadBase64,
        string signatureBase64,
        ulong creationTimeMs,
        string backend,
        byte[] routeConfigurationHash,
        ulong rangeStartHeight,
        ulong rangeEndHeight,
        string expectedAuthority,
        byte[]? expectedProof,
        FeePaymentIntent? expectedFeePayment = null)
    {
        var payload = CanonicalBase64(
            transactionPayloadBase64,
            "transaction_payload_b64",
            maximumBytes: MaximumTransactionPayloadBytes);
        var exactSignature = OptionalSignature(signatureBase64)
            ?? throw new ArgumentException("signature_b64 is required for direct submission.");
        var signature = Convert.FromBase64String(exactSignature);
        RequireCanonicalTransactionPayload(
            payload,
            creationTimeMs,
            backend,
            routeConfigurationHash,
            rangeStartHeight,
            rangeEndHeight,
            expectedAuthority,
            expectedProof,
            expectedFeePayment);

        var address = AccountAddress.Parse(
            expectedAuthority,
            AccountAddress.TestChainDiscriminant);
        if (address.AddressClass != AddressClass.SingleKey
            || !string.Equals(address.Algorithm, "ed25519", StringComparison.Ordinal)
            || !Ed25519Signer.Verify(IrohaHash.Hash(payload), signature, address.PublicKey))
        {
            throw new ArgumentException(
                "signature_b64 does not verify the exact transaction payload for authority.");
        }

        return Convert.ToHexString(DetachedTransactionHash(payload))
            .ToLowerInvariant();
    }

    private readonly record struct DetachedBridgeBinding(
        string Backend,
        byte[] RouteConfigurationHash,
        ulong RangeStartHeight,
        ulong RangeEndHeight,
        bool IsDestination);

    private static void InspectCanonicalDetachedPayload(
        ReadOnlySpan<byte> payload,
        ulong creationTimeMs,
        string expectedAuthority,
        byte[] expectedProof,
        bool expectedDestinationProof,
        FeePaymentIntent expectedFeePayment)
    {
        var cursor = new CompactTransactionCursor(payload);
        var chain = cursor.TakeField("chain_id");
        var authority = cursor.TakeField("authority");
        var creation = cursor.TakeField("creation_time_ms");
        var executable = cursor.TakeField("executable");
        var timeToLive = cursor.TakeField("time_to_live_ms");
        var nonce = cursor.TakeField("nonce");
        var feePayment = cursor.TakeField("fee_payment");
        var metadata = cursor.TakeField("metadata");
        var attachments = cursor.TakeField("attachments");
        if (!cursor.IsFinished
            || creation.Length != sizeof(ulong)
            || BinaryPrimitives.ReadUInt64LittleEndian(creation) != creationTimeMs)
        {
            throw new ArgumentException(
                "transaction_payload_b64 must contain one canonical transaction payload matching creation_time_ms.");
        }

        RequireCanonicalChainId(chain);
        var controller = RequireCanonicalAuthority(authority);
        var expectedController = AccountAddress
            .Parse(expectedAuthority, AccountAddress.TestChainDiscriminant)
            .ControllerBytes();
        if (!controller.AsSpan().SequenceEqual(expectedController))
        {
            throw new ArgumentException(
                "Transaction authority does not match the SCCP submit request.");
        }

        var binding = InspectCanonicalDetachedExecutable(executable, expectedProof);
        RequireClosedBackend(binding.Backend);
        if (binding.IsDestination != expectedDestinationProof)
        {
            throw new ArgumentException(
                "Detached SCCP transaction proof family does not match the submit endpoint.");
        }

        RequireAbsentOption(timeToLive, "time_to_live_ms");
        RequireAbsentOption(nonce, "nonce");
        RequireCanonicalFeePayment(feePayment, expectedFeePayment);
        RequireCanonicalMetadata(metadata);
        RequireAbsentOption(attachments, "attachments");
    }

    private static DetachedBridgeBinding InspectCanonicalDetachedExecutable(
        ReadOnlySpan<byte> payload,
        byte[] expectedProof)
    {
        var executable = new CompactTransactionCursor(payload);
        if (executable.TakeUInt32("executable.kind") != 0)
        {
            throw new ArgumentException(
                "SCCP transaction executable must contain instructions.");
        }

        var instructions = new CompactTransactionCursor(
            executable.TakeField("executable.instructions"));
        if (!executable.IsFinished
            || instructions.TakeUInt64("executable.instructions.count") != 1)
        {
            throw new ArgumentException(
                "SCCP transaction must contain exactly one instruction.");
        }

        var instruction = new CompactTransactionCursor(
            instructions.TakeField("executable.instruction"));
        var wireName = DecodeCompactString(
            instruction.TakeField("executable.instruction.wire_name"),
            "executable.instruction.wire_name");
        var archive = DecodeRawByteVector(
            instruction.TakeField("executable.instruction.payload"),
            "executable.instruction.payload");
        if (!instructions.IsFinished
            || !instruction.IsFinished
            || wireName != SubmitBridgeProofWireName)
        {
            throw new ArgumentException(
                "SCCP transaction instruction must be exactly SubmitBridgeProof.");
        }

        RequireCanonicalNoritoArchive(
            archive,
            "executable.instruction.payload",
            MaximumTransactionPayloadBytes,
            SubmitBridgeProofWireName);
        return InspectCanonicalDetachedSubmitBridgeProof(archive, expectedProof);
    }

    private static DetachedBridgeBinding InspectCanonicalDetachedSubmitBridgeProof(
        ReadOnlySpan<byte> archive,
        byte[] expectedProof)
    {
        if (archive[39] != 0x02)
        {
            throw new ArgumentException(
                "SubmitBridgeProof must use the canonical SCCP compact Norito layout.");
        }

        var payloadLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(
            archive.Slice(23, 8)));
        var submit = new CompactTransactionCursor(
            archive.Slice(NoritoHeader.EncodedLength, payloadLength));
        var proof = new CompactTransactionCursor(
            submit.TakeField("SubmitBridgeProof.proof"));
        if (!submit.IsFinished)
        {
            throw new ArgumentException("SubmitBridgeProof contains trailing fields.");
        }

        var range = new CompactTransactionCursor(
            proof.TakeField("SubmitBridgeProof.proof.range"));
        var start = DecodeFramedUInt64(ref range, "SubmitBridgeProof.proof.range.start");
        var end = DecodeFramedUInt64(ref range, "SubmitBridgeProof.proof.range.end");
        if (!range.IsFinished || start == 0 || end < start)
        {
            throw new ArgumentException("SubmitBridgeProof range is invalid.");
        }

        var proofPayload = new CompactTransactionCursor(
            proof.TakeField("SubmitBridgeProof.proof.payload"));
        var kind = proofPayload.TakeUInt32("SubmitBridgeProof.proof.payload.kind");
        var variant = proofPayload.TakeField("SubmitBridgeProof.proof.payload.value");
        if (!proofPayload.IsFinished)
        {
            throw new ArgumentException("SubmitBridgeProof payload contains trailing bytes.");
        }

        var binding = kind switch
        {
            2 => InspectCanonicalDetachedNativeProof(variant, start, end, expectedProof),
            3 => InspectCanonicalDetachedDestinationProof(variant, start, end, expectedProof),
            0 or 1 => throw new ArgumentException(
                "SCCP SubmitBridgeProof cannot use generic bridge payloads."),
            _ => throw new ArgumentException(
                "SubmitBridgeProof uses an unknown bridge payload."),
        };

        if (!proof.IsFinished)
        {
            throw new ArgumentException(
                "SCCP SubmitBridgeProof must contain no trailing fields.");
        }

        if (binding.RouteConfigurationHash.All(static value => value == 0))
        {
            throw new ArgumentException(
                "SubmitBridgeProof route-configuration binding must be nonzero.");
        }

        return binding;
    }

    private static DetachedBridgeBinding InspectCanonicalDetachedNativeProof(
        ReadOnlySpan<byte> payload,
        ulong start,
        ulong end,
        byte[] expectedProof)
    {
        var cursor = new CompactTransactionCursor(payload);
        var backendField = cursor.TakeField("SubmitBridgeProof.native.backend");
        if (backendField.Length != sizeof(uint))
        {
            throw new ArgumentException("SubmitBridgeProof native backend is malformed.");
        }

        var backend = BinaryPrimitives.ReadUInt32LittleEndian(backendField) switch
        {
            0 => "bridge/sccp/native/ethereum-beacon-v1",
            1 => "bridge/sccp/native/bsc-parlia-v1",
            2 => "bridge/sccp/native/tron-dpos-v1",
            _ => throw new ArgumentException("SubmitBridgeProof native backend is unknown."),
        };
        var routeHash = DecodeFixedByteArray(
            cursor.TakeField("SubmitBridgeProof.native.route_configuration_hash"),
            IrohaHash.Length,
            "SubmitBridgeProof.native.route_configuration_hash");
        var envelope = DecodeRawByteVector(
            cursor.TakeField("SubmitBridgeProof.native.encoded_envelope"),
            "SubmitBridgeProof.native.encoded_envelope");
        if (!cursor.IsFinished || !envelope.SequenceEqual(expectedProof))
        {
            throw new ArgumentException(
                "SubmitBridgeProof native proof does not match the submitted proof.");
        }

        RequireCanonicalNoritoArchive(
            envelope,
            "SubmitBridgeProof.native.encoded_envelope",
            MaximumNativeArtifactBytes,
            NativeInboundProofSchemaName);
        return new DetachedBridgeBinding(backend, routeHash, start, end, false);
    }

    private static DetachedBridgeBinding InspectCanonicalDetachedDestinationProof(
        ReadOnlySpan<byte> payload,
        ulong start,
        ulong end,
        byte[] expectedProof)
    {
        var cursor = new CompactTransactionCursor(payload);
        var backendField = cursor.TakeField("SubmitBridgeProof.destination.backend");
        if (backendField.Length != sizeof(uint))
        {
            throw new ArgumentException("SubmitBridgeProof destination backend is malformed.");
        }

        var backend = BinaryPrimitives.ReadUInt32LittleEndian(backendField) switch
        {
            0 => "evm-groth16-bn254-v1",
            1 => "tron-groth16-bn254-v1",
            _ => throw new ArgumentException("SubmitBridgeProof destination backend is unknown."),
        };
        var routeHash = DecodeFixedByteArray(
            cursor.TakeField("SubmitBridgeProof.destination.route_configuration_hash"),
            IrohaHash.Length,
            "SubmitBridgeProof.destination.route_configuration_hash");
        var artifact = DecodeRawByteVector(
            cursor.TakeField("SubmitBridgeProof.destination.encoded_artifact"),
            "SubmitBridgeProof.destination.encoded_artifact");
        if (!cursor.IsFinished || !artifact.SequenceEqual(expectedProof))
        {
            throw new ArgumentException(
                "SubmitBridgeProof destination artifact does not match the submitted proof.");
        }

        RequireCanonicalNoritoArchive(
            artifact,
            "SubmitBridgeProof.destination.encoded_artifact",
            MaximumDestinationArtifactBytes,
            DestinationArtifactSchemaName);
        return new DetachedBridgeBinding(backend, routeHash, start, end, true);
    }

    internal static byte[] CanonicalNoritoBase64(
        string value,
        string field,
        int maximumBytes,
        string? expectedSchemaName = null)
    {
        var archive = CanonicalBase64(value, field, maximumBytes: maximumBytes);
        RequireCanonicalNoritoArchive(archive, field, maximumBytes, expectedSchemaName);
        return archive;
    }

    private static void RequireCanonicalNoritoArchive(
        ReadOnlySpan<byte> archive,
        string field,
        int maximumBytes,
        string? expectedSchemaName = null)
    {
        if (archive.Length < NoritoHeader.EncodedLength
            || archive.Length > maximumBytes
            || !archive.Slice(0, 4).SequenceEqual("NRT0"u8)
            || archive[4] != 0 || archive[5] != 0
            || archive.Slice(6, 16).IndexOfAnyExcept((byte)0) < 0
            || archive[22] != (byte)NoritoCompression.None
            || !SupportedNoritoFlags(archive[39]))
        {
            throw new ArgumentException($"{field} must contain one canonical uncompressed Norito envelope.");
        }

        if (expectedSchemaName is not null
            && !archive.Slice(6, 16).SequenceEqual(NoritoCodec.SchemaHash(expectedSchemaName)))
        {
            throw new ArgumentException($"{field} does not contain the closed SCCP artifact schema.");
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.Slice(23, 8));
        if (payloadLength > int.MaxValue || payloadLength > (ulong)(archive.Length - NoritoHeader.EncodedLength))
        {
            throw new ArgumentException($"{field} contains an invalid Norito payload length.");
        }

        var padding = archive.Length - NoritoHeader.EncodedLength - (int)payloadLength;
        if (padding != 0)
        {
            throw new ArgumentException($"{field} contains non-canonical Norito header padding.");
        }

        var payload = archive.Slice(NoritoHeader.EncodedLength + padding, (int)payloadLength);
        var checksum = BinaryPrimitives.ReadUInt64LittleEndian(archive.Slice(31, 8));
        if (Crc64Ecma.Compute(payload) != checksum)
        {
            throw new ArgumentException($"{field} has an invalid Norito checksum.");
        }
    }

    internal static byte[] CanonicalBase64(
        string value,
        string field,
        int? exactBytes = null,
        int maximumBytes = MaximumArtifactBytes)
    {
        ArgumentNullException.ThrowIfNull(value);
        var encodedLimit = checked(((maximumBytes + 2) / 3) * 4);
        if (value.Length == 0 || value.Length > encodedLimit || value.Length % 4 != 0
            || value.Any(static character =>
                character is not (>= 'A' and <= 'Z')
                    and not (>= 'a' and <= 'z')
                    and not (>= '0' and <= '9')
                    and not ('+' or '/' or '=')))
        {
            throw new ArgumentException($"{field} must be bounded canonical padded base64.", field);
        }

        if (exactBytes is { } exact
            && value.Length != checked(((exact + 2) / 3) * 4))
        {
            throw new ArgumentException($"{field} must contain exactly {exact} bytes.", field);
        }

        byte[] decoded;
        try
        {
            decoded = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new ArgumentException($"{field} must be canonical padded base64.", field, error);
        }

        if (decoded.Length == 0
            || decoded.Length > maximumBytes
            || !string.Equals(Convert.ToBase64String(decoded), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must be canonical nonempty padded base64.", field);
        }

        if (exactBytes is { } expected && decoded.Length != expected)
        {
            throw new ArgumentException($"{field} must contain exactly {expected} bytes.", field);
        }

        return decoded;
    }

    private static bool SupportedNoritoFlags(byte flags) =>
        (flags & ~0x27) == 0 && ((flags & 0x20) == 0 || (flags & 0x06) == 0x06);

    private static byte[] DetachedTransactionHash(ReadOnlySpan<byte> payload)
    {
        using var entrypoint = new MemoryStream();
        Span<byte> variant = stackalloc byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(variant, 0);
        entrypoint.Write(variant);
        WriteFixedField(entrypoint, payload);
        return IrohaHash.Hash(entrypoint.ToArray());
    }

    private static void WriteFixedField(Stream output, ReadOnlySpan<byte> value)
    {
        WriteLittleEndianUInt64(output, checked((ulong)value.Length));
        output.Write(value);
    }

    private static void WriteLittleEndianUInt64(Stream output, ulong value)
    {
        Span<byte> encoded = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(encoded, value);
        output.Write(encoded);
    }

    internal static void RequireCanonicalTransactionPayload(
        ReadOnlySpan<byte> payload,
        ulong creationTimeMs,
        string backend,
        byte[] routeConfigurationHash,
        ulong rangeStartHeight,
        ulong rangeEndHeight,
        string? expectedAuthority,
        byte[]? expectedProof,
        FeePaymentIntent? expectedFeePayment = null)
    {
        var cursor = new CompactTransactionCursor(payload);
        var chain = cursor.TakeField("chain_id");
        var authority = cursor.TakeField("authority");
        var creation = cursor.TakeField("creation_time_ms");
        var executable = cursor.TakeField("executable");
        var timeToLive = cursor.TakeField("time_to_live_ms");
        var nonce = cursor.TakeField("nonce");
        var feePayment = cursor.TakeField("fee_payment");
        var metadata = cursor.TakeField("metadata");
        var attachments = cursor.TakeField("attachments");
        if (!cursor.IsFinished
            || creation.Length != sizeof(ulong)
            || BinaryPrimitives.ReadUInt64LittleEndian(creation) != creationTimeMs)
        {
            throw new ArgumentException(
                "transaction_payload_b64 must contain one canonical transaction payload matching creation_time_ms.");
        }

        RequireCanonicalChainId(chain);
        var controller = RequireCanonicalAuthority(authority);
        if (expectedAuthority is not null)
        {
            var expectedController = AccountAddress
                .Parse(expectedAuthority, AccountAddress.TestChainDiscriminant)
                .ControllerBytes();
            if (!controller.AsSpan().SequenceEqual(expectedController))
            {
                throw new ArgumentException(
                    "Transaction authority does not match the SCCP submit request.");
            }
        }

        RequireCanonicalExecutable(
            executable,
            backend,
            routeConfigurationHash,
            rangeStartHeight,
            rangeEndHeight,
            expectedProof);
        RequireAbsentOption(timeToLive, "time_to_live_ms");
        RequireAbsentOption(nonce, "nonce");
        RequireCanonicalFeePayment(feePayment, expectedFeePayment);
        RequireCanonicalMetadata(metadata);
        RequireAbsentOption(attachments, "attachments");
    }

    private static void RequireCanonicalChainId(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        var encodedString = cursor.TakeField("chain_id.value");
        var chainId = DecodeCompactString(encodedString, "chain_id.value");
        if (!cursor.IsFinished
            || chainId != TairaChainId)
        {
            throw new ArgumentException("SCCP transaction must target the canonical Taira chain.");
        }
    }

    internal static byte[] RequireCanonicalAuthority(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        var controllerTag = cursor.TakeUInt32("authority.controller");
        byte[] canonicalController;
        switch (controllerTag)
        {
            case 0:
            {
                var publicKey = DecodeByteVector(
                    cursor.TakeField("authority.public_key"),
                    "authority.public_key",
                    byte.MaxValue + 1);
                RequireCanonicalCompactPublicKey(publicKey, byte.MaxValue, "authority.public_key");
                canonicalController = CanonicalSingleController(publicKey);
                break;
            }
            case 1:
                canonicalController = RequireCanonicalMultisigPolicy(
                    cursor.TakeField("authority.multisig"));
                break;
            default:
                throw new ArgumentException("Transaction authority uses an unknown controller tag.");
        }

        if (!cursor.IsFinished)
        {
            throw new ArgumentException("Transaction authority contains trailing bytes.");
        }

        return canonicalController;
    }

    private static byte[] RequireCanonicalMultisigPolicy(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        var version = cursor.TakeByte("authority.multisig.version");
        var threshold = cursor.TakeUInt16("authority.multisig.threshold");
        var memberCount = cursor.TakeUInt64("authority.multisig.members");
        if (version != 1 || threshold == 0 || memberCount is 0 or > ushort.MaxValue)
        {
            throw new ArgumentException("Transaction multisig authority has invalid version, threshold, or member count.");
        }

        ulong totalWeight = 0;
        byte[]? previousMemberSortKey = null;
        var canonicalMembers = new List<(byte[] PublicKey, ushort Weight)>();
        for (var index = 0UL; index < memberCount; index++)
        {
            var member = new CompactTransactionCursor(cursor.TakeField("authority.multisig.member"));
            var publicKey = DecodeByteVector(
                ref member,
                "authority.multisig.member.public_key",
                ushort.MaxValue + 1);
            RequireCanonicalCompactPublicKey(
                publicKey,
                ushort.MaxValue,
                "authority.multisig.member.public_key");
            var memberSortKey = CompactPublicKeySortKey(publicKey);
            var weight = member.TakeUInt16("authority.multisig.member.weight");
            if (weight == 0
                || !member.IsFinished
                || previousMemberSortKey is not null
                    && previousMemberSortKey.AsSpan().SequenceCompareTo(memberSortKey) >= 0)
            {
                throw new ArgumentException(
                    "Transaction multisig members must be nonzero, unique, and canonically sorted.");
            }

            totalWeight = checked(totalWeight + weight);
            previousMemberSortKey = memberSortKey;
            canonicalMembers.Add((publicKey, weight));
        }

        if (!cursor.IsFinished || totalWeight < threshold)
        {
            throw new ArgumentException("Transaction multisig authority is not canonical.");
        }

        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        canonical.WriteByte(version);
        WriteBigEndian(canonical, threshold);
        WriteBigEndian(canonical, checked((ushort)canonicalMembers.Count));
        foreach (var member in canonicalMembers)
        {
            canonical.WriteByte(CanonicalCurveId(member.PublicKey[0]));
            WriteBigEndian(canonical, member.Weight);
            WriteBigEndian(canonical, checked((ushort)(member.PublicKey.Length - 1)));
            canonical.Write(member.PublicKey.AsSpan(1));
        }

        return canonical.ToArray();
    }

    private static byte[] CanonicalSingleController(byte[] publicKey)
    {
        var keyLength = checked((byte)(publicKey.Length - 1));
        var result = new byte[3 + keyLength];
        result[0] = 0;
        result[1] = CanonicalCurveId(publicKey[0]);
        result[2] = keyLength;
        publicKey.AsSpan(1).CopyTo(result.AsSpan(3));
        return result;
    }

    private static byte CanonicalCurveId(byte algorithm) => algorithm switch
    {
        0 => 1,
        1 => 4,
        2 => 3,
        3 => 5,
        4 => 2,
        5 => 10,
        6 => 11,
        7 => 12,
        8 => 13,
        9 => 14,
        10 => 15,
        _ => throw new ArgumentException("Transaction public key algorithm is unknown."),
    };

    private static void WriteBigEndian(Stream output, ushort value)
    {
        Span<byte> encoded = stackalloc byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16BigEndian(encoded, value);
        output.Write(encoded);
    }

    private static byte[] DecodeByteVector(
        ReadOnlySpan<byte> payload,
        string field,
        int maximumBytes)
    {
        var cursor = new CompactTransactionCursor(payload);
        var result = DecodeByteVector(ref cursor, field, maximumBytes);
        if (!cursor.IsFinished)
        {
            throw new ArgumentException($"{field} contains trailing bytes.");
        }

        return result;
    }

    private static byte[] DecodeByteVector(
        ref CompactTransactionCursor cursor,
        string field,
        int maximumBytes)
    {
        var count = cursor.TakeUInt64(field);
        if (count == 0 || count > (ulong)maximumBytes)
        {
            throw new ArgumentException($"{field} length is invalid.");
        }

        var result = new byte[checked((int)count)];
        for (var index = 0; index < result.Length; index++)
        {
            if (cursor.TakeCompactLength(field) != 1)
            {
                throw new ArgumentException($"{field} byte element is not canonically framed.");
            }

            result[index] = cursor.TakeByte(field);
        }

        return result;
    }

    private static void RequireCanonicalCompactPublicKey(
        byte[] payload,
        int maximumKeyBytes,
        string field)
    {
        if (payload.Length is < 2 || payload.Length - 1 > maximumKeyBytes || payload[0] > 10)
        {
            throw new ArgumentException($"{field} is not a closed compact public key.");
        }
    }

    private static byte[] CompactPublicKeySortKey(byte[] payload)
    {
        var algorithm = payload[0] switch
        {
            0 => "ed25519",
            1 => "secp256k1",
            2 => "bls_normal",
            3 => "bls_small",
            4 => "ml-dsa",
            5 => "gost3410-2012-256-paramset-a",
            6 => "gost3410-2012-256-paramset-b",
            7 => "gost3410-2012-256-paramset-c",
            8 => "gost3410-2012-512-paramset-a",
            9 => "gost3410-2012-512-paramset-b",
            10 => "sm2",
            _ => throw new ArgumentException("Transaction public key algorithm is unknown."),
        };
        var prefix = StrictUtf8.GetBytes(algorithm);
        var result = new byte[prefix.Length + payload.Length];
        prefix.CopyTo(result, 0);
        payload.AsSpan(1).CopyTo(result.AsSpan(prefix.Length + 1));
        return result;
    }

    private static void RequireCanonicalExecutable(
        ReadOnlySpan<byte> payload,
        string backend,
        byte[] routeConfigurationHash,
        ulong rangeStartHeight,
        ulong rangeEndHeight,
        byte[]? expectedProof)
    {
        var cursor = new CompactTransactionCursor(payload);
        switch (cursor.TakeUInt32("executable.kind"))
        {
            case 0:
                RequireCanonicalInstructions(
                    cursor.TakeField("executable.instructions"),
                    backend,
                    routeConfigurationHash,
                    rangeStartHeight,
                    rangeEndHeight,
                    expectedProof);
                break;
            case 1 or 2 or 3:
                throw new ArgumentException("SCCP transaction executable must contain instructions.");
            default:
                throw new ArgumentException("Transaction executable variant is unknown.");
        }

        if (!cursor.IsFinished)
        {
            throw new ArgumentException("Transaction executable contains trailing bytes.");
        }
    }

    private static void RequireCanonicalInstructions(
        ReadOnlySpan<byte> payload,
        string backend,
        byte[] routeConfigurationHash,
        ulong rangeStartHeight,
        ulong rangeEndHeight,
        byte[]? expectedProof)
    {
        var cursor = new CompactTransactionCursor(payload);
        var count = cursor.TakeUInt64("executable.instructions.count");
        if (count != 1)
        {
            throw new ArgumentException("SCCP transaction must contain exactly one instruction.");
        }

        for (var index = 0UL; index < count; index++)
        {
            var instruction = new CompactTransactionCursor(cursor.TakeField("executable.instruction"));
            var wireName = DecodeCompactString(
                instruction.TakeField("executable.instruction.wire_name"),
                "executable.instruction.wire_name");
            var archive = DecodeRawByteVector(
                instruction.TakeField("executable.instruction.payload"),
                "executable.instruction.payload");
            if (wireName != SubmitBridgeProofWireName
                || !instruction.IsFinished)
            {
                throw new ArgumentException("SCCP transaction instruction must be SubmitBridgeProof.");
            }

            RequireCanonicalNoritoArchive(
                archive,
                "executable.instruction.payload",
                MaximumTransactionPayloadBytes,
                SubmitBridgeProofWireName);
            RequireCanonicalSubmitBridgeProof(
                archive,
                backend,
                routeConfigurationHash,
                rangeStartHeight,
                rangeEndHeight,
                expectedProof);
        }

        if (!cursor.IsFinished)
        {
            throw new ArgumentException("Transaction instruction sequence contains trailing bytes.");
        }
    }

    private static ReadOnlySpan<byte> DecodeRawByteVector(ReadOnlySpan<byte> payload, string field)
    {
        var cursor = new CompactTransactionCursor(payload);
        var length = cursor.TakeUInt64(field);
        if (length == 0 || length > int.MaxValue)
        {
            throw new ArgumentException($"{field} length is invalid.");
        }

        var result = cursor.TakeExact(checked((int)length), field);
        if (!cursor.IsFinished)
        {
            throw new ArgumentException($"{field} contains trailing bytes.");
        }

        return result;
    }

    private static void RequireCanonicalSubmitBridgeProof(
        ReadOnlySpan<byte> archive,
        string backend,
        byte[] routeConfigurationHash,
        ulong rangeStartHeight,
        ulong rangeEndHeight,
        byte[]? expectedProof)
    {
        if (archive[39] != 0x02)
        {
            throw new ArgumentException(
                "SubmitBridgeProof must use the canonical SCCP compact Norito layout.");
        }

        var payloadLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(archive.Slice(23, 8)));
        var submit = new CompactTransactionCursor(
            archive.Slice(NoritoHeader.EncodedLength, payloadLength));
        var proof = new CompactTransactionCursor(submit.TakeField("SubmitBridgeProof.proof"));
        if (!submit.IsFinished)
        {
            throw new ArgumentException("SubmitBridgeProof contains trailing fields.");
        }

        var range = new CompactTransactionCursor(proof.TakeField("SubmitBridgeProof.proof.range"));
        var start = DecodeFramedUInt64(ref range, "SubmitBridgeProof.proof.range.start");
        var end = DecodeFramedUInt64(ref range, "SubmitBridgeProof.proof.range.end");
        if (!range.IsFinished || start != rangeStartHeight || end != rangeEndHeight)
        {
            throw new ArgumentException("SubmitBridgeProof range contradicts the SCCP response.");
        }

        var proofPayload = new CompactTransactionCursor(
            proof.TakeField("SubmitBridgeProof.proof.payload"));
        var kind = proofPayload.TakeUInt32("SubmitBridgeProof.proof.payload.kind");
        var variant = proofPayload.TakeField("SubmitBridgeProof.proof.payload.value");
        if (!proofPayload.IsFinished)
        {
            throw new ArgumentException("SubmitBridgeProof payload contains trailing bytes.");
        }

        switch (kind)
        {
            case 2:
                RequireCanonicalNativeBridgeProof(
                    variant,
                    backend,
                    routeConfigurationHash,
                    expectedProof);
                break;
            case 3:
                RequireCanonicalDestinationBridgeProof(
                    variant,
                    backend,
                    routeConfigurationHash,
                    expectedProof);
                break;
            case 0 or 1:
                throw new ArgumentException("SCCP SubmitBridgeProof cannot use generic bridge payloads.");
            default:
                throw new ArgumentException("SubmitBridgeProof uses an unknown bridge payload.");
        }

        if (!proof.IsFinished)
        {
            throw new ArgumentException("SCCP SubmitBridgeProof must contain no trailing fields.");
        }
    }

    private static void RequireCanonicalNativeBridgeProof(
        ReadOnlySpan<byte> payload,
        string backend,
        byte[] routeConfigurationHash,
        byte[]? expectedProof)
    {
        var cursor = new CompactTransactionCursor(payload);
        var backendField = cursor.TakeField("SubmitBridgeProof.native.backend");
        if (backendField.Length != sizeof(uint))
        {
            throw new ArgumentException("SubmitBridgeProof native backend is malformed.");
        }

        var backendTag = BinaryPrimitives.ReadUInt32LittleEndian(backendField);
        var expectedBackend = backendTag switch
        {
            0 => "bridge/sccp/native/ethereum-beacon-v1",
            1 => "bridge/sccp/native/bsc-parlia-v1",
            2 => "bridge/sccp/native/tron-dpos-v1",
            _ => throw new ArgumentException("SubmitBridgeProof native backend is unknown."),
        };
        var routeHash = DecodeFixedByteArray(
            cursor.TakeField("SubmitBridgeProof.native.route_configuration_hash"),
            IrohaHash.Length,
            "SubmitBridgeProof.native.route_configuration_hash");
        var envelope = DecodeRawByteVector(
            cursor.TakeField("SubmitBridgeProof.native.encoded_envelope"),
            "SubmitBridgeProof.native.encoded_envelope");
        if (!cursor.IsFinished
            || backend != expectedBackend
            || !routeHash.AsSpan().SequenceEqual(routeConfigurationHash))
        {
            throw new ArgumentException(
                "SubmitBridgeProof native backend or route binding contradicts the SCCP response.");
        }

        RequireCanonicalNoritoArchive(
            envelope,
            "SubmitBridgeProof.native.encoded_envelope",
            MaximumNativeArtifactBytes,
            NativeInboundProofSchemaName);
        if (expectedProof is not null && !envelope.SequenceEqual(expectedProof))
        {
            throw new ArgumentException("SubmitBridgeProof native proof does not match the submitted proof.");
        }
    }

    private static void RequireCanonicalDestinationBridgeProof(
        ReadOnlySpan<byte> payload,
        string backend,
        byte[] routeConfigurationHash,
        byte[]? expectedProof)
    {
        var cursor = new CompactTransactionCursor(payload);
        var backendField = cursor.TakeField("SubmitBridgeProof.destination.backend");
        if (backendField.Length != sizeof(uint))
        {
            throw new ArgumentException("SubmitBridgeProof destination backend is malformed.");
        }

        var backendTag = BinaryPrimitives.ReadUInt32LittleEndian(backendField);
        var expectedBackend = backendTag switch
        {
            0 => "evm-groth16-bn254-v1",
            1 => "tron-groth16-bn254-v1",
            _ => throw new ArgumentException("SubmitBridgeProof destination backend is unknown."),
        };
        var routeHash = DecodeFixedByteArray(
            cursor.TakeField("SubmitBridgeProof.destination.route_configuration_hash"),
            IrohaHash.Length,
            "SubmitBridgeProof.destination.route_configuration_hash");
        var artifact = DecodeRawByteVector(
            cursor.TakeField("SubmitBridgeProof.destination.encoded_artifact"),
            "SubmitBridgeProof.destination.encoded_artifact");
        if (!cursor.IsFinished
            || backend != expectedBackend
            || !routeHash.AsSpan().SequenceEqual(routeConfigurationHash))
        {
            throw new ArgumentException(
                "SubmitBridgeProof destination binding contradicts the SCCP response.");
        }

        RequireCanonicalNoritoArchive(
            artifact,
            "SubmitBridgeProof.destination.encoded_artifact",
            MaximumDestinationArtifactBytes,
            DestinationArtifactSchemaName);
        if (expectedProof is not null && !artifact.SequenceEqual(expectedProof))
        {
            throw new ArgumentException(
                "SubmitBridgeProof destination artifact does not match the submitted proof.");
        }
    }

    private static ulong DecodeFramedUInt64(ref CompactTransactionCursor cursor, string field)
    {
        var value = cursor.TakeField(field);
        if (value.Length != sizeof(ulong))
        {
            throw new ArgumentException($"{field} is not a canonical UInt64.");
        }

        return BinaryPrimitives.ReadUInt64LittleEndian(value);
    }

    private static byte[] DecodeFixedByteArray(
        ReadOnlySpan<byte> payload,
        int length,
        string field)
    {
        if (payload.Length != length)
        {
            throw new ArgumentException($"{field} is not a canonical fixed byte array.");
        }

        return payload.ToArray();
    }

    private static void RequireAbsentOption(ReadOnlySpan<byte> payload, string field)
    {
        var cursor = new CompactTransactionCursor(payload);
        var tag = cursor.TakeByte(field);
        if (tag != 0 || !cursor.IsFinished)
        {
            throw new ArgumentException($"SCCP transaction {field} must use the exact None encoding.");
        }
    }

    private static void RequireCanonicalMetadata(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        var count = cursor.TakeUInt64("metadata.count");
        if (count != 0 || !cursor.IsFinished)
        {
            throw new ArgumentException(
                "SCCP transaction metadata must be empty; fee selection belongs in fee_payment.");
        }
    }

    internal static void RequireEmptyTransactionMetadata(ReadOnlySpan<byte> payload) =>
        RequireCanonicalMetadata(payload);

    private static void RequireCanonicalFeePayment(
        ReadOnlySpan<byte> payload,
        FeePaymentIntent? expectedFeePayment = null)
    {
        var intent = new CompactTransactionCursor(payload);
        var payer = intent.TakeUInt32("fee_payment.payer");
        var value = new CompactTransactionCursor(intent.TakeField("fee_payment.value"));
        if (!intent.IsFinished)
        {
            throw new ArgumentException("SCCP transaction fee_payment contains trailing bytes.");
        }

        byte[]? sponsorController = null;
        string? programName = null;
        ulong? programRevision = null;
        switch (payer)
        {
            case 0:
                break;
            case 1:
            {
                var program = new CompactTransactionCursor(
                    value.TakeField("fee_payment.program_id"));
                sponsorController = RequireCanonicalAuthority(
                    program.TakeField("fee_payment.program_id.sponsor"));
                programName = DecodeCompactString(
                    program.TakeField("fee_payment.program_id.name"),
                    "fee_payment.program_id.name");
                if (!program.IsFinished
                    || string.IsNullOrEmpty(programName)
                    || !string.Equals(
                        programName.Normalize(NormalizationForm.FormC),
                        programName,
                        StringComparison.Ordinal)
                    || programName.Any(static character =>
                        char.IsWhiteSpace(character)
                        || char.IsControl(character)
                        || character is '@' or '#' or '$' or '/'))
                {
                    throw new ArgumentException(
                        "SCCP sponsor fee_payment program id is not canonical.");
                }

                programRevision = DecodeFramedUInt64(
                    ref value,
                    "fee_payment.program_revision");
                if (programRevision == 0)
                {
                    throw new ArgumentException(
                        "SCCP sponsor fee_payment program revision must be positive.");
                }
                break;
            }
            default:
                throw new ArgumentException("SCCP transaction fee_payment payer is unknown.");
        }

        RequireCanonicalFeeChargeLimits(
            value.TakeField("fee_payment.charge_limits"));
        var gasLimit = DecodeOptionalPositiveUInt64(
            value.TakeField("fee_payment.gas_limit"),
            "fee_payment.gas_limit");
        if (!value.IsFinished)
        {
            throw new ArgumentException("SCCP transaction fee_payment contains trailing bytes.");
        }

        if (expectedFeePayment is null)
        {
            return;
        }

        var selectionMatches = expectedFeePayment switch
        {
            AuthorityFeePaymentIntent => payer == 0
                && gasLimit == expectedFeePayment.GasLimit,
            SponsorFeePaymentIntent sponsor => payer == 1
                && gasLimit == sponsor.GasLimit
                && programRevision == sponsor.ProgramRevision
                && string.Equals(programName, sponsor.ProgramId.Name, StringComparison.Ordinal)
                && sponsorController is not null
                && sponsorController.AsSpan().SequenceEqual(
                    AccountAddress.Parse(
                        sponsor.ProgramId.Sponsor,
                        AccountAddress.DefaultChainDiscriminant)
                    .ControllerBytes()),
            _ => false,
        };
        if (!selectionMatches)
        {
            throw new ArgumentException(
                "SCCP transaction fee_payment changed the expected payer, sponsor revision, or gas bound.");
        }
    }

    internal static void RequireCanonicalTransactionFeePayment(
        ReadOnlySpan<byte> payload) =>
        RequireCanonicalFeePayment(payload);

    private static void RequireCanonicalFeeChargeLimits(ReadOnlySpan<byte> payload)
    {
        var limits = new CompactTransactionCursor(payload);
        var count = limits.TakeUInt64("fee_payment.charge_limits.count");
        if (count > 2)
        {
            throw new ArgumentException(
                "SCCP transaction fee_payment contains too many charge limits.");
        }

        var previousKind = -1;
        for (var index = 0UL; index < count; index++)
        {
            var limit = new CompactTransactionCursor(
                limits.TakeField("fee_payment.charge_limits.item"));
            var kindBytes = limit.TakeField("fee_payment.charge_limits.item.kind");
            if (kindBytes.Length != sizeof(uint))
            {
                throw new ArgumentException("SCCP fee charge kind is malformed.");
            }

            var kind = BinaryPrimitives.ReadUInt32LittleEndian(kindBytes);
            if (kind > 1 || checked((int)kind) <= previousKind)
            {
                throw new ArgumentException(
                    "SCCP fee charge limits must be unique and ordered nexus before pipeline gas.");
            }

            RequireCanonicalAssetDefinitionAddress(
                limit.TakeField("fee_payment.charge_limits.item.asset_definition_id"));
            RequireCanonicalPositiveQuantity(
                limit.TakeField("fee_payment.charge_limits.item.max_amount"));
            if (!limit.IsFinished)
            {
                throw new ArgumentException("SCCP fee charge limit contains trailing bytes.");
            }

            previousKind = checked((int)kind);
        }

        if (!limits.IsFinished)
        {
            throw new ArgumentException("SCCP fee charge limits contain trailing bytes.");
        }
    }

    private static void RequireCanonicalAssetDefinitionAddress(ReadOnlySpan<byte> payload)
    {
        var cursor = new CompactTransactionCursor(payload);
        Span<byte> uuid = stackalloc byte[16];
        for (var index = 0; index < uuid.Length; index++)
        {
            if (cursor.TakeCompactLength("fee_payment.asset_definition_id") != 1)
            {
                throw new ArgumentException(
                    "SCCP fee asset definition address is not canonically framed.");
            }
            uuid[index] = cursor.TakeByte("fee_payment.asset_definition_id");
        }

        if (!cursor.IsFinished
            || (uuid[6] >> 4) != 0x4
            || (uuid[8] & 0xc0) != 0x80)
        {
            throw new ArgumentException("SCCP fee asset definition address is not canonical.");
        }
    }

    private static void RequireCanonicalPositiveQuantity(ReadOnlySpan<byte> payload)
    {
        var quantity = new CompactTransactionCursor(payload);
        var encodedMantissa = new CompactTransactionCursor(
            quantity.TakeField("fee_payment.max_amount.mantissa"));
        var mantissaLength = encodedMantissa.TakeUInt32("fee_payment.max_amount.mantissa.length");
        if (mantissaLength is 0 or > 64)
        {
            throw new ArgumentException("SCCP fee maximum mantissa length is invalid.");
        }

        var mantissaBytes = encodedMantissa.TakeExact(
            checked((int)mantissaLength),
            "fee_payment.max_amount.mantissa");
        var scaleBytes = quantity.TakeField("fee_payment.max_amount.scale");
        if (!encodedMantissa.IsFinished
            || scaleBytes.Length != sizeof(uint)
            || !quantity.IsFinished)
        {
            throw new ArgumentException("SCCP fee maximum is malformed.");
        }

        var mantissa = new System.Numerics.BigInteger(
            mantissaBytes,
            isUnsigned: false,
            isBigEndian: false);
        var scale = BinaryPrimitives.ReadUInt32LittleEndian(scaleBytes);
        if (mantissa.Sign <= 0 || scale > NumericV1.MaxScale)
        {
            throw new ArgumentException("SCCP fee maximum must be a positive canonical quantity.");
        }

        NumericV1.QuantityValue canonical;
        try
        {
            canonical = NumericV1.QuantityValue.FromMantissa(mantissa, checked((int)scale));
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException(
                "SCCP fee maximum must be a positive canonical quantity.",
                error);
        }
        if (canonical.Mantissa != mantissa || checked((uint)canonical.Scale) != scale)
        {
            throw new ArgumentException("SCCP fee maximum is not canonically normalized.");
        }
    }

    private static ulong? DecodeOptionalPositiveUInt64(
        ReadOnlySpan<byte> payload,
        string field)
    {
        var option = new CompactTransactionCursor(payload);
        var tag = option.TakeByte(field);
        if (tag == 0)
        {
            if (!option.IsFinished)
            {
                throw new ArgumentException($"{field} None encoding contains trailing bytes.");
            }
            return null;
        }
        if (tag != 1)
        {
            throw new ArgumentException($"{field} option tag is unknown.");
        }

        var value = DecodeFramedUInt64(ref option, field);
        if (value == 0 || !option.IsFinished)
        {
            throw new ArgumentException($"{field} must contain one positive UInt64.");
        }
        return value;
    }

    private static string DecodeCompactString(ReadOnlySpan<byte> payload, string field)
    {
        var cursor = new CompactTransactionCursor(payload);
        var length = cursor.TakeCompactLength(field);
        if (length > int.MaxValue)
        {
            throw new ArgumentException($"{field} length exceeds the runtime bound.");
        }

        var bytes = cursor.TakeExact(checked((int)length), field);
        if (!cursor.IsFinished)
        {
            throw new ArgumentException($"{field} contains trailing bytes.");
        }

        try
        {
            return StrictUtf8.GetString(bytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new ArgumentException($"{field} is not strict UTF-8.", field, error);
        }
    }

    private ref struct CompactTransactionCursor
    {
        private readonly ReadOnlySpan<byte> input;
        private int offset;

        internal CompactTransactionCursor(ReadOnlySpan<byte> input)
        {
            this.input = input;
            offset = 0;
        }

        internal bool IsFinished => offset == input.Length;

        internal byte TakeByte(string field) => TakeExact(1, field)[0];

        internal ushort TakeUInt16(string field) =>
            BinaryPrimitives.ReadUInt16LittleEndian(TakeExact(sizeof(ushort), field));

        internal uint TakeUInt32(string field) =>
            BinaryPrimitives.ReadUInt32LittleEndian(TakeExact(sizeof(uint), field));

        internal ulong TakeUInt64(string field) =>
            BinaryPrimitives.ReadUInt64LittleEndian(TakeExact(sizeof(ulong), field));

        internal ulong TakeCompactLength(string field)
        {
            ulong result = 0;
            var shift = 0;
            while (true)
            {
                var value = TakeByte(field);
                var chunk = value & 0x7f;
                if (shift == 63 && chunk > 1)
                {
                    throw new ArgumentException($"{field} compact length exceeds UInt64.");
                }

                result |= (ulong)chunk << shift;
                if ((value & 0x80) == 0)
                {
                    if (shift > 0 && chunk == 0)
                    {
                        throw new ArgumentException($"{field} compact length is overlong.");
                    }

                    return result;
                }

                shift += 7;
                if (shift >= 64)
                {
                    throw new ArgumentException($"{field} compact length exceeds UInt64.");
                }
            }
        }

        internal ReadOnlySpan<byte> TakeField(string field)
        {
            var length = TakeCompactLength(field);
            if (length > int.MaxValue)
            {
                throw new ArgumentException($"{field} length exceeds the runtime bound.");
            }

            return TakeExact(checked((int)length), field);
        }

        internal ReadOnlySpan<byte> TakeExact(int length, string field)
        {
            if (length < 0 || offset > input.Length - length)
            {
                throw new ArgumentException($"{field} is truncated.");
            }

            var result = input.Slice(offset, length);
            offset += length;
            return result;
        }
    }

    internal static string ResponseHash(string value, string field)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != 64 || value.AsSpan().ContainsAnyExcept("0123456789abcdef")
            || value.AsSpan().IndexOfAnyExcept('0') < 0)
        {
            throw new ArgumentException(
                $"{field} must be canonical lowercase nonzero 32-byte hex.",
                field);
        }

        return value;
    }

    private static bool CanonicalEd25519Scalar(ReadOnlySpan<byte> value)
    {
        ReadOnlySpan<byte> order =
        [
            0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
            0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
        ];
        for (var index = 31; index >= 0; index--)
        {
            if (value[index] < order[index])
            {
                return true;
            }

            if (value[index] > order[index])
            {
                return false;
            }
        }

        return false;
    }

    private static bool CanonicalEd25519Point(ReadOnlySpan<byte> value)
    {
        if (value.Length != 32)
        {
            return false;
        }

        Span<byte> y = stackalloc byte[32];
        value.CopyTo(y);
        y[31] &= 0x7f;
        ReadOnlySpan<byte> prime =
        [
            0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
        ];
        var less = false;
        for (var index = 31; index >= 0; index--)
        {
            if (y[index] < prime[index])
            {
                less = true;
                break;
            }

            if (y[index] > prime[index])
            {
                return false;
            }
        }

        if (!less)
        {
            return false;
        }

        string[] smallOrder =
        [
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000080",
            "0100000000000000000000000000000000000000000000000000000000000000",
            "0100000000000000000000000000000000000000000000000000000000000080",
            "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
            "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05",
            "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a",
            "13888ecb61c5c95739d95c69ce5177c450e99128e7a90b3ecbc595e035c15500",
            "b4dfc53e58080246839b2c4e6f3db63e185f6c730b31e990b6f3f2519295550f",
        ];
        var hex = Convert.ToHexString(value).ToLowerInvariant();
        return !smallOrder.Contains(hex, StringComparer.Ordinal);
    }
}

internal static class SccpJson
{
    internal static JsonDocument Parse(ReadOnlyMemory<byte> json, string label)
    {
        try
        {
            if (json.Length == 0 || json.Length > SccpSubmitValidation.MaximumJsonBytes)
            {
                throw new JsonException("JSON body exceeds the SCCP size bound.");
            }

            var reader = new Utf8JsonReader(json.Span, new JsonReaderOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 128,
            });
            var objects = new Stack<HashSet<string>>();
            while (reader.Read())
            {
                switch (reader.TokenType)
                {
                    case JsonTokenType.StartObject:
                        objects.Push(new HashSet<string>(StringComparer.Ordinal));
                        break;
                    case JsonTokenType.PropertyName:
                        var property = reader.GetString()
                            ?? throw new JsonException("JSON property name must not be null.");
                        if (objects.Count == 0 || !objects.Peek().Add(property))
                        {
                            throw new JsonException($"Duplicate JSON property `{property}`.");
                        }

                        break;
                    case JsonTokenType.EndObject:
                        objects.Pop();
                        break;
                }
            }

            return JsonDocument.Parse(json, new JsonDocumentOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 128,
            });
        }
        catch (JsonException error)
        {
            throw new ArgumentException($"{label} must be strict UTF-8 JSON without duplicate keys.", label, error);
        }
    }

    internal static void ExactFields(JsonElement value, HashSet<string> fields, string label)
        => ExactFields(value, fields, fields, label);

    internal static void ExactFields(
        JsonElement value,
        HashSet<string> allowed,
        HashSet<string> required,
        string label)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{label} must be a JSON object.");
        }

        var observed = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!allowed.Contains(property.Name))
            {
                throw new ArgumentException($"{label} contains unknown or retired field `{property.Name}`.");
            }

            observed.Add(property.Name);
        }

        foreach (var field in required)
        {
            if (!observed.Contains(field))
            {
                throw new ArgumentException($"{label} is missing required field `{field}`.");
            }
        }
    }

    internal static string Text(JsonElement value, string field)
    {
        var property = value.GetProperty(field);
        if (property.ValueKind != JsonValueKind.String)
        {
            throw new ArgumentException($"{field} must be a string.");
        }

        var result = property.GetString()!;
        if (result.Length == 0 || result != result.Trim())
        {
            throw new ArgumentException($"{field} must be canonical nonempty text.");
        }

        return result;
    }

    internal static string? OptionalText(JsonElement value, string field) =>
        value.GetProperty(field).ValueKind == JsonValueKind.Null ? null : Text(value, field);

    internal static bool Boolean(JsonElement value, string field) => value.GetProperty(field).ValueKind switch
    {
        JsonValueKind.True => true,
        JsonValueKind.False => false,
        _ => throw new ArgumentException($"{field} must be boolean."),
    };

    internal static ulong UInt64(
        JsonElement value,
        string field,
        ulong minimum,
        ulong maximum = ulong.MaxValue)
    {
        var property = value.GetProperty(field);
        if (property.ValueKind != JsonValueKind.Number || !property.TryGetUInt64(out var result)
            || result < minimum || result > maximum
            || property.GetRawText() != result.ToString(System.Globalization.CultureInfo.InvariantCulture))
        {
            throw new ArgumentException($"{field} must be a canonical unsigned integer >= {minimum}.");
        }

        return result;
    }

    internal static uint UInt32(JsonElement value, string field, uint minimum, uint maximum)
    {
        var result = UInt64(value, field, minimum);
        if (result > maximum)
        {
            throw new ArgumentOutOfRangeException(field);
        }

        return (uint)result;
    }
}
