using System.Net;
using System.Net.Http.Headers;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

/// <summary>Typed input for one aggregate V1 Hijiri validation-fee quote.</summary>
public sealed class ValidationFeeHijiriQuoteRequestV1
{
    /// <summary>Current quote request layout.</summary>
    public const ushort CurrentVersion = 1;

    /// <summary>Largest aggregate transfer count accepted by the V1 route.</summary>
    public const uint MaximumQualifyingTransferCount = 100_000;

    /// <summary>Largest native-Norito request accepted by the V1 route.</summary>
    public const int MaximumRequestBytes = 4 * 1024;

    /// <summary>Creates one request bound to an exact canonical I105 account.</summary>
    public ValidationFeeHijiriQuoteRequestV1(
        string accountId,
        uint qualifyingTransferCount)
    {
        ArgumentNullException.ThrowIfNull(accountId);
        _ = AccountAddress.Parse(accountId);
        if (qualifyingTransferCount is 0 or > MaximumQualifyingTransferCount)
        {
            throw new ArgumentOutOfRangeException(
                nameof(qualifyingTransferCount),
                $"qualifyingTransferCount must be within 1..{MaximumQualifyingTransferCount}.");
        }

        AccountId = accountId;
        QualifyingTransferCount = qualifyingTransferCount;
    }

    /// <summary>Frozen request layout version.</summary>
    public ushort Version => CurrentVersion;

    /// <summary>Canonical universal account whose effective Hijiri risk is priced.</summary>
    public string AccountId { get; }

    /// <summary>Transfers aggregated before the one required Q16 ceiling operation.</summary>
    public uint QualifyingTransferCount { get; }

    /// <summary>Encodes this request with the authoritative native Norito implementation.</summary>
    public byte[] ToNoritoBytes() => ValidationFeeHijiriQuoteNative.EncodeRequestV1(this);
}

/// <summary>
/// Native-verified V1 Hijiri validation-fee quote evaluated from one committed state snapshot.
/// </summary>
/// <remarks>
/// The assurance marker explicitly states that the live projection is authenticated but is not
/// an independent state witness. Transaction admission later binds the policy and Hijiri hashes
/// and rejects a stale quote.
/// </remarks>
[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class ValidationFeeHijiriQuoteV1
{
    /// <summary>Largest native-Norito response accepted by V1 clients.</summary>
    public const int MaximumResponseBytes = 64 * 1024;

    /// <summary>Stable verified projection schema.</summary>
    public const string SchemaV1 = "iroha.torii.v1.validation_fee.hijiri_quote.response";

    /// <summary>Honest assurance marker for the evaluated live projection.</summary>
    public const string EvaluatedAssuranceV1 =
        "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED";

    [JsonPropertyName("schema")]
    public required string Schema { get; init; }

    [JsonPropertyName("version")]
    public required ushort Version { get; init; }

    [JsonPropertyName("assurance")]
    public required string Assurance { get; init; }

    [JsonPropertyName("evaluatedStateHeight")]
    public required string EvaluatedStateHeight { get; init; }

    [JsonPropertyName("quotedExecutionHeight")]
    public required string QuotedExecutionHeight { get; init; }

    [JsonPropertyName("accountId")]
    public required string AccountId { get; init; }

    [JsonPropertyName("activePolicyVersion")]
    public required string ActivePolicyVersion { get; init; }

    [JsonPropertyName("activePolicyHash")]
    public required string ActivePolicyHash { get; init; }

    [JsonPropertyName("feeAssetDefinitionId")]
    public required string FeeAssetDefinitionId { get; init; }

    [JsonPropertyName("treasuryAccountId")]
    public required string TreasuryAccountId { get; init; }

    [JsonPropertyName("feeScale")]
    public required byte FeeScale { get; init; }

    [JsonPropertyName("hijiriParametersVersion")]
    public required ushort HijiriParametersVersion { get; init; }

    [JsonPropertyName("hijiriParametersRevision")]
    public required string HijiriParametersRevision { get; init; }

    [JsonPropertyName("hijiriParametersDigest")]
    public required string HijiriParametersDigest { get; init; }

    [JsonPropertyName("defaultAccountRiskQ16")]
    public required uint DefaultAccountRiskQ16 { get; init; }

    [JsonPropertyName("effectiveAccountRiskQ16")]
    public required uint EffectiveAccountRiskQ16 { get; init; }

    [JsonPropertyName("accountRiskRevision")]
    public required string? AccountRiskRevision { get; init; }

    [JsonPropertyName("accountRiskDigest")]
    public required string? AccountRiskDigest { get; init; }

    [JsonPropertyName("feeMultiplierQ16")]
    public required uint FeeMultiplierQ16 { get; init; }

    [JsonPropertyName("hijiriFeeQuoteHash")]
    public required string HijiriFeeQuoteHash { get; init; }

    [JsonPropertyName("basePerTransferFeeMinorUnits")]
    public required string BasePerTransferFeeMinorUnits { get; init; }

    [JsonPropertyName("adjustedPerTransferFeeMinorUnits")]
    public required string AdjustedPerTransferFeeMinorUnits { get; init; }

    [JsonPropertyName("qualifyingTransferCount")]
    public required uint QualifyingTransferCount { get; init; }

    [JsonPropertyName("aggregateBaseFeeMinorUnits")]
    public required string AggregateBaseFeeMinorUnits { get; init; }

    [JsonPropertyName("aggregateAdjustedFeeMinorUnits")]
    public required string AggregateAdjustedFeeMinorUnits { get; init; }
}

/// <summary>Authoritative native-Norito codec and verifier for V1 Hijiri quotes.</summary>
public static class ValidationFeeHijiriQuoteNative
{
    /// <summary>Native bridge ABI carrying the additive quote symbols.</summary>
    public const uint RequiredBridgeAbiVersion = 23;

    private const string LibraryName = "connect_norito_bridge";
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private static readonly JsonSerializerOptions ProjectionJson = new()
    {
        PropertyNameCaseInsensitive = false,
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
    };

    /// <summary>Encodes one exact canonical bare-Norito request.</summary>
    public static byte[] EncodeRequestV1(ValidationFeeHijiriQuoteRequestV1 request)
    {
        ArgumentNullException.ThrowIfNull(request);
        EnsureAvailable();
        var account = StrictUtf8.GetBytes(request.AccountId);
        IntPtr output = IntPtr.Zero;
        UIntPtr outputLength = UIntPtr.Zero;
        var status = NativeEncodeRequestV1(
            account,
            new UIntPtr((uint)account.Length),
            request.QualifyingTransferCount,
            out output,
            out outputLength);
        try
        {
            return CopyOutput(
                status,
                output,
                outputLength,
                ValidationFeeHijiriQuoteRequestV1.MaximumRequestBytes,
                "native Hijiri quote request encoder");
        }
        finally
        {
            if (output != IntPtr.Zero)
            {
                NativeFree(output);
            }
        }
    }

    /// <summary>
    /// Verifies a canonical response against the exact request bytes sent to Torii.
    /// </summary>
    public static ValidationFeeHijiriQuoteV1 VerifyResponseV1(
        ReadOnlySpan<byte> responseNorito,
        ReadOnlySpan<byte> requestNorito)
    {
        if (responseNorito.IsEmpty
            || responseNorito.Length > ValidationFeeHijiriQuoteV1.MaximumResponseBytes)
        {
            throw new ArgumentOutOfRangeException(
                nameof(responseNorito),
                $"responseNorito must contain 1..{ValidationFeeHijiriQuoteV1.MaximumResponseBytes} bytes.");
        }
        if (requestNorito.IsEmpty
            || requestNorito.Length > ValidationFeeHijiriQuoteRequestV1.MaximumRequestBytes)
        {
            throw new ArgumentOutOfRangeException(
                nameof(requestNorito),
                $"requestNorito must contain 1..{ValidationFeeHijiriQuoteRequestV1.MaximumRequestBytes} bytes.");
        }

        EnsureAvailable();
        var response = responseNorito.ToArray();
        var request = requestNorito.ToArray();
        IntPtr output = IntPtr.Zero;
        UIntPtr outputLength = UIntPtr.Zero;
        var status = NativeVerifyResponseV1(
            response,
            new UIntPtr((uint)response.Length),
            request,
            new UIntPtr((uint)request.Length),
            out output,
            out outputLength);
        try
        {
            var projection = CopyOutput(
                status,
                output,
                outputLength,
                ValidationFeeHijiriQuoteV1.MaximumResponseBytes,
                "native Hijiri quote response verifier");
            var quote = JsonSerializer.Deserialize<ValidationFeeHijiriQuoteV1>(
                projection,
                ProjectionJson)
                ?? throw new JsonException("Native Hijiri quote projection deserialized to null.");
            if (!string.Equals(quote.Schema, ValidationFeeHijiriQuoteV1.SchemaV1, StringComparison.Ordinal)
                || quote.Version != ValidationFeeHijiriQuoteRequestV1.CurrentVersion
                || !string.Equals(
                    quote.Assurance,
                    ValidationFeeHijiriQuoteV1.EvaluatedAssuranceV1,
                    StringComparison.Ordinal)
                || (quote.AccountRiskRevision is null) != (quote.AccountRiskDigest is null))
            {
                throw new JsonException("Native Hijiri quote projection violates the frozen V1 shape.");
            }
            return quote;
        }
        finally
        {
            if (output != IntPtr.Zero)
            {
                NativeFree(output);
            }
        }
    }

    private static byte[] CopyOutput(
        int status,
        IntPtr output,
        UIntPtr outputLength,
        int maximumBytes,
        string context)
    {
        var length = outputLength.ToUInt64();
        if (status != 0
            || output == IntPtr.Zero
            || length == 0
            || length > (ulong)maximumBytes
            || length > int.MaxValue)
        {
            throw new InvalidDataException($"{context} failed closed (status {status}).");
        }
        var bytes = new byte[(int)length];
        Marshal.Copy(output, bytes, 0, bytes.Length);
        return bytes;
    }

    private static void EnsureAvailable()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            if (!NativeLibrary.TryLoad(
                    LibraryName,
                    typeof(ValidationFeeHijiriQuoteNative).Assembly,
                    null,
                    out handle)
                || !NativeLibrary.TryGetExport(
                    handle,
                    "connect_norito_validation_fee_hijiri_quote_request_v1",
                    out _)
                || !NativeLibrary.TryGetExport(
                    handle,
                    "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
                    out _)
                || !NativeLibrary.TryGetExport(handle, "connect_norito_free", out _)
                || NativeBridgeAbiVersion() != RequiredBridgeAbiVersion)
            {
                throw new InvalidOperationException(
                    "ABI-23 connect_norito_bridge with Hijiri quote symbols is required.");
            }
        }
        catch (Exception error) when (
            error is DllNotFoundException
            or EntryPointNotFoundException
            or BadImageFormatException)
        {
            throw new InvalidOperationException(
                "ABI-23 connect_norito_bridge with Hijiri quote symbols is required.",
                error);
        }
        finally
        {
            if (handle != IntPtr.Zero)
            {
                NativeLibrary.Free(handle);
            }
        }
    }

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_bridge_abi_version",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeBridgeAbiVersion();

    private static int NativeEncodeRequestV1(
        byte[] accountIdUtf8,
        UIntPtr accountIdLength,
        uint qualifyingTransferCount,
        out IntPtr output,
        out UIntPtr outputLength)
    {
        if (!OperatingSystem.IsWindows())
        {
            return NativeEncodeRequestV1Unix(
                accountIdUtf8,
                accountIdLength,
                qualifyingTransferCount,
                out output,
                out outputLength);
        }

        var status = NativeEncodeRequestV1Windows(
            accountIdUtf8,
            checked((uint)accountIdLength.ToUInt64()),
            qualifyingTransferCount,
            out output,
            out var windowsOutputLength);
        outputLength = new UIntPtr(windowsOutputLength);
        return status;
    }

    private static int NativeVerifyResponseV1(
        byte[] responseNorito,
        UIntPtr responseNoritoLength,
        byte[] requestNorito,
        UIntPtr requestNoritoLength,
        out IntPtr output,
        out UIntPtr outputLength)
    {
        if (!OperatingSystem.IsWindows())
        {
            return NativeVerifyResponseV1Unix(
                responseNorito,
                responseNoritoLength,
                requestNorito,
                requestNoritoLength,
                out output,
                out outputLength);
        }

        var status = NativeVerifyResponseV1Windows(
            responseNorito,
            checked((uint)responseNoritoLength.ToUInt64()),
            requestNorito,
            checked((uint)requestNoritoLength.ToUInt64()),
            out output,
            out var windowsOutputLength);
        outputLength = new UIntPtr(windowsOutputLength);
        return status;
    }

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_validation_fee_hijiri_quote_request_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeEncodeRequestV1Unix(
        [In] byte[] accountIdUtf8,
        UIntPtr accountIdLength,
        uint qualifyingTransferCount,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerifyResponseV1Unix(
        [In] byte[] responseNorito,
        UIntPtr responseNoritoLength,
        [In] byte[] requestNorito,
        UIntPtr requestNoritoLength,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_validation_fee_hijiri_quote_request_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeEncodeRequestV1Windows(
        [In] byte[] accountIdUtf8,
        uint accountIdLength,
        uint qualifyingTransferCount,
        out IntPtr output,
        out uint outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerifyResponseV1Windows(
        [In] byte[] responseNorito,
        uint responseNoritoLength,
        [In] byte[] requestNorito,
        uint requestNoritoLength,
        out IntPtr output,
        out uint outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_free",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer);
}

internal interface IValidationFeeHijiriQuoteCodec
{
    byte[] Encode(ValidationFeeHijiriQuoteRequestV1 request);

    ValidationFeeHijiriQuoteV1 Verify(byte[] responseNorito, byte[] requestNorito);
}

internal sealed class NativeValidationFeeHijiriQuoteCodec : IValidationFeeHijiriQuoteCodec
{
    internal static readonly NativeValidationFeeHijiriQuoteCodec Instance = new();

    private NativeValidationFeeHijiriQuoteCodec()
    {
    }

    public byte[] Encode(ValidationFeeHijiriQuoteRequestV1 request) => request.ToNoritoBytes();

    public ValidationFeeHijiriQuoteV1 Verify(byte[] responseNorito, byte[] requestNorito) =>
        ValidationFeeHijiriQuoteNative.VerifyResponseV1(responseNorito, requestNorito);
}

/// <summary>Assurance supplied only by a caller-controlled test transport.</summary>
internal enum ValidationFeeHijiriQuoteTransportAssurance
{
    OneShotWithoutRedirectsRetriesOrDecompression,
}

public sealed partial class ToriiClient
{
    private const string ValidationFeeHijiriQuotePath = "/v1/validation-fee/hijiri/quote";
    private const string NoritoMediaType = "application/x-norito";
    private readonly bool injectedValidationFeeHijiriQuoteTransportIsOneShot;

    internal ToriiClient(
        Uri baseUri,
        HttpClient httpClient,
        ToriiClientOptions? options,
        ValidationFeeHijiriQuoteTransportAssurance transportAssurance)
        : this(baseUri, httpClient, options)
    {
        if (transportAssurance
            != ValidationFeeHijiriQuoteTransportAssurance.OneShotWithoutRedirectsRetriesOrDecompression)
        {
            throw new ArgumentOutOfRangeException(nameof(transportAssurance));
        }
        injectedTransactionSubmissionTransportIsOneShot = true;
        injectedValidationFeeHijiriQuoteTransportIsOneShot = true;
    }

    /// <summary>
    /// Requests one bounded, account-authenticated, native-Norito Hijiri validation-fee quote.
    /// </summary>
    public Task<ValidationFeeHijiriQuoteV1> PostValidationFeeHijiriQuoteAsync(
        ValidationFeeHijiriQuoteRequestV1 request,
        CancellationToken cancellationToken = default) =>
        PostValidationFeeHijiriQuoteAsync(
            request,
            NativeValidationFeeHijiriQuoteCodec.Instance,
            cancellationToken);

    /// <summary>Convenience overload for one canonical account and aggregate transfer count.</summary>
    public Task<ValidationFeeHijiriQuoteV1> PostValidationFeeHijiriQuoteAsync(
        string accountId,
        uint qualifyingTransferCount,
        CancellationToken cancellationToken = default) =>
        PostValidationFeeHijiriQuoteAsync(
            new ValidationFeeHijiriQuoteRequestV1(accountId, qualifyingTransferCount),
            cancellationToken);

    internal async Task<ValidationFeeHijiriQuoteV1> PostValidationFeeHijiriQuoteAsync(
        ValidationFeeHijiriQuoteRequestV1 request,
        IValidationFeeHijiriQuoteCodec codec,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(codec);
        RequireHttpsHijiriQuoteTransport();
        RequireCanonicalRequestCredentials(ValidationFeeHijiriQuotePath);
        if (!ownsHttpClient && !injectedValidationFeeHijiriQuoteTransportIsOneShot)
        {
            throw new InvalidOperationException(
                "Hijiri quote requests require ToriiClient's internally managed one-shot, no-redirect, identity transport.");
        }

        var body = codec.Encode(request);
        if (body.Length is 0 or > ValidationFeeHijiriQuoteRequestV1.MaximumRequestBytes)
        {
            throw new InvalidDataException(
                "Native Hijiri quote request encoder returned an invalid request size.");
        }

        using var content = new ByteArrayContent(body);
        content.Headers.ContentType = new MediaTypeHeaderValue(NoritoMediaType);
        using var httpRequest = await CreateRequestAsync(
            HttpMethod.Post,
            ValidationFeeHijiriQuotePath,
            query: null,
            content,
            accept: NoritoMediaType,
            configureRequest: message =>
            {
                message.Headers.Remove("Accept-Encoding");
                message.Headers.TryAddWithoutValidation("Accept-Encoding", "identity");
                message.Headers.CacheControl = new CacheControlHeaderValue { NoStore = true };
            },
            cancellationToken);
        var expectedUri = httpRequest.RequestUri;
        using var response = await HttpClient.SendAsync(
            httpRequest,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        if (response.RequestMessage?.RequestUri != expectedUri)
        {
            throw new HttpRequestException("Hijiri quote requests must not follow redirects.");
        }
        RequirePrivateNoStoreHijiriQuoteResponse(response);
        RequireExactNoritoResponse(response);
        if (response.StatusCode == HttpStatusCode.OK)
        {
            if (response.Headers.Contains("X-Iroha-Reject-Code"))
            {
                throw new InvalidDataException(
                    "A successful Hijiri quote response must not carry X-Iroha-Reject-Code.");
            }
        }
        byte[] responseBody;
        try
        {
            responseBody = await ReadBoundedExactHijiriQuoteResponseBodyAsync(
                response.Content,
                ValidationFeeHijiriQuoteV1.MaximumResponseBytes,
                "Hijiri quote response",
                cancellationToken);
        }
        catch (InvalidDataException) when (response.StatusCode != HttpStatusCode.OK)
        {
            throw CreateApiExceptionFromBody(
                response,
                "<response body exceeds or violates the Hijiri quote response limit>"u8);
        }
        if (response.StatusCode != HttpStatusCode.OK)
        {
            throw CreateApiExceptionFromBody(response, responseBody);
        }
        var quote = codec.Verify(responseBody, body);
        if (quote.QualifyingTransferCount != request.QualifyingTransferCount
            || !SameAccountIdentity(quote.AccountId, request.AccountId))
        {
            throw new InvalidDataException(
                "Native Hijiri quote projection does not echo the requested account and count.");
        }
        return quote;
    }

    private void RequireHttpsHijiriQuoteTransport()
    {
        if (!string.Equals(BaseUri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidOperationException(
                "Hijiri quote requests require an HTTPS Torii base URI.");
        }
    }

    private static async Task<byte[]> ReadBoundedExactHijiriQuoteResponseBodyAsync(
        HttpContent content,
        int maximumBytes,
        string context,
        CancellationToken cancellationToken)
    {
        var declaredLength = CanonicalContentLength(content, context);
        if (declaredLength is { } declaredOverLimit && declaredOverLimit > maximumBytes)
        {
            throw new InvalidDataException(
                $"{context} declares more than the {maximumBytes}-byte limit.");
        }
        var body = await ReadBoundedExactJsonResponseBodyAsync(
            content,
            maximumBytes,
            context,
            cancellationToken);
        if (declaredLength is { } declared && body.LongLength != declared)
        {
            throw new InvalidDataException(
                $"{context} length does not match its Content-Length header.");
        }
        return body;
    }

    private static void RequirePrivateNoStoreHijiriQuoteResponse(
        HttpResponseMessage response)
    {
        var cache = response.Headers.CacheControl;
        var hasPublicExtension = cache?.Extensions.Any(extension =>
            string.Equals(extension.Name, "public", StringComparison.OrdinalIgnoreCase)) == true;
        // `Private` is also true for field-qualified `private="field"`, which does not
        // protect the whole response. A public extension is contradictory just like `public`.
        if (cache?.NoStore != true
            || !cache.Private
            || cache.PrivateHeaders.Count != 0
            || cache.Public
            || hasPublicExtension)
        {
            throw new InvalidDataException(
                "Hijiri quote response must be private and no-store.");
        }
    }

    private static void RequireExactNoritoResponse(HttpResponseMessage response)
    {
        if (!response.Content.Headers.NonValidated.TryGetValues("Content-Type", out var values))
        {
            throw new InvalidDataException(
                "Hijiri quote response Content-Type must be application/x-norito.");
        }
        var raw = values.ToArray();
        if (raw.Length != 1
            || raw[0].Contains(',', StringComparison.Ordinal)
            || !MediaTypeHeaderValue.TryParse(raw[0], out var contentType)
            || !string.Equals(contentType.MediaType, NoritoMediaType, StringComparison.OrdinalIgnoreCase)
            || contentType.Parameters.Count > 0)
        {
            throw new InvalidDataException(
                "Hijiri quote response Content-Type must be exactly application/x-norito.");
        }
        var encodings = response.Content.Headers.ContentEncoding.ToArray();
        if (encodings.Length > 1
            || (encodings.Length == 1
                && !string.Equals(encodings[0], "identity", StringComparison.OrdinalIgnoreCase)))
        {
            throw new InvalidDataException("Hijiri quote response must use identity encoding.");
        }
    }

    private static bool SameAccountIdentity(string left, string right) =>
        AccountAddress.Parse(left).CanonicalBytes()
            .SequenceEqual(AccountAddress.Parse(right).CanonicalBytes());
}
