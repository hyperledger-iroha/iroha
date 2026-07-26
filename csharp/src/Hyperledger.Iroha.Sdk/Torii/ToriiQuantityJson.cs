using System.Text.Json;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Torii;

/// <summary>Strict quantity validation shared by Torii response decoders.</summary>
internal static class ToriiQuantityJson
{
    internal static string RequireCanonicalQuantity(string? value, string field)
    {
        if (value is null)
        {
            throw new JsonException($"{field} must not be null.");
        }

        try
        {
            return NumericV1.QuantityValue.ParseCanonical(value).ToString();
        }
        catch (NumericV1.NumericException exception)
        {
            throw new JsonException(
                $"{field} must be a canonical non-negative numeric Kotodama V1 Quantity string.",
                exception);
        }
    }
}
