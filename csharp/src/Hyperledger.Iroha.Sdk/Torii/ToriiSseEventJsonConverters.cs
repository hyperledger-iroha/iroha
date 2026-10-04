using System.Text.Json;

namespace Hyperledger.Iroha.Torii;

/// <summary>Exact-text and hex checks shared by the Torii JSON converters.</summary>
internal static class ToriiSseEventJson
{
    internal static string RequireExactTokenText(string? value, string field)
    {
        var exact = RequireExactNonEmptyText(value, field);
        if (ContainsWhitespace(exact))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        return exact;
    }

    internal static string? RequireOptionalExactTokenText(string? value, string field)
    {
        return value is null ? null : RequireExactTokenText(value, field);
    }

    internal static string? RequireOptionalExactNonEmptyText(string? value, string field)
    {
        return value is null ? null : RequireExactNonEmptyText(value, field);
    }

    internal static string RequireExactSizedHex(string? value, string field, int expectedBytes)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty {expectedBytes}-byte hex string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (ContainsWhitespace(value))
        {
            throw new JsonException($"{field} must not contain whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        if (value.Length != expectedBytes * 2 || !IsLowercaseHex(value))
        {
            throw new JsonException($"{field} must be an exact lowercase {expectedBytes}-byte hex string.");
        }

        return value;
    }

    internal static string? RequireOptionalExactSizedHex(string? value, string field, int expectedBytes)
    {
        return value is null ? null : RequireExactSizedHex(value, field, expectedBytes);
    }

    private static string RequireExactNonEmptyText(string? value, string field)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new JsonException($"{field} must be a non-empty string.");
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new JsonException($"{field} must not contain surrounding whitespace.");
        }

        if (ContainsControlCharacter(value))
        {
            throw new JsonException($"{field} must not contain control characters.");
        }

        return value;
    }

    private static bool ContainsWhitespace(string value)
    {
        foreach (var character in value)
        {
            if (char.IsWhiteSpace(character))
            {
                return true;
            }
        }

        return false;
    }

    private static bool ContainsControlCharacter(string value)
    {
        foreach (var character in value)
        {
            if (char.IsControl(character))
            {
                return true;
            }
        }

        return false;
    }

    private static bool IsLowercaseHex(string value)
    {
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
