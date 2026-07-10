using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sccp;

/// <summary>Closed first-release SCCP network inventory.</summary>
public enum SccpNetworkV1 : byte
{
    SoraNexus = 0,
    SoraTaira = 1,
    EthereumMainnet = 2,
    EthereumSepolia = 3,
    BscMainnet = 4,
    BscTestnet = 5,
    SolanaMainnetBeta = 6,
    SolanaTestnet = 7,
    TonMainnet = 8,
    TonTestnet = 9,
    TronMainnet = 10,
    TronNile = 11,
    TronShasta = 12,
}

/// <summary>Exact profile metadata for <see cref="SccpNetworkV1"/>.</summary>
public static class SccpNetworkV1Extensions
{
    public static string ProfileKey(this SccpNetworkV1 network) => network switch
    {
        SccpNetworkV1.SoraNexus => "sora-nexus",
        SccpNetworkV1.SoraTaira => "sora-taira",
        SccpNetworkV1.EthereumMainnet => "ethereum-mainnet",
        SccpNetworkV1.EthereumSepolia => "ethereum-sepolia",
        SccpNetworkV1.BscMainnet => "bsc-mainnet",
        SccpNetworkV1.BscTestnet => "bsc-testnet",
        SccpNetworkV1.SolanaMainnetBeta => "solana-mainnet-beta",
        SccpNetworkV1.SolanaTestnet => "solana-testnet",
        SccpNetworkV1.TonMainnet => "ton-mainnet",
        SccpNetworkV1.TonTestnet => "ton-testnet",
        SccpNetworkV1.TronMainnet => "tron-mainnet",
        SccpNetworkV1.TronNile => "tron-nile",
        SccpNetworkV1.TronShasta => "tron-shasta",
        _ => throw new ArgumentOutOfRangeException(nameof(network)),
    };

    public static uint DomainId(this SccpNetworkV1 network) => network switch
    {
        SccpNetworkV1.SoraNexus or SccpNetworkV1.SoraTaira => 0,
        SccpNetworkV1.EthereumMainnet or SccpNetworkV1.EthereumSepolia => 1,
        SccpNetworkV1.BscMainnet or SccpNetworkV1.BscTestnet => 2,
        SccpNetworkV1.SolanaMainnetBeta or SccpNetworkV1.SolanaTestnet => 3,
        SccpNetworkV1.TonMainnet or SccpNetworkV1.TonTestnet => 4,
        SccpNetworkV1.TronMainnet or SccpNetworkV1.TronNile or SccpNetworkV1.TronShasta => 5,
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
    SolanaPubkey32 = 3,
    TonAccount36 = 4,
    TronAddress21 = 5,
    SoraAssetId = 6,
}

/// <summary>Canonical binary codec validation.</summary>
public static class SccpCodecV1Extensions
{
    public static string WireKey(this SccpCodecV1 codec) => codec switch
    {
        SccpCodecV1.CanonicalText => "canonical_text",
        SccpCodecV1.EvmAddress20 => "evm_address20",
        SccpCodecV1.SolanaPubkey32 => "solana_pubkey32",
        SccpCodecV1.TonAccount36 => "ton_account36",
        SccpCodecV1.TronAddress21 => "tron_address21",
        SccpCodecV1.SoraAssetId => "sora_asset_id",
        _ => throw new ArgumentOutOfRangeException(nameof(codec)),
    };

    public static byte[] Validate(this SccpCodecV1 codec, ReadOnlySpan<byte> value)
    {
        var valid = codec switch
        {
            SccpCodecV1.CanonicalText => value.Length is >= 1 and <= 256
                && value.IndexOfAnyExceptInRange((byte)0x21, (byte)0x7e) < 0,
            SccpCodecV1.EvmAddress20 => value.Length == 20 && !IsZero(value),
            SccpCodecV1.SolanaPubkey32 => value.Length == 32 && !IsZero(value),
            SccpCodecV1.TonAccount36 => value.Length == 36
                && BinaryPrimitives.ReadInt32LittleEndian(value[..4]) is -1 or 0
                && !IsZero(value[4..]),
            SccpCodecV1.TronAddress21 => value.Length == 21 && value[0] == 0x41 && !IsZero(value[1..]),
            SccpCodecV1.SoraAssetId => value.Length == 32 && !IsZero(value),
            _ => false,
        };
        if (!valid)
        {
            throw new ArgumentException($"Value does not match SCCP codec {codec.WireKey()}.", nameof(value));
        }

        return value.ToArray();
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
    SolanaTower,
    TonMasterchain,
    TronDpos,
}

public static class SccpNativeBackendV1Extensions
{
    public static string WireKey(this SccpNativeBackendV1 backend) => backend switch
    {
        SccpNativeBackendV1.EthereumBeacon => "ethereum_beacon_v1",
        SccpNativeBackendV1.BscParlia => "bsc_parlia_v1",
        SccpNativeBackendV1.SolanaTower => "solana_tower_v1",
        SccpNativeBackendV1.TonMasterchain => "ton_masterchain_v1",
        SccpNativeBackendV1.TronDpos => "tron_dpos_v1",
        _ => throw new ArgumentOutOfRangeException(nameof(backend)),
    };

    public static string BackendLabel(this SccpNativeBackendV1 backend) => backend switch
    {
        SccpNativeBackendV1.EthereumBeacon => "bridge/sccp/native/ethereum-beacon-v1",
        SccpNativeBackendV1.BscParlia => "bridge/sccp/native/bsc-parlia-v1",
        SccpNativeBackendV1.SolanaTower => "bridge/sccp/native/solana-tower-v1",
        SccpNativeBackendV1.TonMasterchain => "bridge/sccp/native/ton-masterchain-v1",
        SccpNativeBackendV1.TronDpos => "bridge/sccp/native/tron-dpos-v1",
        _ => throw new ArgumentOutOfRangeException(nameof(backend)),
    };

    public static bool Supports(this SccpNativeBackendV1 backend, SccpNetworkV1 network) => backend switch
    {
        SccpNativeBackendV1.EthereumBeacon => network is SccpNetworkV1.EthereumMainnet or SccpNetworkV1.EthereumSepolia,
        SccpNativeBackendV1.BscParlia => network is SccpNetworkV1.BscMainnet or SccpNetworkV1.BscTestnet,
        SccpNativeBackendV1.SolanaTower => network is SccpNetworkV1.SolanaMainnetBeta or SccpNetworkV1.SolanaTestnet,
        SccpNativeBackendV1.TonMasterchain => network is SccpNetworkV1.TonMainnet or SccpNetworkV1.TonTestnet,
        SccpNativeBackendV1.TronDpos => network is SccpNetworkV1.TronMainnet or SccpNetworkV1.TronNile or SccpNetworkV1.TronShasta,
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

/// <summary>Immutable exact source-emitter identity.</summary>
public abstract class SccpSourceEmitterV1
{
    private protected SccpSourceEmitterV1()
    {
    }

    public sealed class Evm : SccpSourceEmitterV1
    {
        public Evm(byte[] address, byte[] runtimeCodeHash, byte[] routeConfigHash)
        {
            Address = Role(address, 20, nameof(address));
            RuntimeCodeHash = Role(runtimeCodeHash, 32, nameof(runtimeCodeHash));
            RouteConfigHash = Role(routeConfigHash, 32, nameof(routeConfigHash));
            Distinct(RuntimeCodeHash, RouteConfigHash, "EVM emitter hash roles");
        }

        public byte[] Address { get; }
        public byte[] RuntimeCodeHash { get; }
        public byte[] RouteConfigHash { get; }
    }

    public sealed class Solana : SccpSourceEmitterV1
    {
        public Solana(byte[] programId, byte[] executableHash, byte[] authorizedEmitter)
        {
            ProgramId = Role(programId, 32, nameof(programId));
            ExecutableHash = Role(executableHash, 32, nameof(executableHash));
            AuthorizedEmitter = Role(authorizedEmitter, 32, nameof(authorizedEmitter));
            Distinct(ProgramId, ExecutableHash, "Solana emitter roles");
            Distinct(ProgramId, AuthorizedEmitter, "Solana emitter roles");
            Distinct(ExecutableHash, AuthorizedEmitter, "Solana emitter roles");
        }

        public byte[] ProgramId { get; }
        public byte[] ExecutableHash { get; }
        public byte[] AuthorizedEmitter { get; }
    }

    public sealed class Ton : SccpSourceEmitterV1
    {
        public Ton(int workchain, byte[] accountId, byte[] codeHash, byte[] immutableConfigHash)
        {
            if (workchain is not (-1 or 0))
            {
                throw new ArgumentOutOfRangeException(nameof(workchain), "TON workchain must be -1 or 0.");
            }

            Workchain = workchain;
            AccountId = Role(accountId, 32, nameof(accountId));
            CodeHash = Role(codeHash, 32, nameof(codeHash));
            ImmutableConfigHash = Role(immutableConfigHash, 32, nameof(immutableConfigHash));
            Distinct(AccountId, CodeHash, "TON emitter roles");
            Distinct(AccountId, ImmutableConfigHash, "TON emitter roles");
            Distinct(CodeHash, ImmutableConfigHash, "TON emitter roles");
        }

        public int Workchain { get; }
        public byte[] AccountId { get; }
        public byte[] CodeHash { get; }
        public byte[] ImmutableConfigHash { get; }
    }

    public sealed class Tron : SccpSourceEmitterV1
    {
        public Tron(byte[] address, byte[] runtimeCodeHash, byte[] routeConfigHash)
        {
            Address = Role(address, 20, nameof(address));
            RuntimeCodeHash = Role(runtimeCodeHash, 32, nameof(runtimeCodeHash));
            RouteConfigHash = Role(routeConfigHash, 32, nameof(routeConfigHash));
            Distinct(RuntimeCodeHash, RouteConfigHash, "TRON emitter hash roles");
        }

        public byte[] Address { get; }
        public byte[] RuntimeCodeHash { get; }
        public byte[] RouteConfigHash { get; }
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
    private const int Keccak256Rate = 136;
    private static readonly byte[] LaneHashPrefix = "sccp:lane-id:v1"u8.ToArray();
    private static readonly byte[] MessageIdPrefix = "sccp:lane-message-id:v1"u8.ToArray();
    private static readonly byte[] PayloadHashPrefix = "sccp:payload:v1"u8.ToArray();
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
            case SccpNetworkV1.SoraNexus:
                output.Write(Convert.FromHexString("00000000000000000000000000000753"));
                break;
            case SccpNetworkV1.SoraTaira:
                output.Write(Convert.FromHexString("809574f5fee75e69bfcf52451e42d50f"));
                break;
            case SccpNetworkV1.EthereumMainnet:
                WriteUInt64(output, 1);
                break;
            case SccpNetworkV1.EthereumSepolia:
                WriteUInt64(output, 11_155_111);
                break;
            case SccpNetworkV1.BscMainnet:
                WriteUInt64(output, 56);
                break;
            case SccpNetworkV1.BscTestnet:
                WriteUInt64(output, 97);
                break;
            case SccpNetworkV1.SolanaMainnetBeta:
                WriteBytes(output, "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"u8);
                break;
            case SccpNetworkV1.SolanaTestnet:
                WriteBytes(output, "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY"u8);
                break;
            case SccpNetworkV1.TonMainnet:
                WriteInt32(output, -239);
                break;
            case SccpNetworkV1.TonTestnet:
                WriteInt32(output, -3);
                break;
            case SccpNetworkV1.TronMainnet:
                WriteUInt32(output, 0x2b6653dc);
                break;
            case SccpNetworkV1.TronNile:
                WriteUInt32(output, 0xcd8690dc);
                break;
            case SccpNetworkV1.TronShasta:
                WriteUInt32(output, 0x94a9059e);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(network));
        }

        return output.ToArray();
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
        if (canonicalPayload.IsEmpty)
        {
            throw new ArgumentException("Canonical SCCP payload must not be empty.", nameof(canonicalPayload));
        }

        return Blake2b.Hash256(Concat(PayloadHashPrefix, canonicalPayload));
    }

    public static byte[] MessageId(SccpLaneIdV1 lane, ReadOnlySpan<byte> canonicalPayload)
    {
        if (canonicalPayload.IsEmpty)
        {
            throw new ArgumentException("Canonical SCCP payload must not be empty.", nameof(canonicalPayload));
        }

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

    private static void WriteBytes(Stream output, ReadOnlySpan<byte> value)
    {
        WriteUInt32(output, checked((uint)value.Length));
        output.Write(value);
    }

    private static void WriteUInt32(Stream output, uint value)
    {
        Span<byte> bytes = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, value);
        output.Write(bytes);
    }

    private static void WriteUInt64(Stream output, ulong value)
    {
        Span<byte> bytes = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        output.Write(bytes);
    }

    private static void WriteInt32(Stream output, int value)
    {
        Span<byte> bytes = stackalloc byte[4];
        BinaryPrimitives.WriteInt32LittleEndian(bytes, value);
        output.Write(bytes);
    }

    private static byte[] Keccak256(ReadOnlySpan<byte> data)
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
}
