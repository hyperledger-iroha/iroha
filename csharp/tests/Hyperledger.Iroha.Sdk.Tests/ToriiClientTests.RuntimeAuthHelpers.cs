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
                LocalSigningContext = new ToriiLocalSigningContext(OnboardingFixtureNetworkId),
                CanonicalRequestCredentials = new CanonicalRequestCredentials(
                    CanonicalAccountId,
                    CanonicalPrivateKeySeed),
            },
            TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);
}
