using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    /// <summary>
    /// Fetch the canonical Exact12 manifest through HTTPS and canonical request authentication,
    /// then require byte-exact agreement with this binary's native compiled-profile catalog.
    /// </summary>
    /// <remarks>
    /// Redirected responses, JSON/browser fallbacks, bearer-only requests, oversized bodies,
    /// stale local catalogs, and legacy capability snapshots fail closed. The manifest digest is
    /// checked as content identity but is never treated as transport authentication.
    /// </remarks>
    public Task<PrivacyExact12CapabilityManifestV1>
        GetPrivacyExact12CapabilityManifestV1Async(
            CancellationToken cancellationToken = default) =>
        PrivacyExact12CapabilityManifestV1.FetchAuthenticatedToriiAsync(
            this,
            cancellationToken);
}
