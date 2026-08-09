namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    private static string ExpectedVpnEmptyFieldMessage(string operation, string field) =>
        operation == "profile" && field == "relay_endpoint"
            ? "must use /{ip4|ip6|dns|dns4|dns6}"
            : "non-empty";
}
