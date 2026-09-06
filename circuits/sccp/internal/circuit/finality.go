package circuit

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"
)

const (
	sumeragiProtocolVersion       = 4
	sumeragiFinalityFormatVersion = 4
	heightContextIdentityVersion  = 5
	sumeragiModeNPoS              = 1
)

// constrainFinalityStructure derives every semantic digest carried by a v4
// finality witness and enforces the redundant Sumeragi bindings. This layer is
// deliberately independent of signature verification so it can be reused by
// both Groth16 outer curves; the BLS layer authenticates the derived vote
// transcript.
func constrainFinalityStructure(
	api frontend.API,
	finality *FinalityWitness,
	batchContext []uints.U8,
	newRosterPoPs *PoPBatchWitness,
) error {
	if err := constrainExecutionCommitment(api, &finality.ParentExecution); err != nil {
		return fmt.Errorf("parent execution commitment: %w", err)
	}
	api.AssertIsEqual(finality.Height, finality.ContextHeight)
	api.AssertIsEqual(finality.Height, finality.CertificateHeight)
	api.AssertIsEqual(finality.Mode, sumeragiModeNPoS)
	api.AssertIsEqual(finality.ProposalView, finality.RoundView)

	// Range-constrain every protocol integer before arithmetic. The successor
	// equations below therefore cannot wrap through the SNARK scalar modulus.
	for _, value := range []frontend.Variable{
		finality.Height,
		finality.ContextHeight,
		finality.CertificateHeight,
		finality.Epoch,
		finality.EpochEndHeight,
		finality.RoundView,
		finality.ProposalView,
		finality.BlockHeaderView,
		finality.ParentHeight,
		finality.ParentRoundView,
		finality.ParentProposalView,
	} {
		if _, err := u64Bytes(api, value); err != nil {
			return err
		}
	}
	comparator64 := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)
	comparator64.AssertIsLess(0, finality.Height)
	comparator64.AssertIsLess(0, finality.Epoch)
	comparator64.AssertIsLess(0, finality.ParentHeight)
	comparator64.AssertIsLessEq(finality.Height, finality.EpochEndHeight)
	comparator64.AssertIsLessEq(finality.BlockHeaderView, finality.RoundView)
	api.AssertIsEqual(api.Add(finality.ParentHeight, 1), finality.Height)
	api.AssertIsEqual(finality.ParentRoundView, finality.ParentProposalView)
	api.AssertIsBoolean(finality.HasNextEpochSnapshot)
	isEpochBoundary := api.IsZero(api.Sub(finality.Height, finality.EpochEndHeight))
	api.AssertIsEqual(finality.HasNextEpochSnapshot, isEpochBoundary)
	if err := constrainNextEpochSnapshot(api, finality); err != nil {
		return fmt.Errorf("next epoch snapshot: %w", err)
	}
	if err := constrainDataAvailabilityLayout(api, &finality.DALayout); err != nil {
		return fmt.Errorf("data availability layout: %w", err)
	}
	if err := constrainCanonicalCurrentRoster(api, finality); err != nil {
		return err
	}

	if err := assertBytesEqual(api, finality.SubjectParentBlockHash[:], finality.ParentBlockHash[:]); err != nil {
		return err
	}

	countComparator := cmp.NewBoundedComparator(api, big.NewInt(MaxValidators+1), false)
	countComparator.AssertIsLessEq(4, finality.ValidatorCount)
	countComparator.AssertIsLessEq(finality.ValidatorCount, MaxValidators)
	allowedCounts := []int{4, 7, 10, 13, 16, 19, 22, 25, 28, 31}
	selectors := make([]frontend.Variable, len(allowedCounts))
	var expectedSigners frontend.Variable = 0
	for i, count := range allowedCounts {
		selectors[i] = api.IsZero(api.Sub(finality.ValidatorCount, count))
		expectedSigners = api.Add(expectedSigners, api.Mul(selectors[i], 2*((count-1)/3)+1))
	}
	api.AssertIsEqual(sumVariables(api, selectors), 1)
	var signerCount frontend.Variable = 0
	for index := 0; index < MaxValidators; index++ {
		active := countComparator.IsLess(index, finality.ValidatorCount)
		api.AssertIsBoolean(finality.SignerBitmap[index])
		api.AssertIsEqual(finality.SignerBitmap[index], api.Mul(active, finality.SignerBitmap[index]))
		signerCount = api.Add(signerCount, finality.SignerBitmap[index])
		if err := canonicalActiveDigest(api, active, finality.ValidatorKeyHashes[index][:]); err != nil {
			return err
		}
		if err := canonicalActiveDigest(api, active, finality.ValidatorPoPHashes[index][:]); err != nil {
			return err
		}
	}
	if err := assertActiveDigestsDistinct(api, finality.ValidatorCount, &finality.ValidatorKeyHashes); err != nil {
		return err
	}
	api.AssertIsEqual(signerCount, expectedSigners)
	if err := constrainCanonicalSignerIndices(
		api,
		expectedSigners,
		finality.ValidatorCount,
		&finality.SignerIndices,
		&finality.SignerBitmap,
	); err != nil {
		return fmt.Errorf("commit QC signer indices: %w", err)
	}
	_ = parentSignerCountSelectors(api, finality.ParentSignerCount)
	if err := constrainCanonicalSignerIndices(
		api,
		finality.ParentSignerCount,
		MaxValidators,
		&finality.ParentSignerIndices,
		nil,
	); err != nil {
		return fmt.Errorf("parent CommitQC signer indices: %w", err)
	}
	if err := nonZeroBytes(api, finality.ParentAggregateSignature[:]); err != nil {
		return err
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	if err := assertCompressedFinitePrefix(
		api,
		byteAPI,
		finality.ParentAggregateSignature[0],
	); err != nil {
		return fmt.Errorf("parent CommitQC aggregate signature: %w", err)
	}

	// Hash-valued Sumeragi fields use iroha_crypto::Hash's canonical marker.
	for _, digest := range [][]uints.U8{
		finality.SubjectParentBlockHash[:],
		finality.SubjectPayloadHash[:],
		finality.ParentContextID[:],
		finality.ParentSubjectParentBlockHash[:],
		finality.ParentBlockHash[:],
		finality.ParentPayloadHash[:],
		finality.NexusAMXContextHash[:],
		finality.ExecutionPolicyHash[:],
		finality.HeightContextID[:],
		finality.BlockHeaderHash[:],
		finality.FinalityArtifactHash[:],
	} {
		if err := constrainIrohaHash(api, digest); err != nil {
			return err
		}
	}
	for _, digest := range [][]uints.U8{
		finality.LeaderSeed[:],
		finality.AggregateSignatureHash[:],
		finality.VotePreimageHash[:],
	} {
		if err := nonZeroBytes(api, digest); err != nil {
			return err
		}
	}

	context, err := canonicalHeightContextIdentity(api, finality)
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, context[:], finality.HeightContextID[:]); err != nil {
		return err
	}
	voteEncoding, err := constrainCanonicalCommitVote(api, finality)
	if err != nil {
		return err
	}
	if err := constrainBLSNormalFinality(
		api,
		finality,
		batchContext,
		newRosterPoPs,
		voteEncoding,
	); err != nil {
		return err
	}
	artifact, err := canonicalFinalityArtifactHash(api, finality)
	if err != nil {
		return err
	}
	return assertBytesEqual(api, artifact[:], finality.FinalityArtifactHash[:])
}

func finalityRosterCommitment(api frontend.API, finality *FinalityWitness) ([32]uints.U8, error) {
	count, err := u32Bytes(api, finality.ValidatorCount)
	if err != nil {
		return [32]uints.U8{}, err
	}
	preimage := constants([]byte("iroha:sumeragi:v2:roster-semantic:final-v1"))
	preimage = append(preimage, uints.NewU8(sumeragiProtocolVersion))
	preimage = append(preimage, count...)
	for index := 0; index < MaxValidators; index++ {
		preimage = append(preimage, finality.ValidatorKeyHashes[index][:]...)
		preimage = append(preimage, finality.ValidatorPoPHashes[index][:]...)
	}
	return irohaBlake2bHash(api, preimage)
}

func irohaBlake2bHash(api frontend.API, preimage []uints.U8) ([32]uints.U8, error) {
	digest, err := blake2b256(api, preimage, len(preimage))
	if err != nil {
		return [32]uints.U8{}, err
	}
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return [32]uints.U8{}, fmt.Errorf("new byte api for Iroha hash marker: %w", err)
	}
	digest[31] = uapi.Or(digest[31], uints.NewU8(1))
	return digest, nil
}

func constrainIrohaHash(api frontend.API, digest []uints.U8) error {
	if len(digest) != 32 {
		return fmt.Errorf("Iroha hash must contain 32 bytes, got %d", len(digest))
	}
	if err := nonZeroBytes(api, digest); err != nil {
		return err
	}
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	leastBit := api.ToBinary(uapi.Value(digest[31]), 8)[0]
	api.AssertIsEqual(leastBit, 1)
	return nil
}
