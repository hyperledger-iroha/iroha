using Hyperledger.Iroha;
using Hyperledger.Iroha.Torii;

var baseUrl = Environment.GetEnvironmentVariable("IROHA_CSHARP_TORII_BASE_URL")
    ?? "https://taira.sora.org";

using var client = new IrohaClient(new Uri(baseUrl, UriKind.Absolute));
try
{
    var capabilities = await client.Torii.GetNodeCapabilitiesAsync();
    var accounts = await client.Torii.GetAccountsAsync(limit: 5);
    var aliases = accounts.Items.Count == 0
        ? null
        : await client.Torii.LookupAliasesByAccountAsync(accounts.Items[0].Id);
    var faucetPuzzle = await client.Torii.GetAccountFaucetPuzzleAsync();

    Console.WriteLine($"Torii ABI version: {capabilities.AbiVersion}");
    Console.WriteLine($"Torii data model version: {capabilities.DataModelVersion}");
    Console.WriteLine($"Visible accounts in first page: {accounts.Items.Count}");
    Console.WriteLine($"Aliases on first account: {aliases?.Total ?? 0}");
    Console.WriteLine($"Faucet puzzle difficulty bits: {faucetPuzzle.DifficultyBits}");

    var seedHex = Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX");
    if (!string.IsNullOrWhiteSpace(seedHex) && accounts.Items.Count > 0)
    {
        var signed = client.Ledger
            .BuildTransaction("00000042", accounts.Items[0].Id)
            .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "1.0000", accounts.Items[0].Id)
            .SetCreationTime(DateTimeOffset.UtcNow)
            .SetTimeToLiveMilliseconds(5_000)
            .SetNonce(1)
            .BuildSigned(Convert.FromHexString(seedHex));

        Console.WriteLine($"Prepared signed transaction hash: {signed.TransactionHashHex}");
    }
}
catch (ToriiApiException exception)
{
    Console.WriteLine($"Torii call failed with status {(int?)exception.StatusCode}: {exception.ResponseBody ?? exception.Message}");
}
catch (Exception exception)
{
    Console.WriteLine($"Torii call failed: {exception.Message}");
}
