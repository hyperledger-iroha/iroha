using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    [Fact]
    public void FaucetPowComputeChallengeMatchesDeterministicVector()
    {
        var challenge = ToriiAccountFaucetPow.ComputeChallenge(
            CanonicalAccountId,
            OnboardingFixtureNetworkId,
            AccountAddress.DefaultChainDiscriminant,
            68,
            "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef");

        Assert.Equal(
            "21e547302359214b28f0d1e0b04b6aeaf62a0e597dbad018d93ab0ce6af81a05",
            Convert.ToHexString(challenge).ToLowerInvariant());
    }

    [Fact]
    public void FaucetPowChallengeBindsExactNetworkGenesis()
    {
        const string anchorHash =
            "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef";
        var first = ToriiAccountFaucetPow.ComputeChallenge(
            CanonicalAccountId,
            OnboardingFixtureNetworkId,
            AccountAddress.DefaultChainDiscriminant,
            68,
            anchorHash);
        var second = ToriiAccountFaucetPow.ComputeChallenge(
            CanonicalAccountId,
            NetworkId.Parse(AlternateNetworkId),
            AccountAddress.DefaultChainDiscriminant,
            68,
            anchorHash);

        Assert.NotEqual(first, second);
    }

    [Fact]
    public void FaucetPowRejectsNonExactChallengeInputsBeforeDerivation()
    {
        const string accountId = CanonicalAccountId;
        const string anchorHash = "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef";

        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(" " + accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, anchorHash),
            "accountId",
            "whitespace");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId + "\u0001", OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, anchorHash),
            "accountId",
            "control characters");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, " " + anchorHash),
            "anchorBlockHashHex",
            "whitespace");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, anchorHash + "\u0001"),
            "anchorBlockHashHex",
            "control characters");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, "abc"),
            "anchorBlockHashHex",
            "32-byte hex string");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, "zz"),
            "anchorBlockHashHex",
            "32-byte hex string");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, anchorHash, ""),
            "challengeSaltHex",
            "non-empty string");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, anchorHash, " 00"),
            "challengeSaltHex",
            "whitespace");
        AssertRejectsFaucetPowInput(
            () => ToriiAccountFaucetPow.ComputeChallenge(accountId, OnboardingFixtureNetworkId, AccountAddress.DefaultChainDiscriminant, 68, anchorHash, "00"),
            "challengeSaltHex",
            "32-byte hex string");
    }

    [Fact]
    public void FaucetPowComputeDigestMatchesManagedScryptVector()
    {
        var challenge = Convert.FromHexString("8fedfb3e73b08653203dfedc046fe38e523503453d0efb639cfa0e9870550adf");
        var digest = ToriiAccountFaucetPow.ComputeDigest(
            Convert.FromHexString("0000000000000001"),
            challenge,
            scryptLogN: 4,
            scryptR: 1,
            scryptP: 1);

        Assert.Equal(
            "d9dd0907aba2a70b6bdf9b5a9f5b4ef621397e5f637190e80848384b0ac1745c",
            Convert.ToHexString(digest).ToLowerInvariant());
    }

    [Theory]
    [InlineData(0, "positive")]
    [InlineData(31, "less than 31")]
    public void FaucetPowComputeDigestRejectsUnsafeScryptLogN(int scryptLogN, string expectedMessage)
    {
        var challenge = Convert.FromHexString("8fedfb3e73b08653203dfedc046fe38e523503453d0efb639cfa0e9870550adf");

        var error = Assert.Throws<ArgumentOutOfRangeException>(() =>
            ToriiAccountFaucetPow.ComputeDigest(
                Convert.FromHexString("0000000000000001"),
                challenge,
                checked((byte)scryptLogN),
                scryptR: 1,
                scryptP: 1));

        Assert.Equal("scryptLogN", error.ParamName);
        Assert.Contains(expectedMessage, error.Message);
    }
}
