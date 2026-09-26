using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.Torii;

/// <summary>Exact public weights and committed block coordinates for a V1 standalone election.</summary>
public sealed record ToriiElectionTallyV1(
    ulong EvaluatedBlockHeight,
    string EvaluatedBlockHash,
    bool Finalized,
    IReadOnlyList<UInt128> Tally);

public sealed partial class ToriiClient
{
    private const string ElectionTallyPathV1 = "/v1/zk/vote/tally";
    private const int ElectionTallyResponseMaxBytesV1 = 8 * 1024;

    /// <summary>Reads one exact V1 election tally using a signed, network-bound POST.</summary>
    /// <returns>The public tally, or <see langword="null"/> when the election is absent.</returns>
    public async Task<ToriiElectionTallyV1?> GetElectionTallyAsync(
        string electionId,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(electionId);
        if (!IsGovernanceSelectorV1(electionId))
        {
            throw new ArgumentException(
                "Election ID must be a canonical V1 governance selector of 1–128 ASCII bytes.",
                nameof(electionId));
        }
        RequireCanonicalRequestCredentials(ElectionTallyPathV1);
        if (Options.NetworkId is null)
        {
            throw new InvalidOperationException(
                $"Route `{ElectionTallyPathV1}` requires ToriiClientOptions.NetworkId.");
        }

        var expectedUri = BuildRequestUri(ElectionTallyPathV1, query: null);
        using var content = new ByteArrayContent(
            Encoding.UTF8.GetBytes($"{{\"election_id\":\"{electionId}\"}}"));
        content.Headers.ContentType = new MediaTypeHeaderValue("application/json");
        using var response = await SendExpectingStatusAsync(
            HttpMethod.Post,
            ElectionTallyPathV1,
            query: null,
            content,
            HttpStatusCode.OK,
            HttpStatusCode.NotFound,
            cancellationToken,
            accept: "application/json");
        if (response.RequestMessage?.RequestUri is not Uri responseUri
            || !string.Equals(responseUri.AbsoluteUri, expectedUri.AbsoluteUri, StringComparison.Ordinal))
        {
            throw new HttpRequestException(
                $"Authenticated Torii route `{ElectionTallyPathV1}` changed the exact request target.");
        }
        RequireElectionTallyIdentityEncoding(response);
        var body = await ReadBoundedResponseBodyAsync(
            response.Content,
            ElectionTallyResponseMaxBytesV1,
            "Election tally response",
            cancellationToken);
        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            if (body.Length != 0)
            {
                throw new InvalidDataException("Election tally 404 response must have an empty body.");
            }
            return null;
        }

        RequireSingleJsonContentType(response.Content, "Election tally response");
        using var document = JsonDocument.Parse(body, new JsonDocumentOptions { MaxDepth = 3 });
        ToriiIdentifierJson.RejectDuplicateProperties(document.RootElement, "election tally response");
        return ParseElectionTallyV1(document.RootElement);
    }

    private static bool IsGovernanceSelectorV1(string value)
    {
        if (value.Length is < 1 or > 128 || !IsUnreservedWithoutDot(value[0]))
        {
            return false;
        }
        for (var index = 1; index < value.Length; index++)
        {
            if (!IsUnreservedWithoutDot(value[index]) && value[index] != '.')
            {
                return false;
            }
        }
        return true;

        static bool IsUnreservedWithoutDot(char character) =>
            character is >= 'A' and <= 'Z'
                or >= 'a' and <= 'z'
                or >= '0' and <= '9'
                or '-' or '_' or '~';
    }

    private static void RequireElectionTallyIdentityEncoding(HttpResponseMessage response)
    {
        var encodings = response.Content.Headers.ContentEncoding.ToArray();
        if (encodings.Length > 1
            || (encodings.Length == 1
                && !string.Equals(encodings[0], "identity", StringComparison.OrdinalIgnoreCase)))
        {
            throw new InvalidDataException("Election tally response must use identity encoding.");
        }
    }

    private static ToriiElectionTallyV1 ParseElectionTallyV1(JsonElement root)
    {
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Election tally response must be an object.");
        }
        RequireExactJsonFields(
            root,
            "Election tally response",
            "evaluated_block_height",
            "evaluated_block_hash",
            "finalized",
            "tally");
        var height = ReadExactElectionUnsigned<ulong>(
            root.GetProperty("evaluated_block_height"),
            "evaluated_block_height",
            ulong.TryParse);
        var hashElement = root.GetProperty("evaluated_block_hash");
        var hash = hashElement.ValueKind == JsonValueKind.String ? hashElement.GetString() : null;
        if (hash is null || hash.Length != 64
            || hash.Any(static character => character is not (>= '0' and <= '9' or >= 'a' and <= 'f'))
            || (height == 0) != string.Equals(hash, new string('0', 64), StringComparison.Ordinal))
        {
            throw new JsonException("Election tally response has invalid evaluated block coordinates.");
        }
        var finalizedElement = root.GetProperty("finalized");
        if (finalizedElement.ValueKind is not (JsonValueKind.True or JsonValueKind.False))
        {
            throw new JsonException("Election tally.finalized must be a boolean.");
        }
        var tallyElement = root.GetProperty("tally");
        if (tallyElement.ValueKind != JsonValueKind.Array
            || tallyElement.GetArrayLength() is < 2 or > 64)
        {
            throw new JsonException("Election tally must contain 2–64 weights.");
        }
        var weights = new UInt128[tallyElement.GetArrayLength()];
        UInt128 aggregate = 0;
        for (var index = 0; index < weights.Length; index++)
        {
            var weight = ReadExactElectionUnsigned<UInt128>(
                tallyElement[index],
                $"tally[{index}]",
                UInt128.TryParse);
            try
            {
                aggregate = checked(aggregate + weight);
            }
            catch (OverflowException exception)
            {
                throw new JsonException("Election tally aggregate exceeds u128.", exception);
            }
            weights[index] = weight;
        }
        return new ToriiElectionTallyV1(
            height,
            hash,
            finalizedElement.GetBoolean(),
            Array.AsReadOnly(weights));
    }

    private delegate bool TryParseElectionUnsigned<T>(
        string value,
        NumberStyles styles,
        IFormatProvider? provider,
        out T result);

    private static T ReadExactElectionUnsigned<T>(
        JsonElement element,
        string context,
        TryParseElectionUnsigned<T> parse)
    {
        if (element.ValueKind != JsonValueKind.Number)
        {
            throw new JsonException($"Election tally.{context} must be an unquoted unsigned JSON integer.");
        }
        var token = element.GetRawText();
        if (token.Length == 0
            || token.Any(static character => character is < '0' or > '9')
            || !parse(token, NumberStyles.None, CultureInfo.InvariantCulture, out var result))
        {
            throw new JsonException($"Election tally.{context} must fit its unsigned integer bound.");
        }
        return result;
    }
}
