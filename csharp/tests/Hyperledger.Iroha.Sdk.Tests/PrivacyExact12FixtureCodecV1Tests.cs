using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyExact12FixtureCodecV1Tests
{
    private static readonly Lazy<(string Base64, byte[] Archive)> Fixture = new(LoadFixture);

    [Fact]
    public void RustFixtureDecodesAndRoundTripsByteForByteWithoutNativeLibrary()
    {
        var archive = Fixture.Value.Archive;
        var bundle = PrivacyExact12FixtureCodecV1.DecodeCanonical(archive);

        Assert.Equal(
            PrivacyNative.PrivacyExact12FixtureBundleMaxBytes,
            PrivacyExact12FixtureCodecV1.MaxArchiveBytes);
        Assert.Equal(PrivacyExact12FixtureCodecV1.Version, bundle.Version);
        Assert.Equal(PrivacyExact12FixtureCodecV1.RowCount, bundle.Rows.Count);
        Assert.Equal(
            "48c8d56dfc59c50888aef4db2279c3b7",
            Convert.ToHexString(archive.AsSpan(6, 16)).ToLowerInvariant());
        for (var index = 0; index < bundle.Rows.Count; index++)
        {
            var row = bundle.Rows[index];
            Assert.Equal((PrivacyProtocolIdV1)index, row.ProtocolId);
            Assert.Equal(PrivacyExact12FixtureCodecV1.SubmitProofWireId, row.SubmitProofWireId);
            Assert.NotEmpty(row.StatementNorito);
            Assert.NotEmpty(row.EnvelopeNorito);
            Assert.NotEmpty(row.SubmitProofInstructionNorito);
            Assert.NotEmpty(row.TransactionIntentProjectionNorito);
            Assert.Equal(PrivacyExact12FixtureCodecV1.HashBytes, row.TransactionIntentDigest.Length);
            Assert.NotEmpty(row.UnsignedTransactionPayloadNorito);
            Assert.NotEmpty(row.SignedTransactionVersionedNorito);
            Assert.Equal(PrivacyExact12FixtureCodecV1.HashBytes, row.SignedTransactionHash.Length);
        }

        Assert.Equal(archive, PrivacyExact12FixtureCodecV1.EncodeCanonical(bundle));
        Assert.Equal(
            Fixture.Value.Base64,
            PrivacyExact12FixtureCodecV1.EncodeCanonicalBase64(bundle));
        Assert.Equal(
            bundle,
            PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(Fixture.Value.Base64));
        Assert.Equal(
            bundle,
            PrivacyExact12FixtureCodecV1.RequireTrustedCanonical(archive, archive));
    }

    [Fact]
    public void ModelsSnapshotAllMutableInputsAndExposeOnlyDefensiveViews()
    {
        var decoded = PrivacyExact12FixtureCodecV1.DecodeCanonical(Fixture.Value.Archive);
        var source = decoded.Rows[0];
        var statement = source.StatementNorito;
        var envelope = source.EnvelopeNorito;
        var instruction = source.SubmitProofInstructionNorito;
        var projection = source.TransactionIntentProjectionNorito;
        var intentDigest = source.TransactionIntentDigest;
        var unsigned = source.UnsignedTransactionPayloadNorito;
        var signed = source.SignedTransactionVersionedNorito;
        var transactionHash = source.SignedTransactionHash;
        var expectedStatement = statement.ToArray();
        var expectedEnvelope = envelope.ToArray();

        var row = new PrivacyExact12TypedFixtureRowV1(
            source.ProtocolId,
            statement,
            envelope,
            source.SubmitProofWireId,
            instruction,
            projection,
            intentDigest,
            unsigned,
            signed,
            transactionHash);
        statement[0] ^= 0xFF;
        envelope[0] ^= 0xFF;
        instruction[0] ^= 0xFF;
        projection[0] ^= 0xFF;
        intentDigest[0] ^= 0xFF;
        unsigned[0] ^= 0xFF;
        signed[0] ^= 0xFF;
        transactionHash[0] ^= 0xFF;

        Assert.Equal(expectedStatement, row.StatementNorito);
        Assert.Equal(expectedEnvelope, row.EnvelopeNorito);
        var getterCopy = row.StatementNorito;
        getterCopy[0] ^= 0xFF;
        Assert.Equal(expectedStatement, row.StatementNorito);
        Assert.NotSame(getterCopy, row.StatementNorito);

        var rows = decoded.Rows.ToArray();
        rows[0] = row;
        var bundle = new PrivacyExact12FixtureBundleV1(decoded.Version, rows);
        rows[0] = decoded.Rows[0];
        Assert.Same(row, bundle.Rows[0]);
        Assert.Throws<NotSupportedException>(
            () => ((IList<PrivacyExact12TypedFixtureRowV1>)bundle.Rows).Clear());
    }

    [Fact]
    public void ConstructorsRejectEmptyOversizedWrongHashWrongWireAndWrongOrder()
    {
        var bundle = PrivacyExact12FixtureCodecV1.DecodeCanonical(Fixture.Value.Archive);
        var row = bundle.Rows[0];

        Assert.Throws<ArgumentException>(() => CopyRow(row, statement: []));
        foreach (var rejected in new Action[]
                 {
                     () => CopyRow(
                         row,
                         statement: new byte[
                             PrivacyExact12FixtureCodecV1.MaxStatementBytes + 1]),
                     () => CopyRow(
                         row,
                         envelope: new byte[
                             PrivacyExact12FixtureCodecV1.MaxEnvelopeBytes + 1]),
                     () => CopyRow(
                         row,
                         instruction: new byte[
                             PrivacyExact12FixtureCodecV1.MaxInstructionBytes + 1]),
                     () => CopyRow(
                         row,
                         projection: new byte[
                             PrivacyExact12FixtureCodecV1.MaxIntentProjectionBytes + 1]),
                     () => CopyRow(
                         row,
                         unsignedTransaction: new byte[
                             PrivacyExact12FixtureCodecV1.MaxUnsignedTransactionBytes + 1]),
                     () => CopyRow(
                         row,
                         signedTransaction: new byte[
                             PrivacyExact12FixtureCodecV1.MaxSignedTransactionBytes + 1]),
                 })
        {
            Assert.Throws<ArgumentException>(rejected);
        }
        Assert.Throws<ArgumentException>(() => CopyRow(row, intentDigest: new byte[31]));
        Assert.Throws<ArgumentException>(() => CopyRow(row, transactionHash: new byte[33]));
        Assert.Throws<ArgumentException>(() => CopyRow(row, wireId: "privacy.submit_proof.v1"));
        Assert.Throws<ArgumentException>(
            () => CopyRow(row, wireId: "iroha.privacy.submit_proof.v0"));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => CopyRow(row, protocolId: (PrivacyProtocolIdV1)12U));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => CopyRow(row, protocolId: (PrivacyProtocolIdV1)uint.MaxValue));
        Assert.Throws<ArgumentException>(
            () => CopyRow(
                row,
                envelope: new byte[PrivacyExact12FixtureCodecV1.MaxEnvelopeBytes],
                unsignedTransaction: new byte[
                    PrivacyExact12FixtureCodecV1.MaxUnsignedTransactionBytes],
                signedTransaction: new byte[
                    PrivacyExact12FixtureCodecV1.MaxSignedTransactionBytes]));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => new PrivacyExact12FixtureBundleV1(2, bundle.Rows));
        Assert.Throws<ArgumentException>(
            () => new PrivacyExact12FixtureBundleV1(1, bundle.Rows.Take(11).ToArray()));

        var reordered = bundle.Rows.ToArray();
        (reordered[0], reordered[1]) = (reordered[1], reordered[0]);
        Assert.Throws<ArgumentException>(
            () => new PrivacyExact12FixtureBundleV1(1, reordered));
    }

    [Fact]
    public void EveryOpaqueFieldAcceptsItsExactCeilingButNotOneByteMore()
    {
        var row = PrivacyExact12FixtureCodecV1
            .DecodeCanonical(Fixture.Value.Archive)
            .Rows[0];

        Assert.Equal(
            PrivacyExact12FixtureCodecV1.MaxStatementBytes,
            CopyRow(
                row,
                statement: new byte[PrivacyExact12FixtureCodecV1.MaxStatementBytes])
                .StatementNorito.Length);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.MaxEnvelopeBytes,
            CopyRow(
                row,
                envelope: new byte[PrivacyExact12FixtureCodecV1.MaxEnvelopeBytes])
                .EnvelopeNorito.Length);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.MaxInstructionBytes,
            CopyRow(
                row,
                instruction: new byte[PrivacyExact12FixtureCodecV1.MaxInstructionBytes])
                .SubmitProofInstructionNorito.Length);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.MaxIntentProjectionBytes,
            CopyRow(
                row,
                projection: new byte[
                    PrivacyExact12FixtureCodecV1.MaxIntentProjectionBytes])
                .TransactionIntentProjectionNorito.Length);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.MaxUnsignedTransactionBytes,
            CopyRow(
                row,
                unsignedTransaction: new byte[
                    PrivacyExact12FixtureCodecV1.MaxUnsignedTransactionBytes])
                .UnsignedTransactionPayloadNorito.Length);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.MaxSignedTransactionBytes,
            CopyRow(
                row,
                signedTransaction: new byte[
                    PrivacyExact12FixtureCodecV1.MaxSignedTransactionBytes])
                .SignedTransactionVersionedNorito.Length);
    }

    [Fact]
    public void CanonicalBase64RejectsWhitespaceAlphabetPaddingAndOversizeAttacks()
    {
        var encoded = Fixture.Value.Base64;
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(string.Empty));
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(encoded + "\n"));
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(" " + encoded));
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(encoded.TrimEnd('=')));

        var alphabetIndex = encoded.IndexOfAny(['+', '/']);
        Assert.True(alphabetIndex >= 0, "fixture must exercise the standard Base64 alphabet");
        var urlAlphabet = encoded.ToCharArray();
        urlAlphabet[alphabetIndex] = urlAlphabet[alphabetIndex] == '+' ? '-' : '_';
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(new string(urlAlphabet)));

        var oversized = new string(
            'A',
            checked((int)PrivacyExact12FixtureCodecV1.CanonicalBase64EncodedLength(
                PrivacyExact12FixtureCodecV1.MaxArchiveBytes) + 1));
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(oversized));
        var sameEncodedSizeButOversizedDecoded = new string(
            'A',
            checked((int)PrivacyExact12FixtureCodecV1.CanonicalBase64EncodedLength(
                PrivacyExact12FixtureCodecV1.MaxArchiveBytes)));
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(
                sameEncodedSizeButOversizedDecoded));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => PrivacyExact12FixtureCodecV1.CanonicalBase64EncodedLength(-1));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => PrivacyExact12FixtureCodecV1.CanonicalBase64EncodedLength(long.MaxValue));
    }

    [Fact]
    public void HeaderSchemaChecksumFlagsCompressionAndLengthsFailClosed()
    {
        var canonical = Fixture.Value.Archive;
        Assert.Throws<ArgumentNullException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonical(null!));
        Assert.Throws<ArgumentNullException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(null!));
        Assert.Throws<ArgumentNullException>(
            () => PrivacyExact12FixtureCodecV1.EncodeCanonical(null!));
        Assert.Throws<ArgumentNullException>(
            () => PrivacyExact12FixtureCodecV1.EncodeCanonicalBase64(null!));
        Assert.Throws<ArgumentNullException>(
            () => PrivacyExact12FixtureCodecV1.RequireTrustedCanonical(null!, canonical));
        Assert.Throws<ArgumentNullException>(
            () => PrivacyExact12FixtureCodecV1.RequireTrustedCanonical(canonical, null!));
        AssertRejects(Array.Empty<byte>());
        AssertRejects(new byte[PrivacyExact12FixtureCodecV1.MaxArchiveBytes + 1]);
        AssertRejects(canonical[..(NoritoHeader.EncodedLength - 1)]);
        AssertRejects(Mutate(canonical, 0, (byte)'X'));
        AssertRejects(Mutate(canonical, 4, 1));
        AssertRejects(Mutate(canonical, 5, 1));
        AssertRejects(Mutate(canonical, 6, (byte)(canonical[6] ^ 0x80)));
        AssertRejects(Mutate(canonical, 22, 1));
        AssertRejects(Mutate(canonical, 39, 0));
        AssertRejects(Mutate(canonical, 39, 3));
        AssertRejects(Mutate(canonical, 31, (byte)(canonical[31] ^ 1)));

        var shortDeclaration = canonical.ToArray();
        BinaryPrimitives.WriteUInt64LittleEndian(
            shortDeclaration.AsSpan(23, sizeof(ulong)),
            (ulong)(canonical.Length - NoritoHeader.EncodedLength - 1));
        AssertRejects(shortDeclaration);

        var hugeDeclaration = canonical.ToArray();
        BinaryPrimitives.WriteUInt64LittleEndian(
            hugeDeclaration.AsSpan(23, sizeof(ulong)),
            ulong.MaxValue);
        AssertRejects(hugeDeclaration);

        var padded = new byte[canonical.Length + 1];
        canonical.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(padded);
        canonical.AsSpan(NoritoHeader.EncodedLength).CopyTo(
            padded.AsSpan(NoritoHeader.EncodedLength + 1));
        AssertRejects(padded);

        var trailing = AppendPayloadByte(canonical, 0);
        AssertRejects(trailing);
    }

    [Fact]
    public void EveryTruncationClassAndRawMutationIsRejected()
    {
        var canonical = Fixture.Value.Archive;
        foreach (var length in new[]
                 {
                     1,
                     6,
                     22,
                     39,
                     40,
                     canonical.Length / 2,
                     canonical.Length - 1,
                 })
        {
            AssertRejects(canonical[..length]);
        }

        foreach (var index in new[]
                 {
                     NoritoHeader.EncodedLength,
                     NoritoHeader.EncodedLength + 4,
                     canonical.Length / 3,
                     canonical.Length / 2,
                     canonical.Length - 1,
                 })
        {
            AssertRejects(Mutate(canonical, index, (byte)(canonical[index] ^ 0x01)));
        }
    }

    [Fact]
    public void HostileCompactLengthsCountsAndNestedLengthsAreRejectedBeforeAllocation()
    {
        var canonical = Fixture.Value.Archive;

        var overlong = InsertOverlongFirstCompactLength(canonical);
        AssertRejects(overlong);

        var oversizedFirstField = MutatePayload(canonical, payload => payload[0] = 0x7F);
        AssertRejects(oversizedFirstField);

        var rowCountOffset = LocateRows(canonical).RowsStart;
        var zeroRows = MutatePayload(
            canonical,
            payload => BinaryPrimitives.WriteUInt64LittleEndian(
                payload.AsSpan(rowCountOffset - NoritoHeader.EncodedLength, sizeof(ulong)),
                0));
        AssertRejects(zeroRows);

        var hugeRows = MutatePayload(
            canonical,
            payload => BinaryPrimitives.WriteUInt64LittleEndian(
                payload.AsSpan(rowCountOffset - NoritoHeader.EncodedLength, sizeof(ulong)),
                ulong.MaxValue));
        AssertRejects(hugeRows);

        var statementLengthOffset = LocateFirstStatementRawLength(canonical);
        var emptyStatement = MutatePayload(
            canonical,
            payload => BinaryPrimitives.WriteUInt64LittleEndian(
                payload.AsSpan(
                    statementLengthOffset - NoritoHeader.EncodedLength,
                    sizeof(ulong)),
                0));
        AssertRejects(emptyStatement);

        var hostileStatement = MutatePayload(
            canonical,
            payload => BinaryPrimitives.WriteUInt64LittleEndian(
                payload.AsSpan(
                    statementLengthOffset - NoritoHeader.EncodedLength,
                    sizeof(ulong)),
                ulong.MaxValue));
        AssertRejects(hostileStatement);
    }

    [Fact]
    public void ReorderedRowsWrongVersionAndUnknownWireIdRemainInvalidWithFreshChecksums()
    {
        var canonical = Fixture.Value.Archive;
        AssertRejects(SwapFirstTwoEncodedRows(canonical));

        var versionValueOffset = NoritoHeader.EncodedLength + 1;
        var wrongVersion = MutatePayload(
            canonical,
            payload => BinaryPrimitives.WriteUInt32LittleEndian(
                payload.AsSpan(
                    versionValueOffset - NoritoHeader.EncodedLength,
                    sizeof(uint)),
                2));
        AssertRejects(wrongVersion);

        var wireIdOffset = LocateFirstWireIdBytes(canonical);
        var wrongWire = MutatePayload(
            canonical,
            payload => payload[wireIdOffset - NoritoHeader.EncodedLength] ^= 0x01);
        AssertRejects(wrongWire);

        var invalidUtf8Wire = MutatePayload(
            canonical,
            payload => payload[wireIdOffset - NoritoHeader.EncodedLength] = 0xFF);
        AssertRejects(invalidUtf8Wire);

        var retiredWire = ReplaceFirstWireId(
            canonical,
            "iroha.privacy.submit_proof.v0");
        AssertRejects(retiredWire);

        var protocolValueOffset = LocateFirstRowFieldBody(canonical, 0);
        var unknownProtocol = MutatePayload(
            canonical,
            payload => BinaryPrimitives.WriteUInt32LittleEndian(
                payload.AsSpan(
                    protocolValueOffset - NoritoHeader.EncodedLength,
                    sizeof(uint)),
                PrivacyExact12FixtureCodecV1.RowCount));
        AssertRejects(unknownProtocol);

        var intentDigestPrefixOffset = LocateFirstRowFieldPrefix(canonical, 6);
        var shortDigest = MutatePayload(
            canonical,
            payload => payload[intentDigestPrefixOffset - NoritoHeader.EncodedLength] = 31);
        AssertRejects(shortDigest);
    }

    [Fact]
    public void SubmitProofWireIdUsesOneExactLengthAndRejectsLengthConfusion()
    {
        var canonical = Fixture.Value.Archive;
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.SubmitProofWireIdUtf8Bytes,
            Encoding.UTF8.GetByteCount(PrivacyExact12FixtureCodecV1.SubmitProofWireId));

        var outerPrefix = LocateFirstRowFieldPrefix(canonical, 3);
        var innerPrefix = LocateFirstRowFieldBody(canonical, 3);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.SubmitProofWireIdUtf8Bytes + 1,
            canonical[outerPrefix]);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.SubmitProofWireIdUtf8Bytes,
            canonical[innerPrefix]);

        foreach (var length in new byte[] { 0, 28, 30, 31, 127 })
        {
            AssertRejects(MutatePayload(
                canonical,
                payload => payload[innerPrefix - NoritoHeader.EncodedLength] = length));
        }

        foreach (var length in new byte[] { 0, 29, 31, 32, 127 })
        {
            AssertRejects(MutatePayload(
                canonical,
                payload => payload[outerPrefix - NoritoHeader.EncodedLength] = length));
        }
    }

    [Fact]
    public void UnknownAndCrossRowProtocolTagsFailClosedWithFreshChecksums()
    {
        var canonical = Fixture.Value.Archive;
        var protocolValueOffset = LocateFirstRowFieldBody(canonical, 0);
        foreach (var tag in new[] { 1U, 11U, 12U, uint.MaxValue })
        {
            var mutated = MutatePayload(
                canonical,
                payload => BinaryPrimitives.WriteUInt32LittleEndian(
                    payload.AsSpan(
                        protocolValueOffset - NoritoHeader.EncodedLength,
                        sizeof(uint)),
                    tag));
            AssertRejects(mutated);
        }
    }

    [Fact]
    public void TrustedValidationRejectsEveryOpaqueFieldMutationAndCrossRowSubstitution()
    {
        var canonical = Fixture.Value.Archive;
        var bundle = PrivacyExact12FixtureCodecV1.DecodeCanonical(canonical);
        var row = bundle.Rows[0];
        var mutations = new PrivacyExact12TypedFixtureRowV1[]
        {
            CopyRow(row, statement: FlipFirst(row.StatementNorito)),
            CopyRow(row, envelope: FlipFirst(row.EnvelopeNorito)),
            CopyRow(row, instruction: FlipFirst(row.SubmitProofInstructionNorito)),
            CopyRow(row, projection: FlipFirst(row.TransactionIntentProjectionNorito)),
            CopyRow(row, intentDigest: FlipFirst(row.TransactionIntentDigest)),
            CopyRow(row, unsignedTransaction: FlipFirst(row.UnsignedTransactionPayloadNorito)),
            CopyRow(row, signedTransaction: FlipFirst(row.SignedTransactionVersionedNorito)),
            CopyRow(row, transactionHash: FlipFirst(row.SignedTransactionHash)),
            CopyRow(row, statement: bundle.Rows[1].StatementNorito),
        };

        foreach (var mutation in mutations)
        {
            var rows = bundle.Rows.ToArray();
            rows[0] = mutation;
            var substituted = PrivacyExact12FixtureCodecV1.EncodeCanonical(
                new PrivacyExact12FixtureBundleV1(1, rows));

            _ = PrivacyExact12FixtureCodecV1.DecodeCanonical(substituted);
            Assert.Throws<ArgumentException>(
                () => PrivacyExact12FixtureCodecV1.RequireTrustedCanonical(
                    substituted,
                    canonical));
        }

        var checksumRepairedMutation = MutatePayload(
            canonical,
            payload => payload[LocateFirstStatementBytes(canonical)
                - NoritoHeader.EncodedLength] ^= 1);
        _ = PrivacyExact12FixtureCodecV1.DecodeCanonical(checksumRepairedMutation);
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.RequireTrustedCanonical(
                checksumRepairedMutation,
                canonical));
    }

    [Fact]
    public void TrustedReferenceMustItselfBeCanonicalAndInputsAreSnapshotted()
    {
        var canonical = Fixture.Value.Archive;
        var invalidTrusted = Mutate(canonical, 31, (byte)(canonical[31] ^ 1));
        Assert.Throws<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.RequireTrustedCanonical(
                canonical,
                invalidTrusted));

        var candidate = canonical.ToArray();
        var trusted = canonical.ToArray();
        var bundle = PrivacyExact12FixtureCodecV1.RequireTrustedCanonical(candidate, trusted);
        candidate[0] ^= 0xFF;
        trusted[0] ^= 0xFF;
        Assert.Equal(canonical, PrivacyExact12FixtureCodecV1.EncodeCanonical(bundle));
    }

    private static PrivacyExact12TypedFixtureRowV1 CopyRow(
        PrivacyExact12TypedFixtureRowV1 row,
        PrivacyProtocolIdV1? protocolId = null,
        byte[]? statement = null,
        byte[]? envelope = null,
        string? wireId = null,
        byte[]? instruction = null,
        byte[]? projection = null,
        byte[]? intentDigest = null,
        byte[]? unsignedTransaction = null,
        byte[]? signedTransaction = null,
        byte[]? transactionHash = null) =>
        new(
            protocolId ?? row.ProtocolId,
            statement ?? row.StatementNorito,
            envelope ?? row.EnvelopeNorito,
            wireId ?? row.SubmitProofWireId,
            instruction ?? row.SubmitProofInstructionNorito,
            projection ?? row.TransactionIntentProjectionNorito,
            intentDigest ?? row.TransactionIntentDigest,
            unsignedTransaction ?? row.UnsignedTransactionPayloadNorito,
            signedTransaction ?? row.SignedTransactionVersionedNorito,
            transactionHash ?? row.SignedTransactionHash);

    private static byte[] ReplaceFirstWireId(byte[] archive, string replacement)
    {
        var replacementBytes = Encoding.UTF8.GetBytes(replacement);
        Assert.Equal(
            PrivacyExact12FixtureCodecV1.SubmitProofWireIdUtf8Bytes,
            replacementBytes.Length);
        var wireIdOffset = LocateFirstWireIdBytes(archive);
        return MutatePayload(
            archive,
            payload => replacementBytes.CopyTo(
                payload,
                wireIdOffset - NoritoHeader.EncodedLength));
    }

    private static byte[] FlipFirst(byte[] bytes)
    {
        var copy = bytes.ToArray();
        copy[0] ^= 1;
        return copy;
    }

    private static void AssertRejects(byte[] archive) =>
        Assert.ThrowsAny<ArgumentException>(
            () => PrivacyExact12FixtureCodecV1.DecodeCanonical(archive));

    private static byte[] Mutate(byte[] archive, int index, byte value)
    {
        var copy = archive.ToArray();
        copy[index] = value;
        return copy;
    }

    private static byte[] MutatePayload(byte[] archive, Action<byte[]> mutation)
    {
        var copy = archive.ToArray();
        var payload = copy[NoritoHeader.EncodedLength..];
        mutation(payload);
        payload.CopyTo(copy, NoritoHeader.EncodedLength);
        RewriteChecksum(copy);
        return copy;
    }

    private static byte[] AppendPayloadByte(byte[] archive, byte value)
    {
        var copy = new byte[archive.Length + 1];
        archive.CopyTo(copy, 0);
        copy[^1] = value;
        BinaryPrimitives.WriteUInt64LittleEndian(
            copy.AsSpan(23, sizeof(ulong)),
            (ulong)(copy.Length - NoritoHeader.EncodedLength));
        RewriteChecksum(copy);
        return copy;
    }

    private static byte[] InsertOverlongFirstCompactLength(byte[] archive)
    {
        var copy = new byte[archive.Length + 1];
        archive.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(copy);
        copy[NoritoHeader.EncodedLength] = (byte)(archive[NoritoHeader.EncodedLength] | 0x80);
        copy[NoritoHeader.EncodedLength + 1] = 0;
        archive.AsSpan(NoritoHeader.EncodedLength + 1).CopyTo(
            copy.AsSpan(NoritoHeader.EncodedLength + 2));
        BinaryPrimitives.WriteUInt64LittleEndian(
            copy.AsSpan(23, sizeof(ulong)),
            (ulong)(copy.Length - NoritoHeader.EncodedLength));
        RewriteChecksum(copy);
        return copy;
    }

    private static byte[] SwapFirstTwoEncodedRows(byte[] archive)
    {
        var layout = LocateRows(archive);
        var position = layout.RowsStart + sizeof(ulong);
        var firstStart = position;
        var firstLength = checked((int)ReadCompact(archive, ref position));
        var firstEnd = checked(position + firstLength);
        var secondStart = firstEnd;
        position = secondStart;
        var secondLength = checked((int)ReadCompact(archive, ref position));
        var secondEnd = checked(position + secondLength);

        var copy = archive.ToArray();
        archive.AsSpan(secondStart, secondEnd - secondStart).CopyTo(
            copy.AsSpan(firstStart));
        archive.AsSpan(firstStart, firstEnd - firstStart).CopyTo(
            copy.AsSpan(firstStart + secondEnd - secondStart));
        RewriteChecksum(copy);
        return copy;
    }

    private static (int RowsStart, int RowsEnd) LocateRows(byte[] archive)
    {
        var position = NoritoHeader.EncodedLength;
        var versionLength = checked((int)ReadCompact(archive, ref position));
        position += versionLength;
        var rowsLength = checked((int)ReadCompact(archive, ref position));
        return (position, checked(position + rowsLength));
    }

    private static int LocateFirstStatementRawLength(byte[] archive)
    {
        var position = LocateRows(archive).RowsStart + sizeof(ulong);
        _ = ReadCompact(archive, ref position);
        var protocolLength = checked((int)ReadCompact(archive, ref position));
        position += protocolLength;
        _ = ReadCompact(archive, ref position);
        return position;
    }

    private static int LocateFirstStatementBytes(byte[] archive) =>
        LocateFirstStatementRawLength(archive) + sizeof(ulong);

    private static int LocateFirstWireIdBytes(byte[] archive)
    {
        var position = LocateRows(archive).RowsStart + sizeof(ulong);
        _ = ReadCompact(archive, ref position);
        for (var field = 0; field < 3; field++)
        {
            var fieldLength = checked((int)ReadCompact(archive, ref position));
            position += fieldLength;
        }

        _ = ReadCompact(archive, ref position);
        _ = ReadCompact(archive, ref position);
        return position;
    }

    private static int LocateFirstRowFieldPrefix(byte[] archive, int fieldIndex)
    {
        var position = LocateRows(archive).RowsStart + sizeof(ulong);
        _ = ReadCompact(archive, ref position);
        for (var index = 0; index < 10; index++)
        {
            var prefix = position;
            var fieldLength = checked((int)ReadCompact(archive, ref position));
            if (index == fieldIndex)
            {
                return prefix;
            }

            position += fieldLength;
        }

        throw new ArgumentOutOfRangeException(nameof(fieldIndex));
    }

    private static int LocateFirstRowFieldBody(byte[] archive, int fieldIndex)
    {
        var prefix = LocateFirstRowFieldPrefix(archive, fieldIndex);
        var position = prefix;
        _ = ReadCompact(archive, ref position);
        return position;
    }

    private static ulong ReadCompact(byte[] bytes, ref int position)
    {
        ulong value = 0;
        var shift = 0;
        while (true)
        {
            var current = bytes[position++];
            value |= (ulong)(current & 0x7F) << shift;
            if ((current & 0x80) == 0)
            {
                return value;
            }

            shift += 7;
        }
    }

    private static void RewriteChecksum(byte[] archive)
    {
        var checksum = Crc64Ecma.Compute(archive.AsSpan(NoritoHeader.EncodedLength));
        BinaryPrimitives.WriteUInt64LittleEndian(
            archive.AsSpan(31, sizeof(ulong)),
            checksum);
    }

    private static (string Base64, byte[] Archive) LoadFixture()
    {
        var path = Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "privacy",
            "exact12_typed_fixture_bundle_v1.norito.b64");
        var text = File.ReadAllText(path);
        Assert.EndsWith("\n", text, StringComparison.Ordinal);
        Assert.False(text.Contains('\r'));
        var encoded = text[..^1];
        Assert.NotEmpty(encoded);
        Assert.False(encoded.Contains('\n'));
        var archive = Convert.FromBase64String(encoded);
        Assert.Equal(encoded, Convert.ToBase64String(archive));
        return (encoded, archive);
    }
}
