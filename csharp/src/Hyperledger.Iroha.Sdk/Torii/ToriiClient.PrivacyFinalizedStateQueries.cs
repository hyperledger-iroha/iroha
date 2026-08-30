using System.Net;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    /// <summary>Return finalized ZK-ACE replay provenance, or null only on HTTP 404.</summary>
    public Task<PrivacyZkAceReplayNullifierProvenanceV1?>
        GetPrivacyZkAceReplayNullifierV1Async(
            PrivacyZkAceReplayNullifierRequestV1 request,
            PrivacyFinalizedStateQueryOptionsV1 options,
            CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyZkAceReplayNullifierProvenanceV1>(
            request,
            options,
            nameof(GetPrivacyZkAceReplayNullifierV1Async),
            cancellationToken);

    /// <summary>Return finalized FCMP++, private-IVM, or PQ-MASP pool state, or null on 404.</summary>
    public Task<PrivacyProofManagedPoolStateViewV1?>
        GetPrivacyProofManagedPoolStateV1Async(
            PrivacyProofManagedPoolStateRequestV1 request,
            PrivacyFinalizedStateQueryOptionsV1 options,
            CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyProofManagedPoolStateViewV1>(
            request,
            options,
            nameof(GetPrivacyProofManagedPoolStateV1Async),
            cancellationToken);

    /// <summary>Return finalized public state of one governed Orchard pool, or null on 404.</summary>
    public Task<PrivacyOrchardPoolStateViewV1?> GetPrivacyOrchardPoolStateV1Async(
        PrivacyOrchardPoolStateRequestV1 request,
        PrivacyFinalizedStateQueryOptionsV1 options,
        CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyOrchardPoolStateViewV1>(
            request,
            options,
            nameof(GetPrivacyOrchardPoolStateV1Async),
            cancellationToken);

    /// <summary>Return finalized provenance of one consumed Orchard nullifier, or null on 404.</summary>
    public Task<PrivacyOrchardNullifierProvenanceV1?> GetPrivacyOrchardNullifierV1Async(
        PrivacyOrchardNullifierRequestV1 request,
        PrivacyFinalizedStateQueryOptionsV1 options,
        CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyOrchardNullifierProvenanceV1>(
            request,
            options,
            nameof(GetPrivacyOrchardNullifierV1Async),
            cancellationToken);

    /// <summary>Return finalized public state of one Anonymous PGC pool, or null on 404.</summary>
    public Task<PrivacyAnonymousPgcPoolStateViewV1?>
        GetPrivacyAnonymousPgcPoolStateV1Async(
            PrivacyAnonymousPgcPoolStateRequestV1 request,
            PrivacyFinalizedStateQueryOptionsV1 options,
            CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyAnonymousPgcPoolStateViewV1>(
            request,
            options,
            nameof(GetPrivacyAnonymousPgcPoolStateV1Async),
            cancellationToken);

    /// <summary>Return finalized provenance of one admitted ZK-AMS PHC anchor, or null on 404.</summary>
    public Task<PrivacyZkAmsAdmissionViewV1?> GetPrivacyZkAmsAdmissionV1Async(
        PrivacyZkAmsAdmissionRequestV1 request,
        PrivacyFinalizedStateQueryOptionsV1 options,
        CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyZkAmsAdmissionViewV1>(
            request,
            options,
            nameof(GetPrivacyZkAmsAdmissionV1Async),
            cancellationToken);

    /// <summary>Return finalized provenance of one anonymous ZK-AMS provision, or null on 404.</summary>
    public Task<PrivacyZkAmsProvisionViewV1?> GetPrivacyZkAmsProvisionV1Async(
        PrivacyZkAmsProvisionRequestV1 request,
        PrivacyFinalizedStateQueryOptionsV1 options,
        CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyZkAmsProvisionViewV1>(
            request,
            options,
            nameof(GetPrivacyZkAmsProvisionV1Async),
            cancellationToken);

    /// <summary>Return finalized provenance of one consumed ZK-X509 nullifier, or null on 404.</summary>
    public Task<PrivacyZkX509CertificateNullifierProvenanceV1?>
        GetPrivacyZkX509CertificateNullifierV1Async(
            PrivacyZkX509CertificateNullifierRequestV1 request,
            PrivacyFinalizedStateQueryOptionsV1 options,
            CancellationToken cancellationToken = default) =>
        GetPrivacyFinalizedStateV1Async<PrivacyZkX509CertificateNullifierProvenanceV1>(
            request,
            options,
            nameof(GetPrivacyZkX509CertificateNullifierV1Async),
            cancellationToken);

    private async Task<T?> GetPrivacyFinalizedStateV1Async<T>(
        IPrivacyFinalizedStateRequestV1 request,
        PrivacyFinalizedStateQueryOptionsV1 options,
        string operation,
        CancellationToken cancellationToken)
        where T : PrivacyFinalizedStateViewV1
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(options);
        var context = RequirePrivacyActionContextV1(
            options.NetworkId,
            options.Scope,
            nameof(options));
        var requestBinding = request.RequestBinding;
        var nativeQuery = PrivacyNative.BuildAuthenticatedPrivacyStateQueryV1(
            context.NetworkId,
            context.Credentials,
            request.QueryId,
            request.ProtocolIndex,
            requestBinding);
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
            $"{operation} authenticated finalized state query");
        if (response.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }
        if (response.StatusCode != HttpStatusCode.OK)
        {
            throw new HttpRequestException(
                $"{operation} authenticated finalized state query requires HTTP 200 OK.");
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
                $"{operation} response requires exact application/x-norito content.");
        }
        var responseBytes = await ReadBoundedPrivacyActionBodyV1Async(
            response.Content,
            PrivacyNative.PrivacyAuthenticatedStateQueryResponseMaxBytes,
            cancellationToken);
        var projection = PrivacyNative.ProjectAuthenticatedPrivacyStateQueryResultV1(
            nativeQuery,
            responseBytes);
        var view = PrivacyFinalizedStateContractV1.ParseProjectionV1(projection, nativeQuery);
        return view as T
            ?? throw new InvalidDataException(
                $"{operation} returned a different authenticated typed state variant.");
    }
}
