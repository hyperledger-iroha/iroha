using System.Text.Json;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    /// <summary>Resolves an identifier with exact-network canonical account authentication.</summary>
    public async Task<ToriiIdentifierResolveResponse> ResolveIdentifierAsync(
        ToriiIdentifierResolveRequest request,
        CancellationToken cancellationToken = default)
    {
        RequireCanonicalRequestCredentials("/v1/identifiers/resolve");
        ArgumentNullException.ThrowIfNull(request);
        ValidateIdentifierResolveRequest(request);
        var normalizedRequest = request with
        {
            PolicyId = NormalizeIdentifierPolicyId(request.PolicyId, nameof(request.PolicyId)),
            EncryptedInput = NormalizeOptionalIdentifierCiphertext(
                request.EncryptedInput,
                nameof(request.EncryptedInput)),
        };

        ToriiIdentifierResolveResponse response;
        try
        {
            response = await PostAsync<ToriiIdentifierResolveRequest, ToriiIdentifierResolveResponse>(
                "/v1/identifiers/resolve",
                normalizedRequest,
                cancellationToken: cancellationToken);
        }
        catch (JsonException exception)
        {
            throw RewriteIdentifierResolveJsonException(exception);
        }

        ValidateIdentifierResolveResponse(response);
        return response;
    }
}
