using System.Diagnostics;
using System.Net;
using System.Security.Cryptography;
using System.Text.Json;
using Hyperledger.Iroha.Privacy;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Torii;

/// <summary>Closed submission and polling policy for one authenticated Exact12 action.</summary>
public sealed class PrivacyActionSubmitOptionsV1
{
    public required NetworkId NetworkId { get; init; }

    public bool Wait { get; init; } = true;

    public TimeSpan PollInterval { get; init; } = TimeSpan.FromSeconds(1);

    public TimeSpan Timeout { get; init; } = TimeSpan.FromSeconds(30);

    public int? MaxAttempts { get; init; }

    public string Scope { get; init; } = "global";
}

/// <summary>Closed authenticated status-query policy for one Exact12 action.</summary>
public sealed class PrivacyActionStatusOptionsV1
{
    public required NetworkId NetworkId { get; init; }

    public string Scope { get; init; } = "global";
}

public sealed partial class ToriiClient
{
    private const string PrivacyActionSubmissionPathV1 = "/v1/pipeline/transactions";
    private const string PrivacyActionStatusPathV1 =
        "/v1/pipeline/transactions/status";
    private const string PrivacyActionDetailsPathV1 =
        "/v1/pipeline/transactions/details";
    private const string PrivacyActionReceiptQueryPathV1 = "/v1/query";
    private const string PrivacyActionNoritoMediaTypeV1 = "application/x-norito";
    private readonly object privacyActionProvenanceOwnerV1 = new();

    /// <summary>
    /// Authenticates, fresh-gates, and submits one member of the closed thirteen-operation
    /// Exact12 action union.
    /// </summary>
    /// <remarks>
    /// Native inspection authenticates transaction structure, signature, exact NetworkId, exact
    /// canonical-request authority, and operation binding, but never accepts the proof. A terminal
    /// return requires a fresh
    /// authenticated manifest, canonical Torii submission, authenticated pipeline state, and a
    /// native-verified committed transaction-details result and, for Applied, a native-inspected
    /// finalized ID105 execution receipt. Pre-submit admission and execution capability evidence
    /// remain distinct because the finalized manifest may advance before execution.
    /// </remarks>
    public async Task<PrivacyActionOperationViewV1> SubmitSignedPrivacyActionV1Async(
        PrivacyExact12ActionRequestV1 request,
        PrivacyActionSubmitOptionsV1 options,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(options);
        var context = RequirePrivacyActionContextV1(
            options.NetworkId,
            options.Scope,
            nameof(options));
        ValidatePrivacyActionPollingV1(options);

        var operation = request.Operation;
        var protocol = operation.ProtocolId();
        var inspection = PrivacyNative.InspectSignedExact12ActionV1(
            request.SignedTransactionVersioned,
            context.NetworkId,
            context.Credentials.AccountId,
            operation);

        var manifest = await GetPrivacyExact12CapabilityManifestV1Async(cancellationToken);
        var manifestDigest = manifest.ManifestDigest;
        var expectedManifestDigest = request.ExpectedManifestDigest;
        if (expectedManifestDigest is not null
            && !CryptographicOperations.FixedTimeEquals(
                expectedManifestDigest,
                manifestDigest))
        {
            throw new InvalidOperationException(
                "Fresh Exact12 capability manifest does not match the requested digest.");
        }
        var admission = PrivacyExact12CapabilityAdmissionV1
            .RequireExact12CapabilityTupleV1(manifest, protocol, operation);
        PrivacyExact12CapabilityAdmissionV1.RequireForConstruction(
            admission,
            protocol,
            operation);
        var admittedDigest = admission.ManifestDigest;
        if (!CryptographicOperations.FixedTimeEquals(admittedDigest, manifestDigest)
            || admission.CommittedHeight != manifest.CommittedHeight)
        {
            throw new InvalidOperationException(
                "Exact12 capability admission is not bound to the fresh manifest.");
        }

        var submitted = new PrivacyActionOperationViewV1(
            protocol,
            operation,
            inspection.TransactionHash,
            inspection.TransactionIntentDigest,
            inspection.StatementDigest,
            inspection.ProofEnvelopeHash,
            PrivacyActionLocalStateV1.Submitted,
            terminalChainState: null,
            committedHeight: null,
            rejectionReason: null,
            operation.LedgerEffectKind(),
            admittedDigest,
            admission.CommittedHeight)
            .BindAuthenticatedSubmissionV1(
                privacyActionProvenanceOwnerV1,
                context.NetworkId);

        await SubmitPrivacyActionWireOnceV1Async(
            request.SignedTransactionVersioned,
            cancellationToken);
        if (!options.Wait)
        {
            return submitted;
        }
        return await WaitForPrivacyActionTerminalV1Async(
            submitted,
            context,
            options,
            cancellationToken);
    }

    /// <summary>
    /// Refreshes one typed Exact12 operation without accepting local proof checks, HTTP
    /// admission, or reasonless rejection as terminal ledger semantics.
    /// </summary>
    public async Task<PrivacyActionOperationViewV1> GetPrivacyActionStatusV1Async(
        PrivacyActionOperationViewV1 operation,
        PrivacyActionStatusOptionsV1 options,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(operation);
        ArgumentNullException.ThrowIfNull(options);
        var context = RequirePrivacyActionContextV1(
            options.NetworkId,
            options.Scope,
            nameof(options));
        return await RefreshPrivacyActionStatusV1Async(
            operation,
            context,
            cancellationToken);
    }

    private async Task SubmitPrivacyActionWireOnceV1Async(
        byte[] signedTransactionVersioned,
        CancellationToken cancellationToken)
    {
        await EnsureTransactionSubmissionCompatibilityAsync(cancellationToken);
        var expectedUri = BuildRequestUri(PrivacyActionSubmissionPathV1, query: null);
        using var content = CreateBinaryContent(
            signedTransactionVersioned,
            PrivacyActionNoritoMediaTypeV1);
        using var response = await SendAsync(
            HttpMethod.Post,
            PrivacyActionSubmissionPathV1,
            content: content,
            accept: $"{PrivacyActionNoritoMediaTypeV1}, application/json",
            cancellationToken: cancellationToken);
        RequireExactPrivacyActionResponseUriV1(
            response,
            expectedUri,
            "Exact12 transaction submission");
        if (response.StatusCode != HttpStatusCode.Accepted)
        {
            throw new HttpRequestException(
                "Exact12 transaction submission requires HTTP 202 Accepted.");
        }
    }

    private async Task<PrivacyActionOperationViewV1> WaitForPrivacyActionTerminalV1Async(
        PrivacyActionOperationViewV1 operation,
        PrivacyActionContextV1 context,
        PrivacyActionSubmitOptionsV1 options,
        CancellationToken cancellationToken)
    {
        var elapsed = Stopwatch.StartNew();
        var attempts = 0;
        while (true)
        {
            cancellationToken.ThrowIfCancellationRequested();
            attempts = checked(attempts + 1);
            var refreshed = await RefreshPrivacyActionStatusV1Async(
                operation,
                context,
                cancellationToken);
            if (refreshed.LocalState == PrivacyActionLocalStateV1.Terminal)
            {
                return refreshed;
            }
            if (options.MaxAttempts is { } maximumAttempts
                && attempts >= maximumAttempts)
            {
                throw new TimeoutException(
                    $"Exact12 action did not reach terminal state after {attempts} attempts.");
            }
            if (elapsed.Elapsed >= options.Timeout)
            {
                throw new TimeoutException(
                    $"Exact12 action did not reach terminal state within {options.Timeout}.");
            }
            await Task.Delay(options.PollInterval, cancellationToken);
        }
    }

    private async Task<PrivacyActionOperationViewV1> RefreshPrivacyActionStatusV1Async(
        PrivacyActionOperationViewV1 operation,
        PrivacyActionContextV1 context,
        CancellationToken cancellationToken)
    {
        operation.RequireAuthenticatedProvenanceV1(
            privacyActionProvenanceOwnerV1,
            context.NetworkId);
        var hashHex = Convert.ToHexString(operation.TransactionHash).ToLowerInvariant();
        var status = await GetAuthenticatedPrivacyActionPipelineStatusV1Async(
            hashHex,
            cancellationToken);
        if (status is null
            || status.State is PipelineTransactionState.Queued
                or PipelineTransactionState.Approved
                or PipelineTransactionState.Committed
                or PipelineTransactionState.Expired)
        {
            return ResolvePrivacyActionEvidenceV1(
                operation,
                status,
                details: null,
                receipt: null);
        }
        if (status.State is not (
            PipelineTransactionState.Applied
            or PipelineTransactionState.Rejected))
        {
            throw new InvalidOperationException(
                "Authenticated Exact12 status is outside the closed state union.");
        }

        var detailsTask = GetAuthenticatedPrivacyActionDetailsV1Async(
            hashHex,
            context,
            cancellationToken);
        var receiptTask = GetAuthenticatedPrivacyActionReceiptV1Async(
            operation,
            context,
            cancellationToken);
        await Task.WhenAll(detailsTask, receiptTask);
        return ResolvePrivacyActionEvidenceV1(
            operation,
            status,
            await detailsTask,
            await receiptTask);
    }

    internal static PrivacyActionOperationViewV1 ResolvePrivacyActionEvidenceV1(
        PrivacyActionOperationViewV1 operation,
        PipelineTransactionStatus? status,
        PrivacyAuthenticatedCommittedResultV1? details,
        PrivacyAuthenticatedActionExecutionReceiptV1? receipt)
    {
        ArgumentNullException.ThrowIfNull(operation);
        if (status is null)
        {
            if (operation.LocalState == PrivacyActionLocalStateV1.Terminal)
            {
                throw new InvalidOperationException(
                    "Terminal Exact12 action disappeared from authenticated status.");
            }
            return operation;
        }

        if (status.State is PipelineTransactionState.Queued
            or PipelineTransactionState.Approved
            or PipelineTransactionState.Committed)
        {
            if (operation.LocalState == PrivacyActionLocalStateV1.Terminal)
            {
                throw new InvalidOperationException(
                    "Terminal Exact12 action status regressed.");
            }
            return operation;
        }
        if (status.State == PipelineTransactionState.Expired)
        {
            if (string.Equals(status.ResolvedFrom, "cache", StringComparison.Ordinal))
            {
                return operation;
            }
            if (!string.Equals(status.ResolvedFrom, "state", StringComparison.Ordinal)
                || status.BlockHeight is not null)
            {
                throw new InvalidOperationException(
                    "Expired Exact12 action lacks durable authenticated state evidence.");
            }
            return RequireStablePrivacyActionTerminalV1(
                operation,
                CreatePrivacyActionTerminalViewV1(
                    operation,
                    PrivacyActionTerminalChainStateV1.Expired,
                    committedHeight: null,
                    rejectionReason: null));
        }
        if (status.State is not (
            PipelineTransactionState.Applied
            or PipelineTransactionState.Rejected))
        {
            throw new InvalidOperationException(
                "Authenticated Exact12 status is outside the closed state union.");
        }
        if (status.ResolvedFrom is not ("state" or "cache"))
        {
            throw new InvalidOperationException(
                "Terminal Exact12 action lacks committed-state status evidence.");
        }
        if (status.BlockHeight is { } publicHeight)
        {
            if (details is not null && publicHeight != details.CommittedBlockHeight)
            {
                throw new InvalidOperationException(
                    "Exact12 pipeline status height differs from authenticated committed details.");
            }
            if (receipt is not null && publicHeight != receipt.AdmittedAtHeight)
            {
                throw new InvalidOperationException(
                    "Exact12 pipeline status height differs from the authenticated execution receipt.");
            }
        }

        if (status.State == PipelineTransactionState.Rejected)
        {
            if (receipt is not null)
            {
                throw new InvalidOperationException(
                    "Rejected Exact12 status contradicts an authenticated execution receipt.");
            }
            if (details is null)
            {
                return operation;
            }
            if (details.ResultOk || string.IsNullOrEmpty(details.RejectionMessage))
            {
                throw new InvalidOperationException(
                    "Rejected Exact12 status omitted its authenticated committed reason.");
            }
            return RequireStablePrivacyActionTerminalV1(
                operation,
                CreatePrivacyActionTerminalViewV1(
                    operation,
                    PrivacyActionTerminalChainStateV1.Rejected,
                    details.CommittedBlockHeight,
                    details.RejectionMessage));
        }

        if (details is not null && !details.ResultOk)
        {
            throw new InvalidOperationException(
                "Applied Exact12 status resolved to a rejected ledger result.");
        }
        if (details is null || receipt is null)
        {
            return operation;
        }
        if (receipt.AdmittedAtHeight != details.CommittedBlockHeight)
        {
            throw new InvalidOperationException(
                "Authenticated Exact12 execution receipt differs from the committed transaction height.");
        }
        return RequireStablePrivacyActionTerminalV1(
            operation,
            CreatePrivacyActionTerminalViewV1(
                operation,
                PrivacyActionTerminalChainStateV1.Applied,
                details.CommittedBlockHeight,
                rejectionReason: null,
                executionCapabilityManifestDigest: receipt.CapabilityManifestDigest,
                executionCapabilityCommittedHeight: receipt.CapabilityCommittedHeight,
                executionReceiptFinalizedHeight: receipt.FinalizedHeight,
                executionReceiptFinalizedBlockHash: receipt.FinalizedBlockHash));
    }

    private async Task<PipelineTransactionStatus?>
        GetAuthenticatedPrivacyActionPipelineStatusV1Async(
            string transactionHashHex,
            CancellationToken cancellationToken)
    {
        var query = BuildQueryString(
        [
            new KeyValuePair<string, string?>("hash", transactionHashHex),
            new KeyValuePair<string, string?>("scope", "global"),
        ]);
        var expectedUri = BuildRequestUri(PrivacyActionStatusPathV1, query);
        using var response = await SendAllowingStatusAsync(
            HttpMethod.Get,
            PrivacyActionStatusPathV1,
            query,
            content: null,
            HttpStatusCode.NotFound,
            cancellationToken);
        RequireExactPrivacyActionResponseUriV1(
            response,
            expectedUri,
            "Exact12 pipeline status");
        if (response.StatusCode is HttpStatusCode.NotFound or HttpStatusCode.NoContent)
        {
            return null;
        }
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            "authenticated Exact12 pipeline status response",
            cancellationToken);
        var status = ParsePipelineTransactionStatus(
            document.RootElement,
            transactionHashHex);
        if (!string.Equals(status.Scope, "global", StringComparison.Ordinal))
        {
            throw new JsonException(
                "Authenticated Exact12 pipeline status did not resolve global scope.");
        }
        return status;
    }

    private async Task<PrivacyAuthenticatedCommittedResultV1?>
        GetAuthenticatedPrivacyActionDetailsV1Async(
            string transactionHashHex,
            PrivacyActionContextV1 context,
            CancellationToken cancellationToken)
    {
        var nativeQuery = PrivacyNative.BuildAuthenticatedTransactionDetailsQueryV1(
            context.NetworkId,
            context.Credentials,
            transactionHashHex);
        var expectedUri = BuildRequestUri(PrivacyActionDetailsPathV1, query: null);
        using var content = CreateBinaryContent(
            nativeQuery.SignedQuery,
            PrivacyActionNoritoMediaTypeV1);
        using var response = await SendAuthenticatedPrivacyActionNoritoAllowingNotFoundV1Async(
            HttpMethod.Post,
            PrivacyActionDetailsPathV1,
            content,
            cancellationToken);
        RequireExactPrivacyActionResponseUriV1(
            response,
            expectedUri,
            "Exact12 authenticated transaction details");
        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }
        var contentType = response.Content.Headers.ContentType;
        if (contentType is null
            || !string.Equals(
                contentType.MediaType,
                PrivacyActionNoritoMediaTypeV1,
                StringComparison.Ordinal)
            || contentType.Parameters.Count != 0)
        {
            throw new InvalidDataException(
                "Authenticated Exact12 transaction details require exact application/x-norito content.");
        }
        var responseBytes = await ReadBoundedPrivacyActionBodyV1Async(
            response.Content,
            PrivacyNative.PrivacyAuthenticatedTransactionDetailsResponseMaxBytes,
            cancellationToken);
        return PrivacyNative.ProjectAuthenticatedTransactionDetailsResultV1(
            nativeQuery,
            responseBytes);
    }

    private async Task<PrivacyAuthenticatedActionExecutionReceiptV1?>
        GetAuthenticatedPrivacyActionReceiptV1Async(
            PrivacyActionOperationViewV1 operation,
            PrivacyActionContextV1 context,
            CancellationToken cancellationToken)
    {
        var nativeQuery = PrivacyNative.BuildAuthenticatedPrivacyActionReceiptQueryV1(
            context.NetworkId,
            context.Credentials,
            operation);
        var expectedUri = BuildRequestUri(PrivacyActionReceiptQueryPathV1, query: null);
        using var content = CreateBinaryContent(
            nativeQuery.SignedQuery,
            PrivacyActionNoritoMediaTypeV1);
        using var response = await SendAuthenticatedPrivacyActionNoritoAllowingNotFoundV1Async(
            HttpMethod.Post,
            PrivacyActionReceiptQueryPathV1,
            content,
            cancellationToken);
        RequireExactPrivacyActionResponseUriV1(
            response,
            expectedUri,
            "Exact12 authenticated action receipt");
        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }
        var contentType = response.Content.Headers.ContentType;
        if (contentType is null
            || !string.Equals(
                contentType.MediaType,
                PrivacyActionNoritoMediaTypeV1,
                StringComparison.Ordinal)
            || contentType.Parameters.Count != 0)
        {
            throw new InvalidDataException(
                "Authenticated Exact12 action receipts require exact application/x-norito content.");
        }
        var responseBytes = await ReadBoundedPrivacyActionBodyV1Async(
            response.Content,
            PrivacyNative.PrivacyAuthenticatedActionReceiptResponseMaxBytes,
            cancellationToken);
        return PrivacyNative.ProjectAuthenticatedPrivacyActionReceiptResultV1(
            nativeQuery,
            responseBytes);
    }

    private async Task<HttpResponseMessage>
        SendAuthenticatedPrivacyActionNoritoAllowingNotFoundV1Async(
            HttpMethod method,
            string path,
            HttpContent content,
            CancellationToken cancellationToken)
    {
        var request = await CreateRequestAsync(
            method: method,
            path: path,
            query: null,
            content: content,
            accept: PrivacyActionNoritoMediaTypeV1,
            configureRequest: null,
            cancellationToken: cancellationToken);
        var response = await HttpClient.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        if (response.IsSuccessStatusCode
            || response.StatusCode == HttpStatusCode.NotFound)
        {
            return response;
        }
        var exception = await CreateApiExceptionAsync(response, cancellationToken);
        response.Dispose();
        throw exception;
    }

    private PrivacyActionContextV1 RequirePrivacyActionContextV1(
        NetworkId? networkId,
        string scope,
        string parameterName)
    {
        ArgumentNullException.ThrowIfNull(networkId, parameterName);
        if (!string.Equals(BaseUri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidOperationException(
                "Authenticated Exact12 actions require an HTTPS Torii endpoint.");
        }
        var credentials = Options.CanonicalRequestCredentials
            ?? throw new InvalidOperationException(
                "Authenticated Exact12 actions require ToriiClientOptions.CanonicalRequestCredentials.");
        var signingContext = Options.LocalSigningContext
            ?? throw new InvalidOperationException(
                "Authenticated Exact12 actions require ToriiClientOptions.LocalSigningContext.");
        if (signingContext.NetworkId != networkId)
        {
            throw new ArgumentException(
                "Exact12 NetworkId must equal ToriiClientOptions.LocalSigningContext.NetworkId.",
                parameterName);
        }
        if (!string.Equals(scope, "global", StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Authenticated Exact12 action scope must be exactly global.",
                parameterName);
        }
        return new PrivacyActionContextV1(networkId, credentials);
    }

    private static void ValidatePrivacyActionPollingV1(PrivacyActionSubmitOptionsV1 options)
    {
        if (options.PollInterval <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(
                nameof(options),
                "Exact12 poll interval must be positive.");
        }
        if (options.Timeout <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(
                nameof(options),
                "Exact12 wait timeout must be positive.");
        }
        if (options.MaxAttempts is <= 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(options),
                "Exact12 maximum attempts must be positive when provided.");
        }
    }

    private static PrivacyActionOperationViewV1 CreatePrivacyActionTerminalViewV1(
        PrivacyActionOperationViewV1 operation,
        PrivacyActionTerminalChainStateV1 terminalState,
        ulong? committedHeight,
        string? rejectionReason,
        byte[]? executionCapabilityManifestDigest = null,
        ulong? executionCapabilityCommittedHeight = null,
        ulong? executionReceiptFinalizedHeight = null,
        byte[]? executionReceiptFinalizedBlockHash = null) =>
        operation.WithAuthenticatedTerminalStateV1(
            terminalState,
            committedHeight,
            rejectionReason,
            executionCapabilityManifestDigest,
            executionCapabilityCommittedHeight,
            executionReceiptFinalizedHeight,
            executionReceiptFinalizedBlockHash);

    private static PrivacyActionOperationViewV1 RequireStablePrivacyActionTerminalV1(
        PrivacyActionOperationViewV1 previous,
        PrivacyActionOperationViewV1 refreshed)
    {
        if (previous.LocalState != PrivacyActionLocalStateV1.Terminal)
        {
            return refreshed;
        }
        if (!previous.Equals(refreshed))
        {
            throw new InvalidOperationException(
                "Terminal Exact12 action status changed.");
        }
        return previous;
    }

    private static async Task<byte[]> ReadBoundedPrivacyActionBodyV1Async(
        HttpContent content,
        int maximumBytes,
        CancellationToken cancellationToken)
    {
        var declaredLength = content.Headers.ContentLength;
        if (declaredLength is <= 0 || declaredLength > maximumBytes)
        {
            throw new InvalidDataException(
                "Authenticated Exact12 Norito response Content-Length is invalid.");
        }
        await using var stream = await content.ReadAsStreamAsync(cancellationToken);
        using var output = declaredLength is > 0
            ? new MemoryStream(checked((int)declaredLength.Value))
            : new MemoryStream();
        var buffer = new byte[8192];
        while (true)
        {
            var count = await stream.ReadAsync(buffer, cancellationToken);
            if (count == 0)
            {
                break;
            }
            if (output.Length > maximumBytes - count)
            {
                throw new InvalidDataException(
                    "Authenticated Exact12 Norito response exceeds its bound.");
            }
            output.Write(buffer, 0, count);
        }
        if (output.Length == 0
            || (declaredLength.HasValue && output.Length != declaredLength.Value))
        {
            throw new InvalidDataException(
                "Authenticated Exact12 Norito response length is invalid.");
        }
        return output.ToArray();
    }

    private static void RequireExactPrivacyActionResponseUriV1(
        HttpResponseMessage response,
        Uri expectedUri,
        string context)
    {
        if (response.RequestMessage?.RequestUri is not Uri responseUri
            || !string.Equals(
                responseUri.AbsoluteUri,
                expectedUri.AbsoluteUri,
                StringComparison.Ordinal))
        {
            throw new HttpRequestException($"{context} must not follow redirects.");
        }
    }

    private sealed record class PrivacyActionContextV1(
        NetworkId NetworkId,
        Hyperledger.Iroha.Http.CanonicalRequestCredentials Credentials);
}
