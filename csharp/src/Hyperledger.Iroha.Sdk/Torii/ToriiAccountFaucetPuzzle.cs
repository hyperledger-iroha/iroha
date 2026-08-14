using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

[JsonConverter(typeof(ToriiAccountFaucetPuzzleJsonConverter))]
public sealed record class ToriiAccountFaucetPuzzle
{
    private string algorithm = string.Empty;
    private NetworkId networkId = null!;
    private byte difficultyBits;
    private ulong anchorHeight;
    private string anchorBlockHashHex = string.Empty;
    private string? challengeSaltHex;
    private byte scryptLogN;
    private uint scryptR;
    private uint scryptP;
    private bool scryptLogNSet;
    private bool scryptRSet;
    private bool scryptPSet;
    private ulong maxAnchorAgeBlocks;

    [JsonPropertyName("algorithm")]
    public string Algorithm
    {
        get => algorithm;
        init => algorithm = ToriiOnboardingDirectMetadata.RequireFaucetAlgorithm(value, nameof(Algorithm));
    }

    [JsonPropertyName("network_id")]
    public NetworkId NetworkId
    {
        get => networkId;
        init => networkId = value ?? throw new ArgumentNullException(nameof(NetworkId));
    }

    [JsonPropertyName("chain_discriminant")]
    public ushort ChainDiscriminant { get; init; }

    [JsonPropertyName("difficulty_bits")]
    public byte DifficultyBits
    {
        get => difficultyBits;
        init => difficultyBits = value == 0
            ? throw new ArgumentOutOfRangeException(
                nameof(DifficultyBits),
                "Faucet PoW difficulty must be positive.")
            : value;
    }

    [JsonPropertyName("anchor_height")]
    public ulong AnchorHeight
    {
        get => anchorHeight;
        init => anchorHeight = ToriiOnboardingDirectMetadata.RequirePositive(value, nameof(AnchorHeight));
    }

    [JsonPropertyName("anchor_block_hash_hex")]
    public string AnchorBlockHashHex
    {
        get => anchorBlockHashHex;
        init => anchorBlockHashHex = ToriiExplorerDirectMetadata.RequireExactSizedHex(
            value,
            nameof(AnchorBlockHashHex),
            32);
    }

    [JsonPropertyName("challenge_salt_hex")]
    public string? ChallengeSaltHex
    {
        get => challengeSaltHex;
        init => challengeSaltHex = ToriiExplorerDirectMetadata.RequireOptionalExactSizedHex(
            value,
            nameof(ChallengeSaltHex),
            32);
    }

    [JsonPropertyName("scrypt_log_n")]
    public byte ScryptLogN
    {
        get => scryptLogN;
        init
        {
            scryptLogN = value;
            scryptLogNSet = true;
            ValidateCompleteScryptParameters();
        }
    }

    [JsonPropertyName("scrypt_r")]
    public uint ScryptR
    {
        get => scryptR;
        init
        {
            scryptR = value;
            scryptRSet = true;
            ValidateCompleteScryptParameters();
        }
    }

    [JsonPropertyName("scrypt_p")]
    public uint ScryptP
    {
        get => scryptP;
        init
        {
            scryptP = value;
            scryptPSet = true;
            ValidateCompleteScryptParameters();
        }
    }

    [JsonPropertyName("max_anchor_age_blocks")]
    public ulong MaxAnchorAgeBlocks
    {
        get => maxAnchorAgeBlocks;
        init => maxAnchorAgeBlocks = ToriiOnboardingDirectMetadata.RequirePositive(
            value,
            nameof(MaxAnchorAgeBlocks));
    }

    private void ValidateCompleteScryptParameters()
    {
        if (scryptLogNSet && scryptRSet && scryptPSet)
        {
            ToriiOnboardingDirectMetadata.RequireCheckedScryptParameters(
                scryptLogN,
                scryptR,
                scryptP,
                nameof(ScryptLogN));
        }
    }
}
