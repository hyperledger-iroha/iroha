using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// Increases the total bond or extends the lock of an existing public standalone ballot.
/// The ballot choice remains in finalized state and is never supplied by an update.
/// </summary>
public sealed record class UpdatePlainConvictionInstruction : TransactionInstruction
{
    /// <summary>The sole registered first-release wire identifier.</summary>
    public const string NativeWireId =
        "iroha.instruction.v1::governance::UpdatePlainConviction";

    /// <summary>The concrete first-release Norito schema.</summary>
    public const string NativeTypeName =
        "iroha_data_model::isi::governance::UpdatePlainConviction";

    private static readonly HashSet<string> CanonicalFields =
        new(["referendum_id", "owner", "amount", "duration_blocks"], StringComparer.Ordinal);

    private readonly NumericV1.QuantityValue amount;

    /// <summary>Creates a choice-free update with the new total bond and requested lock duration.</summary>
    public UpdatePlainConvictionInstruction(
        string referendumId,
        string ownerAccountId,
        string amount,
        ulong durationBlocks)
    {
        ReferendumId = RequireGovernanceSelectorV1(referendumId);
        OwnerAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            ownerAccountId, nameof(ownerAccountId));
        this.amount = AssetQuantityValidation.RequireCanonicalQuantity(amount, nameof(amount));
        if (this.amount.Mantissa.IsZero)
        {
            throw new ArgumentOutOfRangeException(nameof(amount), "The total ballot bond must be positive.");
        }
        DurationBlocks = durationBlocks;
    }

    /// <summary>Canonical referendum selector.</summary>
    public string ReferendumId { get; }

    /// <summary>Canonical, domainless account that owns the existing ballot.</summary>
    public string OwnerAccountId { get; }

    /// <summary>New total bond in canonical quantity spelling.</summary>
    public string Amount => amount.ToString();

    /// <summary>Exact new lock duration, including the full unsigned 64-bit range.</summary>
    public ulong DurationBlocks { get; }

    /// <summary>
    /// Constructs only the four registered native fields. Extra fields, including a
    /// replacement choice, cannot enter the signed instruction.
    /// </summary>
    public static UpdatePlainConvictionInstruction FromCanonicalFields(
        IReadOnlyDictionary<string, string> fields)
    {
        ArgumentNullException.ThrowIfNull(fields);
        if (fields.Count != CanonicalFields.Count
            || fields.Keys.Any(static key => !CanonicalFields.Contains(key))
            || !fields.TryGetValue("referendum_id", out var referendumId)
            || !fields.TryGetValue("owner", out var owner)
            || !fields.TryGetValue("amount", out var amount)
            || !fields.TryGetValue("duration_blocks", out var durationText)
            || !ulong.TryParse(durationText, System.Globalization.NumberStyles.None,
                System.Globalization.CultureInfo.InvariantCulture, out var durationBlocks)
            || !string.Equals(durationBlocks.ToString(System.Globalization.CultureInfo.InvariantCulture),
                durationText, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "UpdatePlainConviction requires exactly canonical referendum_id, owner, amount, and duration_blocks.",
                nameof(fields));
        }
        return new UpdatePlainConvictionInstruction(
            referendumId, owner, amount, durationBlocks);
    }

    internal override string WireId => NativeWireId;

    internal override string TypeName => NativeTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        ArgumentNullException.ThrowIfNull(context);
        if (AccountAddress.Parse(OwnerAccountId) != AccountAddress.Parse(context.AuthorityAccountId))
        {
            throw new ArgumentException(
                "UpdatePlainConviction owner must equal transaction authority.",
                nameof(context));
        }

        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeString(ReferendumId));
        writer.WriteField(context.EncodeAccountId(OwnerAccountId));
        writer.WriteField(context.EncodeQuantity(amount));
        writer.WriteField(context.EncodeUInt64(DurationBlocks));
        return writer.ToArray();
    }

    private static string RequireGovernanceSelectorV1(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length is < 1 or > 128 || !IsUnreservedWithoutDot(value[0]))
        {
            throw new ArgumentException("Referendum id must be a canonical V1 governance selector.", nameof(value));
        }
        for (var index = 1; index < value.Length; index++)
        {
            if (!IsUnreservedWithoutDot(value[index]) && value[index] != '.')
            {
                throw new ArgumentException("Referendum id must be a canonical V1 governance selector.", nameof(value));
            }
        }
        return value;

        static bool IsUnreservedWithoutDot(char character) =>
            character is >= 'A' and <= 'Z'
                or >= 'a' and <= 'z'
                or >= '0' and <= '9'
                or '-' or '_' or '~';
    }
}
