using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

/// <summary>The encrypted-input representation admitted by a programmed RAM-FHE profile.</summary>
public enum ToriiRamFheEncryptedInputMode
{
    /// <summary>Canonical version-one encrypted envelope.</summary>
    EncryptedEnvelopeV1,
}

/// <summary>Immutable public RAM-FHE dimensions and initializer identity advertised by an identifier policy.</summary>
[JsonConverter(typeof(ToriiRamFheProfileJsonConverter))]
public sealed record class ToriiRamFheProfile
{
    /// <summary>Constructs a profile with positive dimensions and an exact, marked Iroha hash.</summary>
    public ToriiRamFheProfile(
        byte profileVersion,
        ushort registerCount,
        ushort memoryLaneCount,
        byte ciphertextMulPerStep,
        ToriiRamFheEncryptedInputMode encryptedInputMode,
        ulong minCiphertextModulus,
        string initializerDescriptorHash)
    {
        ArgumentOutOfRangeException.ThrowIfZero(profileVersion);
        ArgumentOutOfRangeException.ThrowIfZero(registerCount);
        ArgumentOutOfRangeException.ThrowIfZero(memoryLaneCount);
        ArgumentOutOfRangeException.ThrowIfZero(ciphertextMulPerStep);
        ArgumentOutOfRangeException.ThrowIfZero(minCiphertextModulus);
        if (encryptedInputMode != ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1)
        {
            throw new ArgumentOutOfRangeException(nameof(encryptedInputMode));
        }
        if (!IsCanonicalHash(initializerDescriptorHash))
        {
            throw new ArgumentException(
                "Initializer descriptor hash must contain exactly 64 lowercase hex digits with the Iroha Hash marker bit set.",
                nameof(initializerDescriptorHash));
        }

        ProfileVersion = profileVersion;
        RegisterCount = registerCount;
        MemoryLaneCount = memoryLaneCount;
        CiphertextMulPerStep = ciphertextMulPerStep;
        EncryptedInputMode = encryptedInputMode;
        MinCiphertextModulus = minCiphertextModulus;
        InitializerDescriptorHash = initializerDescriptorHash;
    }

    /// <summary>Positive unsigned eight-bit profile version.</summary>
    public byte ProfileVersion { get; }
    /// <summary>Positive unsigned sixteen-bit register count.</summary>
    public ushort RegisterCount { get; }
    /// <summary>Positive unsigned sixteen-bit memory lane count.</summary>
    public ushort MemoryLaneCount { get; }
    /// <summary>Positive unsigned eight-bit ciphertext multiplication count per step.</summary>
    public byte CiphertextMulPerStep { get; }
    /// <summary>The supported encrypted envelope format.</summary>
    public ToriiRamFheEncryptedInputMode EncryptedInputMode { get; }
    /// <summary>Positive unsigned sixty-four-bit minimum ciphertext modulus.</summary>
    public ulong MinCiphertextModulus { get; }
    /// <summary>Exact lowercase initializer descriptor hash, including the Iroha marker bit.</summary>
    public string InitializerDescriptorHash { get; }

    internal static bool IsCanonicalHash(string? value)
    {
        if (value is null || value.Length != 64 || !"13579bdf".Contains(value[^1], StringComparison.Ordinal))
        {
            return false;
        }
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

internal sealed class ToriiRamFheProfileJsonConverter : JsonConverter<ToriiRamFheProfile>
{
    public override bool HandleNull => true;

    public override ToriiRamFheProfile Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        const string path = "policy.ram_fhe_profile";
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"{path} must be an object.");
        }
        var seen = new HashSet<string>(StringComparer.Ordinal);
        byte? version = null;
        ushort? registers = null;
        ushort? lanes = null;
        byte? multiplications = null;
        ToriiRamFheEncryptedInputMode? mode = null;
        ulong? modulus = null;
        string? hash = null;
        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject)
            {
                return new ToriiRamFheProfile(
                    version ?? throw Missing("profile_version"),
                    registers ?? throw Missing("register_count"),
                    lanes ?? throw Missing("memory_lane_count"),
                    multiplications ?? throw Missing("ciphertext_mul_per_step"),
                    mode ?? throw Missing("encrypted_input_mode"),
                    modulus ?? throw Missing("min_ciphertext_modulus"),
                    hash ?? throw Missing("initializer_descriptor_hash"));
            }
            if (reader.TokenType != JsonTokenType.PropertyName)
            {
                throw new JsonException($"{path} property name expected.");
            }
            var name = reader.GetString()!;
            ToriiIdentifierJson.RequireUniqueProperty(seen, name, path);
            if (!reader.Read())
            {
                throw new JsonException($"{path}.{name} is truncated.");
            }
            switch (name)
            {
                case "profile_version":
                    version = (byte)ReadPositive(ref reader, byte.MaxValue, name);
                    break;
                case "register_count":
                    registers = (ushort)ReadPositive(ref reader, ushort.MaxValue, name);
                    break;
                case "memory_lane_count":
                    lanes = (ushort)ReadPositive(ref reader, ushort.MaxValue, name);
                    break;
                case "ciphertext_mul_per_step":
                    multiplications = (byte)ReadPositive(ref reader, byte.MaxValue, name);
                    break;
                case "min_ciphertext_modulus":
                    modulus = ReadPositive(ref reader, ulong.MaxValue, name);
                    break;
                case "encrypted_input_mode":
                    if (reader.TokenType != JsonTokenType.String || reader.GetString() != "encrypted_envelope_v1")
                    {
                        throw new JsonException($"{path}.{name} must be encrypted_envelope_v1.");
                    }
                    mode = ToriiRamFheEncryptedInputMode.EncryptedEnvelopeV1;
                    break;
                case "initializer_descriptor_hash":
                    if (reader.TokenType != JsonTokenType.String || !ToriiRamFheProfile.IsCanonicalHash(reader.GetString()))
                    {
                        throw new JsonException($"{path}.{name} must be 64 lowercase hex digits with the Iroha Hash marker bit set.");
                    }
                    hash = reader.GetString();
                    break;
                default:
                    throw new JsonException($"{path}.{name} is not a recognized profile field.");
            }
        }
        throw new JsonException($"{path} is truncated.");

        static JsonException Missing(string name) => new($"{path}.{name} is required.");
    }

    public override void Write(Utf8JsonWriter writer, ToriiRamFheProfile value, JsonSerializerOptions options)
    {
        if (value is null)
        {
            throw new JsonException("RAM-FHE profile must not be null.");
        }
        writer.WriteStartObject();
        writer.WriteNumber("profile_version", value.ProfileVersion);
        writer.WriteNumber("register_count", value.RegisterCount);
        writer.WriteNumber("memory_lane_count", value.MemoryLaneCount);
        writer.WriteNumber("ciphertext_mul_per_step", value.CiphertextMulPerStep);
        writer.WriteString("encrypted_input_mode", "encrypted_envelope_v1");
        writer.WriteNumber("min_ciphertext_modulus", value.MinCiphertextModulus);
        writer.WriteString("initializer_descriptor_hash", value.InitializerDescriptorHash);
        writer.WriteEndObject();
    }

    private static ulong ReadPositive(ref Utf8JsonReader reader, ulong maximum, string name)
    {
        if (reader.TokenType != JsonTokenType.Number || !reader.TryGetUInt64(out var value) || value == 0 || value > maximum)
        {
            throw new JsonException($"policy.ram_fhe_profile.{name} must be an integer in 1..{maximum}.");
        }
        return value;
    }
}
