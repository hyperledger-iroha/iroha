using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Torii;

public sealed record class ToriiServerSentEvent
{
    private JsonNode? jsonData;
    private string? eventName;
    private string? id;
    private int? retryMilliseconds;

    public string? Event
    {
        get => eventName;
        init
        {
            ValidateOptionalExactText(value, nameof(Event));
            eventName = value;
        }
    }

    public string? Id
    {
        get => id;
        init
        {
            ValidateOptionalExactText(value, nameof(Id));
            id = value;
        }
    }

    public int? RetryMilliseconds
    {
        get => retryMilliseconds;
        init
        {
            if (value is < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(RetryMilliseconds), "Retry milliseconds must be non-negative.");
            }

            retryMilliseconds = value;
        }
    }

    public string? RawData { get; init; }

    public JsonNode? JsonData
    {
        get => ToriiJsonSnapshots.Copy(jsonData);
        init => jsonData = ToriiJsonSnapshots.Copy(value);
    }

    public string? Comment { get; init; }

    public bool IsComment => Comment is not null && RawData is null;

    private static void ValidateOptionalExactText(string? value, string paramName)
    {
        if (value is null)
        {
            return;
        }

        if (value.Length == 0)
        {
            throw new ArgumentException("Value must be non-empty when provided.", paramName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }
    }
}
