package circuit

import (
	"fmt"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
)

var sumeragiVoteSignatureDomain = []byte("iroha:sumeragi:v2:vote")

type canonicalVoteEncoding struct {
	payloadVariants [8][]uints.U8
	selectors       [8]frontend.Variable
}

func constrainCanonicalCommitVote(
	api frontend.API,
	finality *FinalityWitness,
) (*canonicalVoteEncoding, error) {
	if err := constrainExecutionCommitment(api, &finality.Execution); err != nil {
		return nil, err
	}
	encoding, err := canonicalCommitVoteEncoding(api, finality)
	if err != nil {
		return nil, err
	}

	if _, err := u32Bytes(api, finality.VoteSignaturePayloadLength); err != nil {
		return nil, err
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return nil, fmt.Errorf("initialize canonical vote byte API: %w", err)
	}
	expectedLength := frontend.Variable(0)
	for variant := range encoding.payloadVariants {
		expectedLength = api.Add(
			expectedLength,
			api.Mul(encoding.selectors[variant], len(encoding.payloadVariants[variant])),
		)
	}
	api.AssertIsEqual(finality.VoteSignaturePayloadLength, expectedLength)
	for byteIndex := 0; byteIndex < maxVoteSignaturePayloadBytes; byteIndex++ {
		expected := frontend.Variable(0)
		for variant, payload := range encoding.payloadVariants {
			if byteIndex < len(payload) {
				expected = api.Add(
					expected,
					api.Mul(encoding.selectors[variant], byteAPI.Value(payload[byteIndex])),
				)
			}
		}
		api.AssertIsEqual(byteAPI.Value(finality.VoteSignaturePayload[byteIndex]), expected)
	}

	preimage := constants(sumeragiVoteSignatureDomain)
	preimage = append(preimage, finality.VoteSignaturePayload[:]...)
	digest, err := blake2b256(
		api,
		preimage,
		api.Add(len(sumeragiVoteSignatureDomain), finality.VoteSignaturePayloadLength),
	)
	if err != nil {
		return nil, fmt.Errorf("hash exact canonical vote preimage: %w", err)
	}
	byteAPI, err = uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	digest[31] = byteAPI.Or(digest[31], uints.NewU8(1))
	if err := assertBytesEqual(api, digest[:], finality.VotePreimageHash[:]); err != nil {
		return nil, err
	}
	return encoding, nil
}

func canonicalCommitVoteEncoding(
	api frontend.API,
	finality *FinalityWitness,
) (*canonicalVoteEncoding, error) {
	var result canonicalVoteEncoding
	for variant := range result.payloadVariants {
		execution, err := canonicalExecutionCommitmentBody(api, &finality.Execution, variant)
		if err != nil {
			return nil, err
		}
		payload, err := canonicalVoteSignaturePayload(api, finality, execution)
		if err != nil {
			return nil, err
		}
		if len(payload) > maxVoteSignaturePayloadBytes {
			return nil, fmt.Errorf(
				"canonical vote payload variant %d is %d bytes, maximum is %d",
				variant,
				len(payload),
				maxVoteSignaturePayloadBytes,
			)
		}
		result.payloadVariants[variant] = payload
		result.selectors[variant] = optionVariantSelector(api, &finality.Execution, variant)
	}
	api.AssertIsEqual(sumVariables(api, result.selectors[:]), 1)
	return &result, nil
}

func canonicalExecutionCommitmentHash(
	api frontend.API,
	execution *ExecutionCommitmentWitness,
) ([32]uints.U8, error) {
	var digests [8][32]uints.U8
	var selectors [8]frontend.Variable
	for variant := range digests {
		body, err := canonicalExecutionCommitmentBody(api, execution, variant)
		if err != nil {
			return [32]uints.U8{}, err
		}
		digests[variant], err = irohaBlake2bHash(api, body)
		if err != nil {
			return [32]uints.U8{}, err
		}
		selectors[variant] = optionVariantSelector(api, execution, variant)
	}
	api.AssertIsEqual(sumVariables(api, selectors[:]), 1)
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return [32]uints.U8{}, err
	}
	var selected [32]uints.U8
	for byteIndex := range selected {
		value := frontend.Variable(0)
		for variant := range digests {
			value = api.Add(
				value,
				api.Mul(selectors[variant], byteAPI.Value(digests[variant][byteIndex])),
			)
		}
		selected[byteIndex] = byteAPI.ValueOf(value)
	}
	return selected, nil
}

func optionVariantSelector(
	api frontend.API,
	execution *ExecutionCommitmentWitness,
	variant int,
) frontend.Variable {
	selector := frontend.Variable(1)
	for bit, present := range []frontend.Variable{
		execution.HasKagemushaTopUpRoot,
		execution.HasLaneFinalityManifest,
		execution.HasMergeCarrier,
	} {
		if variant&(1<<bit) == 0 {
			selector = api.Mul(selector, api.Sub(1, present))
		} else {
			selector = api.Mul(selector, present)
		}
	}
	api.AssertIsBoolean(selector)
	return selector
}

func canonicalVoteSignaturePayload(
	api frontend.API,
	finality *FinalityWitness,
	executionBody []uints.U8,
) ([]uints.U8, error) {
	protocol, err := noritoU16Field(api, sumeragiProtocolVersion)
	if err != nil {
		return nil, err
	}
	round, err := canonicalConsensusRound(api, finality, finality.RoundView)
	if err != nil {
		return nil, err
	}
	proposalRound, err := canonicalConsensusRound(api, finality, finality.ProposalView)
	if err != nil {
		return nil, err
	}
	phase, err := noritoU32Field(api, 2)
	if err != nil {
		return nil, err
	}
	subject := canonicalBlockSubject(finality)
	payload := append([]uints.U8(nil), protocol...)
	payload = append(payload, noritoField(round)...)
	payload = append(payload, noritoField(proposalRound)...)
	payload = append(payload, phase...)
	payload = append(payload, noritoField(subject)...)
	payload = append(payload, noritoField(executionBody)...)
	return payload, nil
}

func canonicalConsensusRound(
	api frontend.API,
	finality *FinalityWitness,
	view frontend.Variable,
) ([]uints.U8, error) {
	// HeightContextId is a transparent HashOf<HeightContext>; both wrappers are
	// visible as the outer struct field and the inner Hash field respectively.
	contextID := noritoField(noritoHashField(finality.HeightContextID[:]))
	height, err := noritoU64Field(api, finality.Height)
	if err != nil {
		return nil, err
	}
	viewBytes, err := noritoU64Field(api, view)
	if err != nil {
		return nil, err
	}
	body := append([]uints.U8(nil), contextID...)
	body = append(body, height...)
	body = append(body, viewBytes...)
	return body, nil
}

func canonicalBlockSubject(finality *FinalityWitness) []uints.U8 {
	parent := append(constants([]byte{1}), noritoHashField(finality.SubjectParentBlockHash[:])...)
	body := noritoField(parent)
	body = append(body, noritoHashField(finality.BlockHeaderHash[:])...)
	body = append(body, noritoHashField(finality.SubjectPayloadHash[:])...)
	return body
}

func canonicalExecutionCommitmentBody(
	api frontend.API,
	execution *ExecutionCommitmentWitness,
	variant int,
) ([]uints.U8, error) {
	body := noritoHashField(execution.ParentStateRoot[:])
	body = append(body, noritoHashField(execution.PostStateRoot[:])...)
	body = append(body, noritoHashField(execution.OrdinaryWritesRoot[:])...)

	if variant&1 != 0 {
		option := append(constants([]byte{1}), noritoHashField(execution.KagemushaTopUpRoot[:])...)
		body = append(body, noritoField(option)...)
	} else {
		body = append(body, noritoField(constants([]byte{0}))...)
	}
	kagemushaTopUpCount, err := noritoU32Field(api, execution.KagemushaTopUpCount)
	if err != nil {
		return nil, err
	}
	body = append(body, kagemushaTopUpCount...)
	nativeVersion, err := noritoU16Field(api, execution.NativeAMXApplicationManifestVer)
	if err != nil {
		return nil, err
	}
	body = append(body, nativeVersion...)
	body = append(body, noritoHashField(execution.NativeAMXApplicationManifestRoot[:])...)
	nativeCount, err := noritoU32Field(api, execution.NativeAMXApplicationManifestCount)
	if err != nil {
		return nil, err
	}
	body = append(body, nativeCount...)

	if variant&2 != 0 {
		leafCount, err := noritoU64Field(api, execution.LaneFinalityManifestLeafCount)
		if err != nil {
			return nil, err
		}
		commitment := noritoHashField(execution.LaneFinalityManifestRoot[:])
		commitment = append(commitment, leafCount...)
		option := append(constants([]byte{1}), noritoField(commitment)...)
		body = append(body, noritoField(option)...)
	} else {
		body = append(body, noritoField(constants([]byte{0}))...)
	}

	if variant&4 != 0 {
		version, err := noritoU16Field(api, execution.MergeCarrierVersion)
		if err != nil {
			return nil, err
		}
		carrier := append([]uints.U8(nil), version...)
		carrier = append(carrier, noritoHashField(execution.MergeCarrierEntryHash[:])...)
		option := append(constants([]byte{1}), noritoField(carrier)...)
		body = append(body, noritoField(option)...)
	} else {
		body = append(body, noritoField(constants([]byte{0}))...)
	}

	wireLength, err := noritoU64Field(api, execution.ExecutedBlockWireLength)
	if err != nil {
		return nil, err
	}
	body = append(body, wireLength...)
	body = append(body, noritoHashField(execution.ExecutedBlockWireHash[:])...)
	return body, nil
}

func nativeCanonicalExecutionCommitmentBody(execution ExecutionCommitmentWitness) []byte {
	value32 := func(value [32]uints.U8) [32]byte { return u8Array32(value) }
	body := nativeNoritoHash(value32(execution.ParentStateRoot))
	body = append(body, nativeNoritoHash(value32(execution.PostStateRoot))...)
	body = append(body, nativeNoritoHash(value32(execution.OrdinaryWritesRoot))...)
	if execution.HasKagemushaTopUpRoot.(int) == 1 {
		option := append([]byte{1}, nativeNoritoHash(value32(execution.KagemushaTopUpRoot))...)
		body = append(body, nativeNoritoField(option)...)
	} else {
		body = append(body, nativeNoritoField([]byte{0})...)
	}
	body = append(body, nativeNoritoU32(uint32(execution.KagemushaTopUpCount.(int)))...)
	body = append(body, nativeNoritoU16(uint16(execution.NativeAMXApplicationManifestVer.(int)))...)
	body = append(body, nativeNoritoHash(value32(execution.NativeAMXApplicationManifestRoot))...)
	body = append(body, nativeNoritoU32(uint32(execution.NativeAMXApplicationManifestCount.(int)))...)
	if execution.HasLaneFinalityManifest.(int) == 1 {
		commitment := nativeNoritoHash(value32(execution.LaneFinalityManifestRoot))
		commitment = append(
			commitment,
			nativeNoritoU64(uint64(execution.LaneFinalityManifestLeafCount.(int)))...,
		)
		option := append([]byte{1}, nativeNoritoField(commitment)...)
		body = append(body, nativeNoritoField(option)...)
	} else {
		body = append(body, nativeNoritoField([]byte{0})...)
	}
	if execution.HasMergeCarrier.(int) == 1 {
		carrier := nativeNoritoU16(uint16(execution.MergeCarrierVersion.(int)))
		carrier = append(carrier, nativeNoritoHash(value32(execution.MergeCarrierEntryHash))...)
		option := append([]byte{1}, nativeNoritoField(carrier)...)
		body = append(body, nativeNoritoField(option)...)
	} else {
		body = append(body, nativeNoritoField([]byte{0})...)
	}
	body = append(body, nativeNoritoU64(uint64(execution.ExecutedBlockWireLength.(int)))...)
	body = append(body, nativeNoritoHash(value32(execution.ExecutedBlockWireHash))...)
	return body
}

func nativeCanonicalVoteSignaturePayload(finality FinalityWitness) []byte {
	context := u8Array32(finality.HeightContextID)
	round := func(view uint64) []byte {
		body := nativeNoritoField(nativeNoritoHash(context))
		body = append(body, nativeNoritoU64(uint64(finality.Height.(int)))...)
		body = append(body, nativeNoritoU64(view)...)
		return nativeNoritoField(body)
	}
	parent := u8Array32(finality.SubjectParentBlockHash)
	block := u8Array32(finality.BlockHeaderHash)
	payloadHash := u8Array32(finality.SubjectPayloadHash)
	parentOption := append([]byte{1}, nativeNoritoHash(parent)...)
	subject := nativeNoritoField(parentOption)
	subject = append(subject, nativeNoritoHash(block)...)
	subject = append(subject, nativeNoritoHash(payloadHash)...)

	payload := nativeNoritoU16(sumeragiProtocolVersion)
	payload = append(payload, round(uint64(finality.RoundView.(int)))...)
	payload = append(payload, round(uint64(finality.ProposalView.(int)))...)
	payload = append(payload, nativeNoritoU32(2)...)
	payload = append(payload, nativeNoritoField(subject)...)
	payload = append(
		payload,
		nativeNoritoField(nativeCanonicalExecutionCommitmentBody(finality.Execution))...,
	)
	return payload
}
