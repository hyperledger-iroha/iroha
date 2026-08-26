using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    private static ToriiClient CreateRuntimeAuthenticatedClient(HttpMessageHandler handler) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(OnboardingFixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            });
}
