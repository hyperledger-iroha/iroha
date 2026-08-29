package circuit

import (
	"fmt"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
)

const (
	blockHeaderConsensusProjectionVersion = 1
	// Exact largest bare-Norito BlockHeaderConsensusProjectionV1 encoding:
	// every optional 32-byte commitment and every confidential-feature field
	// is present. The tight bound keeps the variable BLAKE2b circuit finite.
	maxBlockHeaderConsensusProjectionBytes = 404
)

// OptionalHeaderHashWitness is one explicit Option<HashOf<_>> in the
// consensus block-header projection. Inactive bytes must be zero.
type OptionalHeaderHashWitness struct {
	Present frontend.Variable
	Value   [32]uints.U8
}

// OptionalHeaderU32Witness is one explicit Option<u32> in the confidential
// feature digest. Inactive values must be zero.
type OptionalHeaderU32Witness struct {
	Present frontend.Variable
	Value   frontend.Variable
}

// ConfidentialFeatureDigestWitness is the exact wire shape used by the
// block-header consensus projection.
type ConfidentialFeatureDigestWitness struct {
	VKSetHash      OptionalHeaderHashWitness
	PoseidonParams OptionalHeaderU32Witness
	PedersenParams OptionalHeaderU32Witness
	RulesVersion   OptionalHeaderU32Witness
	ZKPolicyHash   OptionalHeaderHashWitness
}

// BlockHeaderProjectionWitness supplies every semantic field in
// BlockHeaderConsensusProjectionV1. result_merkle_root is intentionally absent:
// Iroha excludes execution results from the consensus header hash.
type BlockHeaderProjectionWitness struct {
	Height                   frontend.Variable
	PreviousBlockHash        OptionalHeaderHashWitness
	ExternalEntrypointRoot   OptionalHeaderHashWitness
	DAProofPoliciesHash      OptionalHeaderHashWitness
	DACommitmentsHash        OptionalHeaderHashWitness
	DAPinIntentsHash         OptionalHeaderHashWitness
	NPoSEffectsHash          OptionalHeaderHashWitness
	ExecutionContextHash     OptionalHeaderHashWitness
	CreationTimeMilliseconds frontend.Variable
	ViewChangeIndex          frontend.Variable
	HasConfidentialFeatures  frontend.Variable
	ConfidentialFeatures     ConfidentialFeatureDigestWitness
}

// constrainBlockHeaderCommitment proves that the exact SCCP root is a field of
// the canonical consensus projection whose Iroha hash the CommitQC signs. This
// prevents combining a valid message tree with an unrelated finalized block.
func (c *MessageCircuit) constrainBlockHeaderCommitment(api frontend.API) error {
	header := &c.BlockHeader
	api.AssertIsEqual(header.Height, c.Finality.Height)
	api.AssertIsEqual(header.ViewChangeIndex, c.Finality.BlockHeaderView)
	if _, err := u64Bytes(api, header.Height); err != nil {
		return err
	}
	if _, err := u64Bytes(api, header.CreationTimeMilliseconds); err != nil {
		return err
	}
	if _, err := u64Bytes(api, header.ViewChangeIndex); err != nil {
		return err
	}

	for _, field := range []struct {
		name     string
		optional *OptionalHeaderHashWitness
	}{
		{"previous block", &header.PreviousBlockHash},
		{"external entrypoints", &header.ExternalEntrypointRoot},
		{"DA proof policies", &header.DAProofPoliciesHash},
		{"DA commitments", &header.DACommitmentsHash},
		{"DA pin intents", &header.DAPinIntentsHash},
		{"NPoS effects", &header.NPoSEffectsHash},
		{"execution context", &header.ExecutionContextHash},
	} {
		if err := constrainOptionalHeaderHash(api, field.optional); err != nil {
			return fmt.Errorf("%s header commitment: %w", field.name, err)
		}
	}
	// SCCP finality proofs are never genesis proofs and Core requires an
	// external-entrypoint tree whenever an SCCP root is present.
	api.AssertIsEqual(header.PreviousBlockHash.Present, 1)
	api.AssertIsEqual(header.ExternalEntrypointRoot.Present, 1)
	if err := assertBytesEqual(
		api,
		header.PreviousBlockHash.Value[:],
		c.Finality.SubjectParentBlockHash[:],
	); err != nil {
		return err
	}

	confidential, err := constrainConfidentialFeatureDigest(api, header)
	if err != nil {
		return err
	}
	projection, length, err := canonicalBlockHeaderConsensusProjection(
		api,
		header,
		c.RawSignals[3],
		confidential,
	)
	if err != nil {
		return err
	}
	digest, err := blake2b256(api, projection, length)
	if err != nil {
		return fmt.Errorf("hash canonical block-header consensus projection: %w", err)
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	digest[31] = byteAPI.Or(digest[31], uints.NewU8(1))
	return assertBytesEqual(api, digest[:], c.Finality.BlockHeaderHash[:])
}

func constrainOptionalHeaderHash(api frontend.API, value *OptionalHeaderHashWitness) error {
	api.AssertIsBoolean(value.Present)
	return canonicalActiveIrohaHash(api, value.Present, value.Value[:])
}

func constrainOptionalRawHeaderBytes(
	api frontend.API,
	present frontend.Variable,
	value []uints.U8,
) error {
	api.AssertIsBoolean(present)
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	for index := range value {
		api.AssertIsEqual(
			byteAPI.Value(value[index]),
			api.Mul(present, byteAPI.Value(value[index])),
		)
	}
	return nil
}

func constrainOptionalHeaderU32(api frontend.API, value *OptionalHeaderU32Witness) error {
	api.AssertIsBoolean(value.Present)
	if _, err := u32Bytes(api, value.Value); err != nil {
		return err
	}
	api.AssertIsEqual(value.Value, api.Mul(value.Present, value.Value))
	return nil
}

func constrainConfidentialFeatureDigest(
	api frontend.API,
	header *BlockHeaderProjectionWitness,
) ([]selectedNoritoBytes, error) {
	api.AssertIsBoolean(header.HasConfidentialFeatures)
	confidential := &header.ConfidentialFeatures
	for _, value := range []*OptionalHeaderHashWitness{
		&confidential.VKSetHash,
		&confidential.ZKPolicyHash,
	} {
		api.AssertIsEqual(value.Present, api.Mul(header.HasConfidentialFeatures, value.Present))
		if err := constrainOptionalRawHeaderBytes(api, value.Present, value.Value[:]); err != nil {
			return nil, err
		}
	}
	for _, value := range []*OptionalHeaderU32Witness{
		&confidential.PoseidonParams,
		&confidential.PedersenParams,
		&confidential.RulesVersion,
	} {
		api.AssertIsEqual(value.Present, api.Mul(header.HasConfidentialFeatures, value.Present))
		if err := constrainOptionalHeaderU32(api, value); err != nil {
			return nil, err
		}
	}

	poseidon, err := noritoU32Field(api, confidential.PoseidonParams.Value)
	if err != nil {
		return nil, err
	}
	pedersen, err := noritoU32Field(api, confidential.PedersenParams.Value)
	if err != nil {
		return nil, err
	}
	rules, err := noritoU32Field(api, confidential.RulesVersion.Value)
	if err != nil {
		return nil, err
	}
	fields := []struct {
		present frontend.Variable
		encoded []uints.U8
	}{
		{confidential.VKSetHash.Present, noritoHashField(confidential.VKSetHash.Value[:])},
		{confidential.PoseidonParams.Present, poseidon},
		{confidential.PedersenParams.Present, pedersen},
		{confidential.RulesVersion.Present, rules},
		{confidential.ZKPolicyHash.Present, noritoHashField(confidential.ZKPolicyHash.Value[:])},
	}
	choices := []selectedNoritoBytes{{
		selector: api.Sub(1, header.HasConfidentialFeatures),
		bytes:    noritoField(constants([]byte{0})),
	}}
	// The nested digest contains five independently optional fields. Enumerate
	// their 32 canonical wire shapes so the outer Option length remains exact;
	// every selector is derived from constrained boolean presence bits.
	for mask := 0; mask < 1<<len(fields); mask++ {
		selector := frontend.Variable(header.HasConfidentialFeatures)
		body := make([]uints.U8, 0, 91)
		for index, field := range fields {
			present := mask&(1<<index) != 0
			if present {
				selector = api.Mul(selector, field.present)
			} else {
				selector = api.Mul(selector, api.Sub(1, field.present))
			}
			body = append(body, canonicalNoritoOptionBytes(present, field.encoded)...)
		}
		someBody := append(constants([]byte{1}), noritoField(body)...)
		choices = append(choices, selectedNoritoBytes{
			selector: selector,
			bytes:    noritoField(someBody),
		})
	}
	return choices, nil
}

func canonicalNoritoOptionBytes(present bool, encodedValue []uints.U8) []uints.U8 {
	if !present {
		return noritoField(constants([]byte{0}))
	}
	return noritoField(append(constants([]byte{1}), encodedValue...))
}

func canonicalNoritoOption(
	api frontend.API,
	present frontend.Variable,
	encodedValue []uints.U8,
) []selectedNoritoBytes {
	someBody := append(constants([]byte{1}), encodedValue...)
	return []selectedNoritoBytes{
		{selector: api.Sub(1, present), bytes: noritoField(constants([]byte{0}))},
		{selector: present, bytes: noritoField(someBody)},
	}
}

func appendCanonicalHeaderOption(
	assembler *noritoAssembler,
	api frontend.API,
	value *OptionalHeaderHashWitness,
) error {
	return assembler.appendChoices(canonicalNoritoOption(
		api,
		value.Present,
		noritoHashField(value.Value[:]),
	))
}

func canonicalBlockHeaderConsensusProjection(
	api frontend.API,
	header *BlockHeaderProjectionWitness,
	sccpRoot [32]uints.U8,
	confidentialChoices []selectedNoritoBytes,
) ([]uints.U8, frontend.Variable, error) {
	assembler, err := newNoritoAssembler(api, maxBlockHeaderConsensusProjectionBytes)
	if err != nil {
		return nil, nil, err
	}
	version, err := noritoU16Field(api, blockHeaderConsensusProjectionVersion)
	if err != nil {
		return nil, nil, err
	}
	height, err := noritoU64Field(api, header.Height)
	if err != nil {
		return nil, nil, err
	}
	if err := assembler.appendFixed(version); err != nil {
		return nil, nil, err
	}
	if err := assembler.appendFixed(height); err != nil {
		return nil, nil, err
	}
	for _, optional := range []*OptionalHeaderHashWitness{
		&header.PreviousBlockHash,
		&header.ExternalEntrypointRoot,
		&header.DAProofPoliciesHash,
		&header.DACommitmentsHash,
		&header.DAPinIntentsHash,
		&header.NPoSEffectsHash,
		&header.ExecutionContextHash,
	} {
		if err := appendCanonicalHeaderOption(assembler, api, optional); err != nil {
			return nil, nil, err
		}
	}
	// The final-V1 message circuit accepts only a present, nonzero SCCP root,
	// and that value is exactly the already constrained public commitment root.
	if err := assembler.appendFixed(noritoField(append(
		constants([]byte{1}),
		noritoField(sccpRoot[:])...,
	))); err != nil {
		return nil, nil, err
	}
	creation, err := noritoU64Field(api, header.CreationTimeMilliseconds)
	if err != nil {
		return nil, nil, err
	}
	view, err := noritoU64Field(api, header.ViewChangeIndex)
	if err != nil {
		return nil, nil, err
	}
	if err := assembler.appendFixed(creation); err != nil {
		return nil, nil, err
	}
	if err := assembler.appendFixed(view); err != nil {
		return nil, nil, err
	}
	if err := assembler.appendChoices(confidentialChoices); err != nil {
		return nil, nil, err
	}
	return assembler.finish()
}
