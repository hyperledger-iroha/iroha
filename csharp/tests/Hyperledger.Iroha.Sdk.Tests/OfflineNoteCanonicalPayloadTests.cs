using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Offline;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineNoteCanonicalPayloadTests
{
    private const byte CompactLenFlag = 0x02;
    private const string AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string SeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

    [Fact]
    public void ConstructorsCanonicalizeValuesAndDefensivelyCopyMutableInputs()
    {
        var publicKey = FixedBytes(0x10, 32, oddLastByte: false);
        var assertionPublicKey = FixedBytes(0x40, 65, oddLastByte: false);
        var certificate = ValidCertificatePayload(publicKey: publicKey, assertionPublicKey: assertionPublicKey);

        publicKey[0] ^= 0xff;
        assertionPublicKey[0] ^= 0xff;
        Assert.NotEqual(publicKey[0], certificate.PublicKey[0]);
        Assert.NotEqual(assertionPublicKey[0], certificate.AssertionPublicKey[0]);

        var returnedPublicKey = certificate.PublicKey;
        returnedPublicKey[1] ^= 0xff;
        Assert.NotEqual(returnedPublicKey[1], certificate.PublicKey[1]);

        var noteCommitment = Hash(0x20);
        var keyCertificatePayloadHash = Hash(0x60);
        var claim = new OfflineNoteIssuedClaim(
            noteCommitment,
            keyCertificatePayloadHash,
            AssetId(),
            "001.2300");
        noteCommitment[0] ^= 0xff;
        keyCertificatePayloadHash[0] ^= 0xff;

        Assert.NotEqual(noteCommitment[0], claim.NoteCommitment[0]);
        Assert.NotEqual(keyCertificatePayloadHash[0], claim.KeyCertificatePayloadHash[0]);
        Assert.Equal("1.2300", claim.Amount);
        Assert.Equal(AssetId(), claim.AssetId);
        Assert.Equal(
            AssetId(dataspaceId: "7"),
            new OfflineNoteIssuedClaim(
                Hash(0x21),
                Hash(0x61),
                AssetId(dataspaceId: "7"),
                "5").AssetId);

        var inputNullifier = Hash(0x80);
        var outputCommitment = claim.NoteCommitment;
        var audit = new OfflineNoteAuditPublicInputs(
            Hash(0xa0),
            Hash(0xc0),
            new[] { inputNullifier },
            new[] { claim },
            new[] { outputCommitment },
            new[] { claim });
        inputNullifier[0] ^= 0xff;
        outputCommitment[0] ^= 0xff;
        Assert.NotEqual(inputNullifier[0], audit.InputNullifiers[0][0]);
        Assert.NotEqual(outputCommitment[0], audit.OutputCommitments[0][0]);
    }

    [Fact]
    public void PreimageConstructorsCanonicalizeValuesAndDefensivelyCopyMutableInputs()
    {
        var ownerHash = Hash(0x60);
        var noteSecret = FixedBytes(0x30, 32, oddLastByte: false);
        var commitment = new OfflineNoteCommitmentPreimage(
            "chain-main",
            ownerHash,
            AssetId(),
            "0002.5000",
            noteSecret,
            new OfflineNoteCommitmentOrigin.P2pOutput("payment-1", 2));

        ownerHash[0] ^= 0xff;
        noteSecret[0] ^= 0xff;
        Assert.NotEqual(ownerHash[0], commitment.OwnerKeyCertificatePayloadHash[0]);
        Assert.NotEqual(noteSecret[0], commitment.NoteSecret[0]);
        Assert.Equal("2.5000", commitment.Amount);
        Assert.Equal(AssetId(), commitment.AssetId);

        var returnedSecret = commitment.NoteSecret;
        returnedSecret[1] ^= 0xff;
        Assert.NotEqual(returnedSecret[1], commitment.NoteSecret[1]);

        var sourceCommitment = Hash(0x20);
        var nullifierSecret = FixedBytes(0x34, 32, oddLastByte: false);
        var nullifier = new OfflineNoteInputNullifierPreimage(
            "chain-main",
            sourceCommitment,
            Hash(0x60),
            nullifierSecret);

        sourceCommitment[0] ^= 0xff;
        nullifierSecret[0] ^= 0xff;
        Assert.NotEqual(sourceCommitment[0], nullifier.SourceNoteCommitment[0]);
        Assert.NotEqual(nullifierSecret[0], nullifier.NoteSecret[0]);

        var tokenNonce = FixedBytes(0x38, 32, oddLastByte: false);
        var inputNullifier = Hash(0x80);
        var outputCommitment = Hash(0x90);
        var token = new OfflineNotePaymentTokenIdPreimage(
            "chain-main",
            "payment-1",
            42,
            tokenNonce,
            Hash(0x60),
            new[] { inputNullifier },
            new[] { outputCommitment });

        tokenNonce[0] ^= 0xff;
        inputNullifier[0] ^= 0xff;
        outputCommitment[0] ^= 0xff;
        Assert.NotEqual(tokenNonce[0], token.TokenNonce[0]);
        Assert.NotEqual(inputNullifier[0], token.InputNullifiers[0][0]);
        Assert.NotEqual(outputCommitment[0], token.OutputCommitments[0][0]);
    }

    [Fact]
    public void ConstructorsRejectExactDomainAndInvariantViolations()
    {
        var certificate = ValidCertificatePayload();
        var claim = ValidClaim();
        var redeem = ValidRedeemPublicInputs();
        var audit = ValidAuditPublicInputs();

        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteKeyCertificatePayload(
            OfflineNoteCanonicalPayloadDomains.KeyCertificatePayload + " ",
            certificate.Version,
            certificate.Platform,
            certificate.KeyId,
            certificate.DeviceId,
            certificate.AccountId,
            certificate.PublicKey,
            certificate.AssertionScheme,
            certificate.AssertionKeyAlgorithm,
            certificate.AssertionPublicKey,
            certificate.AssertionUsageCountLimit,
            certificate.OneUse));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteIssuedClaim(
            OfflineNoteCanonicalPayloadDomains.IssuedClaim + "\n",
            claim.NoteCommitment,
            claim.KeyCertificatePayloadHash,
            claim.AssetId,
            claim.Amount));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteRedeemPublicInputs(
            "forged:" + OfflineNoteCanonicalPayloadDomains.RedeemPublicInputs,
            redeem.SourceNoteCommitment,
            redeem.InputNullifiers,
            redeem.KeyCertificatePayloadHash,
            redeem.Recipient,
            redeem.AssetId,
            redeem.Amount));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteAuditPublicInputs(
            " " + OfflineNoteCanonicalPayloadDomains.AuditPublicInputs,
            audit.TokenId,
            audit.KeyCertificatePayloadHash,
            audit.InputNullifiers,
            audit.InputClaims,
            audit.OutputCommitments,
            audit.OutputClaims));

        Assert.ThrowsAny<ArgumentException>(() => ValidCertificatePayload(version: 2));
        Assert.ThrowsAny<ArgumentException>(() => ValidCertificatePayload(oneUse: false));
        Assert.ThrowsAny<ArgumentException>(() => ValidCertificatePayload(assertionUsageCountLimit: 2));
        Assert.ThrowsAny<ArgumentException>(() => ValidCertificatePayload(publicKey: FixedBytes(0x10, 31, oddLastByte: false)));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteIssuedClaim(
            EvenHash(0x20),
            Hash(0x60),
            AssetId(),
            "1"));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteIssuedClaim(
            Hash(0x20),
            Hash(0x60),
            " " + AssetId(),
            "1"));
        foreach (var dataspaceId in new[]
                 {
                     "",
                     "+7",
                     "-7",
                     "007",
                     "7 ",
                     "7.0",
                     "18446744073709551616",
                 })
        {
            Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteIssuedClaim(
                Hash(0x20),
                Hash(0x60),
                AssetId(dataspaceId: dataspaceId),
                "1"));
        }
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteIssuedClaim(
            Hash(0x20),
            Hash(0x60),
            AssetId(),
            "1." + new string('0', 29)));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteRedeemPublicInputs(
            Hash(0x20),
            Array.Empty<byte[]>(),
            Hash(0x60),
            AccountId(),
            AssetId(),
            "1"));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteAuditPublicInputs(
            Hash(0xa0),
            Hash(0xc0),
            new[] { Hash(0x80) },
            Array.Empty<OfflineNoteIssuedClaim>(),
            new[] { claim.NoteCommitment },
            new[] { claim }));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteAuditPublicInputs(
            Hash(0xa0),
            Hash(0xc0),
            new[] { Hash(0x80) },
            new[] { claim },
            new[] { Hash(0xe0) },
            new[] { claim }));

        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteCommitmentPreimage(
            OfflineNoteCanonicalPayloadDomains.NoteCommitment + " ",
            "chain-main",
            Hash(0x60),
            AssetId(),
            "1",
            FixedBytes(0x30, 32, oddLastByte: false),
            new OfflineNoteCommitmentOrigin.IssuerLoad("operation-1", "lineage-1", 1)));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteInputNullifierPreimage(
            "forged:" + OfflineNoteCanonicalPayloadDomains.InputNullifier,
            "chain-main",
            Hash(0x20),
            Hash(0x60),
            FixedBytes(0x30, 32, oddLastByte: false)));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentTokenIdPreimage(
            OfflineNoteCanonicalPayloadDomains.PaymentTokenId + "\n",
            "chain-main",
            "payment-1",
            42,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x60),
            new[] { Hash(0x80) },
            new[] { Hash(0x90) }));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteCommitmentPreimage(
            " chain-main",
            Hash(0x60),
            AssetId(),
            "1",
            FixedBytes(0x30, 32, oddLastByte: false),
            new OfflineNoteCommitmentOrigin.IssuerLoad("operation-1", "lineage-1", 1)));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteCommitmentPreimage(
            "chain main",
            Hash(0x60),
            AssetId(),
            "1",
            FixedBytes(0x30, 32, oddLastByte: false),
            new OfflineNoteCommitmentOrigin.IssuerLoad("operation-1", "lineage-1", 1)));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteCommitmentPreimage(
            "chain-main",
            Hash(0x60),
            AssetId(),
            "1",
            FixedBytes(0x30, 31, oddLastByte: false),
            new OfflineNoteCommitmentOrigin.IssuerLoad("operation-1", "lineage-1", 1)));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentTokenIdPreimage(
            "chain-main",
            "payment 1",
            42,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x60),
            new[] { Hash(0x80) },
            new[] { Hash(0x90) }));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentTokenIdPreimage(
            "chain-main",
            "payment-1",
            0,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x60),
            new[] { Hash(0x80) },
            new[] { Hash(0x90) }));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentTokenIdPreimage(
            "chain-main",
            "payment-1",
            42,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x60),
            Array.Empty<byte[]>(),
            new[] { Hash(0x90) }));
    }

    [Fact]
    public void CodecsRoundTripWithCompactNoritoArchives()
    {
        var certificate = ValidCertificatePayload();
        var claim = ValidClaim();
        var redeem = ValidRedeemPublicInputs();
        var audit = ValidAuditPublicInputs();

        var encodedCertificate = OfflineNoteCanonicalPayloadCodec.EncodeKeyCertificatePayload(certificate);
        var encodedClaim = OfflineNoteCanonicalPayloadCodec.EncodeIssuedClaim(claim);
        var encodedRedeem = OfflineNoteCanonicalPayloadCodec.EncodeRedeemPublicInputs(redeem);
        var encodedAudit = OfflineNoteCanonicalPayloadCodec.EncodeAuditPublicInputs(audit);

        AssertArchiveHeader(encodedCertificate, OfflineNoteCanonicalPayloadCodec.KeyCertificatePayloadTypeName);
        AssertArchiveHeader(encodedClaim, OfflineNoteCanonicalPayloadCodec.IssuedClaimTypeName);
        AssertArchiveHeader(encodedRedeem, OfflineNoteCanonicalPayloadCodec.RedeemPublicInputsTypeName);
        AssertArchiveHeader(encodedAudit, OfflineNoteCanonicalPayloadCodec.AuditPublicInputsTypeName);

        var decodedCertificate = OfflineNoteCanonicalPayloadCodec.DecodeKeyCertificatePayload(encodedCertificate);
        Assert.Equal(certificate.Domain, decodedCertificate.Domain);
        Assert.Equal(certificate.Version, decodedCertificate.Version);
        Assert.Equal(certificate.AccountId, decodedCertificate.AccountId);
        Assert.Equal(certificate.PublicKey, decodedCertificate.PublicKey);
        Assert.Equal(certificate.AssertionPublicKey, decodedCertificate.AssertionPublicKey);
        Assert.Equal(certificate.AssertionUsageCountLimit, decodedCertificate.AssertionUsageCountLimit);
        Assert.Equal(certificate.OneUse, decodedCertificate.OneUse);

        var decodedClaim = OfflineNoteCanonicalPayloadCodec.DecodeIssuedClaim(encodedClaim);
        Assert.Equal(claim.Domain, decodedClaim.Domain);
        Assert.Equal(claim.NoteCommitment, decodedClaim.NoteCommitment);
        Assert.Equal(claim.KeyCertificatePayloadHash, decodedClaim.KeyCertificatePayloadHash);
        Assert.Equal(claim.AssetId, decodedClaim.AssetId);
        Assert.Equal(claim.Amount, decodedClaim.Amount);

        var decodedRedeem = OfflineNoteCanonicalPayloadCodec.DecodeRedeemPublicInputs(encodedRedeem);
        Assert.Equal(redeem.Domain, decodedRedeem.Domain);
        Assert.Equal(redeem.SourceNoteCommitment, decodedRedeem.SourceNoteCommitment);
        Assert.Equal(redeem.InputNullifiers[0], decodedRedeem.InputNullifiers[0]);
        Assert.Equal(redeem.KeyCertificatePayloadHash, decodedRedeem.KeyCertificatePayloadHash);
        Assert.Equal(redeem.Recipient, decodedRedeem.Recipient);
        Assert.Equal(redeem.AssetId, decodedRedeem.AssetId);
        Assert.Equal(redeem.Amount, decodedRedeem.Amount);

        var decodedAudit = OfflineNoteCanonicalPayloadCodec.DecodeAuditPublicInputs(encodedAudit);
        Assert.Equal(audit.Domain, decodedAudit.Domain);
        Assert.Equal(audit.TokenId, decodedAudit.TokenId);
        Assert.Equal(audit.KeyCertificatePayloadHash, decodedAudit.KeyCertificatePayloadHash);
        Assert.Equal(audit.InputNullifiers[0], decodedAudit.InputNullifiers[0]);
        Assert.Equal(audit.InputClaims[0].NoteCommitment, decodedAudit.InputClaims[0].NoteCommitment);
        Assert.Equal(audit.OutputCommitments[0], decodedAudit.OutputCommitments[0]);
        Assert.Equal(audit.OutputClaims[0].NoteCommitment, decodedAudit.OutputClaims[0].NoteCommitment);
    }

    [Fact]
    public void PreimageCodecsRoundTripAndDeriveStableHashes()
    {
        var commitment = ValidCommitmentPreimage();
        var p2pCommitment = ValidCommitmentPreimage(
            new OfflineNoteCommitmentOrigin.P2pOutput("payment-2", 7));
        var inputNullifier = ValidInputNullifierPreimage();
        var paymentToken = ValidPaymentTokenIdPreimage();

        var encodedCommitment = OfflineNoteCanonicalPayloadCodec.EncodeNoteCommitmentPreimage(commitment);
        var encodedP2pCommitment = OfflineNoteCanonicalPayloadCodec.EncodeNoteCommitmentPreimage(p2pCommitment);
        var encodedInputNullifier = OfflineNoteCanonicalPayloadCodec.EncodeInputNullifierPreimage(inputNullifier);
        var encodedPaymentToken = OfflineNoteCanonicalPayloadCodec.EncodePaymentTokenIdPreimage(paymentToken);

        AssertArchiveHeader(encodedCommitment, OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName);
        AssertArchiveHeader(encodedP2pCommitment, OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName);
        AssertArchiveHeader(encodedInputNullifier, OfflineNoteCanonicalPayloadCodec.InputNullifierPreimageTypeName);
        AssertArchiveHeader(encodedPaymentToken, OfflineNoteCanonicalPayloadCodec.PaymentTokenIdPreimageTypeName);

        var decodedCommitment = OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(encodedCommitment);
        Assert.Equal(commitment.Domain, decodedCommitment.Domain);
        Assert.Equal(commitment.ChainId, decodedCommitment.ChainId);
        Assert.Equal(commitment.OwnerKeyCertificatePayloadHash, decodedCommitment.OwnerKeyCertificatePayloadHash);
        Assert.Equal(commitment.AssetId, decodedCommitment.AssetId);
        Assert.Equal(commitment.Amount, decodedCommitment.Amount);
        Assert.Equal(commitment.NoteSecret, decodedCommitment.NoteSecret);
        AssertOriginEqual(commitment.Origin, decodedCommitment.Origin);

        var decodedP2pCommitment = OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(encodedP2pCommitment);
        AssertOriginEqual(p2pCommitment.Origin, decodedP2pCommitment.Origin);

        var decodedInputNullifier = OfflineNoteCanonicalPayloadCodec.DecodeInputNullifierPreimage(encodedInputNullifier);
        Assert.Equal(inputNullifier.Domain, decodedInputNullifier.Domain);
        Assert.Equal(inputNullifier.ChainId, decodedInputNullifier.ChainId);
        Assert.Equal(inputNullifier.SourceNoteCommitment, decodedInputNullifier.SourceNoteCommitment);
        Assert.Equal(
            inputNullifier.OwnerKeyCertificatePayloadHash,
            decodedInputNullifier.OwnerKeyCertificatePayloadHash);
        Assert.Equal(inputNullifier.NoteSecret, decodedInputNullifier.NoteSecret);

        var decodedPaymentToken = OfflineNoteCanonicalPayloadCodec.DecodePaymentTokenIdPreimage(encodedPaymentToken);
        Assert.Equal(paymentToken.Domain, decodedPaymentToken.Domain);
        Assert.Equal(paymentToken.ChainId, decodedPaymentToken.ChainId);
        Assert.Equal(paymentToken.PaymentRequestId, decodedPaymentToken.PaymentRequestId);
        Assert.Equal(paymentToken.CreatedAtMs, decodedPaymentToken.CreatedAtMs);
        Assert.Equal(paymentToken.TokenNonce, decodedPaymentToken.TokenNonce);
        Assert.Equal(
            paymentToken.SenderKeyCertificatePayloadHash,
            decodedPaymentToken.SenderKeyCertificatePayloadHash);
        Assert.Equal(paymentToken.InputNullifiers[0], decodedPaymentToken.InputNullifiers[0]);
        Assert.Equal(paymentToken.OutputCommitments[0], decodedPaymentToken.OutputCommitments[0]);

        AssertDerivedHash(IrohaHash.Hash(encodedCommitment), commitment.DeriveNoteCommitment());
        AssertDerivedHash(IrohaHash.Hash(encodedInputNullifier), inputNullifier.DeriveInputNullifier());
        AssertDerivedHash(IrohaHash.Hash(encodedPaymentToken), paymentToken.DerivePaymentTokenId());
    }

    [Fact]
    public void DecodersRejectPaddedAndForgedDomainsFromWire()
    {
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeKeyCertificatePayload(ReplaceFieldChild(
                OfflineNoteCanonicalPayloadCodec.EncodeKeyCertificatePayload(ValidCertificatePayload()),
                OfflineNoteCanonicalPayloadCodec.KeyCertificatePayloadTypeName,
                0,
                StringPayload(OfflineNoteCanonicalPayloadDomains.KeyCertificatePayload + " "))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeIssuedClaim(ReplaceFieldChild(
                OfflineNoteCanonicalPayloadCodec.EncodeIssuedClaim(ValidClaim()),
                OfflineNoteCanonicalPayloadCodec.IssuedClaimTypeName,
                0,
                StringPayload(OfflineNoteCanonicalPayloadDomains.IssuedClaim + "\n"))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeRedeemPublicInputs(ReplaceFieldChild(
                OfflineNoteCanonicalPayloadCodec.EncodeRedeemPublicInputs(ValidRedeemPublicInputs()),
                OfflineNoteCanonicalPayloadCodec.RedeemPublicInputsTypeName,
                0,
                StringPayload("forged:" + OfflineNoteCanonicalPayloadDomains.RedeemPublicInputs))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeAuditPublicInputs(ReplaceFieldChild(
                OfflineNoteCanonicalPayloadCodec.EncodeAuditPublicInputs(ValidAuditPublicInputs()),
                OfflineNoteCanonicalPayloadCodec.AuditPublicInputsTypeName,
                0,
                StringPayload(" " + OfflineNoteCanonicalPayloadDomains.AuditPublicInputs))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(ReplaceFieldChild(
                OfflineNoteCanonicalPayloadCodec.EncodeNoteCommitmentPreimage(ValidCommitmentPreimage()),
                OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName,
                0,
                StringPayload(OfflineNoteCanonicalPayloadDomains.NoteCommitment + " "))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeInputNullifierPreimage(ReplaceFieldChild(
                OfflineNoteCanonicalPayloadCodec.EncodeInputNullifierPreimage(ValidInputNullifierPreimage()),
                OfflineNoteCanonicalPayloadCodec.InputNullifierPreimageTypeName,
                0,
                StringPayload("forged:" + OfflineNoteCanonicalPayloadDomains.InputNullifier))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodePaymentTokenIdPreimage(ReplaceFieldChild(
                OfflineNoteCanonicalPayloadCodec.EncodePaymentTokenIdPreimage(ValidPaymentTokenIdPreimage()),
                OfflineNoteCanonicalPayloadCodec.PaymentTokenIdPreimageTypeName,
                0,
                StringPayload(OfflineNoteCanonicalPayloadDomains.PaymentTokenId + "\n"))));
    }

    [Fact]
    public void DecodersRejectMalformedArchivesAndFieldBoundaries()
    {
        var encodedClaim = OfflineNoteCanonicalPayloadCodec.EncodeIssuedClaim(ValidClaim());

        AssertRejectsClaim(encodedClaim[..(NoritoHeader.EncodedLength - 1)]);

        var wrongSchema = encodedClaim.ToArray();
        wrongSchema[6] ^= 0xff;
        AssertRejectsClaim(wrongSchema);

        var missingCompact = encodedClaim.ToArray();
        missingCompact[39] = 0;
        AssertRejectsClaim(missingCompact);

        foreach (var forgedKnownLayoutFlags in new byte[] { 0x03, 0x06, 0x26 })
        {
            var forgedKnownLayout = encodedClaim.ToArray();
            forgedKnownLayout[39] = forgedKnownLayoutFlags;
            AssertRejectsClaim(forgedKnownLayout);
        }

        var unsupportedFlag = encodedClaim.ToArray();
        unsupportedFlag[39] = 0x08;
        AssertRejectsClaim(unsupportedFlag);

        var badChecksum = encodedClaim.ToArray();
        badChecksum[31] ^= 0xff;
        AssertRejectsClaim(badChecksum);

        AssertRejectsClaim(Wrap(
            OfflineNoteCanonicalPayloadCodec.IssuedClaimTypeName,
            Payload(encodedClaim).Concat(new byte[] { 0 }).ToArray()));
        AssertRejectsClaim(ReplaceFieldChild(
            encodedClaim,
            OfflineNoteCanonicalPayloadCodec.IssuedClaimTypeName,
            0,
            new byte[] { 0x01, 0xff }));
        AssertRejectsClaim(ReplaceFieldLength(
            encodedClaim,
            OfflineNoteCanonicalPayloadCodec.IssuedClaimTypeName,
            0,
            new byte[] { 0xa5, 0x00 }));
    }

    [Fact]
    public void DecodersRejectAdversarialNestedPayloads()
    {
        var encodedCertificate = OfflineNoteCanonicalPayloadCodec.EncodeKeyCertificatePayload(ValidCertificatePayload());
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeKeyCertificatePayload(ReplaceFieldChild(
                encodedCertificate,
                OfflineNoteCanonicalPayloadCodec.KeyCertificatePayloadTypeName,
                5,
                UInt32Payload(1))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeKeyCertificatePayload(ReplaceFieldChild(
                encodedCertificate,
                OfflineNoteCanonicalPayloadCodec.KeyCertificatePayloadTypeName,
                6,
                BytesVec(FixedBytes(0x10, 31, oddLastByte: false)))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeKeyCertificatePayload(ReplaceFieldChild(
                encodedCertificate,
                OfflineNoteCanonicalPayloadCodec.KeyCertificatePayloadTypeName,
                10,
                new byte[] { 2 })));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeKeyCertificatePayload(ReplaceFieldChild(
                encodedCertificate,
                OfflineNoteCanonicalPayloadCodec.KeyCertificatePayloadTypeName,
                11,
                new byte[] { 2 })));

        var encodedRedeem = OfflineNoteCanonicalPayloadCodec.EncodeRedeemPublicInputs(ValidRedeemPublicInputs());
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeRedeemPublicInputs(ReplaceFieldChild(
                encodedRedeem,
                OfflineNoteCanonicalPayloadCodec.RedeemPublicInputsTypeName,
                2,
                UInt64Payload(0))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeRedeemPublicInputs(ReplaceFieldChild(
                encodedRedeem,
                OfflineNoteCanonicalPayloadCodec.RedeemPublicInputsTypeName,
                2,
                Vec(FieldPayload(Hash(0x80).Concat(new byte[] { 0 }).ToArray())))));

        var encodedAudit = OfflineNoteCanonicalPayloadCodec.EncodeAuditPublicInputs(ValidAuditPublicInputs());
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeAuditPublicInputs(ReplaceFieldChild(
                encodedAudit,
                OfflineNoteCanonicalPayloadCodec.AuditPublicInputsTypeName,
                4,
                UInt64Payload(0))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeAuditPublicInputs(ReplaceFieldChild(
                encodedAudit,
                OfflineNoteCanonicalPayloadCodec.AuditPublicInputsTypeName,
                5,
                Vec(FieldPayload(Hash(0xe0))))));

        var encodedCommitment = OfflineNoteCanonicalPayloadCodec.EncodeNoteCommitmentPreimage(ValidCommitmentPreimage());
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(ReplaceFieldChild(
                encodedCommitment,
                OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName,
                1,
                FieldPayload(StringPayload(" chain-main")))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(ReplaceFieldChild(
                encodedCommitment,
                OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName,
                1,
                FieldPayload(StringPayload("chain-main").Concat(new byte[] { 0 }).ToArray()))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(ReplaceFieldChild(
                encodedCommitment,
                OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName,
                5,
                BytesVec(FixedBytes(0x30, 31, oddLastByte: false)))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(ReplaceFieldChild(
                encodedCommitment,
                OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName,
                6,
                UInt32Payload(2))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeNoteCommitmentPreimage(ReplaceFieldChild(
                encodedCommitment,
                OfflineNoteCanonicalPayloadCodec.NoteCommitmentPreimageTypeName,
                6,
                UInt32Payload(0)
                    .Concat(FieldPayload(IssuerLoadOriginPayload().Concat(new byte[] { 0 }).ToArray()))
                    .ToArray())));

        var encodedInputNullifier =
            OfflineNoteCanonicalPayloadCodec.EncodeInputNullifierPreimage(ValidInputNullifierPreimage());
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodeInputNullifierPreimage(ReplaceFieldChild(
                encodedInputNullifier,
                OfflineNoteCanonicalPayloadCodec.InputNullifierPreimageTypeName,
                4,
                BytesVec(FixedBytes(0x30, 31, oddLastByte: false)))));

        var encodedPaymentToken = OfflineNoteCanonicalPayloadCodec.EncodePaymentTokenIdPreimage(ValidPaymentTokenIdPreimage());
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodePaymentTokenIdPreimage(ReplaceFieldChild(
                encodedPaymentToken,
                OfflineNoteCanonicalPayloadCodec.PaymentTokenIdPreimageTypeName,
                3,
                UInt64Payload(0))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodePaymentTokenIdPreimage(ReplaceFieldChild(
                encodedPaymentToken,
                OfflineNoteCanonicalPayloadCodec.PaymentTokenIdPreimageTypeName,
                4,
                BytesVec(FixedBytes(0x30, 31, oddLastByte: false)))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodePaymentTokenIdPreimage(ReplaceFieldChild(
                encodedPaymentToken,
                OfflineNoteCanonicalPayloadCodec.PaymentTokenIdPreimageTypeName,
                6,
                UInt64Payload(0))));
        Assert.ThrowsAny<ArgumentException>(() =>
            OfflineNoteCanonicalPayloadCodec.DecodePaymentTokenIdPreimage(ReplaceFieldChild(
                encodedPaymentToken,
                OfflineNoteCanonicalPayloadCodec.PaymentTokenIdPreimageTypeName,
                7,
                UInt64Payload(0))));
    }

    private static OfflineNoteKeyCertificatePayload ValidCertificatePayload(
        ushort version = OfflineNoteCanonicalPayloadCodec.KeyCertificateVersion,
        byte[]? publicKey = null,
        byte[]? assertionPublicKey = null,
        uint? assertionUsageCountLimit = 1,
        bool oneUse = true)
    {
        return new OfflineNoteKeyCertificatePayload(
            version,
            "ios-app-attest",
            Convert.ToBase64String(FixedBytes(0x77, 16, oddLastByte: false)),
            "device-7",
            AccountId(),
            publicKey ?? FixedBytes(0x10, 32, oddLastByte: false),
            "apple-app-attest-v1",
            "ecdsa-p256-sha256",
            assertionPublicKey ?? FixedBytes(0x04, 65, oddLastByte: false),
            assertionUsageCountLimit,
            oneUse);
    }

    private static OfflineNoteIssuedClaim ValidClaim()
    {
        return new OfflineNoteIssuedClaim(Hash(0x20), Hash(0x60), AssetId(), "15.7500");
    }

    private static OfflineNoteRedeemPublicInputs ValidRedeemPublicInputs()
    {
        return new OfflineNoteRedeemPublicInputs(
            Hash(0x20),
            new[] { Hash(0x80) },
            Hash(0x60),
            AccountId(),
            AssetId(),
            "15.7500");
    }

    private static OfflineNoteAuditPublicInputs ValidAuditPublicInputs()
    {
        var claim = ValidClaim();
        return new OfflineNoteAuditPublicInputs(
            Hash(0xa0),
            Hash(0xc0),
            new[] { Hash(0x80) },
            new[] { claim },
            new[] { claim.NoteCommitment },
            new[] { claim });
    }

    private static OfflineNoteCommitmentPreimage ValidCommitmentPreimage(
        OfflineNoteCommitmentOrigin? origin = null)
    {
        return new OfflineNoteCommitmentPreimage(
            "chain-main",
            Hash(0x60),
            AssetId(),
            "15.7500",
            FixedBytes(0x30, 32, oddLastByte: false),
            origin ?? new OfflineNoteCommitmentOrigin.IssuerLoad("operation-1", "lineage-1", 3));
    }

    private static OfflineNoteInputNullifierPreimage ValidInputNullifierPreimage()
    {
        return new OfflineNoteInputNullifierPreimage(
            "chain-main",
            Hash(0x20),
            Hash(0x60),
            FixedBytes(0x34, 32, oddLastByte: false));
    }

    private static OfflineNotePaymentTokenIdPreimage ValidPaymentTokenIdPreimage()
    {
        return new OfflineNotePaymentTokenIdPreimage(
            "chain-main",
            "payment-1",
            42,
            FixedBytes(0x38, 32, oddLastByte: false),
            Hash(0x60),
            new[] { Hash(0x80) },
            new[] { Hash(0x90) });
    }

    private static string AccountId()
    {
        return Ed25519KeyPair.FromSeed(Convert.FromHexString(SeedHex))
            .ToAccountAddress()
            .ToI105(AccountAddress.DefaultChainDiscriminant);
    }

    private static string AssetId(string? dataspaceId = null)
    {
        var baseId = AssetDefinitionId + "#" + AccountId();
        return dataspaceId is null ? baseId : baseId + "#dataspace:" + dataspaceId;
    }

    private static void AssertArchiveHeader(byte[] archive, string typeName)
    {
        Assert.Equal("NRT0"u8.ToArray(), archive[..4]);
        Assert.Equal(0, archive[4]);
        Assert.Equal(0, archive[5]);
        Assert.Equal(NoritoCodec.SchemaHash(typeName), archive.AsSpan(6, 16).ToArray());
        Assert.Equal(0, archive[22]);
        Assert.Equal(CompactLenFlag, archive[39]);
        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, sizeof(ulong)));
        Assert.Equal((ulong)(archive.Length - NoritoHeader.EncodedLength), payloadLength);
    }

    private static void AssertRejectsClaim(byte[] archive)
    {
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteCanonicalPayloadCodec.DecodeIssuedClaim(archive));
    }

    private static void AssertDerivedHash(byte[] expected, byte[] actual)
    {
        Assert.Equal(expected, actual);
        Assert.Equal(32, actual.Length);
        Assert.Equal(1, actual[^1] & 1);
    }

    private static void AssertOriginEqual(OfflineNoteCommitmentOrigin expected, OfflineNoteCommitmentOrigin actual)
    {
        switch (expected)
        {
            case OfflineNoteCommitmentOrigin.IssuerLoad expectedIssuer:
                var actualIssuer = Assert.IsType<OfflineNoteCommitmentOrigin.IssuerLoad>(actual);
                Assert.Equal(expectedIssuer.OperationId, actualIssuer.OperationId);
                Assert.Equal(expectedIssuer.LineageId, actualIssuer.LineageId);
                Assert.Equal(expectedIssuer.LocalRevision, actualIssuer.LocalRevision);
                return;
            case OfflineNoteCommitmentOrigin.P2pOutput expectedP2p:
                var actualP2p = Assert.IsType<OfflineNoteCommitmentOrigin.P2pOutput>(actual);
                Assert.Equal(expectedP2p.PaymentRequestId, actualP2p.PaymentRequestId);
                Assert.Equal(expectedP2p.OutputIndex, actualP2p.OutputIndex);
                return;
            default:
                throw new InvalidOperationException("unsupported origin fixture");
        }
    }

    private static byte[] ReplaceFieldChild(byte[] archive, string typeName, int fieldIndex, byte[] childPayload)
    {
        var payload = Payload(archive);
        var field = LocateField(payload, fieldIndex);
        var replacement = FieldPayload(childPayload);
        var rebuilt = payload[..field.FieldStart]
            .Concat(replacement)
            .Concat(payload[field.Next..])
            .ToArray();
        return Wrap(typeName, rebuilt);
    }

    private static byte[] ReplaceFieldLength(byte[] archive, string typeName, int fieldIndex, byte[] lengthPayload)
    {
        var payload = Payload(archive);
        var field = LocateField(payload, fieldIndex);
        var rebuilt = payload[..field.FieldStart]
            .Concat(lengthPayload)
            .Concat(payload[field.ChildStart..field.Next])
            .Concat(payload[field.Next..])
            .ToArray();
        return Wrap(typeName, rebuilt);
    }

    private static (int FieldStart, int ChildStart, int ChildLength, int Next) LocateField(byte[] payload, int fieldIndex)
    {
        var offset = 0;
        for (var index = 0; index <= fieldIndex; index++)
        {
            var fieldStart = offset;
            var childLength = (int)ReadCompactLength(payload, ref offset);
            var childStart = offset;
            var next = childStart + childLength;
            if (next > payload.Length)
            {
                throw new InvalidOperationException("invalid fixture payload");
            }

            if (index == fieldIndex)
            {
                return (fieldStart, childStart, childLength, next);
            }

            offset = next;
        }

        throw new InvalidOperationException("field not found");
    }

    private static byte[] Payload(byte[] archive)
    {
        return archive[NoritoHeader.EncodedLength..];
    }

    private static byte[] Wrap(string typeName, byte[] payload)
    {
        return NoritoCodec.Encode(typeName, payload, CompactLenFlag);
    }

    private static byte[] FieldPayload(byte[] payload)
    {
        using var writer = new MemoryStream();
        WriteCompactLength(writer, (ulong)payload.Length);
        writer.Write(payload);
        return writer.ToArray();
    }

    private static byte[] IssuerLoadOriginPayload()
    {
        using var writer = new MemoryStream();
        writer.Write(FieldPayload(StringPayload("operation-1")));
        writer.Write(FieldPayload(StringPayload("lineage-1")));
        writer.Write(FieldPayload(UInt64Payload(3)));
        return writer.ToArray();
    }

    private static byte[] StringPayload(string value)
    {
        using var writer = new MemoryStream();
        var bytes = Encoding.UTF8.GetBytes(value);
        WriteCompactLength(writer, (ulong)bytes.Length);
        writer.Write(bytes);
        return writer.ToArray();
    }

    private static byte[] BytesVec(byte[] value)
    {
        using var writer = new MemoryStream();
        writer.Write(UInt64Payload((ulong)value.Length));
        writer.Write(value);
        return writer.ToArray();
    }

    private static byte[] Vec(params byte[][] fields)
    {
        using var writer = new MemoryStream();
        writer.Write(UInt64Payload((ulong)fields.Length));
        foreach (var field in fields)
        {
            writer.Write(field);
        }

        return writer.ToArray();
    }

    private static byte[] UInt32Payload(uint value)
    {
        var bytes = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] UInt64Payload(ulong value)
    {
        var bytes = new byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        return bytes;
    }

    private static void WriteCompactLength(MemoryStream writer, ulong value)
    {
        while (value >= 0x80)
        {
            writer.WriteByte((byte)((value & 0x7f) | 0x80));
            value >>= 7;
        }

        writer.WriteByte((byte)value);
    }

    private static ulong ReadCompactLength(byte[] payload, ref int offset)
    {
        ulong value = 0;
        var shift = 0;
        while (offset < payload.Length)
        {
            var current = payload[offset++];
            value |= (ulong)(current & 0x7f) << shift;
            if ((current & 0x80) == 0)
            {
                return value;
            }

            shift += 7;
        }

        throw new InvalidOperationException("invalid compact length");
    }

    private static byte[] Hash(byte seed)
    {
        return FixedBytes(seed, 32, oddLastByte: true);
    }

    private static byte[] EvenHash(byte seed)
    {
        var bytes = FixedBytes(seed, 32, oddLastByte: true);
        bytes[^1] &= 0xfe;
        return bytes;
    }

    private static byte[] FixedBytes(byte seed, int length, bool oddLastByte)
    {
        var bytes = new byte[length];
        for (var index = 0; index < bytes.Length; index++)
        {
            bytes[index] = (byte)(seed + index);
        }

        if (oddLastByte && bytes.Length > 0)
        {
            bytes[^1] |= 1;
        }

        return bytes;
    }
}
