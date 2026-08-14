using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiAccountFaucetJson
{
    internal static void ValidateAccountFaucetPuzzle(ToriiAccountFaucetPuzzle response, string context)
    {
        ArgumentNullException.ThrowIfNull(response);

        ToriiSseEventJson.RequireExactTokenText(response.Algorithm, $"{context}.algorithm");
        if (!string.Equals(response.Algorithm, ToriiAccountFaucetPow.Algorithm, StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.algorithm must be {ToriiAccountFaucetPow.Algorithm}.");
        }

        if (response.NetworkId is null)
        {
            throw new JsonException($"{context}.network_id must not be null.");
        }

        if (response.DifficultyBits == 0)
        {
            throw new JsonException($"{context}.difficulty_bits must be positive.");
        }

        if (response.AnchorHeight == 0)
        {
            throw new JsonException($"{context}.anchor_height must be positive.");
        }

        ToriiSseEventJson.RequireExactSizedHex(response.AnchorBlockHashHex, $"{context}.anchor_block_hash_hex", 32);
        if (response.ChallengeSaltHex is not null)
        {
            ToriiSseEventJson.RequireExactSizedHex(
                response.ChallengeSaltHex,
                $"{context}.challenge_salt_hex",
                32);
        }

        try
        {
            _ = ToriiAccountFaucetPow.CheckedScryptParameters(
                response.ScryptLogN,
                response.ScryptR,
                response.ScryptP);
        }
        catch (ArgumentOutOfRangeException exception)
        {
            throw new JsonException(
                $"{context}.{ScryptParameterName(exception.ParamName)} {exception.Message}",
                exception);
        }

        if (response.MaxAnchorAgeBlocks == 0)
        {
            throw new JsonException($"{context}.max_anchor_age_blocks must be positive.");
        }
    }

    private static string ScryptParameterName(string? paramName)
    {
        return paramName switch
        {
            "scryptLogN" => "scrypt_log_n",
            "scryptR" => "scrypt_r",
            "scryptP" => "scrypt_p",
            _ => "scrypt",
        };
    }

    internal static string? ReadOptionalString(ref Utf8JsonReader reader, string field)
    {
        return reader.TokenType switch
        {
            JsonTokenType.Null => null,
            JsonTokenType.String => reader.GetString(),
            _ => throw new JsonException($"{field} must be a string."),
        };
    }

    internal static byte ReadByte(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetByte(out var value))
        {
            throw new JsonException($"{field} must be an unsigned 8-bit integer.");
        }

        return value;
    }

    internal static ushort ReadUInt16(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt16(out var value))
        {
            throw new JsonException($"{field} must be an unsigned 16-bit integer.");
        }

        return value;
    }

    internal static NetworkId ReadNetworkId(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            throw new JsonException($"{field} must be a canonical NetworkId string.");
        }

        try
        {
            return NetworkId.Parse(reader.GetString()!);
        }
        catch (FormatException error)
        {
            throw new JsonException($"{field} must be a canonical checksummed NetworkId.", error);
        }
    }

    internal static uint ReadUInt32(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt32(out var value))
        {
            throw new JsonException($"{field} must be an unsigned integer.");
        }

        return value;
    }

    internal static ulong ReadUInt64(ref Utf8JsonReader reader, string field)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt64(out var value))
        {
            throw new JsonException($"{field} must be an unsigned integer.");
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

    internal static uint RequireUInt32(uint? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    internal static ulong RequireUInt64(ulong? value, string context, string propertyName)
    {
        if (!value.HasValue)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value.Value;
    }

    internal static string RequireString(string? value, string context, string propertyName)
    {
        if (value is null)
        {
            throw new JsonException($"{context}.{propertyName} must not be null.");
        }

        return value;
    }

}

internal sealed class ToriiAccountFaucetPuzzleJsonConverter : JsonConverter<ToriiAccountFaucetPuzzle>
{
    public override bool HandleNull => true;

    public override ToriiAccountFaucetPuzzle Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Null)
        {
            throw new JsonException("account faucet puzzle must not be null.");
        }

        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException("account faucet puzzle must be an object.");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        string? algorithm = null;
        NetworkId? networkId = null;
        ushort? chainDiscriminant = null;
        byte? difficultyBits = null;
        ulong? anchorHeight = null;
        string? anchorBlockHashHex = null;
        string? challengeSaltHex = null;
        byte? scryptLogN = null;
        uint? scryptR = null;
        uint? scryptP = null;
        ulong? maxAnchorAgeBlocks = null;

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                try
                {
                    var response = new ToriiAccountFaucetPuzzle
                    {
                        Algorithm = ToriiAccountFaucetJson.RequireString(
                            algorithm,
                            "account faucet puzzle",
                            "algorithm"),
                        NetworkId = networkId
                            ?? throw new JsonException(
                                "account faucet puzzle.network_id must not be null."),
                        ChainDiscriminant = ToriiAccountFaucetJson.RequireUInt16(
                            chainDiscriminant,
                            "account faucet puzzle",
                            "chain_discriminant"),
                        DifficultyBits = ToriiAccountFaucetJson.RequireByte(
                            difficultyBits,
                            "account faucet puzzle",
                            "difficulty_bits"),
                        AnchorHeight = ToriiAccountFaucetJson.RequireUInt64(
                            anchorHeight,
                            "account faucet puzzle",
                            "anchor_height"),
                        AnchorBlockHashHex = ToriiAccountFaucetJson.RequireString(
                            anchorBlockHashHex,
                            "account faucet puzzle",
                            "anchor_block_hash_hex"),
                        ChallengeSaltHex = challengeSaltHex,
                        ScryptLogN = ToriiAccountFaucetJson.RequireByte(
                            scryptLogN,
                            "account faucet puzzle",
                            "scrypt_log_n"),
                        ScryptR = ToriiAccountFaucetJson.RequireUInt32(
                            scryptR,
                            "account faucet puzzle",
                            "scrypt_r"),
                        ScryptP = ToriiAccountFaucetJson.RequireUInt32(
                            scryptP,
                            "account faucet puzzle",
                            "scrypt_p"),
                        MaxAnchorAgeBlocks = ToriiAccountFaucetJson.RequireUInt64(
                            maxAnchorAgeBlocks,
                            "account faucet puzzle",
                            "max_anchor_age_blocks"),
                    };
                    ToriiAccountFaucetJson.ValidateAccountFaucetPuzzle(response, "account faucet puzzle");
                    return response;
                }
                catch (ArgumentException error) when (error.ParamName is not null)
                {
                    throw DirectMetadataErrorToJsonException(error, "account faucet puzzle");
                }
            }

            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException("account faucet puzzle property name expected.");
            }

            var propertyName = reader.GetString() ?? throw new JsonException("account faucet puzzle property name must be a string.");
            ToriiIdentifierJson.RequireUniqueProperty(seen, propertyName, "account faucet puzzle");
            if (!reader.Read())
            {
                throw new JsonException($"account faucet puzzle.{propertyName} is truncated.");
            }

            switch (propertyName)
            {
                case "algorithm":
                    algorithm = ToriiAccountFaucetJson.ReadOptionalString(ref reader, "account faucet puzzle.algorithm");
                    break;
                case "network_id":
                    networkId = ToriiAccountFaucetJson.ReadNetworkId(
                        ref reader,
                        "account faucet puzzle.network_id");
                    break;
                case "chain_discriminant":
                    chainDiscriminant = ToriiAccountFaucetJson.ReadUInt16(
                        ref reader,
                        "account faucet puzzle.chain_discriminant");
                    break;
                case "difficulty_bits":
                    difficultyBits = ToriiAccountFaucetJson.ReadByte(ref reader, "account faucet puzzle.difficulty_bits");
                    break;
                case "anchor_height":
                    anchorHeight = ToriiAccountFaucetJson.ReadUInt64(ref reader, "account faucet puzzle.anchor_height");
                    break;
                case "anchor_block_hash_hex":
                    anchorBlockHashHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, "account faucet puzzle.anchor_block_hash_hex");
                    break;
                case "challenge_salt_hex":
                    challengeSaltHex = ToriiAccountFaucetJson.ReadOptionalString(ref reader, "account faucet puzzle.challenge_salt_hex");
                    break;
                case "scrypt_log_n":
                    scryptLogN = ToriiAccountFaucetJson.ReadByte(ref reader, "account faucet puzzle.scrypt_log_n");
                    break;
                case "scrypt_r":
                    scryptR = ToriiAccountFaucetJson.ReadUInt32(ref reader, "account faucet puzzle.scrypt_r");
                    break;
                case "scrypt_p":
                    scryptP = ToriiAccountFaucetJson.ReadUInt32(ref reader, "account faucet puzzle.scrypt_p");
                    break;
                case "max_anchor_age_blocks":
                    maxAnchorAgeBlocks = ToriiAccountFaucetJson.ReadUInt64(ref reader, "account faucet puzzle.max_anchor_age_blocks");
                    break;
                default:
                    ToriiIdentifierJson.SkipRejectingDuplicateProperties(
                        ref reader,
                        $"account faucet puzzle.{propertyName}");
                    break;
            }
        }

        throw new JsonException("account faucet puzzle is truncated.");
    }

    private static JsonException DirectMetadataErrorToJsonException(ArgumentException error, string context)
    {
        var field = error.ParamName switch
        {
            "Algorithm" => "algorithm",
            "NetworkId" => "network_id",
            "ChainDiscriminant" => "chain_discriminant",
            "DifficultyBits" => "difficulty_bits",
            "AnchorHeight" => "anchor_height",
            "AnchorBlockHashHex" => "anchor_block_hash_hex",
            "ChallengeSaltHex" => "challenge_salt_hex",
            "ScryptLogN" => "scrypt_log_n",
            "ScryptR" => "scrypt_r",
            "ScryptP" => "scrypt_p",
            "MaxAnchorAgeBlocks" => "max_anchor_age_blocks",
            _ => error.ParamName,
        };
        return new JsonException($"{context}.{field}: {error.Message}", error);
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiAccountFaucetPuzzle value,
        JsonSerializerOptions options)
    {
        ToriiAccountFaucetJson.ValidateAccountFaucetPuzzle(value, "account faucet puzzle");

        writer.WriteStartObject();
        writer.WriteString("algorithm", value.Algorithm);
        writer.WriteString("network_id", value.NetworkId.ToString());
        writer.WriteNumber("chain_discriminant", value.ChainDiscriminant);
        writer.WriteNumber("difficulty_bits", value.DifficultyBits);
        writer.WriteNumber("anchor_height", value.AnchorHeight);
        writer.WriteString("anchor_block_hash_hex", value.AnchorBlockHashHex);
        if (value.ChallengeSaltHex is null)
        {
            writer.WriteNull("challenge_salt_hex");
        }
        else
        {
            writer.WriteString("challenge_salt_hex", value.ChallengeSaltHex);
        }

        writer.WriteNumber("scrypt_log_n", value.ScryptLogN);
        writer.WriteNumber("scrypt_r", value.ScryptR);
        writer.WriteNumber("scrypt_p", value.ScryptP);
        writer.WriteNumber("max_anchor_age_blocks", value.MaxAnchorAgeBlocks);
        writer.WriteEndObject();
    }
}
