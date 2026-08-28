using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sccp;

/// <summary>Closed first-release SCCP network inventory.</summary>
public enum SccpNetworkV1 : byte
{
    SoraTaira = 0x40,
    EthereumMainnet = 0x41,
    BscMainnet = 0x42,
    TronMainnet = 0x43,
    TonMainnet = 0x44,
}

/// <summary>Exact profile metadata for <see cref="SccpNetworkV1"/>.</summary>
public static class SccpNetworkV1Extensions
{
    public static string ProfileKey(this SccpNetworkV1 network) => network switch
    {
        SccpNetworkV1.SoraTaira => "sora-taira",
        SccpNetworkV1.EthereumMainnet => "ethereum-mainnet",
        SccpNetworkV1.BscMainnet => "bsc-mainnet",
        SccpNetworkV1.TronMainnet => "tron-mainnet",
        SccpNetworkV1.TonMainnet => "ton-mainnet",
        _ => throw new ArgumentOutOfRangeException(nameof(network)),
    };

    public static uint DomainId(this SccpNetworkV1 network) => network switch
    {
        SccpNetworkV1.SoraTaira => 0,
        SccpNetworkV1.EthereumMainnet => 1,
        SccpNetworkV1.BscMainnet => 2,
        SccpNetworkV1.TonMainnet => 4,
        SccpNetworkV1.TronMainnet => 5,
        _ => throw new ArgumentOutOfRangeException(nameof(network)),
    };

    public static bool IsSora(this SccpNetworkV1 network) => network.DomainId() == 0;

    public static bool IsExternal(this SccpNetworkV1 network) => !network.IsSora();

    public static SccpNetworkV1 ParseProfileKey(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        foreach (var candidate in Enum.GetValues<SccpNetworkV1>())
        {
            if (string.Equals(candidate.ProfileKey(), value, StringComparison.Ordinal))
            {
                return candidate;
            }
        }

        throw new ArgumentException("Value is not an exact SCCP profile key.", nameof(value));
    }
}

/// <summary>Directed exact SCCP lane joining one SORA and one external profile.</summary>
public sealed record SccpLaneIdV1
{
    public SccpLaneIdV1(SccpNetworkV1 source, SccpNetworkV1 target)
    {
        if (source.IsSora() == target.IsSora() || source.DomainId() == target.DomainId())
        {
            throw new ArgumentException(
                "SCCP lane must join exactly one SORA profile and one external profile.");
        }

        Source = source;
        Target = target;
    }

    public SccpNetworkV1 Source { get; }

    public SccpNetworkV1 Target { get; }

    public bool IsOutbound => Source.IsSora() && Target.IsExternal();

    public bool IsInbound => Source.IsExternal() && Target.IsSora();
}

/// <summary>Closed first-release SCCP binary codec inventory.</summary>
public enum SccpCodecV1 : byte
{
    CanonicalText = 1,
    EvmAddress20 = 2,
    TronAddress21 = 5,
    TonAccount36 = 7,
}

/// <summary>Canonical binary codec validation.</summary>
public static class SccpCodecV1Extensions
{
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    public static string WireKey(this SccpCodecV1 codec) => codec switch
    {
        SccpCodecV1.CanonicalText => "canonical_text",
        SccpCodecV1.EvmAddress20 => "evm_address20",
        SccpCodecV1.TronAddress21 => "tron_address21",
        SccpCodecV1.TonAccount36 => "ton_account36",
        _ => throw new ArgumentOutOfRangeException(nameof(codec)),
    };

    public static byte[] Validate(this SccpCodecV1 codec, ReadOnlySpan<byte> value)
    {
        var valid = codec switch
        {
            SccpCodecV1.CanonicalText => IsCanonicalText(value),
            SccpCodecV1.EvmAddress20 => value.Length == 20 && !IsZero(value),
            SccpCodecV1.TronAddress21 => value.Length == 21 && value[0] == 0x41 && !IsZero(value[1..]),
            SccpCodecV1.TonAccount36 => value.Length == 36 && IsZero(value[..4]) && !IsZero(value[4..]),
            _ => false,
        };
        if (!valid)
        {
            throw new ArgumentException($"Value does not match SCCP codec {codec.WireKey()}.", nameof(value));
        }

        return value.ToArray();
    }

    private static bool IsCanonicalText(ReadOnlySpan<byte> value)
    {
        if (value.Length is < 1 or > 256)
        {
            return false;
        }

        if (value.IndexOfAnyExceptInRange((byte)0x21, (byte)0x7e) < 0)
        {
            return true;
        }

        try
        {
            _ = AccountAddress.Parse(StrictUtf8.GetString(value));
            return true;
        }
        catch (DecoderFallbackException)
        {
            return false;
        }
        catch (AccountAddressException)
        {
            return false;
        }
    }

    private static bool IsZero(ReadOnlySpan<byte> value)
    {
        foreach (var item in value)
        {
            if (item != 0)
            {
                return false;
            }
        }

        return true;
    }
}

/// <summary>Canonical native verifier backends admitted by SCCP V1.</summary>
public enum SccpNativeBackendV1
{
    EthereumBeacon,
    BscParlia,
    TronDpos,
    TonMasterchain,
}

public static class SccpNativeBackendV1Extensions
{
    public static string WireKey(this SccpNativeBackendV1 backend) => backend switch
    {
        SccpNativeBackendV1.EthereumBeacon => "ethereum_beacon_v1",
        SccpNativeBackendV1.BscParlia => "bsc_parlia_v1",
        SccpNativeBackendV1.TronDpos => "tron_dpos_v1",
        SccpNativeBackendV1.TonMasterchain => "ton_masterchain_v1",
        _ => throw new ArgumentOutOfRangeException(nameof(backend)),
    };

    public static string BackendLabel(this SccpNativeBackendV1 backend) => backend switch
    {
        SccpNativeBackendV1.EthereumBeacon => "bridge/sccp/native/ethereum-beacon-v1",
        SccpNativeBackendV1.BscParlia => "bridge/sccp/native/bsc-parlia-v1",
        SccpNativeBackendV1.TronDpos => "bridge/sccp/native/tron-dpos-v1",
        SccpNativeBackendV1.TonMasterchain => "bridge/sccp/native/ton-masterchain-v1",
        _ => throw new ArgumentOutOfRangeException(nameof(backend)),
    };

    public static bool Supports(this SccpNativeBackendV1 backend, SccpNetworkV1 network) => backend switch
    {
        SccpNativeBackendV1.EthereumBeacon => network == SccpNetworkV1.EthereumMainnet,
        SccpNativeBackendV1.BscParlia => network == SccpNetworkV1.BscMainnet,
        SccpNativeBackendV1.TronDpos => network == SccpNetworkV1.TronMainnet,
        SccpNativeBackendV1.TonMasterchain => network == SccpNetworkV1.TonMainnet,
        _ => false,
    };

    public static SccpNativeBackendV1 ParseWireKey(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        foreach (var candidate in Enum.GetValues<SccpNativeBackendV1>())
        {
            if (string.Equals(candidate.WireKey(), value, StringComparison.Ordinal))
            {
                return candidate;
            }
        }

        throw new ArgumentException("Unknown SCCP native backend.", nameof(value));
    }
}

/// <summary>Stable fixed-width kind tags used by canonical SCCP commitments.</summary>
public enum SccpHubMessageKindV1 : byte
{
    Transfer = 5,
}

/// <summary>Closed canonical SCCP payload. V1 admits only transfer.</summary>
public abstract class SccpPayloadV1
{
    private protected SccpPayloadV1()
    {
    }

    public abstract SccpHubMessageKindV1 Kind { get; }

    internal abstract uint SourceDomain { get; }

    internal abstract uint DestinationDomain { get; }

    internal abstract void WriteCanonicalBody(Stream output);

    /// <summary>Encode this payload in the exact, versioned SCCP V1 layout.</summary>
    public byte[] CanonicalBytes()
    {
        using var output = new MemoryStream();
        output.WriteByte(2);
        WriteCanonicalBody(output);
        return output.ToArray();
    }
}

/// <summary>The sole value-moving payload admitted by SCCP V1.</summary>
public sealed class SccpTransferPayloadV1 : SccpPayloadV1
{
    private readonly byte[] assetId;
    private readonly byte[] sender;
    private readonly byte[] recipient;
    private readonly byte[] routeId;

    public SccpTransferPayloadV1(
        uint sourceDomain,
        uint destinationDomain,
        ulong nonce,
        uint routeRevision,
        uint assetHomeDomain,
        SccpCodecV1 assetIdCodec,
        byte[] assetId,
        UInt128 amount,
        SccpCodecV1 senderCodec,
        byte[] sender,
        SccpCodecV1 recipientCodec,
        byte[] recipient,
        SccpCodecV1 routeIdCodec,
        byte[] routeId)
    {
        RequireDomain(sourceDomain, nameof(sourceDomain));
        RequireDomain(destinationDomain, nameof(destinationDomain));
        RequireDomain(assetHomeDomain, nameof(assetHomeDomain));
        if (sourceDomain == destinationDomain)
        {
            throw new ArgumentException("SCCP transfer endpoints must differ.");
        }

        if (routeRevision == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(routeRevision), "routeRevision must be nonzero.");
        }

        if (amount == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(amount), "amount must be nonzero.");
        }

        if (senderCodec != AccountCodec(sourceDomain))
        {
            throw new ArgumentException("senderCodec does not match the source domain.", nameof(senderCodec));
        }

        if (recipientCodec != AccountCodec(destinationDomain))
        {
            throw new ArgumentException(
                "recipientCodec does not match the destination domain.",
                nameof(recipientCodec));
        }

        Source = sourceDomain;
        Destination = destinationDomain;
        Nonce = nonce;
        RouteRevision = routeRevision;
        AssetHomeDomain = assetHomeDomain;
        AssetIdCodec = assetIdCodec;
        this.assetId = ValidateCodec(assetIdCodec, assetId, nameof(assetId));
        Amount = amount;
        SenderCodec = senderCodec;
        this.sender = ValidateCodec(senderCodec, sender, nameof(sender));
        RecipientCodec = recipientCodec;
        this.recipient = ValidateCodec(recipientCodec, recipient, nameof(recipient));
        RouteIdCodec = routeIdCodec;
        this.routeId = ValidateCodec(routeIdCodec, routeId, nameof(routeId));
    }

    public override SccpHubMessageKindV1 Kind => SccpHubMessageKindV1.Transfer;

    public uint Source { get; }

    public uint Destination { get; }

    public ulong Nonce { get; }

    /// <summary>Nonzero governed route revision committed between nonce and asset-home domain.</summary>
    public uint RouteRevision { get; }

    public uint AssetHomeDomain { get; }

    public SccpCodecV1 AssetIdCodec { get; }

    public byte[] AssetId => [.. assetId];

    public UInt128 Amount { get; }

    public SccpCodecV1 SenderCodec { get; }

    public byte[] Sender => [.. sender];

    public SccpCodecV1 RecipientCodec { get; }

    public byte[] Recipient => [.. recipient];

    public SccpCodecV1 RouteIdCodec { get; }

    public byte[] RouteId => [.. routeId];

    internal override uint SourceDomain => Source;

    internal override uint DestinationDomain => Destination;

    internal override void WriteCanonicalBody(Stream output)
    {
        output.WriteByte(1);
        SccpV1.WriteUInt32Canonical(output, Source);
        SccpV1.WriteUInt32Canonical(output, Destination);
        SccpV1.WriteUInt64Canonical(output, Nonce);
        SccpV1.WriteUInt32Canonical(output, RouteRevision);
        SccpV1.WriteUInt32Canonical(output, AssetHomeDomain);
        output.WriteByte((byte)AssetIdCodec);
        SccpV1.WriteBytesCanonical(output, assetId);
        SccpV1.WriteUInt128Canonical(output, Amount);
        output.WriteByte((byte)SenderCodec);
        SccpV1.WriteBytesCanonical(output, sender);
        output.WriteByte((byte)RecipientCodec);
        SccpV1.WriteBytesCanonical(output, recipient);
        output.WriteByte((byte)RouteIdCodec);
        SccpV1.WriteBytesCanonical(output, routeId);
    }

    private static byte[] ValidateCodec(SccpCodecV1 codec, byte[] value, string field)
    {
        ArgumentNullException.ThrowIfNull(value);
        try
        {
            return codec.Validate(value);
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException($"{field} does not match closed SCCP codec {(byte)codec}.", field, error);
        }
    }

    private static void RequireDomain(uint value, string field)
    {
        if (value is not (0 or 1 or 2 or 4 or 5))
        {
            throw new ArgumentOutOfRangeException(field, "SCCP domain is unsupported or retired.");
        }
    }

    private static SccpCodecV1 AccountCodec(uint domain) => domain switch
    {
        0 => SccpCodecV1.CanonicalText,
        1 or 2 => SccpCodecV1.EvmAddress20,
        4 => SccpCodecV1.TonAccount36,
        5 => SccpCodecV1.TronAddress21,
        _ => throw new ArgumentOutOfRangeException(nameof(domain)),
    };
}

/// <summary>Exact governed context for one SORA-origin message.</summary>
public sealed class SccpOutboundMessageContextV1
{
    private readonly byte[] destinationBindingHash;
    private readonly byte[] routeConfigurationHash;

    public SccpOutboundMessageContextV1(
        SccpLaneIdV1 lane,
        byte[] destinationBindingHash,
        byte[] routeConfigurationHash)
    {
        ArgumentNullException.ThrowIfNull(lane);
        if (!lane.IsOutbound)
        {
            throw new ArgumentException("Outbound SCCP context must use a SORA-to-external lane.", nameof(lane));
        }

        this.destinationBindingHash = RequireHash(destinationBindingHash, nameof(destinationBindingHash));
        this.routeConfigurationHash = RequireHash(routeConfigurationHash, nameof(routeConfigurationHash));
        if (this.destinationBindingHash.AsSpan().SequenceEqual(this.routeConfigurationHash))
        {
            throw new ArgumentException("Destination binding and route configuration must be distinct.");
        }

        Lane = lane;
    }

    public SccpLaneIdV1 Lane { get; }

    public byte[] DestinationBindingHash => [.. destinationBindingHash];

    public byte[] RouteConfigurationHash => [.. routeConfigurationHash];

    private static byte[] RequireHash(byte[] value, string field)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != 32 || value.All(static item => item == 0))
        {
            throw new ArgumentException($"{field} must be a nonzero 32-byte hash.", field);
        }

        return [.. value];
    }
}

/// <summary>Exact fixed-width SORA hub commitment.</summary>
public sealed class SccpHubCommitmentV1
{
    private readonly byte[] messageId;
    private readonly byte[] payloadHash;

    internal SccpHubCommitmentV1(
        SccpHubMessageKindV1 kind,
        SccpOutboundMessageContextV1 context,
        byte[] messageId,
        byte[] payloadHash)
    {
        if (kind != SccpHubMessageKindV1.Transfer)
        {
            throw new ArgumentException("SCCP commitment kind is unsupported or retired.", nameof(kind));
        }

        ArgumentNullException.ThrowIfNull(context);
        Kind = kind;
        Context = context;
        this.messageId = RequireHash(messageId, nameof(messageId));
        this.payloadHash = RequireHash(payloadHash, nameof(payloadHash));
    }

    public SccpHubMessageKindV1 Kind { get; }

    public SccpOutboundMessageContextV1 Context { get; }

    public byte[] MessageId => [.. messageId];

    public byte[] PayloadHash => [.. payloadHash];

    private static byte[] RequireHash(byte[] value, string field)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != 32 || value.All(static item => item == 0))
        {
            throw new ArgumentException($"{field} must be a nonzero 32-byte hash.", field);
        }

        return [.. value];
    }
}

/// <summary>One bounded sibling step in a canonical SCCP commitment proof.</summary>
public sealed class SccpMerkleStepV1
{
    private readonly byte[] siblingHash;

    public SccpMerkleStepV1(byte[] siblingHash, bool siblingIsLeft)
    {
        ArgumentNullException.ThrowIfNull(siblingHash);
        if (siblingHash.Length != 32 || siblingHash.All(static item => item == 0))
        {
            throw new ArgumentException("siblingHash must be a nonzero 32-byte hash.", nameof(siblingHash));
        }

        this.siblingHash = [.. siblingHash];
        SiblingIsLeft = siblingIsLeft;
    }

    public byte[] SiblingHash => [.. siblingHash];

    public bool SiblingIsLeft { get; }
}

/// <summary>Strictly decoded summary of one canonical SCCP message-bundle byte archive.</summary>
public sealed class SccpCanonicalMessageBundleV1
{
    private readonly byte[] commitmentRoot;
    private readonly byte[] finalityProof;

    internal SccpCanonicalMessageBundleV1(
        SccpHubCommitmentV1 commitment,
        SccpTransferPayloadV1 payload,
        IReadOnlyList<SccpMerkleStepV1> merkleProof,
        byte[] commitmentRoot,
        byte[] finalityProof)
    {
        Commitment = commitment;
        Payload = payload;
        MerkleProof = merkleProof;
        this.commitmentRoot = [.. commitmentRoot];
        this.finalityProof = [.. finalityProof];
    }

    public SccpHubCommitmentV1 Commitment { get; }

    public SccpTransferPayloadV1 Payload { get; }

    public IReadOnlyList<SccpMerkleStepV1> MerkleProof { get; }

    public byte[] CommitmentRoot => [.. commitmentRoot];

    public byte[] FinalityProof => [.. finalityProof];
}

/// <summary>Immutable exact source-emitter identity.</summary>
public abstract class SccpSourceEmitterV1
{
    private protected SccpSourceEmitterV1()
    {
    }

    public sealed class Evm : SccpSourceEmitterV1
    {
        private readonly byte[] address;
        private readonly byte[] runtimeCodeHash;
        private readonly byte[] routeConfigHash;

        public Evm(byte[] address, byte[] runtimeCodeHash, byte[] routeConfigHash)
        {
            this.address = Role(address, 20, nameof(address));
            this.runtimeCodeHash = Role(runtimeCodeHash, 32, nameof(runtimeCodeHash));
            this.routeConfigHash = Role(routeConfigHash, 32, nameof(routeConfigHash));
            Distinct(this.runtimeCodeHash, this.routeConfigHash, "EVM emitter hash roles");
        }

        public byte[] Address => [.. address];
        public byte[] RuntimeCodeHash => [.. runtimeCodeHash];
        public byte[] RouteConfigHash => [.. routeConfigHash];
    }

    public sealed class Tron : SccpSourceEmitterV1
    {
        private readonly byte[] address;
        private readonly byte[] runtimeCodeHash;
        private readonly byte[] routeConfigHash;

        public Tron(byte[] address, byte[] runtimeCodeHash, byte[] routeConfigHash)
        {
            this.address = Role(address, 20, nameof(address));
            this.runtimeCodeHash = Role(runtimeCodeHash, 32, nameof(runtimeCodeHash));
            this.routeConfigHash = Role(routeConfigHash, 32, nameof(routeConfigHash));
            Distinct(this.runtimeCodeHash, this.routeConfigHash, "TRON emitter hash roles");
        }

        public byte[] Address => [.. address];
        public byte[] RuntimeCodeHash => [.. runtimeCodeHash];
        public byte[] RouteConfigHash => [.. routeConfigHash];
    }

    private static byte[] Role(byte[] value, int length, string name)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length != length || value.All(static item => item == 0))
        {
            throw new ArgumentException($"{name} must be a nonzero {length}-byte value.", name);
        }

        return [.. value];
    }

    private static void Distinct(byte[] left, byte[] right, string label)
    {
        if (left.AsSpan().SequenceEqual(right))
        {
            throw new ArgumentException($"{label} must be distinct.");
        }
    }
}

/// <summary>Consensus-compatible exact-lane hashes and source-event digest for SCCP V1.</summary>
public static class SccpV1
{
    private const int MaximumBundleBytes = 16 * 1024 * 1024;
    private const int Keccak256Rate = 136;
    private static readonly byte[] LaneHashPrefix = "sccp:lane-id:v1"u8.ToArray();
    private static readonly byte[] MessageIdPrefix = "sccp:lane-message-id:v1"u8.ToArray();
    private static readonly byte[] PayloadHashPrefix = "sccp:payload:v1"u8.ToArray();
    private static readonly byte[] CommitmentLeafPrefix = "sccp:hub:leaf:v1"u8.ToArray();
    private static readonly byte[] CommitmentNodePrefix = "sccp:hub:node:v1"u8.ToArray();
    private static readonly byte[] SourceEventPrefix = "sccp:source:event:v1"u8.ToArray();
    private static readonly int[] KeccakRhoOffsets =
    [
        0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39,
        41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
    ];
    private static readonly ulong[] KeccakRoundConstants =
    [
        0x0000000000000001UL, 0x0000000000008082UL, 0x800000000000808aUL,
        0x8000000080008000UL, 0x000000000000808bUL, 0x0000000080000001UL,
        0x8000000080008081UL, 0x8000000000008009UL, 0x000000000000008aUL,
        0x0000000000000088UL, 0x0000000080008009UL, 0x000000008000000aUL,
        0x000000008000808bUL, 0x800000000000008bUL, 0x8000000000008089UL,
        0x8000000000008003UL, 0x8000000000008002UL, 0x8000000000000080UL,
        0x000000000000800aUL, 0x800000008000000aUL, 0x8000000080008081UL,
        0x8000000000008080UL, 0x0000000080000001UL, 0x8000000080008008UL,
    ];

    public static byte[] CanonicalNetworkBytes(SccpNetworkV1 network)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        output.WriteByte((byte)network);
        WriteUInt32(output, network.DomainId());
        switch (network)
        {
            case SccpNetworkV1.SoraTaira:
                output.Write(Convert.FromHexString("fc56984b2be7431d840e21514d1883f0"));
                break;
            case SccpNetworkV1.EthereumMainnet:
                WriteUInt64(output, 1);
                break;
            case SccpNetworkV1.BscMainnet:
                WriteUInt64(output, 56);
                break;
            case SccpNetworkV1.TronMainnet:
                WriteUInt32(output, 0x2b6653dc);
                break;
            case SccpNetworkV1.TonMainnet:
                WriteTonNetwork(
                    output,
                    -239,
                    "17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb05569",
                    "5e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e");
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(network));
        }

        return output.ToArray();
    }

    private static void WriteTonNetwork(
        Stream output,
        int globalId,
        string zeroStateRootHex,
        string zeroStateFileHex)
    {
        Span<byte> signed = stackalloc byte[4];
        BinaryPrimitives.WriteInt32LittleEndian(signed, globalId);
        output.Write(signed);
        BinaryPrimitives.WriteInt32LittleEndian(signed, -1);
        output.Write(signed);
        WriteUInt64(output, 0x8000_0000_0000_0000UL);
        WriteUInt32(output, 0);
        output.Write(Convert.FromHexString(zeroStateRootHex));
        output.Write(Convert.FromHexString(zeroStateFileHex));
    }

    public static byte[] CanonicalLaneBytes(SccpLaneIdV1 lane)
    {
        ArgumentNullException.ThrowIfNull(lane);
        using var output = new MemoryStream();
        output.WriteByte(1);
        WriteBytes(output, CanonicalNetworkBytes(lane.Source));
        WriteBytes(output, CanonicalNetworkBytes(lane.Target));
        return output.ToArray();
    }

    public static byte[] LaneHash(SccpLaneIdV1 lane) =>
        Blake2b.Hash256(Concat(LaneHashPrefix, CanonicalLaneBytes(lane)));

    public static byte[] PayloadHash(ReadOnlySpan<byte> canonicalPayload)
    {
        var payload = DecodeCanonicalPayload(canonicalPayload);
        return PayloadHash(payload);
    }

    public static byte[] PayloadHash(SccpPayloadV1 payload)
    {
        ArgumentNullException.ThrowIfNull(payload);
        return Blake2b.Hash256(Concat(PayloadHashPrefix, payload.CanonicalBytes()));
    }

    public static byte[] MessageId(SccpLaneIdV1 lane, ReadOnlySpan<byte> canonicalPayload)
    {
        var payload = DecodeCanonicalPayload(canonicalPayload);
        return MessageId(lane, payload);
    }

    public static byte[] MessageId(SccpLaneIdV1 lane, SccpPayloadV1 payload)
    {
        ArgumentNullException.ThrowIfNull(lane);
        ArgumentNullException.ThrowIfNull(payload);
        if (lane.Source.DomainId() != payload.SourceDomain
            || lane.Target.DomainId() != payload.DestinationDomain)
        {
            throw new ArgumentException("Payload domains do not match the exact SCCP lane.", nameof(payload));
        }

        var canonicalPayload = payload.CanonicalBytes();
        using var body = new MemoryStream();
        body.WriteByte(1);
        WriteBytes(body, CanonicalLaneBytes(lane));
        WriteBytes(body, canonicalPayload);
        var result = Keccak256(Concat(MessageIdPrefix, body.ToArray()));
        if (result.All(static item => item == 0))
        {
            throw new InvalidOperationException("SCCP message id must be nonzero.");
        }

        return result;
    }

    /// <summary>Decode exactly one canonical transfer and reject retired, truncated, or trailing forms.</summary>
    public static SccpTransferPayloadV1 DecodeCanonicalPayload(ReadOnlySpan<byte> bytes)
    {
        var cursor = new PayloadCursor(bytes);
        if (cursor.TakeByte() != 2)
        {
            throw new ArgumentException("Unsupported or retired SCCP payload discriminant.", nameof(bytes));
        }

        if (cursor.TakeByte() != 1)
        {
            throw new ArgumentException("Unsupported SCCP transfer version.", nameof(bytes));
        }

        var payload = new SccpTransferPayloadV1(
            cursor.TakeUInt32(),
            cursor.TakeUInt32(),
            cursor.TakeUInt64(),
            cursor.TakeUInt32(),
            cursor.TakeUInt32(),
            (SccpCodecV1)cursor.TakeByte(),
            cursor.TakeVector(),
            cursor.TakeUInt128(),
            (SccpCodecV1)cursor.TakeByte(),
            cursor.TakeVector(),
            (SccpCodecV1)cursor.TakeByte(),
            cursor.TakeVector(),
            (SccpCodecV1)cursor.TakeByte(),
            cursor.TakeVector());
        if (!cursor.IsFinished || !payload.CanonicalBytes().AsSpan().SequenceEqual(bytes))
        {
            throw new ArgumentException("Canonical SCCP payload must not contain trailing or non-canonical bytes.", nameof(bytes));
        }

        return payload;
    }

    /// <summary>Construct a role-separated exact outbound commitment.</summary>
    public static SccpHubCommitmentV1 Commitment(
        SccpOutboundMessageContextV1 context,
        SccpPayloadV1 payload)
    {
        ArgumentNullException.ThrowIfNull(context);
        ArgumentNullException.ThrowIfNull(payload);
        var messageId = MessageId(context.Lane, payload);
        var payloadHash = PayloadHash(payload);
        var commitment = new SccpHubCommitmentV1(payload.Kind, context, messageId, payloadHash);
        RequireDistinctHashes(
        [
            LaneHash(context.Lane), context.DestinationBindingHash, context.RouteConfigurationHash,
            messageId, payloadHash,
        ], "SCCP commitment");
        return commitment;
    }

    /// <summary>Encode one exact commitment in its fixed 132-byte layout.</summary>
    public static byte[] CanonicalCommitmentBytes(SccpHubCommitmentV1 commitment)
    {
        ArgumentNullException.ThrowIfNull(commitment);
        var output = new byte[132];
        output[0] = 1;
        output[1] = (byte)commitment.Kind;
        output[2] = (byte)commitment.Context.Lane.Source;
        output[3] = (byte)commitment.Context.Lane.Target;
        commitment.Context.DestinationBindingHash.CopyTo(output, 4);
        commitment.Context.RouteConfigurationHash.CopyTo(output, 36);
        commitment.MessageId.CopyTo(output, 68);
        commitment.PayloadHash.CopyTo(output, 100);
        return output;
    }

    /// <summary>Decode exactly one fixed commitment and reject reserved tags and colliding roles.</summary>
    public static SccpHubCommitmentV1 DecodeCanonicalCommitment(ReadOnlySpan<byte> bytes)
    {
        if (bytes.Length != 132 || bytes[0] != 1 || bytes[1] != (byte)SccpHubMessageKindV1.Transfer)
        {
            throw new ArgumentException("Canonical SCCP commitment header or length is invalid.", nameof(bytes));
        }

        var source = NetworkFromTag(bytes[2]);
        var target = NetworkFromTag(bytes[3]);
        var context = new SccpOutboundMessageContextV1(
            new SccpLaneIdV1(source, target),
            bytes.Slice(4, 32).ToArray(),
            bytes.Slice(36, 32).ToArray());
        var commitment = new SccpHubCommitmentV1(
            SccpHubMessageKindV1.Transfer,
            context,
            bytes.Slice(68, 32).ToArray(),
            bytes.Slice(100, 32).ToArray());
        RequireDistinctHashes(
        [
            LaneHash(context.Lane), context.DestinationBindingHash, context.RouteConfigurationHash,
            commitment.MessageId, commitment.PayloadHash,
        ], "SCCP commitment");
        if (!CanonicalCommitmentBytes(commitment).AsSpan().SequenceEqual(bytes))
        {
            throw new ArgumentException("SCCP commitment is not canonical.", nameof(bytes));
        }

        return commitment;
    }

    /// <summary>Hash one canonical commitment as an SCCP Merkle leaf.</summary>
    public static byte[] CommitmentRoot(SccpHubCommitmentV1 commitment) =>
        Blake2b.Hash256(Concat(CommitmentLeafPrefix, CanonicalCommitmentBytes(commitment)));

    /// <summary>Reconstruct the commitment root from a bounded bottom-up sibling path.</summary>
    public static byte[] MerkleRootFromCommitment(
        SccpHubCommitmentV1 commitment,
        IReadOnlyList<SccpMerkleStepV1> steps)
    {
        ArgumentNullException.ThrowIfNull(commitment);
        ArgumentNullException.ThrowIfNull(steps);
        if (steps.Count > 64)
        {
            throw new ArgumentException("SCCP Merkle proof is too deep.", nameof(steps));
        }

        var current = CommitmentRoot(commitment);
        foreach (var step in steps)
        {
            ArgumentNullException.ThrowIfNull(step);
            current = step.SiblingIsLeft
                ? Blake2b.Hash256(Concat(CommitmentNodePrefix, step.SiblingHash, current))
                : Blake2b.Hash256(Concat(CommitmentNodePrefix, current, step.SiblingHash));
        }

        return current;
    }

    /// <summary>Encode one structurally exact canonical bundle byte archive.</summary>
    public static byte[] CanonicalMessageBundleBytes(
        SccpHubCommitmentV1 commitment,
        SccpTransferPayloadV1 payload,
        IReadOnlyList<SccpMerkleStepV1> steps,
        ReadOnlySpan<byte> finalityProof)
    {
        ArgumentNullException.ThrowIfNull(commitment);
        ArgumentNullException.ThrowIfNull(payload);
        ArgumentNullException.ThrowIfNull(steps);
        if (finalityProof.IsEmpty || finalityProof.Length > MaximumBundleBytes || steps.Count > 64)
        {
            throw new ArgumentException("SCCP finality proof or Merkle path exceeds its V1 bound.");
        }
        RequireCanonicalFinalityFrame(finalityProof);

        var expected = Commitment(commitment.Context, payload);
        if (expected.Kind != commitment.Kind
            || !expected.MessageId.AsSpan().SequenceEqual(commitment.MessageId)
            || !expected.PayloadHash.AsSpan().SequenceEqual(commitment.PayloadHash))
        {
            throw new ArgumentException("SCCP commitment does not match its canonical payload.", nameof(commitment));
        }

        using var proofBytes = new MemoryStream();
        WriteUInt32(proofBytes, checked((uint)steps.Count));
        foreach (var step in steps)
        {
            ArgumentNullException.ThrowIfNull(step);
            proofBytes.Write(step.SiblingHash);
            proofBytes.WriteByte(step.SiblingIsLeft ? (byte)1 : (byte)0);
        }

        using var output = new MemoryStream();
        output.WriteByte(1);
        output.Write(MerkleRootFromCommitment(commitment, steps));
        WriteBytes(output, CanonicalCommitmentBytes(commitment));
        WriteBytes(output, proofBytes.ToArray());
        WriteBytes(output, payload.CanonicalBytes());
        WriteBytes(output, finalityProof);
        var result = output.ToArray();
        if (result.Length > MaximumBundleBytes)
        {
            throw new ArgumentException("Canonical SCCP message bundle exceeds its V1 bound.");
        }

        return result;
    }

    /// <summary>Decode and re-encode one canonical bundle, rejecting all trailing and cross-role aliases.</summary>
    public static SccpCanonicalMessageBundleV1 DecodeCanonicalMessageBundle(ReadOnlySpan<byte> bytes)
    {
        if (bytes.IsEmpty || bytes.Length > MaximumBundleBytes)
        {
            throw new ArgumentException("Canonical SCCP message bundle exceeds its V1 bound.", nameof(bytes));
        }

        var cursor = new PayloadCursor(bytes);
        if (cursor.TakeByte() != 1)
        {
            throw new ArgumentException("Unsupported SCCP message-bundle version.", nameof(bytes));
        }

        var root = cursor.TakeExact(32).ToArray();
        RequireHash(root, "commitment root");
        var commitmentBytes = cursor.TakeVector();
        var proofBytes = cursor.TakeVector();
        var payloadBytes = cursor.TakeVector();
        var finalityProof = cursor.TakeVector();
        if (!cursor.IsFinished || finalityProof.Length == 0)
        {
            throw new ArgumentException("SCCP message bundle is truncated or contains trailing bytes.", nameof(bytes));
        }
        RequireCanonicalFinalityFrame(finalityProof);

        var commitment = DecodeCanonicalCommitment(commitmentBytes);
        var payload = DecodeCanonicalPayload(payloadBytes);
        var expected = Commitment(commitment.Context, payload);
        if (expected.Kind != commitment.Kind
            || !expected.MessageId.AsSpan().SequenceEqual(commitment.MessageId)
            || !expected.PayloadHash.AsSpan().SequenceEqual(commitment.PayloadHash))
        {
            throw new ArgumentException("SCCP bundle commitment does not match its canonical payload.", nameof(bytes));
        }

        var proofCursor = new PayloadCursor(proofBytes);
        var count = proofCursor.TakeUInt32();
        if (count > 64 || count > (proofBytes.Length - 4) / 33)
        {
            throw new ArgumentException("SCCP bundle Merkle path exceeds its V1 bound.", nameof(bytes));
        }

        var steps = new List<SccpMerkleStepV1>(checked((int)count));
        for (var index = 0U; index < count; index++)
        {
            var sibling = proofCursor.TakeExact(32).ToArray();
            var direction = proofCursor.TakeByte();
            if (direction > 1)
            {
                throw new ArgumentException("SCCP Merkle direction must be exactly zero or one.", nameof(bytes));
            }

            steps.Add(new SccpMerkleStepV1(sibling, direction == 1));
        }

        if (!proofCursor.IsFinished
            || !MerkleRootFromCommitment(commitment, steps).AsSpan().SequenceEqual(root))
        {
            throw new ArgumentException("SCCP bundle Merkle root or proof encoding is invalid.", nameof(bytes));
        }

        var canonical = CanonicalMessageBundleBytes(commitment, payload, steps, finalityProof);
        if (!canonical.AsSpan().SequenceEqual(bytes))
        {
            throw new ArgumentException("SCCP message bundle bytes are not canonical.", nameof(bytes));
        }

        return new SccpCanonicalMessageBundleV1(
            commitment,
            payload,
            steps.AsReadOnly(),
            root,
            finalityProof);
    }

    internal static void RequireCanonicalFinalityFrame(ReadOnlySpan<byte> finalityProof)
    {
        try
        {
            _ = SccpSubmitValidation.CanonicalNoritoBase64(
                Convert.ToBase64String(finalityProof),
                "finality_proof",
                MaximumBundleBytes);
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException(
                "SCCP finality proof must be one canonical uncompressed Norito envelope.",
                nameof(finalityProof),
                error);
        }
    }

    internal static (byte[] StatementHash, byte[] RequestHash) CanonicalProofRequestHashes(
        SccpDestinationProofBackendV1 backend,
        SccpNetworkV1 sourceNetwork,
        SccpNetworkV1 targetNetwork,
        byte[] messageId,
        byte[] payloadHash,
        uint targetDomain,
        byte[] commitmentRoot,
        ulong finalityHeight,
        byte[] finalityBlockHash,
        byte[] bundleBytes,
        byte[] verifyingKeyBytes,
        byte[] verifierKeyHash,
        SccpSemanticProofProfileV1 semanticProfile,
        byte[] semanticProfileHash,
        SccpSoraFinalityAnchorV1 finalityAnchor,
        byte[] finalityAnchorHash,
        byte[] destinationBindingHash,
        byte[] routeConfigurationHash)
    {
        foreach (var (value, name) in new[]
        {
            (messageId, "message id"), (payloadHash, "payload hash"),
            (commitmentRoot, "commitment root"), (finalityBlockHash, "finality block hash"),
            (verifierKeyHash, "verifier-key hash"), (semanticProfileHash, "semantic-profile hash"),
            (finalityAnchorHash, "finality-anchor hash"),
            (destinationBindingHash, "destination-binding hash"),
            (routeConfigurationHash, "route-configuration hash"),
        })
        {
            RequireHash(value, name);
        }

        if (sourceNetwork != SccpNetworkV1.SoraTaira
            || !targetNetwork.IsExternal()
            || targetNetwork.DomainId() != targetDomain
            || finalityHeight == 0
            || bundleBytes.Length == 0
            || verifyingKeyBytes.Length != 38 * 32)
        {
            throw new ArgumentException("SCCP proof-request base material is invalid.");
        }

        RequireDistinctHashes(
        [
            messageId, payloadHash, commitmentRoot, finalityBlockHash, verifierKeyHash,
            semanticProfileHash, finalityAnchorHash, destinationBindingHash, routeConfigurationHash,
        ], "SCCP proof request");
        var summary = DecodeCanonicalMessageBundle(bundleBytes);
        var canonicalPayload = summary.Payload.CanonicalBytes();
        var publicInputs = CanonicalPublicInputs(
            messageId,
            payloadHash,
            targetDomain,
            commitmentRoot,
            finalityHeight,
            finalityBlockHash);
        var backendTag = backend switch
        {
            SccpDestinationProofBackendV1.EvmGroth16Bn254 => (byte)0,
            SccpDestinationProofBackendV1.TronGroth16Bn254 => (byte)1,
            _ => throw new ArgumentOutOfRangeException(nameof(backend)),
        };

        using var statement = new MemoryStream();
        statement.WriteByte(1);
        statement.WriteByte(backendTag);
        WriteBytes(statement, CanonicalNetworkBytes(sourceNetwork));
        WriteBytes(statement, CanonicalNetworkBytes(targetNetwork));
        statement.Write(destinationBindingHash);
        statement.Write(routeConfigurationHash);
        statement.Write(verifierKeyHash);
        statement.Write(semanticProfileHash);
        statement.Write(finalityAnchorHash);
        statement.Write(publicInputs);
        WriteBytes(statement, canonicalPayload);
        WriteBytes(statement, bundleBytes);
        var statementHash = Blake2b.Hash256(Concat(
            "sccp:groth16-bn254:statement:v1"u8.ToArray(),
            statement.ToArray()));

        var semanticBytes = CanonicalSemanticProfileBytes(semanticProfile);
        var anchorBytes = CanonicalFinalityAnchorBytes(finalityAnchor);
        var signalValues = new byte[][]
        {
            messageId,
            payloadHash,
            AbiWord(targetDomain),
            commitmentRoot,
            AbiWord(finalityHeight),
            finalityBlockHash,
            AbiWord(sourceNetwork.DomainId()),
            statementHash,
            destinationBindingHash,
            routeConfigurationHash,
            finalityAnchorHash,
        };
        var signalWords = new byte[11][];
        for (var index = 0; index < signalWords.Length; index++)
        {
            signalWords[index] = SignalWord(PublicSignalLabels[index], signalValues[index]);
        }

        using var request = new MemoryStream();
        request.WriteByte(1);
        request.WriteByte(backendTag);
        WriteBytes(request, CanonicalNetworkBytes(sourceNetwork));
        WriteBytes(request, CanonicalNetworkBytes(targetNetwork));
        WriteBytes(request, publicInputs);
        WriteBytes(request, canonicalPayload);
        WriteBytes(request, bundleBytes);
        WriteBytes(request, semanticBytes);
        WriteBytes(request, anchorBytes);
        request.Write(statementHash);
        request.Write(destinationBindingHash);
        request.Write(routeConfigurationHash);
        request.Write(verifierKeyHash);
        request.Write(semanticProfileHash);
        request.Write(finalityAnchorHash);
        WriteBytes(request, verifyingKeyBytes);
        foreach (var word in signalWords)
        {
            request.Write(word);
        }

        var requestHash = Blake2b.Hash256(Concat(
            "sccp:groth16-bn254:proof-request:v1"u8.ToArray(),
            request.ToArray()));
        return (statementHash, requestHash);
    }

    private static readonly byte[][] PublicSignalLabels =
    [
        "sccp:groth16-bn254:signal:message-id:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:payload-hash:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:target-domain:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:commitment-root:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:finality-height:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:finality-block-hash:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:source-domain:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:statement-hash:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:destination-binding-hash:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:route-configuration-hash:v1"u8.ToArray(),
        "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1"u8.ToArray(),
    ];

    private static byte[] CanonicalPublicInputs(
        byte[] messageId,
        byte[] payloadHash,
        uint targetDomain,
        byte[] commitmentRoot,
        ulong finalityHeight,
        byte[] finalityBlockHash)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        output.Write(messageId);
        output.Write(payloadHash);
        WriteUInt32(output, targetDomain);
        output.Write(commitmentRoot);
        WriteUInt64(output, finalityHeight);
        output.Write(finalityBlockHash);
        return output.ToArray();
    }

    private static byte[] CanonicalSemanticProfileBytes(SccpSemanticProofProfileV1 profile) =>
        Concat(
            [1, 0, 1],
            profile.CircuitCommitment,
            profile.WitnessGeneratorCommitment,
            profile.PublicSignalSchemaHash);

    private static byte[] CanonicalFinalityAnchorBytes(SccpSoraFinalityAnchorV1 anchor)
    {
        if (anchor.ProtocolVersion != 4 || anchor.CheckpointHeight == 0)
        {
            throw new ArgumentException("SCCP finality anchor must bind protocol version 4 and a nonzero height.");
        }
        foreach (var (value, name) in new[]
        {
            (anchor.ChainIdHash, "chain-id hash"),
            (anchor.CheckpointBlockHash, "checkpoint block hash"),
            (anchor.CheckpointContextId, "checkpoint context id"),
            (anchor.CheckpointFinalityArtifactHash, "checkpoint finality-artifact hash"),
        })
        {
            RequireHash(value, name);
        }
        RequireDistinctHashes(
        [
            anchor.ChainIdHash, anchor.CheckpointBlockHash,
            anchor.CheckpointContextId, anchor.CheckpointFinalityArtifactHash,
        ], "SCCP finality anchor");

        using var output = new MemoryStream();
        output.WriteByte(1);
        output.WriteByte((byte)SccpNetworkV1.SoraTaira);
        Span<byte> protocol = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(protocol, anchor.ProtocolVersion);
        output.Write(protocol);
        output.Write(anchor.ChainIdHash);
        WriteUInt64(output, anchor.CheckpointHeight);
        output.Write(anchor.CheckpointBlockHash);
        output.Write(anchor.CheckpointContextId);
        output.Write(anchor.CheckpointFinalityArtifactHash);
        return output.ToArray();
    }

    private static byte[] AbiWord(ulong value)
    {
        var word = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(word.AsSpan(24), value);
        return word;
    }

    private static byte[] SignalWord(byte[] label, byte[] value)
    {
        var hash = Keccak256(Concat(Keccak256(label), value));
        ReadOnlySpan<byte> modulus =
        [
            0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29,
            0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
            0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91,
            0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00, 0x00, 0x01,
        ];
        while (hash.AsSpan().SequenceCompareTo(modulus) >= 0)
        {
            SubtractBigEndian(hash, modulus);
        }

        return hash;
    }

    private static void SubtractBigEndian(Span<byte> left, ReadOnlySpan<byte> right)
    {
        var borrow = 0;
        for (var index = left.Length - 1; index >= 0; index--)
        {
            var difference = left[index] - right[index] - borrow;
            if (difference < 0)
            {
                difference += 256;
                borrow = 1;
            }
            else
            {
                borrow = 0;
            }

            left[index] = checked((byte)difference);
        }
    }

    public static byte[] CanonicalSourceEventBytes(
        SccpLaneIdV1 lane,
        ReadOnlySpan<byte> messageId,
        ReadOnlySpan<byte> payloadHash)
    {
        RequireHash(messageId, nameof(messageId));
        RequireHash(payloadHash, nameof(payloadHash));
        var laneHash = LaneHash(lane);
        if (laneHash.AsSpan().SequenceEqual(messageId)
            || laneHash.AsSpan().SequenceEqual(payloadHash)
            || messageId.SequenceEqual(payloadHash))
        {
            throw new ArgumentException("SCCP lane, message, and payload hash roles must be distinct.");
        }

        return Concat([1], laneHash, messageId, payloadHash);
    }

    public static byte[] SourceEventDigest(
        SccpLaneIdV1 lane,
        ReadOnlySpan<byte> messageId,
        ReadOnlySpan<byte> payloadHash) =>
        Keccak256(Concat(SourceEventPrefix, CanonicalSourceEventBytes(lane, messageId, payloadHash)));

    public static string LowerHex(ReadOnlySpan<byte> value) => Convert.ToHexString(value).ToLowerInvariant();

    public static byte[] DecodeLowerHex(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length == 0 || value.Length % 2 != 0
            || value.Any(static item => !char.IsAsciiDigit(item) && item is not (>= 'a' and <= 'f')))
        {
            throw new ArgumentException("Hex must be canonical lowercase without 0x.", nameof(value));
        }

        return Convert.FromHexString(value);
    }

    private static void RequireHash(ReadOnlySpan<byte> value, string name)
    {
        if (value.Length != 32 || value.IndexOfAnyExcept((byte)0) < 0)
        {
            throw new ArgumentException($"{name} must be a nonzero 32-byte value.", name);
        }
    }

    private static SccpNetworkV1 NetworkFromTag(byte tag)
    {
        var network = (SccpNetworkV1)tag;
        if (!Enum.IsDefined(network))
        {
            throw new ArgumentException("SCCP network tag is unknown or permanently reserved.", nameof(tag));
        }

        return network;
    }

    private static void RequireDistinctHashes(IEnumerable<byte[]> values, string label)
    {
        var observed = new HashSet<string>(StringComparer.Ordinal);
        foreach (var value in values)
        {
            RequireHash(value, label);
            if (!observed.Add(Convert.ToHexString(value)))
            {
                throw new ArgumentException($"{label} hash roles must be pairwise distinct.");
            }
        }
    }

    private static byte[] Concat(byte[] first, ReadOnlySpan<byte> second)
    {
        var result = new byte[first.Length + second.Length];
        first.CopyTo(result, 0);
        second.CopyTo(result.AsSpan(first.Length));
        return result;
    }

    private static byte[] Concat(
        byte[] first,
        byte[] second,
        ReadOnlySpan<byte> third,
        ReadOnlySpan<byte> fourth)
    {
        var result = new byte[first.Length + second.Length + third.Length + fourth.Length];
        first.CopyTo(result, 0);
        second.CopyTo(result, first.Length);
        third.CopyTo(result.AsSpan(first.Length + second.Length));
        fourth.CopyTo(result.AsSpan(first.Length + second.Length + third.Length));
        return result;
    }

    private static byte[] Concat(params byte[][] values)
    {
        var length = values.Aggregate(0, static (current, value) => checked(current + value.Length));
        var result = new byte[length];
        var offset = 0;
        foreach (var value in values)
        {
            value.CopyTo(result, offset);
            offset += value.Length;
        }

        return result;
    }

    private static void WriteBytes(Stream output, ReadOnlySpan<byte> value)
    {
        WriteUInt32(output, checked((uint)value.Length));
        output.Write(value);
    }

    internal static void WriteBytesCanonical(Stream output, ReadOnlySpan<byte> value) => WriteBytes(output, value);

    private static void WriteUInt32(Stream output, uint value)
    {
        Span<byte> bytes = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, value);
        output.Write(bytes);
    }

    internal static void WriteUInt32Canonical(Stream output, uint value) => WriteUInt32(output, value);

    private static void WriteUInt64(Stream output, ulong value)
    {
        Span<byte> bytes = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        output.Write(bytes);
    }

    internal static void WriteUInt64Canonical(Stream output, ulong value) => WriteUInt64(output, value);

    internal static void WriteUInt128Canonical(Stream output, UInt128 value)
    {
        Span<byte> bytes = stackalloc byte[16];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, (ulong)value);
        BinaryPrimitives.WriteUInt64LittleEndian(bytes[8..], (ulong)(value >> 64));
        output.Write(bytes);
    }

    public static byte[] Keccak256(ReadOnlySpan<byte> data)
    {
        var state = new ulong[25];
        var offset = 0;
        while (offset + Keccak256Rate <= data.Length)
        {
            AbsorbKeccakBlock(state, data.Slice(offset, Keccak256Rate));
            KeccakF1600(state);
            offset += Keccak256Rate;
        }

        Span<byte> block = stackalloc byte[Keccak256Rate];
        block.Clear();
        data[offset..].CopyTo(block);
        block[data.Length - offset] ^= 0x01;
        block[Keccak256Rate - 1] ^= 0x80;
        AbsorbKeccakBlock(state, block);
        KeccakF1600(state);
        var output = new byte[32];
        for (var lane = 0; lane < 4; lane++)
        {
            BinaryPrimitives.WriteUInt64LittleEndian(output.AsSpan(lane * 8, 8), state[lane]);
        }

        return output;
    }

    private static void AbsorbKeccakBlock(ulong[] state, ReadOnlySpan<byte> block)
    {
        for (var lane = 0; lane < Keccak256Rate / 8; lane++)
        {
            state[lane] ^= BinaryPrimitives.ReadUInt64LittleEndian(block.Slice(lane * 8, 8));
        }
    }

    private static void KeccakF1600(ulong[] state)
    {
        Span<ulong> c = stackalloc ulong[5];
        Span<ulong> d = stackalloc ulong[5];
        Span<ulong> b = stackalloc ulong[25];
        foreach (var roundConstant in KeccakRoundConstants)
        {
            for (var x = 0; x < 5; x++)
            {
                c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
            }

            for (var x = 0; x < 5; x++)
            {
                d[x] = c[(x + 4) % 5] ^ RotateLeft(c[(x + 1) % 5], 1);
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] ^= d[x];
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    var sourceIndex = x + 5 * y;
                    var targetIndex = y + 5 * ((2 * x + 3 * y) % 5);
                    b[targetIndex] = RotateLeft(state[sourceIndex], KeccakRhoOffsets[sourceIndex]);
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] =
                        b[x + 5 * y] ^ ((~b[((x + 1) % 5) + 5 * y]) & b[((x + 2) % 5) + 5 * y]);
                }
            }

            state[0] ^= roundConstant;
        }
    }

    private static ulong RotateLeft(ulong value, int amount) =>
        amount == 0 ? value : (value << amount) | (value >> (64 - amount));

    private ref struct PayloadCursor
    {
        private readonly ReadOnlySpan<byte> input;
        private int offset;

        internal PayloadCursor(ReadOnlySpan<byte> input)
        {
            this.input = input;
            offset = 0;
        }

        internal bool IsFinished => offset == input.Length;

        internal byte TakeByte() => TakeExact(1)[0];

        internal uint TakeUInt32() => BinaryPrimitives.ReadUInt32LittleEndian(TakeExact(4));

        internal ulong TakeUInt64() => BinaryPrimitives.ReadUInt64LittleEndian(TakeExact(8));

        internal UInt128 TakeUInt128()
        {
            var value = TakeExact(16);
            var low = BinaryPrimitives.ReadUInt64LittleEndian(value);
            var high = BinaryPrimitives.ReadUInt64LittleEndian(value[8..]);
            return ((UInt128)high << 64) | low;
        }

        internal byte[] TakeVector()
        {
            var length = TakeUInt32();
            if (length > int.MaxValue)
            {
                throw new ArgumentException("SCCP byte vector length exceeds the runtime bound.");
            }

            return TakeExact((int)length).ToArray();
        }

        internal ReadOnlySpan<byte> TakeExact(int length)
        {
            if (length < 0 || offset > input.Length - length)
            {
                throw new ArgumentException("SCCP canonical payload is truncated.");
            }

            var value = input.Slice(offset, length);
            offset += length;
            return value;
        }
    }
}
