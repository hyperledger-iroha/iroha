using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Queries;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SignedQueryBuilderTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureAccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string FixtureNetworkIdLiteral = "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";
    private const string AlternateNetworkIdLiteral = "82531ce8eae8bff6beeca4698bfd13a3bc8bec5f0ee0d23d428c97fc17ab0f3b";
    private const string FixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string FixtureContractCodeHash = "0x00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEE00";
    private const string FixtureProofHash = "0x111122223333444455556666777788889999AAAABBBBCCCCDDDDEEEEFFFF0000";
    private const string FixtureTwitterDigest = "0x1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890ABCDE0";
    private const string FixtureStorageTicket = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    private const string FixtureManifestDigest = "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    private const string FixtureProviderId = "0xCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
    private static readonly NetworkId FixtureNetworkId = NetworkId.Parse(FixtureNetworkIdLiteral);
    private static readonly NetworkId AlternateNetworkId = NetworkId.Parse(AlternateNetworkIdLiteral);

    [Fact]
    public void BuildSignedEncodesFindParametersQuery()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindParameters()
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        Assert.Equal(1, envelope.VersionedNoritoBytes[0]);

        var (singularDiscriminant, singularPayload) = ReadSingularQuery(envelope);
        Assert.Equal(1u, singularDiscriminant);
        Assert.Empty(ReadField(singularPayload, out _));

        AssertSignatureVerifies(envelope);
    }

    [Fact]
    public void BuildSignedEncodesFindAbiVersionQuery()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindAbiVersion()
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (singularDiscriminant, singularPayload) = ReadSingularQuery(envelope);
        Assert.Equal(5u, singularDiscriminant);
        Assert.Empty(ReadField(singularPayload, out _));

        AssertSignatureVerifies(envelope);
    }

    [Fact]
    public void BuildSignedEncodesFindExecutorDataModelQuery()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindExecutorDataModel()
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (singularDiscriminant, singularPayload) = ReadSingularQuery(envelope);
        Assert.Equal(0u, singularDiscriminant);
        Assert.Empty(ReadField(singularPayload, out _));

        AssertSignatureVerifies(envelope);
    }

    [Fact]
    public void SignedQueryEnvelopeDefensivelyCopiesBytes()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindParameters()
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var expectedVersionedNoritoBytes = envelope.VersionedNoritoBytes;
        var expectedSignedQueryBytes = envelope.SignedQueryBytes;
        var expectedPayloadBytes = envelope.PayloadBytes;
        var expectedSignatureBytes = envelope.SignatureBytes;

        MutateFirstByte(envelope.VersionedNoritoBytes);
        MutateFirstByte(envelope.SignedQueryBytes);
        MutateFirstByte(envelope.PayloadBytes);
        MutateFirstByte(envelope.SignatureBytes);

        Assert.Equal(expectedVersionedNoritoBytes, envelope.VersionedNoritoBytes);
        Assert.Equal(expectedSignedQueryBytes, envelope.SignedQueryBytes);
        Assert.Equal(expectedPayloadBytes, envelope.PayloadBytes);
        Assert.Equal(expectedSignatureBytes, envelope.SignatureBytes);
        AssertSignatureVerifies(envelope);

        var constructorPayloadBytes = new byte[] { 0x07, 0x08, 0x09 };
        var constructorSignatureBytes = Enumerable
            .Range(0, Ed25519Signer.SignatureLength)
            .Select(index => (byte)(0x0a + index))
            .ToArray();
        var constructorSignedQueryBytes = BuildSignedQueryBytes(constructorSignatureBytes, constructorPayloadBytes);
        var constructorVersionedNoritoBytes = VersionSignedQueryBytes(constructorSignedQueryBytes);
        var expectedConstructorSignedQueryBytes = constructorSignedQueryBytes.ToArray();
        var expectedConstructorVersionedNoritoBytes = constructorVersionedNoritoBytes.ToArray();
        var expectedConstructorPayloadBytes = constructorPayloadBytes.ToArray();
        var expectedConstructorSignatureBytes = constructorSignatureBytes.ToArray();
        var direct = new SignedQueryEnvelope(
            constructorVersionedNoritoBytes,
            constructorSignedQueryBytes,
            constructorPayloadBytes,
            constructorSignatureBytes);

        MutateFirstByte(constructorVersionedNoritoBytes);
        MutateFirstByte(constructorSignedQueryBytes);
        MutateFirstByte(constructorPayloadBytes);
        MutateFirstByte(constructorSignatureBytes);
        MutateFirstByte(direct.VersionedNoritoBytes);
        MutateFirstByte(direct.SignedQueryBytes);
        MutateFirstByte(direct.PayloadBytes);
        MutateFirstByte(direct.SignatureBytes);

        Assert.Equal(expectedConstructorVersionedNoritoBytes, direct.VersionedNoritoBytes);
        Assert.Equal(expectedConstructorSignedQueryBytes, direct.SignedQueryBytes);
        Assert.Equal(expectedConstructorPayloadBytes, direct.PayloadBytes);
        Assert.Equal(expectedConstructorSignatureBytes, direct.SignatureBytes);

        static void MutateFirstByte(byte[] value)
        {
            Assert.NotEmpty(value);
            value[0] ^= 0xff;
        }
    }

    [Fact]
    public void SignedQueryEnvelopeRejectsMalformedConstructorBytes()
    {
        var payloadBytes = new byte[] { 0x07 };
        var signatureBytes = Enumerable.Repeat((byte)0x08, Ed25519Signer.SignatureLength).ToArray();
        var signedQueryBytes = BuildSignedQueryBytes(signatureBytes, payloadBytes);
        var versionedNoritoBytes = VersionSignedQueryBytes(signedQueryBytes);
        var malformedSignedQueryBytes = new byte[] { 0x04, 0x05, 0x06 };

        AssertArgumentException(
            "versionedNoritoBytes",
            () => new SignedQueryEnvelope([], signedQueryBytes, payloadBytes, signatureBytes));
        AssertArgumentException(
            "signedQueryBytes",
            () => new SignedQueryEnvelope([0x01], [], payloadBytes, signatureBytes));
        AssertArgumentException(
            "payloadBytes",
            () => new SignedQueryEnvelope(versionedNoritoBytes, signedQueryBytes, [], signatureBytes));
        AssertArgumentException(
            "signatureBytes",
            () => new SignedQueryEnvelope(versionedNoritoBytes, signedQueryBytes, payloadBytes, signatureBytes[..^1]));
        AssertArgumentException(
            "versionedNoritoBytes",
            () => new SignedQueryEnvelope([0x02, 0x04, 0x05, 0x06], signedQueryBytes, payloadBytes, signatureBytes));
        AssertArgumentException(
            "versionedNoritoBytes",
            () => new SignedQueryEnvelope([0x01, 0x04, 0xff, 0x06], signedQueryBytes, payloadBytes, signatureBytes));
        AssertArgumentException(
            "signedQueryBytes",
            () => new SignedQueryEnvelope(
                VersionSignedQueryBytes(malformedSignedQueryBytes),
                malformedSignedQueryBytes,
                payloadBytes,
                signatureBytes));
        AssertArgumentException(
            "payloadBytes",
            () => new SignedQueryEnvelope(versionedNoritoBytes, signedQueryBytes, [0x08], signatureBytes));
        AssertArgumentException(
            "signatureBytes",
            () =>
            {
                var mismatchedSignatureBytes = signatureBytes.ToArray();
                mismatchedSignatureBytes[0] ^= 0xff;
                _ = new SignedQueryEnvelope(
                    versionedNoritoBytes,
                    signedQueryBytes,
                    payloadBytes,
                    mismatchedSignatureBytes);
            });
        AssertArgumentException(
            "signedQueryBytes",
            () =>
            {
                var withTrailingField = signedQueryBytes.Concat(new byte[8]).ToArray();
                _ = new SignedQueryEnvelope(
                    VersionSignedQueryBytes(withTrailingField),
                    withTrailingField,
                    payloadBytes,
                    signatureBytes);
            });
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("merchant@sora")]
    [InlineData("0x0a00012022d3c25e96fa1178ae08b3d30081a31a0d09e8f7321b1e015140cd37b332109ca")]
    [InlineData("n753uﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void ConstructorRejectsNonExactAuthority(string authorityAccountId)
    {
        Assert.Throws<ArgumentException>(() => new SignedQueryBuilder(authorityAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant));
    }

    [Fact]
    public void ConstructorRejectsInternalWhitespaceAuthority()
    {
        AssertArgumentException(
            "authorityAccountId",
            () => new SignedQueryBuilder(FixtureAccountId.Insert(8, " "), FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant));
    }

    [Fact]
    public void ConstructorRequiresNominalNetworkId()
    {
        Assert.Throws<ArgumentNullException>(
            () => new SignedQueryBuilder(FixtureAccountId, null!, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant));
        Assert.Throws<FormatException>(() => NetworkId.Parse("chain/dev"));
        Assert.Throws<FormatException>(() => NetworkId.Parse("genesis"));
        Assert.Throws<FormatException>(() => NetworkId.Parse(""));
    }

    [Fact]
    public void SignedQueryBuildersExposeNominalNetworkAndExplicitChainContexts()
    {
        foreach (var builderType in new[]
                 {
                     typeof(SignedQueryBuilder),
                     typeof(SignedIterableQueryBuilder),
                 })
        {
            var constructors = builderType.GetConstructors()
                .OrderBy(static constructor => constructor.GetParameters().Length)
                .ToArray();
            Assert.Collection(
                constructors,
                defaultConstructor =>
                {
                    var parameters = defaultConstructor.GetParameters();
                    Assert.Equal(2, parameters.Length);
                    Assert.Equal(typeof(NetworkId), parameters[1].ParameterType);
                },
                explicitConstructor =>
                {
                    var parameters = explicitConstructor.GetParameters();
                    Assert.Equal(3, parameters.Length);
                    Assert.Equal(typeof(NetworkId), parameters[1].ParameterType);
                    Assert.Equal(typeof(ushort), parameters[2].ParameterType);
                });
            Assert.Equal(typeof(NetworkId), builderType.GetProperty("NetworkId")!.PropertyType);
            Assert.Equal(typeof(ushort), builderType.GetProperty("ChainDiscriminant")!.PropertyType);
        }

        var tairaAccountId = AccountAddress
            .Parse(FixtureAccountId, AccountAddress.DefaultChainDiscriminant)
            .ToI105(AccountAddress.TairaTestnetChainDiscriminant);
        Assert.Equal(
            AccountAddress.TairaTestnetChainDiscriminant,
            new SignedQueryBuilder(tairaAccountId, FixtureNetworkId).ChainDiscriminant);
        Assert.Equal(
            AccountAddress.TairaTestnetChainDiscriminant,
            new SignedIterableQueryBuilder(tairaAccountId, FixtureNetworkId).ChainDiscriminant);
        Assert.Throws<ArgumentException>(() => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId));
        Assert.Throws<ArgumentException>(() => new SignedIterableQueryBuilder(FixtureAccountId, FixtureNetworkId));
        Assert.Equal(
            AccountAddress.DefaultChainDiscriminant,
            new SignedQueryBuilder(
                FixtureAccountId,
                FixtureNetworkId,
                AccountAddress.DefaultChainDiscriminant).ChainDiscriminant);
    }

    [Fact]
    public void BuildSignedRejectsInvalidReplayContext()
    {
        var seed = Convert.FromHexString(FixtureSeedHex);
        var builder = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindParameters();

        Assert.Throws<ArgumentOutOfRangeException>(() =>
            builder.BuildSigned(seed, 1_000, 0, Enumerable.Repeat((byte)0x5A, 32).ToArray()));
        AssertArgumentException(
            "nonce",
            () => builder.BuildSigned(seed, 1_000, 10_000, new byte[32]));
        AssertArgumentException(
            "nonce",
            () => builder.BuildSigned(seed, 1_000, 10_000, new byte[31]));
    }

    [Fact]
    public void NetworkIdentityIsBoundIntoPayloadAndSignature()
    {
        var seed = Convert.FromHexString(FixtureSeedHex);
        var nonce = Enumerable.Repeat((byte)0x5A, 32).ToArray();
        var first = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindParameters()
            .BuildSigned(seed, 1_000, 10_000, nonce);
        var second = new SignedQueryBuilder(FixtureAccountId, AlternateNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindParameters()
            .BuildSigned(seed, 1_000, 10_000, nonce);

        Assert.False(first.PayloadBytes.SequenceEqual(second.PayloadBytes));
        Assert.False(first.SignatureBytes.SequenceEqual(second.SignatureBytes));
        AssertSignatureVerifies(first);
        AssertSignatureVerifies(second);
    }

    [Fact]
    public void QueryOperandSettersRejectNonExactRequiredValues()
    {
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAliasesByAccountId(" " + FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAliasesByAccountId("merchant@sora");
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAssetById(" " + FixtureAssetDefinitionId, FixtureAccountId);
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAssetById(
                FixtureAssetDefinitionId,
                "n753uﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53");
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAssetById(
                FixtureAssetDefinitionId,
                "0x0a00012022d3c25e96fa1178ae08b3d30081a31a0d09e8f7321b1e015140cd37b332109ca");
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDomainEndorsements("banka ");
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDomainCommittee("committee-7\u0000");
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDaPinIntentByAlias(" manifest-root");
        });
        Assert.Throws<ArgumentException>(() =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDaPinIntentByTicket(FixtureStorageTicket + " ");
        });
    }

    [Fact]
    public void QueryOperandSettersRejectInternalWhitespaceBeforeBuild()
    {
        AssertArgumentException(
            "accountId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAliasesByAccountId(FixtureAccountId.Insert(8, " ")));
        AssertArgumentException(
            "assetDefinitionId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAssetById("asset def", FixtureAccountId));
        AssertArgumentException(
            "accountId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAssetById(
                FixtureAssetDefinitionId,
                FixtureAccountId.Insert(8, "\t")));
        AssertArgumentException(
            "codeHash",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindContractManifestByCodeHash(
                FixtureContractCodeHash.Insert(10, " ")));
        AssertArgumentException(
            "pepperId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindTwitterBindingByHash("pepper v1", FixtureTwitterDigest));
        AssertArgumentException(
            "digestHex",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindTwitterBindingByHash(
                "pepper-v1",
                FixtureTwitterDigest.Insert(10, " ")));
        AssertArgumentException(
            "domainId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDomainEndorsements("ban ka"));
        AssertArgumentException(
            "domainId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDomainEndorsementPolicy("ban ka"));
        AssertArgumentException(
            "committeeId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDomainCommittee("committee 7"));
        AssertArgumentException(
            "storageTicket",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDaPinIntentByTicket(FixtureStorageTicket.Insert(12, " ")));
        AssertArgumentException(
            "manifestDigest",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDaPinIntentByManifest(
                FixtureManifestDigest.Insert(12, "\u00A0")));
        AssertArgumentException(
            "alias",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindDaPinIntentByAlias("manifest root"));
        AssertArgumentException(
            "providerId",
            () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindSorafsProviderOwner(FixtureProviderId.Insert(12, " ")));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" paynet")]
    [InlineData("paynet ")]
    [InlineData("pay net")]
    [InlineData("pay\u00A0net")]
    [InlineData("pay\u0000net")]
    public void FindAliasesByAccountIdRejectsNonExactOptionalFilters(string filter)
    {
        AssertArgumentException("dataspace", () =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAliasesByAccountId(
                FixtureAccountId,
                dataspace: filter);
        });
        AssertArgumentException("domain", () =>
        {
            new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindAliasesByAccountId(
                FixtureAccountId,
                domain: filter);
        });
    }

    [Fact]
    public void BuildSignedEncodesFindAliasesByAccountIdWithFilters()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindAliasesByAccountId(FixtureAccountId, dataspace: "paynet", domain: "banka")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (singularDiscriminant, singularPayload) = ReadSingularQuery(envelope);
        Assert.Equal(2u, singularDiscriminant);

        var structPayload = ReadField(singularPayload, out _);
        _ = ReadField(structPayload, out var offsetAfterAccountId);
        var dataspaceOption = ReadField(structPayload[offsetAfterAccountId..], out var offsetAfterDataspace);
        var domainOption = ReadField(structPayload[(offsetAfterAccountId + offsetAfterDataspace)..], out _);

        Assert.Equal("paynet", ReadOptionalString(dataspaceOption));
        Assert.Equal("banka", ReadOptionalString(domainOption));

        AssertSignatureVerifies(envelope);
    }

    [Fact]
    public void BuildSignedEncodesAssetLookupQueries()
    {
        var assetEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindAssetById(FixtureAssetDefinitionId, FixtureAccountId, dataspaceId: 9)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (assetDiscriminant, assetPayload) = ReadSingularQuery(assetEnvelope);
        Assert.Equal(6u, assetDiscriminant);

        var assetStruct = ReadField(assetPayload, out _);
        var assetId = ReadField(assetStruct, out _);
        _ = ReadField(assetId, out var offsetAfterAccountId);
        _ = ReadField(assetId[offsetAfterAccountId..], out var offsetAfterDefinitionId);
        var scopeBytes = ReadField(assetId[(offsetAfterAccountId + offsetAfterDefinitionId)..], out _);
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(scopeBytes[..4]));
        var dataspacePayload = ReadField(scopeBytes[4..], out _);
        Assert.Equal(9ul, BinaryPrimitives.ReadUInt64LittleEndian(dataspacePayload));

        var definitionEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindAssetDefinitionById(FixtureAssetDefinitionId)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (definitionDiscriminant, definitionPayload) = ReadSingularQuery(definitionEnvelope);
        Assert.Equal(7u, definitionDiscriminant);
        var definitionStruct = ReadField(definitionPayload, out _);
        var definitionIdBytes = ReadField(definitionStruct, out _);
        AssertCanonicalAssetDefinitionId(definitionIdBytes);

        AssertSignatureVerifies(assetEnvelope);
        AssertSignatureVerifies(definitionEnvelope);
    }

    [Fact]
    public void BuildSignedEncodesContractManifestAndDataspaceOwnerQueries()
    {
        var manifestEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindContractManifestByCodeHash(FixtureContractCodeHash)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (manifestDiscriminant, manifestPayload) = ReadSingularQuery(manifestEnvelope);
        Assert.Equal(4u, manifestDiscriminant);
        var manifestStruct = ReadField(manifestPayload, out _);
        var manifestHashBytes = ReadField(manifestStruct, out _);
        var expectedHashBytes = Convert.FromHexString(FixtureContractCodeHash[2..]);
        expectedHashBytes[^1] |= 0x01;
        Assert.Equal(expectedHashBytes, manifestHashBytes);

        var dataspaceEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDataspaceNameOwnerById(42)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (dataspaceDiscriminant, dataspacePayload) = ReadSingularQuery(dataspaceEnvelope);
        Assert.Equal(17u, dataspaceDiscriminant);
        var dataspaceStruct = ReadField(dataspacePayload, out _);
        var dataspaceIdBytes = ReadField(dataspaceStruct, out _);
        Assert.Equal(42ul, BinaryPrimitives.ReadUInt64LittleEndian(dataspaceIdBytes));

        AssertSignatureVerifies(manifestEnvelope);
        AssertSignatureVerifies(dataspaceEnvelope);
    }

    [Fact]
    public void BuildSignedEncodesDomainEndorsementQueries()
    {
        var endorsementsEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDomainEndorsements("banka")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (endorsementsDiscriminant, endorsementsPayload) = ReadSingularQuery(endorsementsEnvelope);
        Assert.Equal(9u, endorsementsDiscriminant);
        var endorsementsStruct = ReadField(endorsementsPayload, out _);
        var endorsementsDomain = ReadNoritoString(ReadField(endorsementsStruct, out _));
        Assert.Equal("banka", endorsementsDomain);

        var policyEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDomainEndorsementPolicy("banka")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (policyDiscriminant, policyPayload) = ReadSingularQuery(policyEnvelope);
        Assert.Equal(10u, policyDiscriminant);
        var policyStruct = ReadField(policyPayload, out _);
        var policyDomain = ReadNoritoString(ReadField(policyStruct, out _));
        Assert.Equal("banka", policyDomain);

        var committeeEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDomainCommittee("committee-7")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (committeeDiscriminant, committeePayload) = ReadSingularQuery(committeeEnvelope);
        Assert.Equal(11u, committeeDiscriminant);
        var committeeStruct = ReadField(committeePayload, out _);
        var committeeId = ReadNoritoString(ReadField(committeeStruct, out _));
        Assert.Equal("committee-7", committeeId);

        AssertSignatureVerifies(endorsementsEnvelope);
        AssertSignatureVerifies(policyEnvelope);
        AssertSignatureVerifies(committeeEnvelope);
    }

    [Fact]
    public void BuildSignedEncodesProofAndTwitterBindingQueries()
    {
        var proofEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindProofRecordById("halo2/ipa", FixtureProofHash)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (proofDiscriminant, proofPayload) = ReadSingularQuery(proofEnvelope);
        Assert.Equal(3u, proofDiscriminant);
        var proofStruct = ReadField(proofPayload, out _);
        var proofBackend = ReadNoritoString(ReadField(proofStruct, out var proofOffsetAfterBackend));
        var proofHash = ReadField(proofStruct[proofOffsetAfterBackend..], out _);
        Assert.Equal("halo2/ipa", proofBackend);
        Assert.Equal(Convert.FromHexString(FixtureProofHash[2..]), proofHash);

        var twitterEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindTwitterBindingByHash("pepper-v1", FixtureTwitterDigest)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (twitterDiscriminant, twitterPayload) = ReadSingularQuery(twitterEnvelope);
        Assert.Equal(8u, twitterDiscriminant);
        var twitterStruct = ReadField(twitterPayload, out _);
        var pepperId = ReadNoritoString(ReadField(twitterStruct, out var twitterOffsetAfterPepperId));
        var digestBytes = ReadField(twitterStruct[twitterOffsetAfterPepperId..], out _);
        var expectedDigestBytes = Convert.FromHexString(FixtureTwitterDigest[2..]);
        expectedDigestBytes[^1] |= 0x01;
        Assert.Equal("pepper-v1", pepperId);
        Assert.Equal(expectedDigestBytes, digestBytes);

        AssertSignatureVerifies(proofEnvelope);
        AssertSignatureVerifies(twitterEnvelope);
    }

    [Fact]
    public void FindProofRecordByIdRejectsUnsupportedBackendsAndMalformedHashesBeforeBuild()
    {
        var validHash = new string('a', 64);
        foreach (var backend in new[]
        {
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "halo2/ipa/orchard",
            "groth16/bls12-377",
            "halo2/kzg",
            "mock/dev",
        })
        {
            Assert.Throws<ArgumentException>(
                () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindProofRecordById(backend, validHash));
        }

        foreach (var proofHash in new[]
        {
            "",
            "abc",
            new string('z', 64),
            new string('a', 63),
            " " + new string('a', 64),
            new string('a', 64) + " ",
            new string('a', 32) + " " + new string('a', 32),
            new string('a', 32) + "\u0000" + new string('a', 31),
            "0x0x" + new string('a', 64),
        })
        {
            Assert.Throws<ArgumentException>(
                () => new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant).FindProofRecordById("halo2/ipa", proofHash));
        }
    }

    [Fact]
    public void BuildSignedEncodesDaPinAndSorafsQueries()
    {
        var ticketEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDaPinIntentByTicket(FixtureStorageTicket)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (ticketDiscriminant, ticketPayload) = ReadSingularQuery(ticketEnvelope);
        Assert.Equal(12u, ticketDiscriminant);
        var ticketStruct = ReadField(ticketPayload, out _);
        Assert.Equal(Convert.FromHexString(FixtureStorageTicket[2..]), ReadField(ticketStruct, out _));

        var manifestEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDaPinIntentByManifest(FixtureManifestDigest)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (manifestDiscriminant, manifestPayload) = ReadSingularQuery(manifestEnvelope);
        Assert.Equal(13u, manifestDiscriminant);
        var manifestStruct = ReadField(manifestPayload, out _);
        Assert.Equal(Convert.FromHexString(FixtureManifestDigest[2..]), ReadField(manifestStruct, out _));

        var aliasEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDaPinIntentByAlias("manifest-root")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (aliasDiscriminant, aliasPayload) = ReadSingularQuery(aliasEnvelope);
        Assert.Equal(14u, aliasDiscriminant);
        var aliasStruct = ReadField(aliasPayload, out _);
        Assert.Equal("manifest-root", ReadNoritoString(ReadField(aliasStruct, out _)));

        var laneEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindDaPinIntentByLaneEpochSequence(7, 11, 13)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (laneDiscriminant, lanePayload) = ReadSingularQuery(laneEnvelope);
        Assert.Equal(15u, laneDiscriminant);
        var laneStruct = ReadField(lanePayload, out _);
        var laneId = ReadField(laneStruct, out var laneOffsetAfterLaneId);
        var epoch = ReadField(laneStruct[laneOffsetAfterLaneId..], out var laneOffsetAfterEpoch);
        var sequence = ReadField(laneStruct[(laneOffsetAfterLaneId + laneOffsetAfterEpoch)..], out _);
        Assert.Equal(7u, BinaryPrimitives.ReadUInt32LittleEndian(laneId));
        Assert.Equal(11ul, BinaryPrimitives.ReadUInt64LittleEndian(epoch));
        Assert.Equal(13ul, BinaryPrimitives.ReadUInt64LittleEndian(sequence));

        var providerEnvelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindSorafsProviderOwner(FixtureProviderId)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (providerDiscriminant, providerPayload) = ReadSingularQuery(providerEnvelope);
        Assert.Equal(16u, providerDiscriminant);
        var providerStruct = ReadField(providerPayload, out _);
        Assert.Equal(Convert.FromHexString(FixtureProviderId[2..]), ReadField(providerStruct, out _));

        AssertSignatureVerifies(ticketEnvelope);
        AssertSignatureVerifies(manifestEnvelope);
        AssertSignatureVerifies(aliasEnvelope);
        AssertSignatureVerifies(laneEnvelope);
        AssertSignatureVerifies(providerEnvelope);
    }

    [Fact]
    public void SignedQueryUsesCanonicalCompactFieldsAndRejectsFixedFieldAlias()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId, FixtureNetworkId, global::Hyperledger.Iroha.Address.AccountAddress.DefaultChainDiscriminant)
            .FindParameters()
            .BuildSigned(
                Convert.FromHexString(FixtureSeedHex),
                creationTimeMilliseconds: 1_736_000_000_000,
                timeToLiveMilliseconds: 100_000,
                nonce: Enumerable.Repeat((byte)0x01, 32).ToArray());

        Assert.Equal(new byte[] { 0x88, 0x01 }, envelope.SignedQueryBytes[..2]);
        var signatureField = ReadField(
            envelope.SignedQueryBytes,
            out var signatureConsumed);
        Assert.Equal(138, signatureConsumed);
        Assert.Equal(136, signatureField.Length);
        Assert.Equal(
            (ulong)Ed25519Signer.SignatureLength,
            BinaryPrimitives.ReadUInt64LittleEndian(signatureField));
        var offset = sizeof(ulong);
        for (var index = 0; index < Ed25519Signer.SignatureLength; index++)
        {
            Assert.Equal(1, signatureField[offset]);
            offset += 2;
        }
        Assert.Equal(signatureField.Length, offset);

        var fixedFields = new OfflineNoritoWriter();
        fixedFields.WriteField(signatureField);
        fixedFields.WriteField(envelope.PayloadBytes);
        var obsolete = fixedFields.ToArray();
        AssertArgumentException(
            "signedQueryBytes",
            () => new SignedQueryEnvelope(
                VersionSignedQueryBytes(obsolete),
                obsolete,
                envelope.PayloadBytes,
                envelope.SignatureBytes));
    }

    private static byte[] VersionSignedQueryBytes(byte[] signedQueryBytes)
    {
        var versionedNoritoBytes = new byte[signedQueryBytes.Length + 1];
        versionedNoritoBytes[0] = 1;
        signedQueryBytes.CopyTo(versionedNoritoBytes.AsSpan(1));
        return versionedNoritoBytes;
    }

    private static byte[] BuildSignedQueryBytes(byte[] signatureBytes, byte[] payloadBytes)
    {
        var signedQuery = new CanonicalNoritoWriter();
        signedQuery.WriteField(EncodeConstVec(signatureBytes));
        signedQuery.WriteField(payloadBytes);
        return signedQuery.ToArray();
    }

    private static byte[] EncodeConstVec(byte[] value)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteSequenceLength((ulong)value.Length);
        foreach (var item in value)
        {
            writer.WriteField([item]);
        }

        return writer.ToArray();
    }

    private static void AssertSignatureVerifies(SignedQueryEnvelope envelope)
    {
        Assert.Equal(envelope.VersionedNoritoBytes[1..], envelope.SignedQueryBytes);

        var signatureField = ReadField(envelope.SignedQueryBytes, out var offsetAfterSignature);
        var payloadField = ReadField(envelope.SignedQueryBytes[offsetAfterSignature..], out _);

        Assert.Equal(envelope.PayloadBytes, payloadField);
        Assert.Equal(envelope.SignatureBytes, DecodeConstVec(signatureField));

        var payloadHash = IrohaHash.Hash(envelope.PayloadBytes);
        var publicKey = Ed25519Signer.GetPublicKey(Convert.FromHexString(FixtureSeedHex));
        Assert.True(Ed25519Signer.Verify(payloadHash, envelope.SignatureBytes, publicKey));
    }

    private static void AssertArgumentException(string paramName, Action action)
    {
        var exception = Assert.Throws<ArgumentException>(action);
        Assert.Equal(paramName, exception.ParamName);
    }

    private static (uint SingularDiscriminant, byte[] SingularPayload) ReadSingularQuery(SignedQueryEnvelope envelope)
    {
        var requestField = ReadContextBoundRequest(envelope);
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(requestField[..4]));

        var singularField = ReadField(requestField[4..], out _);
        var discriminant = BinaryPrimitives.ReadUInt32LittleEndian(singularField[..4]);
        return (discriminant, singularField[4..].ToArray());
    }

    private static byte[] ReadContextBoundRequest(SignedQueryEnvelope envelope)
    {
        var payload = envelope.PayloadBytes.AsSpan();
        var offset = 0;
        var networkId = ReadField(payload[offset..], out var consumed);
        offset += consumed;
        var authority = ReadField(payload[offset..], out consumed);
        offset += consumed;
        var creationTime = ReadField(payload[offset..], out consumed);
        offset += consumed;
        var timeToLive = ReadField(payload[offset..], out consumed);
        offset += consumed;
        var nonce = ReadField(payload[offset..], out consumed);
        offset += consumed;
        var request = ReadField(payload[offset..], out consumed);
        offset += consumed;

        Assert.Equal(payload.Length, offset);
        Assert.Equal(Convert.FromHexString(FixtureNetworkIdLiteral), networkId);
        Assert.NotEmpty(authority);
        Assert.True(BinaryPrimitives.ReadUInt64LittleEndian(creationTime) > 0);
        Assert.Equal(100_000ul, BinaryPrimitives.ReadUInt64LittleEndian(timeToLive));
        Assert.Equal(32, nonce.Length);
        Assert.Contains(nonce, static value => value != 0);
        return request;
    }

    private static byte[] ReadField(ReadOnlySpan<byte> bytes, out int consumed)
    {
        var reader = new CanonicalNoritoReader(bytes, "signed-query test field", nameof(bytes));
        var field = reader.ReadField("field").ToArray();
        consumed = bytes.Length - reader.Remaining;
        return field;
    }

    private static void AssertCanonicalAssetDefinitionId(ReadOnlySpan<byte> bytes)
    {
        Assert.Equal(16 * 2, bytes.Length);
        var uuid = new byte[16];
        var offset = 0;
        for (var index = 0; index < uuid.Length; index++)
        {
            var component = ReadField(bytes[offset..], out var consumed);
            Assert.Single(component);
            Assert.Equal(2, consumed);
            uuid[index] = component[0];
            offset += consumed;
        }

        Assert.Equal(bytes.Length, offset);
        Assert.Equal(4, uuid[6] >> 4);
        Assert.Equal(2, uuid[8] >> 6);
    }

    private static byte[] DecodeConstVec(ReadOnlySpan<byte> bytes)
    {
        var reader = new CanonicalNoritoReader(bytes, "signed-query test signature", nameof(bytes));
        var count = checked((int)reader.ReadSequenceLength("count"));
        var output = new byte[count];
        for (var index = 0; index < count; index++)
        {
            var item = reader.ReadField($"signature[{index}]");
            Assert.Single(item.ToArray());
            output[index] = item[0];
        }

        reader.RequireEnd();
        return output;
    }

    private static string? ReadOptionalString(ReadOnlySpan<byte> bytes)
    {
        return bytes[0] switch
        {
            0 => null,
            1 => ReadNoritoString(ReadField(bytes[1..], out _)),
            _ => throw new InvalidOperationException("Unexpected option tag."),
        };
    }

    private static string ReadNoritoString(ReadOnlySpan<byte> bytes)
    {
        var reader = new CanonicalNoritoReader(bytes, "signed-query test string", nameof(bytes));
        var length = checked((int)reader.ReadCompactLength("length"));
        var value = Encoding.UTF8.GetString(reader.ReadExact(length, "value"));
        reader.RequireEnd();
        return value;
    }
}
