using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Queries;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SignedQueryBuilderTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureAccountId = "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53";
    private const string FixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string FixtureContractCodeHash = "0x00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEE00";
    private const string FixtureProofHash = "0x111122223333444455556666777788889999AAAABBBBCCCCDDDDEEEEFFFF0000";
    private const string FixtureTwitterDigest = "0x1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890ABCDE0";
    private const string FixtureStorageTicket = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    private const string FixtureManifestDigest = "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    private const string FixtureProviderId = "0xCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

    [Fact]
    public void BuildSignedEncodesFindParametersQuery()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId)
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
        var envelope = new SignedQueryBuilder(FixtureAccountId)
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
        var envelope = new SignedQueryBuilder(FixtureAccountId)
            .FindExecutorDataModel()
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (singularDiscriminant, singularPayload) = ReadSingularQuery(envelope);
        Assert.Equal(0u, singularDiscriminant);
        Assert.Empty(ReadField(singularPayload, out _));

        AssertSignatureVerifies(envelope);
    }

    [Fact]
    public void BuildSignedEncodesFindAliasesByAccountIdWithFilters()
    {
        var envelope = new SignedQueryBuilder(FixtureAccountId)
            .FindAliasesByAccountId(FixtureAccountId, dataspace: "sbp", domain: "banka")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (singularDiscriminant, singularPayload) = ReadSingularQuery(envelope);
        Assert.Equal(2u, singularDiscriminant);

        var structPayload = ReadField(singularPayload, out _);
        _ = ReadField(structPayload, out var offsetAfterAccountId);
        var dataspaceOption = ReadField(structPayload[offsetAfterAccountId..], out var offsetAfterDataspace);
        var domainOption = ReadField(structPayload[(offsetAfterAccountId + offsetAfterDataspace)..], out _);

        Assert.Equal("sbp", ReadOptionalString(dataspaceOption));
        Assert.Equal("banka", ReadOptionalString(domainOption));

        AssertSignatureVerifies(envelope);
    }

    [Fact]
    public void BuildSignedEncodesAssetLookupQueries()
    {
        var assetEnvelope = new SignedQueryBuilder(FixtureAccountId)
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

        var definitionEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindAssetDefinitionById(FixtureAssetDefinitionId)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (definitionDiscriminant, definitionPayload) = ReadSingularQuery(definitionEnvelope);
        Assert.Equal(7u, definitionDiscriminant);
        var definitionStruct = ReadField(definitionPayload, out _);
        var definitionIdBytes = ReadField(definitionStruct, out _);
        Assert.Equal(16 * 9, definitionIdBytes.Length);

        AssertSignatureVerifies(assetEnvelope);
        AssertSignatureVerifies(definitionEnvelope);
    }

    [Fact]
    public void BuildSignedEncodesContractManifestAndDataspaceOwnerQueries()
    {
        var manifestEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindContractManifestByCodeHash(FixtureContractCodeHash)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (manifestDiscriminant, manifestPayload) = ReadSingularQuery(manifestEnvelope);
        Assert.Equal(4u, manifestDiscriminant);
        var manifestStruct = ReadField(manifestPayload, out _);
        var manifestHashBytes = ReadField(manifestStruct, out _);
        var expectedHashBytes = Convert.FromHexString(FixtureContractCodeHash[2..]);
        expectedHashBytes[^1] |= 0x01;
        Assert.Equal(expectedHashBytes, manifestHashBytes);

        var dataspaceEnvelope = new SignedQueryBuilder(FixtureAccountId)
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
        var endorsementsEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindDomainEndorsements("banka")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (endorsementsDiscriminant, endorsementsPayload) = ReadSingularQuery(endorsementsEnvelope);
        Assert.Equal(9u, endorsementsDiscriminant);
        var endorsementsStruct = ReadField(endorsementsPayload, out _);
        var endorsementsDomain = ReadNoritoString(ReadField(endorsementsStruct, out _));
        Assert.Equal("banka", endorsementsDomain);

        var policyEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindDomainEndorsementPolicy("banka")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (policyDiscriminant, policyPayload) = ReadSingularQuery(policyEnvelope);
        Assert.Equal(10u, policyDiscriminant);
        var policyStruct = ReadField(policyPayload, out _);
        var policyDomain = ReadNoritoString(ReadField(policyStruct, out _));
        Assert.Equal("banka", policyDomain);

        var committeeEnvelope = new SignedQueryBuilder(FixtureAccountId)
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
        var proofEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindProofRecordById("halo2/ipa", FixtureProofHash)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (proofDiscriminant, proofPayload) = ReadSingularQuery(proofEnvelope);
        Assert.Equal(3u, proofDiscriminant);
        var proofStruct = ReadField(proofPayload, out _);
        var proofBackend = ReadNoritoString(ReadField(proofStruct, out var proofOffsetAfterBackend));
        var proofHash = ReadField(proofStruct[proofOffsetAfterBackend..], out _);
        Assert.Equal("halo2/ipa", proofBackend);
        Assert.Equal(Convert.FromHexString(FixtureProofHash[2..]), proofHash);

        var twitterEnvelope = new SignedQueryBuilder(FixtureAccountId)
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
    public void BuildSignedEncodesDaPinAndSorafsQueries()
    {
        var ticketEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindDaPinIntentByTicket(FixtureStorageTicket)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (ticketDiscriminant, ticketPayload) = ReadSingularQuery(ticketEnvelope);
        Assert.Equal(12u, ticketDiscriminant);
        var ticketStruct = ReadField(ticketPayload, out _);
        Assert.Equal(Convert.FromHexString(FixtureStorageTicket[2..]), ReadField(ticketStruct, out _));

        var manifestEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindDaPinIntentByManifest(FixtureManifestDigest)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (manifestDiscriminant, manifestPayload) = ReadSingularQuery(manifestEnvelope);
        Assert.Equal(13u, manifestDiscriminant);
        var manifestStruct = ReadField(manifestPayload, out _);
        Assert.Equal(Convert.FromHexString(FixtureManifestDigest[2..]), ReadField(manifestStruct, out _));

        var aliasEnvelope = new SignedQueryBuilder(FixtureAccountId)
            .FindDaPinIntentByAlias("manifest-root")
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var (aliasDiscriminant, aliasPayload) = ReadSingularQuery(aliasEnvelope);
        Assert.Equal(14u, aliasDiscriminant);
        var aliasStruct = ReadField(aliasPayload, out _);
        Assert.Equal("manifest-root", ReadNoritoString(ReadField(aliasStruct, out _)));

        var laneEnvelope = new SignedQueryBuilder(FixtureAccountId)
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

        var providerEnvelope = new SignedQueryBuilder(FixtureAccountId)
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

    private static (uint SingularDiscriminant, byte[] SingularPayload) ReadSingularQuery(SignedQueryEnvelope envelope)
    {
        var authorityField = ReadField(envelope.PayloadBytes, out var offsetAfterAuthority);
        var requestField = ReadField(envelope.PayloadBytes[offsetAfterAuthority..], out _);

        Assert.NotEmpty(authorityField);
        Assert.Equal(0u, BinaryPrimitives.ReadUInt32LittleEndian(requestField[..4]));

        var singularField = ReadField(requestField[4..], out _);
        var discriminant = BinaryPrimitives.ReadUInt32LittleEndian(singularField[..4]);
        return (discriminant, singularField[4..].ToArray());
    }

    private static byte[] ReadField(ReadOnlySpan<byte> bytes, out int consumed)
    {
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        consumed = 8 + length;
        return bytes.Slice(8, length).ToArray();
    }

    private static byte[] DecodeConstVec(ReadOnlySpan<byte> bytes)
    {
        var count = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        var output = new byte[count];
        var offset = 8;
        for (var index = 0; index < count; index++)
        {
            var fieldLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes.Slice(offset, 8)));
            Assert.Equal(1, fieldLength);
            output[index] = bytes[offset + 8];
            offset += 9;
        }

        Assert.Equal(bytes.Length, offset);
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
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        return Encoding.UTF8.GetString(bytes.Slice(8, length));
    }
}
