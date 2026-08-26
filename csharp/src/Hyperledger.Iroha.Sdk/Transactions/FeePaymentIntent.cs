using System.Collections.ObjectModel;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Transactions;

/// <summary>Fee component constrained by a signature-bound maximum charge.</summary>
[JsonConverter(typeof(FeeChargeKindJsonConverter))]
public enum FeeChargeKind
{
    /// <summary>Nexus admission and execution fee.</summary>
    Nexus,

    /// <summary>Pipeline gas charged for contract or IVM execution.</summary>
    PipelineGas,
}

/// <summary>Exact asset and maximum amount authorized for one fee component.</summary>
public sealed record class FeeChargeLimit
{
    /// <summary>Creates a canonical, strictly positive fee charge limit.</summary>
    public FeeChargeLimit(FeeChargeKind kind, string assetDefinitionId, string maxAmount)
    {
        Kind = kind;
        AssetDefinitionId = TransactionEncodingContext.CanonicalizeAssetDefinitionId(
            assetDefinitionId,
            nameof(assetDefinitionId));

        NumericV1.QuantityValue quantity;
        try
        {
            quantity = NumericV1.QuantityValue.ParseCanonical(maxAmount);
        }
        catch (Exception exception) when (exception is ArgumentException or ArgumentNullException)
        {
            throw new ArgumentException(
                "Fee maximum must be a canonical, strictly positive quantity.",
                nameof(maxAmount),
                exception);
        }

        if (quantity.Mantissa.IsZero)
        {
            throw new ArgumentOutOfRangeException(nameof(maxAmount), "Fee maximum must be strictly positive.");
        }

        MaxAmount = quantity.ToString();
    }

    /// <summary>Fee component represented by this limit.</summary>
    [JsonPropertyName("kind")]
    public FeeChargeKind Kind { get; }

    /// <summary>Canonical unprefixed Base58 asset-definition identifier.</summary>
    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId { get; }

    /// <summary>Canonical strictly positive quantity.</summary>
    [JsonPropertyName("max_amount")]
    public string MaxAmount { get; }
}

/// <summary>Exact immutable sponsor-program identifier.</summary>
public sealed record class FeeSponsorProgramId
{
    /// <summary>Creates an exact sponsor-local program identifier.</summary>
    public FeeSponsorProgramId(string sponsor, string name)
    {
        var canonicalSponsor = TransactionEncodingContext.CanonicalizeAccountId(sponsor, nameof(sponsor));
        if (!string.Equals(canonicalSponsor, sponsor, StringComparison.Ordinal))
        {
            throw new ArgumentException("Sponsor must use the exact canonical I105 account encoding.", nameof(sponsor));
        }

        if (string.IsNullOrEmpty(name)
            || !string.Equals(name.Normalize(NormalizationForm.FormC), name, StringComparison.Ordinal)
            || name.Any(static character =>
                char.IsWhiteSpace(character)
                || char.IsControl(character)
                || character is '@' or '#' or '$' or '/'))
        {
            throw new ArgumentException(
                "Program name must be non-empty NFC text without whitespace or reserved characters.",
                nameof(name));
        }

        Sponsor = canonicalSponsor;
        Name = name;
    }

    /// <summary>Canonical sponsor account.</summary>
    [JsonPropertyName("sponsor")]
    public string Sponsor { get; }

    /// <summary>Sponsor-local program name.</summary>
    [JsonPropertyName("name")]
    public string Name { get; }

    /// <summary>Parses an exact <c>sponsor/program</c> selector without rewriting it.</summary>
    public static FeeSponsorProgramId Parse(string literal)
    {
        ArgumentNullException.ThrowIfNull(literal);
        if (!string.Equals(literal.Trim(), literal, StringComparison.Ordinal))
        {
            throw new ArgumentException("Program id must not contain surrounding whitespace.", nameof(literal));
        }

        var separator = literal.IndexOf('/');
        if (separator <= 0
            || separator != literal.LastIndexOf('/')
            || separator == literal.Length - 1)
        {
            throw new ArgumentException("Program id must use the exact sponsor/program form.", nameof(literal));
        }

        var parsed = new FeeSponsorProgramId(literal[..separator], literal[(separator + 1)..]);
        if (!string.Equals(parsed.ToString(), literal, StringComparison.Ordinal))
        {
            throw new ArgumentException("Program id must use the exact canonical sponsor/program form.", nameof(literal));
        }
        return parsed;
    }

    /// <inheritdoc />
    public override string ToString() => $"{Sponsor}/{Name}";
}

/// <summary>Required signature-bound choice of fee payer, charge maxima, and gas bound.</summary>
[JsonConverter(typeof(FeePaymentIntentJsonConverter))]
public abstract class FeePaymentIntent : IEquatable<FeePaymentIntent>
{
    private readonly ReadOnlyCollection<FeeChargeLimit> chargeLimits;

    private protected FeePaymentIntent(IEnumerable<FeeChargeLimit> chargeLimits, ulong? gasLimit)
    {
        ArgumentNullException.ThrowIfNull(chargeLimits);
        var snapshot = chargeLimits.ToArray();
        if (snapshot.Any(static limit => limit is null))
        {
            throw new ArgumentException("Charge limits must not contain null entries.", nameof(chargeLimits));
        }
        if (gasLimit == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(gasLimit), "Gas limit must be positive when provided.");
        }

        var previous = -1;
        foreach (var limit in snapshot)
        {
            var current = limit.Kind switch
            {
                FeeChargeKind.Nexus => 0,
                FeeChargeKind.PipelineGas => 1,
                _ => throw new ArgumentOutOfRangeException(nameof(chargeLimits), "Unknown fee charge kind."),
            };
            if (current <= previous)
            {
                throw new ArgumentException(
                    "Charge limits must be unique and ordered nexus before pipeline gas.",
                    nameof(chargeLimits));
            }
            previous = current;
        }

        this.chargeLimits = Array.AsReadOnly(snapshot);
        GasLimit = gasLimit;
    }

    /// <summary>Canonically ordered per-component maximum charges.</summary>
    public IReadOnlyList<FeeChargeLimit> ChargeLimits => chargeLimits;

    /// <summary>Signature-bound maximum executable gas, when applicable.</summary>
    public ulong? GasLimit { get; }

    internal abstract uint PayerTag { get; }

    /// <summary>Constructs an authority-paid intent.</summary>
    public static FeePaymentIntent Authority(
        IEnumerable<FeeChargeLimit> chargeLimits,
        ulong? gasLimit = null) => new AuthorityFeePaymentIntent(chargeLimits, gasLimit);

    /// <summary>Constructs an exact sponsor-program revision intent.</summary>
    public static FeePaymentIntent Sponsor(
        FeeSponsorProgramId programId,
        ulong programRevision,
        IEnumerable<FeeChargeLimit> chargeLimits,
        ulong? gasLimit = null) =>
        new SponsorFeePaymentIntent(programId, programRevision, chargeLimits, gasLimit);

    /// <summary>
    /// Returns true only when a quote preserves the exact payer, sponsor revision, and gas bound.
    /// </summary>
    public bool HasSamePayerAndGasBound(FeePaymentIntent other)
    {
        ArgumentNullException.ThrowIfNull(other);
        if (GasLimit != other.GasLimit)
        {
            return false;
        }

        return (this, other) switch
        {
            (AuthorityFeePaymentIntent, AuthorityFeePaymentIntent) => true,
            (SponsorFeePaymentIntent left, SponsorFeePaymentIntent right) =>
                left.ProgramId == right.ProgramId && left.ProgramRevision == right.ProgramRevision,
            _ => false,
        };
    }

    /// <inheritdoc />
    public bool Equals(FeePaymentIntent? other)
    {
        if (other is null || GetType() != other.GetType() || GasLimit != other.GasLimit)
        {
            return false;
        }
        if (!ChargeLimits.SequenceEqual(other.ChargeLimits))
        {
            return false;
        }

        return (this, other) switch
        {
            (AuthorityFeePaymentIntent, AuthorityFeePaymentIntent) => true,
            (SponsorFeePaymentIntent left, SponsorFeePaymentIntent right) =>
                left.ProgramId == right.ProgramId && left.ProgramRevision == right.ProgramRevision,
            _ => false,
        };
    }

    /// <inheritdoc />
    public override bool Equals(object? obj) => obj is FeePaymentIntent other && Equals(other);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(GetType());
        hash.Add(GasLimit);
        foreach (var limit in ChargeLimits)
        {
            hash.Add(limit);
        }
        if (this is SponsorFeePaymentIntent sponsor)
        {
            hash.Add(sponsor.ProgramId);
            hash.Add(sponsor.ProgramRevision);
        }
        return hash.ToHashCode();
    }
}

/// <summary>Authority-paid fee intent.</summary>
public sealed class AuthorityFeePaymentIntent : FeePaymentIntent
{
    internal AuthorityFeePaymentIntent(IEnumerable<FeeChargeLimit> chargeLimits, ulong? gasLimit)
        : base(chargeLimits, gasLimit)
    {
    }

    internal override uint PayerTag => 0;
}

/// <summary>Exact sponsor-program revision fee intent.</summary>
public sealed class SponsorFeePaymentIntent : FeePaymentIntent
{
    internal SponsorFeePaymentIntent(
        FeeSponsorProgramId programId,
        ulong programRevision,
        IEnumerable<FeeChargeLimit> chargeLimits,
        ulong? gasLimit)
        : base(chargeLimits, gasLimit)
    {
        ArgumentNullException.ThrowIfNull(programId);
        if (programRevision == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(programRevision), "Program revision must be positive.");
        }
        ProgramId = programId;
        ProgramRevision = programRevision;
    }

    /// <summary>Exact sponsor program selected before signing.</summary>
    public FeeSponsorProgramId ProgramId { get; }

    /// <summary>Exact immutable program revision selected before signing.</summary>
    public ulong ProgramRevision { get; }

    internal override uint PayerTag => 1;
}

internal sealed class FeeChargeKindJsonConverter : JsonConverter<FeeChargeKind>
{
    public override FeeChargeKind Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var document = JsonDocument.ParseValue(ref reader);
        var properties = FeePaymentIntentJsonConverter.RequireExactObject(
            document.RootElement,
            "fee charge kind",
            ["kind", "value"]);
        if (properties["value"].ValueKind != JsonValueKind.Null)
        {
            throw new JsonException("fee charge kind.value must be null.");
        }
        return properties["kind"].GetString() switch
        {
            "nexus" => FeeChargeKind.Nexus,
            "pipeline_gas" => FeeChargeKind.PipelineGas,
            _ => throw new JsonException("fee charge kind.kind must be nexus or pipeline_gas."),
        };
    }

    public override void Write(Utf8JsonWriter writer, FeeChargeKind value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        writer.WriteString("kind", value switch
        {
            FeeChargeKind.Nexus => "nexus",
            FeeChargeKind.PipelineGas => "pipeline_gas",
            _ => throw new JsonException("Unknown fee charge kind."),
        });
        writer.WriteNull("value");
        writer.WriteEndObject();
    }
}

internal sealed class FeePaymentIntentJsonConverter : JsonConverter<FeePaymentIntent>
{
    public override FeePaymentIntent Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var document = JsonDocument.ParseValue(ref reader);
        var root = RequireExactObject(document.RootElement, "fee payment", ["payer", "value"]);
        var payer = RequireString(root["payer"], "fee payment.payer");
        var value = payer switch
        {
            "authority" => RequireExactObject(
                root["value"],
                "fee payment.value",
                ["charge_limits"],
                ["charge_limits", "gas_limit"]),
            "sponsor" => RequireExactObject(
                root["value"],
                "fee payment.value",
                ["program_id", "program_revision", "charge_limits"],
                ["program_id", "program_revision", "charge_limits", "gas_limit"]),
            _ => throw new JsonException("fee payment.payer must be authority or sponsor."),
        };

        if (value["charge_limits"].ValueKind != JsonValueKind.Array)
        {
            throw new JsonException("fee payment.value.charge_limits must be an array.");
        }
        var limits = value["charge_limits"]
            .EnumerateArray()
            .Select((element, index) => ReadLimit(element, options, $"fee payment.value.charge_limits[{index}]"))
            .ToArray();
        ulong? gasLimit = value.TryGetValue("gas_limit", out var gasElement)
            && gasElement.ValueKind != JsonValueKind.Null
                ? RequirePositiveUInt64(gasElement, "fee payment.value.gas_limit")
                : null;

        if (payer == "authority")
        {
            return FeePaymentIntent.Authority(limits, gasLimit);
        }

        var program = RequireExactObject(
            value["program_id"],
            "fee payment.value.program_id",
            ["sponsor", "name"]);
        var programId = new FeeSponsorProgramId(
            RequireString(program["sponsor"], "fee payment.value.program_id.sponsor"),
            RequireString(program["name"], "fee payment.value.program_id.name"));
        var revision = RequirePositiveUInt64(
            value["program_revision"],
            "fee payment.value.program_revision");
        return FeePaymentIntent.Sponsor(programId, revision, limits, gasLimit);
    }

    public override void Write(Utf8JsonWriter writer, FeePaymentIntent value, JsonSerializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(value);
        writer.WriteStartObject();
        writer.WriteString("payer", value is AuthorityFeePaymentIntent ? "authority" : "sponsor");
        writer.WritePropertyName("value");
        writer.WriteStartObject();
        if (value is SponsorFeePaymentIntent sponsor)
        {
            writer.WritePropertyName("program_id");
            JsonSerializer.Serialize(writer, sponsor.ProgramId, options);
            writer.WriteNumber("program_revision", sponsor.ProgramRevision);
        }
        writer.WritePropertyName("charge_limits");
        JsonSerializer.Serialize(writer, value.ChargeLimits, options);
        if (value.GasLimit.HasValue)
        {
            writer.WriteNumber("gas_limit", value.GasLimit.Value);
        }
        else
        {
            writer.WriteNull("gas_limit");
        }
        writer.WriteEndObject();
        writer.WriteEndObject();
    }

    internal static Dictionary<string, JsonElement> RequireExactObject(
        JsonElement element,
        string path,
        IReadOnlyCollection<string> required,
        IReadOnlyCollection<string>? allowed = null)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException($"{path} must be an object.");
        }

        allowed ??= required;
        var allowedSet = new HashSet<string>(allowed, StringComparer.Ordinal);
        var values = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
        foreach (var property in element.EnumerateObject())
        {
            if (!allowedSet.Contains(property.Name))
            {
                throw new JsonException($"{path} contains unknown field `{property.Name}`.");
            }
            if (!values.TryAdd(property.Name, property.Value))
            {
                throw new JsonException($"{path}.{property.Name} must not be duplicated.");
            }
        }
        foreach (var name in required)
        {
            if (!values.ContainsKey(name))
            {
                throw new JsonException($"{path}.{name} is required.");
            }
        }
        return values;
    }

    private static FeeChargeLimit ReadLimit(JsonElement element, JsonSerializerOptions options, string path)
    {
        var item = RequireExactObject(
            element,
            path,
            ["kind", "asset_definition_id", "max_amount"]);
        FeeChargeKind kind;
        try
        {
            kind = item["kind"].Deserialize<FeeChargeKind>(options);
        }
        catch (Exception exception) when (exception is JsonException or NotSupportedException)
        {
            throw new JsonException($"{path}.kind is invalid.", exception);
        }
        return new FeeChargeLimit(
            kind,
            RequireString(item["asset_definition_id"], $"{path}.asset_definition_id"),
            RequireString(item["max_amount"], $"{path}.max_amount"));
    }

    private static string RequireString(JsonElement element, string path)
    {
        if (element.ValueKind != JsonValueKind.String || element.GetString() is not { } value)
        {
            throw new JsonException($"{path} must be a string.");
        }
        return value;
    }

    private static ulong RequirePositiveUInt64(JsonElement element, string path)
    {
        if (element.ValueKind != JsonValueKind.Number
            || !element.TryGetUInt64(out var value)
            || value == 0)
        {
            throw new JsonException($"{path} must be a positive uint64.");
        }
        return value;
    }
}
