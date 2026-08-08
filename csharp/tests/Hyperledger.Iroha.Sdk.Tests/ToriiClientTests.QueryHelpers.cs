namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    private static void WriteUInt64BigEndian(byte[] target, int offset, ulong value)
    {
        for (var index = 7; index >= 0; index--)
        {
            target[offset + index] = (byte)(value & 0xff);
            value >>= 8;
        }
    }

    private static string EventFilterQuery(string filterJson)
    {
        return "filter=" + Uri.EscapeDataString(filterJson);
    }

    private static string QueryParameter(string query, string name)
    {
        var queryText = query.StartsWith("?", StringComparison.Ordinal) ? query[1..] : query;
        foreach (var segment in queryText.Split('&'))
        {
            var equalsIndex = segment.IndexOf('=');
            var rawName = equalsIndex >= 0 ? segment[..equalsIndex] : segment;
            if (!string.Equals(Uri.UnescapeDataString(rawName), name, StringComparison.Ordinal))
            {
                continue;
            }

            var rawValue = equalsIndex >= 0 ? segment[(equalsIndex + 1)..] : string.Empty;
            return Uri.UnescapeDataString(rawValue.Replace("+", " ", StringComparison.Ordinal));
        }

        throw new InvalidOperationException($"Query parameter {name} was not present.");
    }
}
