using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Queries;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SignedIterableQueryBuilderTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
    private const string FixtureAccountId = "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53";
    private const string FixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string FixtureCertificateId = "1111111111111111111111111111111111111111111111111111111111111122";
    private const string FixtureOfflineTransferId = "2222222222222222222222222222222222222222222222222222222222222240";

    [Fact]
    public void BuildSignedEncodesZeroPayloadIterableQueries()
    {
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindDomains().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 0,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindAccounts().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 1,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindAssets().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 2,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindAssetDefinitions().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 3,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindRepoAgreements().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 4,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindNfts().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 5,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindRwas().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 6,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindTransactions().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 12,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindPeers().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 9,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindActiveTriggerIds().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 10,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindTriggers().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 11,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindRoles().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 7,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindRoleIds().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 8,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindBlocks().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 13,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindBlockHeaders().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 14,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindProofRecords().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 15,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindOfflineAllowances().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 17,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindOfflineToOnlineTransfers().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 18,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindOfflineCounterSummaries().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 19,
            expectedQueryPayload: Array.Empty<byte>());
        AssertIterableStart(
            new SignedIterableQueryBuilder(FixtureAccountId).FindOfflineVerdictRevocations().BuildSigned(Convert.FromHexString(FixtureSeedHex)),
            expectedItemKindDiscriminant: 20,
            expectedQueryPayload: Array.Empty<byte>());
    }

    [Fact]
    public void BuildSignedEncodesParameterizedIterableQueries()
    {
        var accountsWithAsset = new SignedIterableQueryBuilder(FixtureAccountId)
            .FindAccountsWithAsset(FixtureAssetDefinitionId)
            .SetLimit(25)
            .SetOffset(7)
            .SetFetchSize(50)
            .SortByMetadata("rank", descending: true)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var accountsWithAssetParams = ReadIterableStart(accountsWithAsset);
        Assert.Equal(1u, accountsWithAssetParams.ItemKindDiscriminant);
        var assetDefinitionBytes = ReadField(accountsWithAssetParams.QueryPayload, out _);
        Assert.Equal(16 * 9, assetDefinitionBytes.Length);
        Assert.Equal((ulong)25, accountsWithAssetParams.Limit);
        Assert.Equal((ulong)7, accountsWithAssetParams.Offset);
        Assert.Equal((ulong)50, accountsWithAssetParams.FetchSize);
        Assert.Equal("rank", accountsWithAssetParams.SortByMetadataKey);
        Assert.Equal((uint)1, accountsWithAssetParams.SortOrderDiscriminant);

        var permissions = new SignedIterableQueryBuilder(FixtureAccountId)
            .FindPermissionsByAccountId(FixtureAccountId)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var permissionsParams = ReadIterableStart(permissions);
        Assert.Equal(16u, permissionsParams.ItemKindDiscriminant);
        Assert.NotEmpty(ReadField(permissionsParams.QueryPayload, out _));

        var roles = new SignedIterableQueryBuilder(FixtureAccountId)
            .FindRolesByAccountId(FixtureAccountId)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var rolesParams = ReadIterableStart(roles);
        Assert.Equal(8u, rolesParams.ItemKindDiscriminant);
        Assert.NotEmpty(ReadField(rolesParams.QueryPayload, out _));

        var allowanceByCertificate = new SignedIterableQueryBuilder(FixtureAccountId)
            .FindOfflineAllowanceByCertificateId(FixtureCertificateId)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var allowanceParams = ReadIterableStart(allowanceByCertificate);
        Assert.Equal(17u, allowanceParams.ItemKindDiscriminant);
        var certificateBytes = ReadField(allowanceParams.QueryPayload, out _);
        var expectedCertificateBytes = Convert.FromHexString(FixtureCertificateId);
        expectedCertificateBytes[^1] |= 0x01;
        Assert.Equal(expectedCertificateBytes, certificateBytes);

        var transferById = new SignedIterableQueryBuilder(FixtureAccountId)
            .FindOfflineToOnlineTransferById(FixtureOfflineTransferId)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));
        var transferParams = ReadIterableStart(transferById);
        Assert.Equal(18u, transferParams.ItemKindDiscriminant);
        var transferBytes = ReadField(transferParams.QueryPayload, out _);
        var expectedTransferBytes = Convert.FromHexString(FixtureOfflineTransferId);
        expectedTransferBytes[^1] |= 0x01;
        Assert.Equal(expectedTransferBytes, transferBytes);
    }

    [Fact]
    public void BuildSignedEncodesContinueCursorRequests()
    {
        var envelope = new SignedIterableQueryBuilder(FixtureAccountId)
            .Continue("cursor-1", 5, gasBudget: 9)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var (requestDiscriminant, requestPayload) = ReadQueryRequest(envelope);
        Assert.Equal(2u, requestDiscriminant);

        var cursorStruct = ReadField(requestPayload, out _);
        var queryId = ReadNoritoString(ReadField(cursorStruct, out var offsetAfterQueryId));
        var cursorBytes = ReadField(cursorStruct[offsetAfterQueryId..], out var offsetAfterCursor);
        var gasBudgetOption = ReadField(cursorStruct[(offsetAfterQueryId + offsetAfterCursor)..], out _);

        Assert.Equal("cursor-1", queryId);
        Assert.Equal(5ul, BinaryPrimitives.ReadUInt64LittleEndian(cursorBytes));
        Assert.Equal(9ul, ReadOptionalUInt64(gasBudgetOption));

        AssertSignatureVerifies(envelope);
    }

    private static void AssertIterableStart(
        SignedQueryEnvelope envelope,
        uint expectedItemKindDiscriminant,
        byte[] expectedQueryPayload)
    {
        var start = ReadIterableStart(envelope);
        Assert.Equal(expectedItemKindDiscriminant, start.ItemKindDiscriminant);
        Assert.Equal(expectedQueryPayload, start.QueryPayload);
        Assert.Equal(0u, start.PredicateDiscriminant);
        Assert.Empty(start.SelectorBytes);
        Assert.Null(start.Limit);
        Assert.Equal((ulong)0, start.Offset);
        Assert.Null(start.FetchSize);
        Assert.Null(start.SortByMetadataKey);
        Assert.Null(start.SortOrderDiscriminant);
        AssertSignatureVerifies(envelope);
    }

    private static IterableStartPayload ReadIterableStart(SignedQueryEnvelope envelope)
    {
        var (requestDiscriminant, requestPayload) = ReadQueryRequest(envelope);
        Assert.Equal(1u, requestDiscriminant);

        var queryWithParams = ReadField(requestPayload, out _);
        var queryField = ReadField(queryWithParams, out var offsetAfterQueryField);
        var queryPayloadField = ReadField(queryWithParams[offsetAfterQueryField..], out var offsetAfterQueryPayload);
        var itemField = ReadField(queryWithParams[(offsetAfterQueryField + offsetAfterQueryPayload)..], out var offsetAfterItem);
        var predicateField = ReadField(queryWithParams[(offsetAfterQueryField + offsetAfterQueryPayload + offsetAfterItem)..], out var offsetAfterPredicate);
        var selectorField = ReadField(queryWithParams[(offsetAfterQueryField + offsetAfterQueryPayload + offsetAfterItem + offsetAfterPredicate)..], out var offsetAfterSelector);
        var paramsField = ReadField(queryWithParams[(offsetAfterQueryField + offsetAfterQueryPayload + offsetAfterItem + offsetAfterPredicate + offsetAfterSelector)..], out _);

        Assert.Empty(queryField);

        var queryPayload = ReadByteVector(queryPayloadField);
        var itemKindDiscriminant = ReadFieldlessEnumDiscriminant(itemField);
        var predicateBytes = ReadByteVector(predicateField);
        var selectorBytes = ReadByteVector(selectorField);
        var predicateDiscriminant = ReadFieldlessEnumDiscriminant(predicateBytes);

        var paginationField = ReadField(paramsField, out var offsetAfterPagination);
        var sortingField = ReadField(paramsField[offsetAfterPagination..], out var offsetAfterSorting);
        var fetchSizeField = ReadField(paramsField[(offsetAfterPagination + offsetAfterSorting)..], out _);

        var limitField = ReadField(paginationField, out var offsetAfterLimit);
        var offsetField = ReadField(paginationField[offsetAfterLimit..], out _);

        var sortMetadataKeyField = ReadField(sortingField, out var offsetAfterSortMetadataKey);
        var sortOrderField = ReadField(sortingField[offsetAfterSortMetadataKey..], out _);

        var fetchSizeOptionField = ReadField(fetchSizeField, out _);

        return new IterableStartPayload(
            queryPayload,
            itemKindDiscriminant,
            predicateDiscriminant,
            selectorBytes,
            ReadOptionalUInt64(limitField),
            BinaryPrimitives.ReadUInt64LittleEndian(offsetField),
            ReadOptionalString(sortMetadataKeyField),
            ReadOptionalFieldlessEnumDiscriminant(sortOrderField),
            ReadOptionalUInt64(fetchSizeOptionField));
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

    private static (uint RequestDiscriminant, byte[] RequestPayload) ReadQueryRequest(SignedQueryEnvelope envelope)
    {
        _ = ReadField(envelope.PayloadBytes, out var offsetAfterAuthority);
        var requestField = ReadField(envelope.PayloadBytes[offsetAfterAuthority..], out _);

        var discriminant = BinaryPrimitives.ReadUInt32LittleEndian(requestField[..4]);
        return (discriminant, requestField[4..].ToArray());
    }

    private static byte[] ReadField(ReadOnlySpan<byte> bytes, out int consumed)
    {
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        consumed = 8 + length;
        return bytes.Slice(8, length).ToArray();
    }

    private static byte[] ReadByteVector(ReadOnlySpan<byte> bytes)
    {
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        return bytes.Slice(8, length).ToArray();
    }

    private static uint ReadFieldlessEnumDiscriminant(ReadOnlySpan<byte> bytes)
    {
        var discriminant = BinaryPrimitives.ReadUInt32LittleEndian(bytes[..4]);
        Assert.Empty(ReadField(bytes[4..], out _));
        return discriminant;
    }

    private static uint? ReadOptionalFieldlessEnumDiscriminant(ReadOnlySpan<byte> bytes)
    {
        return bytes[0] switch
        {
            0 => null,
            1 => ReadFieldlessEnumDiscriminant(ReadField(bytes[1..], out _)),
            _ => throw new InvalidOperationException("Unexpected option tag."),
        };
    }

    private static ulong? ReadOptionalUInt64(ReadOnlySpan<byte> bytes)
    {
        return bytes[0] switch
        {
            0 => null,
            1 => BinaryPrimitives.ReadUInt64LittleEndian(ReadField(bytes[1..], out _)),
            _ => throw new InvalidOperationException("Unexpected option tag."),
        };
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

    private sealed record IterableStartPayload(
        byte[] QueryPayload,
        uint ItemKindDiscriminant,
        uint PredicateDiscriminant,
        byte[] SelectorBytes,
        ulong? Limit,
        ulong Offset,
        string? SortByMetadataKey,
        uint? SortOrderDiscriminant,
        ulong? FetchSize);
}
