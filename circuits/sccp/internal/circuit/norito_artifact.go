package circuit

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

const (
	maxHeightContextBytes = 13_824
	// Exact maximum across every final-V1 4..31-validator artifact shape:
	// full parent/current QCs, all ExecutionCommitment options, a 31-member
	// successor snapshot, and both durable 96-byte PoP vectors. Keeping this
	// bound tight avoids thousands of unnecessary variable-hash byte slots.
	maxFinalityArtifactBytes = 15_796
)

var canonicalParentSignerCounts = [...]int{3, 5, 7, 9, 11, 13, 15, 17, 19, 21}

type canonicalNoritoSection struct {
	choices []selectedNoritoBytes
}

func fixedNoritoSection(value []uints.U8) canonicalNoritoSection {
	return canonicalNoritoSection{
		choices: []selectedNoritoBytes{{selector: 1, bytes: value}},
	}
}

func appendNoritoSections(
	assembler *noritoAssembler,
	sections []canonicalNoritoSection,
) error {
	for _, section := range sections {
		if err := assembler.appendChoices(section.choices); err != nil {
			return err
		}
	}
	return nil
}

func noritoSectionsLength(
	api frontend.API,
	sections []canonicalNoritoSection,
) frontend.Variable {
	length := frontend.Variable(0)
	for _, section := range sections {
		sectionLength := frontend.Variable(0)
		for _, choice := range section.choices {
			sectionLength = api.Add(sectionLength, api.Mul(choice.selector, len(choice.bytes)))
		}
		length = api.Add(length, sectionLength)
	}
	return length
}

func compactLengthU14(api frontend.API, length frontend.Variable) ([]uints.U8, error) {
	bits := api.ToBinary(length, 14)
	low := api.FromBinary(bits[:7]...)
	high := api.FromBinary(bits[7:]...)
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	return []uints.U8{
		byteAPI.ValueOf(api.Add(low, 0x80)),
		byteAPI.ValueOf(high),
	}, nil
}

func parentSignerCountSelectors(
	api frontend.API,
	count frontend.Variable,
) []frontend.Variable {
	selectors := make([]frontend.Variable, len(canonicalParentSignerCounts))
	for index, allowed := range canonicalParentSignerCounts {
		selectors[index] = api.IsZero(api.Sub(count, allowed))
	}
	api.AssertIsEqual(sumVariables(api, selectors), 1)
	return selectors
}

func constrainCanonicalSignerIndices(
	api frontend.API,
	count frontend.Variable,
	rosterBound frontend.Variable,
	indices *[MaxValidators]frontend.Variable,
	bitmap *[MaxValidators]frontend.Variable,
) error {
	if _, err := u32Bytes(api, count); err != nil {
		return err
	}
	comparison := cmp.NewBoundedComparator(api, big.NewInt(MaxValidators+1), false)
	comparison.AssertIsLess(0, count)
	comparison.AssertIsLessEq(count, MaxValidators)
	for slot := 0; slot < MaxValidators; slot++ {
		active := comparison.IsLess(slot, count)
		if _, err := u32Bytes(api, indices[slot]); err != nil {
			return err
		}
		api.AssertIsEqual(indices[slot], api.Mul(active, indices[slot]))
		inRange := comparison.IsLess(indices[slot], rosterBound)
		api.AssertIsEqual(api.Mul(active, api.Sub(inRange, 1)), 0)
		if slot > 0 {
			strict := comparison.IsLess(indices[slot-1], indices[slot])
			api.AssertIsEqual(api.Mul(active, api.Sub(strict, 1)), 0)
		}
	}
	if bitmap == nil {
		return nil
	}
	for validator := 0; validator < MaxValidators; validator++ {
		found := frontend.Variable(0)
		for slot := 0; slot < MaxValidators; slot++ {
			active := comparison.IsLess(slot, count)
			found = api.Add(
				found,
				api.Mul(active, api.IsZero(api.Sub(indices[slot], validator))),
			)
		}
		api.AssertIsEqual(bitmap[validator], found)
	}
	return nil
}

func canonicalConsensusRoundFor(
	api frontend.API,
	contextID []uints.U8,
	height frontend.Variable,
	view frontend.Variable,
) ([]uints.U8, error) {
	context := noritoField(noritoHashField(contextID))
	heightField, err := noritoU64Field(api, height)
	if err != nil {
		return nil, err
	}
	viewField, err := noritoU64Field(api, view)
	if err != nil {
		return nil, err
	}
	body := append([]uints.U8(nil), context...)
	body = append(body, heightField...)
	body = append(body, viewField...)
	return body, nil
}

func canonicalSignerVectorBody(
	api frontend.API,
	indices *[MaxValidators]frontend.Variable,
	count int,
) ([]uints.U8, error) {
	body := constants([]byte{byte(count), 0, 0, 0, 0, 0, 0, 0})
	for index := 0; index < count; index++ {
		encoded, err := noritoU32Field(api, indices[index])
		if err != nil {
			return nil, err
		}
		body = append(body, encoded...)
	}
	return body, nil
}

func canonicalSignatureVectorBody(signature []uints.U8) []uints.U8 {
	if len(signature) != 96 {
		panic("BLS-normal signature must contain 96 bytes")
	}
	body := constants([]byte{96, 0, 0, 0, 0, 0, 0, 0})
	return append(body, signature...)
}

func canonicalQuorumCertificateBody(
	api frontend.API,
	finality *FinalityWitness,
	parent bool,
	executionVariant int,
	signerCount int,
) ([]uints.U8, error) {
	var contextID []uints.U8
	var height, roundView, proposalView frontend.Variable
	var subject []uints.U8
	var execution *ExecutionCommitmentWitness
	var indices *[MaxValidators]frontend.Variable
	var aggregate []uints.U8
	if parent {
		contextID = finality.ParentContextID[:]
		height = finality.ParentHeight
		roundView = finality.ParentRoundView
		proposalView = finality.ParentProposalView
		parentOption := append(
			constants([]byte{1}),
			noritoHashField(finality.ParentSubjectParentBlockHash[:])...,
		)
		subject = noritoField(parentOption)
		subject = append(subject, noritoHashField(finality.ParentBlockHash[:])...)
		subject = append(subject, noritoHashField(finality.ParentPayloadHash[:])...)
		execution = &finality.ParentExecution
		indices = &finality.ParentSignerIndices
		aggregate = finality.ParentAggregateSignature[:]
	} else {
		contextID = finality.HeightContextID[:]
		height = finality.Height
		roundView = finality.RoundView
		proposalView = finality.ProposalView
		subject = canonicalBlockSubject(finality)
		execution = &finality.Execution
		indices = &finality.SignerIndices
		aggregate = finality.AggregateSignature[:]
	}
	round, err := canonicalConsensusRoundFor(api, contextID, height, roundView)
	if err != nil {
		return nil, err
	}
	proposalRound, err := canonicalConsensusRoundFor(api, contextID, height, proposalView)
	if err != nil {
		return nil, err
	}
	phase, err := noritoU32Field(api, 2)
	if err != nil {
		return nil, err
	}
	executionBody, err := canonicalExecutionCommitmentBody(api, execution, executionVariant)
	if err != nil {
		return nil, err
	}
	signers, err := canonicalSignerVectorBody(api, indices, signerCount)
	if err != nil {
		return nil, err
	}
	body := noritoField(round)
	body = append(body, noritoField(proposalRound)...)
	body = append(body, phase...)
	body = append(body, noritoField(subject)...)
	body = append(body, noritoField(executionBody)...)
	body = append(body, noritoField(signers)...)
	body = append(body, noritoField(canonicalSignatureVectorBody(aggregate))...)
	return body, nil
}

func nextSnapshotOptionChoices(
	api frontend.API,
	finality *FinalityWitness,
) ([]selectedNoritoBytes, error) {
	choices := []selectedNoritoBytes{{
		selector: api.Sub(1, finality.HasNextEpochSnapshot),
		bytes:    noritoField(constants([]byte{0})),
	}}
	for _, count := range canonicalCommitteeSizes {
		body, err := canonicalNextEpochSnapshotBody(api, &finality.NextEpochSnapshot, count)
		if err != nil {
			return nil, err
		}
		option := append(constants([]byte{1}), noritoField(body)...)
		choices = append(choices, selectedNoritoBytes{
			selector: api.Mul(
				finality.HasNextEpochSnapshot,
				api.IsZero(api.Sub(finality.NextEpochSnapshot.ValidatorCount, count)),
			),
			bytes: noritoField(option),
		})
	}
	return choices, nil
}

func rosterFieldChoices(
	api frontend.API,
	finality *FinalityWitness,
	countSelectors []frontend.Variable,
) ([]selectedNoritoBytes, error) {
	choices := make([]selectedNoritoBytes, len(canonicalCommitteeSizes))
	for index, count := range canonicalCommitteeSizes {
		roster, err := canonicalRosterVectorBody(api, &finality.ValidatorPublicKeys, count)
		if err != nil {
			return nil, err
		}
		choices[index] = selectedNoritoBytes{
			selector: countSelectors[index],
			bytes:    noritoField(roster),
		}
	}
	return choices, nil
}

func quorumFieldChoices(
	api frontend.API,
	countSelectors []frontend.Variable,
) ([]selectedNoritoBytes, error) {
	choices := make([]selectedNoritoBytes, len(canonicalCommitteeSizes))
	for index, count := range canonicalCommitteeSizes {
		quorum, err := canonicalDualQuorumBody(api, count)
		if err != nil {
			return nil, err
		}
		choices[index] = selectedNoritoBytes{
			selector: countSelectors[index],
			bytes:    noritoField(quorum),
		}
	}
	return choices, nil
}

func parentQuorumCertificateOptionChoices(
	api frontend.API,
	finality *FinalityWitness,
	parentCountSelectors []frontend.Variable,
) ([]selectedNoritoBytes, error) {
	choices := make([]selectedNoritoBytes, 0, 8*len(canonicalParentSignerCounts))
	for executionVariant := 0; executionVariant < 8; executionVariant++ {
		executionSelector := optionVariantSelector(api, &finality.ParentExecution, executionVariant)
		for countIndex, signerCount := range canonicalParentSignerCounts {
			body, err := canonicalQuorumCertificateBody(
				api,
				finality,
				true,
				executionVariant,
				signerCount,
			)
			if err != nil {
				return nil, err
			}
			option := append(constants([]byte{1}), noritoField(body)...)
			choices = append(choices, selectedNoritoBytes{
				selector: api.Mul(executionSelector, parentCountSelectors[countIndex]),
				bytes:    noritoField(option),
			})
		}
	}
	return choices, nil
}

func canonicalHeightContextSections(
	api frontend.API,
	finality *FinalityWitness,
) ([]canonicalNoritoSection, error) {
	protocol, err := noritoU16Field(api, sumeragiProtocolVersion)
	if err != nil {
		return nil, err
	}
	height, err := noritoU64Field(api, finality.Height)
	if err != nil {
		return nil, err
	}
	epoch, err := noritoU64Field(api, finality.Epoch)
	if err != nil {
		return nil, err
	}
	epochEnd, err := noritoU64Field(api, finality.EpochEndHeight)
	if err != nil {
		return nil, err
	}
	mode, err := noritoU32Field(api, finality.Mode)
	if err != nil {
		return nil, err
	}
	snapshotChoices, err := nextSnapshotOptionChoices(api, finality)
	if err != nil {
		return nil, err
	}
	parentCountSelectors := parentSignerCountSelectors(api, finality.ParentSignerCount)
	parentChoices, err := parentQuorumCertificateOptionChoices(api, finality, parentCountSelectors)
	if err != nil {
		return nil, err
	}
	countSelectors := committeeSizeSelectors(api, finality.ValidatorCount)
	rosterChoices, err := rosterFieldChoices(api, finality, countSelectors)
	if err != nil {
		return nil, err
	}
	quorumChoices, err := quorumFieldChoices(api, countSelectors)
	if err != nil {
		return nil, err
	}
	daLayout, err := canonicalDataAvailabilityLayoutBody(api, &finality.DALayout)
	if err != nil {
		return nil, err
	}
	return []canonicalNoritoSection{
		fixedNoritoSection(noritoHashField(constants(tairaFinalityNetworkID[:]))),
		fixedNoritoSection(protocol),
		fixedNoritoSection(height),
		fixedNoritoSection(epoch),
		fixedNoritoSection(epochEnd),
		{choices: snapshotChoices},
		fixedNoritoSection(mode),
		{choices: parentChoices},
		fixedNoritoSection(noritoField(constants([]byte{0}))),
		{choices: rosterChoices},
		{choices: quorumChoices},
		fixedNoritoSection(noritoHashField(finality.NexusAMXContextHash[:])),
		fixedNoritoSection(noritoHashField(finality.ExecutionPolicyHash[:])),
		fixedNoritoSection(noritoField(daLayout)),
		fixedNoritoSection(noritoField(finality.LeaderSeed[:])),
	}, nil
}

func currentQuorumCertificateChoices(
	api frontend.API,
	finality *FinalityWitness,
) ([]selectedNoritoBytes, error) {
	countSelectors := committeeSizeSelectors(api, finality.ValidatorCount)
	choices := make([]selectedNoritoBytes, 0, 8*len(canonicalCommitteeSizes))
	for executionVariant := 0; executionVariant < 8; executionVariant++ {
		executionSelector := optionVariantSelector(api, &finality.Execution, executionVariant)
		for countIndex, validatorCount := range canonicalCommitteeSizes {
			signerCount := 2*((validatorCount-1)/3) + 1
			body, err := canonicalQuorumCertificateBody(
				api,
				finality,
				false,
				executionVariant,
				signerCount,
			)
			if err != nil {
				return nil, err
			}
			choices = append(choices, selectedNoritoBytes{
				selector: api.Mul(executionSelector, countSelectors[countIndex]),
				bytes:    noritoField(body),
			})
		}
	}
	return choices, nil
}

func currentPoPFieldChoices(
	api frontend.API,
	finality *FinalityWitness,
) []selectedNoritoBytes {
	countSelectors := committeeSizeSelectors(api, finality.ValidatorCount)
	choices := make([]selectedNoritoBytes, len(canonicalCommitteeSizes))
	for index, count := range canonicalCommitteeSizes {
		body := constants([]byte{byte(count), 0, 0, 0, 0, 0, 0, 0})
		for proofIndex := 0; proofIndex < count; proofIndex++ {
			proof := constants([]byte{96, 0, 0, 0, 0, 0, 0, 0})
			proof = append(proof, finality.ValidatorPoPs[proofIndex][:]...)
			body = append(body, noritoField(proof)...)
		}
		choices[index] = selectedNoritoBytes{
			selector: countSelectors[index],
			bytes:    noritoField(body),
		}
	}
	return choices
}

func canonicalFinalityArtifactSections(
	api frontend.API,
	finality *FinalityWitness,
) ([]canonicalNoritoSection, error) {
	contextSections, err := canonicalHeightContextSections(api, finality)
	if err != nil {
		return nil, err
	}
	contextLength := noritoSectionsLength(api, contextSections)
	contextPrefix, err := compactLengthU14(api, contextLength)
	if err != nil {
		return nil, err
	}
	formatVersion, err := noritoU16Field(api, sumeragiFinalityFormatVersion)
	if err != nil {
		return nil, err
	}
	protocol, err := noritoU16Field(api, sumeragiProtocolVersion)
	if err != nil {
		return nil, err
	}
	height, err := noritoU64Field(api, finality.Height)
	if err != nil {
		return nil, err
	}
	certificateChoices, err := currentQuorumCertificateChoices(api, finality)
	if err != nil {
		return nil, err
	}
	sections := []canonicalNoritoSection{
		fixedNoritoSection(formatVersion),
		fixedNoritoSection(protocol),
		fixedNoritoSection(height),
		fixedNoritoSection(contextPrefix),
	}
	sections = append(sections, contextSections...)
	sections = append(sections,
		fixedNoritoSection(noritoField(canonicalBlockSubject(finality))),
		fixedNoritoSection(noritoHashField(finality.BlockHeaderHash[:])),
		canonicalNoritoSection{choices: certificateChoices},
		canonicalNoritoSection{choices: currentPoPFieldChoices(api, finality)},
	)
	return sections, nil
}

type rollingNoritoCommitment struct {
	api         frontend.API
	byteAPI     *uints.Bytes
	challenge   frontend.Variable
	accumulator frontend.Variable
	length      frontend.Variable
}

func newRollingNoritoCommitment(
	api frontend.API,
	challenge frontend.Variable,
) (*rollingNoritoCommitment, error) {
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	return &rollingNoritoCommitment{
		api:         api,
		byteAPI:     byteAPI,
		challenge:   challenge,
		accumulator: 1,
		length:      0,
	}, nil
}

func (commitment *rollingNoritoCommitment) appendChoices(
	choices []selectedNoritoBytes,
) {
	selectors := make([]frontend.Variable, len(choices))
	nextAccumulator := frontend.Variable(0)
	selectedLength := frontend.Variable(0)
	for index, choice := range choices {
		selectors[index] = choice.selector
		candidate := commitment.accumulator
		for _, value := range choice.bytes {
			candidate = commitment.api.Add(
				commitment.api.Mul(candidate, commitment.challenge),
				commitment.byteAPI.Value(value),
			)
		}
		nextAccumulator = commitment.api.Add(
			nextAccumulator,
			commitment.api.Mul(choice.selector, candidate),
		)
		selectedLength = commitment.api.Add(
			selectedLength,
			commitment.api.Mul(choice.selector, len(choice.bytes)),
		)
	}
	commitment.api.AssertIsEqual(sumVariables(commitment.api, selectors), 1)
	commitment.accumulator = materializeNoritoVariable(commitment.api, nextAccumulator)
	commitment.length = commitment.api.Add(commitment.length, selectedLength)
}

func canonicalRollingNoritoCommitment(
	api frontend.API,
	challenge frontend.Variable,
	sections []canonicalNoritoSection,
) (frontend.Variable, frontend.Variable, error) {
	commitment, err := newRollingNoritoCommitment(api, challenge)
	if err != nil {
		return nil, nil, err
	}
	for _, section := range sections {
		commitment.appendChoices(section.choices)
	}
	return commitment.accumulator, commitment.length, nil
}

func rawRollingNoritoCommitment(
	api frontend.API,
	challenge frontend.Variable,
	raw []uints.U8,
	length frontend.Variable,
) (frontend.Variable, error) {
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	comparison := cmp.NewBoundedComparator(api, big.NewInt(int64(len(raw)+1)), false)
	comparison.AssertIsLess(0, length)
	comparison.AssertIsLessEq(length, len(raw))
	accumulator := frontend.Variable(1)
	for index := range raw {
		active := comparison.IsLess(index, length)
		value := byteAPI.Value(raw[index])
		api.AssertIsEqual(value, api.Mul(active, value))
		next := api.Add(api.Mul(accumulator, challenge), value)
		accumulator = api.Select(active, next, accumulator)
	}
	return accumulator, nil
}

func finalityArtifactChallenge(
	api frontend.API,
	finality *FinalityWitness,
	round byte,
) (frontend.Variable, error) {
	preimage := constants([]byte("sccp:final-v1:norito-artifact-equality:v1"))
	preimage = append(preimage, uints.NewU8(round))
	preimage = append(preimage, finality.FinalityArtifactHash[:]...)
	preimage = append(preimage, finality.HeightContextID[:]...)
	preimage = append(preimage, finality.VotePreimageHash[:]...)
	preimage = append(preimage, finality.AggregateSignatureHash[:]...)
	digest, err := hashFixed(api, profile.SHA256Signal, preimage)
	if err != nil {
		return nil, err
	}
	challenge, err := bytesToFieldBE(api, digest[:31])
	if err != nil {
		return nil, err
	}
	api.AssertIsEqual(api.IsZero(challenge), 0)
	api.AssertIsEqual(api.IsZero(api.Sub(challenge, 1)), 0)
	return challenge, nil
}

// canonicalFinalityArtifactHash hashes caller-supplied exact artifact bytes and
// proves, with two transcript-derived polynomial identities, that those bytes
// equal the canonical bare-Norito V2FinalityArtifact v4 encoding assembled from
// the structured witness.  Each Fiat-Shamir challenge commits the BLAKE2b
// artifact hash and authenticated context/vote, so it is not witness-supplied
// randomness; the degree bound is 15,796 over at least a 248-bit challenge.
func canonicalFinalityArtifactHash(
	api frontend.API,
	finality *FinalityWitness,
) ([32]uints.U8, error) {
	if _, err := u32Bytes(api, finality.FinalityArtifactLength); err != nil {
		return [32]uints.U8{}, err
	}
	sections, err := canonicalFinalityArtifactSections(api, finality)
	if err != nil {
		return [32]uints.U8{}, err
	}
	for round := byte(0); round < 2; round++ {
		challenge, err := finalityArtifactChallenge(api, finality, round)
		if err != nil {
			return [32]uints.U8{}, err
		}
		canonical, canonicalLength, err := canonicalRollingNoritoCommitment(
			api,
			challenge,
			sections,
		)
		if err != nil {
			return [32]uints.U8{}, err
		}
		raw, err := rawRollingNoritoCommitment(
			api,
			challenge,
			finality.FinalityArtifactBytes[:],
			finality.FinalityArtifactLength,
		)
		if err != nil {
			return [32]uints.U8{}, err
		}
		api.AssertIsEqual(finality.FinalityArtifactLength, canonicalLength)
		api.AssertIsEqual(raw, canonical)
	}
	digest, err := blake2b256(
		api,
		finality.FinalityArtifactBytes[:],
		finality.FinalityArtifactLength,
	)
	if err != nil {
		return [32]uints.U8{}, fmt.Errorf("hash exact V2FinalityArtifact v4: %w", err)
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return [32]uints.U8{}, err
	}
	digest[31] = byteAPI.Or(digest[31], uints.NewU8(1))
	return digest, nil
}
