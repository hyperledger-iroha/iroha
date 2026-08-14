using Hyperledger.Iroha;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

var baseUrl = Environment.GetEnvironmentVariable("IROHA_CSHARP_TORII_BASE_URL")
    ?? "https://taira.sora.org";
var seedHex = Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX");
var canonicalAccountId = Environment.GetEnvironmentVariable("IROHA_CSHARP_CANONICAL_ACCOUNT_ID");
var networkId = Environment.GetEnvironmentVariable("IROHA_CSHARP_NETWORK_ID");
if (string.IsNullOrWhiteSpace(seedHex)
    || string.IsNullOrWhiteSpace(canonicalAccountId)
    || string.IsNullOrWhiteSpace(networkId))
{
    throw new InvalidOperationException(
        "The C# sample requires IROHA_CSHARP_PRIVATE_KEY_SEED_HEX, "
        + "IROHA_CSHARP_CANONICAL_ACCOUNT_ID, and the exact IROHA_CSHARP_NETWORK_ID.");
}

var privateKeySeed = Convert.FromHexString(seedHex);
var exactNetworkId = NetworkId.Parse(networkId);
var toriiOptions = new ToriiClientOptions
{
    LocalSigningContext = new ToriiLocalSigningContext(exactNetworkId),
    CanonicalRequestCredentials = new CanonicalRequestCredentials(
        canonicalAccountId,
        privateKeySeed),
};

using var client = new IrohaClient(
    new Uri(baseUrl, UriKind.Absolute),
    toriiOptions: toriiOptions);
try
{
    var capabilities = await client.Torii.GetNodeCapabilitiesAsync();
    var accounts = await client.Torii.GetAccountsAsync(limit: 5);
    var aliases = accounts.Items.Count == 0
        ? null
        : await client.Torii.LookupAliasesByAccountAsync(accounts.Items[0].Id);
    var faucetPuzzle = await client.Torii.GetAccountFaucetPuzzleAsync();
    if (faucetPuzzle.NetworkId != exactNetworkId)
    {
        throw new InvalidOperationException(
            $"Faucet puzzle targets {faucetPuzzle.NetworkId}, not configured network {exactNetworkId}.");
    }

    Console.WriteLine($"Torii ABI version: {capabilities.AbiVersion}");
    Console.WriteLine($"Torii data model version: {capabilities.DataModelVersion}");
    Console.WriteLine($"Visible accounts in first page: {accounts.Items.Count}");
    Console.WriteLine($"Aliases on first account: {aliases?.Total ?? 0}");
    Console.WriteLine($"Faucet puzzle difficulty bits: {faucetPuzzle.DifficultyBits}");
    Console.WriteLine($"Faucet puzzle exact network: {faucetPuzzle.NetworkId}");

    var transaction = client.Ledger
        .BuildTransaction(
            exactNetworkId,
            canonicalAccountId,
            FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
        .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "1", canonicalAccountId)
        .SetCreationTime(DateTimeOffset.UtcNow)
        .SetTimeToLiveMilliseconds(5_000)
        .SetNonce(1);
    var signed = await client.Ledger.QuoteAndSignAsync(transaction, privateKeySeed);

    Console.WriteLine($"Prepared quoted transaction hash: {signed.Transaction.TransactionHashHex}");
}
catch (ToriiApiException exception)
{
    Console.WriteLine($"Torii call failed with status {(int?)exception.StatusCode}: {exception.ResponseBody ?? exception.Message}");
}
catch (Exception exception)
{
    Console.WriteLine($"Torii call failed: {exception.Message}");
}
