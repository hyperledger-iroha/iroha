namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    private static string ExpectedVpnEmptyFieldMessage(string operation, string field) =>
        operation == "profile" && field == "relay_endpoint"
            ? "non-empty string"
            : "non-empty";
}
