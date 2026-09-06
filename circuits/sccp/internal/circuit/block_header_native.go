package circuit

import (
	"encoding/binary"
)

func nativeNoritoOption(present bool, encodedValue []byte) []byte {
	if !present {
		return nativeNoritoField([]byte{0})
	}
	body := append([]byte{1}, encodedValue...)
	return nativeNoritoField(body)
}

func nativeOptionalHeaderHash(value OptionalHeaderHashWitness) []byte {
	digest := u8Array32(value.Value)
	return nativeNoritoOption(value.Present.(int) == 1, nativeNoritoHash(digest))
}

func nativeOptionalHeaderU32(value OptionalHeaderU32Witness) []byte {
	return nativeNoritoOption(value.Present.(int) == 1, nativeNoritoU32(uint32(value.Value.(int))))
}

func nativeConfidentialFeatureDigest(value ConfidentialFeatureDigestWitness) []byte {
	body := nativeNoritoOption(
		value.VKSetHash.Present.(int) == 1,
		nativeNoritoHash(u8Array32(value.VKSetHash.Value)),
	)
	body = append(body, nativeOptionalHeaderU32(value.PoseidonParams)...)
	body = append(body, nativeOptionalHeaderU32(value.PedersenParams)...)
	body = append(body, nativeOptionalHeaderU32(value.RulesVersion)...)
	body = append(body, nativeNoritoOption(
		value.ZKPolicyHash.Present.(int) == 1,
		nativeNoritoHash(u8Array32(value.ZKPolicyHash.Value)),
	)...)
	return body
}

func nativeBlockHeaderConsensusProjection(
	header BlockHeaderProjectionWitness,
	sccpRoot [32]byte,
) []byte {
	body := nativeNoritoU16(blockHeaderConsensusProjectionVersion)
	body = append(body, nativeNoritoU64(uint64(header.Height.(int)))...)
	for _, value := range []OptionalHeaderHashWitness{
		header.PreviousBlockHash,
		header.ExternalEntrypointRoot,
		header.DAProofPoliciesHash,
		header.DACommitmentsHash,
		header.DAPinIntentsHash,
		header.NPoSEffectsHash,
		header.ExecutionContextHash,
	} {
		body = append(body, nativeOptionalHeaderHash(value)...)
	}
	body = append(body, nativeNoritoOption(true, nativeNoritoField(sccpRoot[:]))...)
	body = append(body, nativeNoritoU64(uint64(header.CreationTimeMilliseconds.(int)))...)
	body = append(body, nativeNoritoU64(uint64(header.ViewChangeIndex.(int)))...)
	body = append(body, nativeNoritoOption(
		header.HasConfidentialFeatures.(int) == 1,
		nativeNoritoField(nativeConfidentialFeatureDigest(header.ConfidentialFeatures)),
	)...)
	return body
}

func nativeBlockHeaderConsensusHash(
	header BlockHeaderProjectionWitness,
	sccpRoot [32]byte,
) [32]byte {
	return nativeIrohaHash(nativeBlockHeaderConsensusProjection(header, sccpRoot))
}

func initializeOptionalHeaderHash(value *OptionalHeaderHashWitness) {
	value.Present = 0
	zeroU8s(value.Value[:])
}

func initializeOptionalHeaderU32(value *OptionalHeaderU32Witness) {
	value.Present = 0
	value.Value = 0
}

func initializeBlockHeaderProjection(value *BlockHeaderProjectionWitness) {
	value.Height = 0
	for _, optional := range []*OptionalHeaderHashWitness{
		&value.PreviousBlockHash,
		&value.ExternalEntrypointRoot,
		&value.DAProofPoliciesHash,
		&value.DACommitmentsHash,
		&value.DAPinIntentsHash,
		&value.NPoSEffectsHash,
		&value.ExecutionContextHash,
		&value.ConfidentialFeatures.VKSetHash,
		&value.ConfidentialFeatures.ZKPolicyHash,
	} {
		initializeOptionalHeaderHash(optional)
	}
	for _, optional := range []*OptionalHeaderU32Witness{
		&value.ConfidentialFeatures.PoseidonParams,
		&value.ConfidentialFeatures.PedersenParams,
		&value.ConfidentialFeatures.RulesVersion,
	} {
		initializeOptionalHeaderU32(optional)
	}
	value.CreationTimeMilliseconds = 0
	value.ViewChangeIndex = 0
	value.HasConfidentialFeatures = 0
}

func populateKATBlockHeaderProjection(
	header *BlockHeaderProjectionWitness,
	finality FinalityWitness,
	scope string,
) {
	initializeBlockHeaderProjection(header)
	header.Height = finality.Height
	header.PreviousBlockHash.Present = 1
	header.PreviousBlockHash.Value = finality.SubjectParentBlockHash
	header.ExternalEntrypointRoot.Present = 1
	set32(&header.ExternalEntrypointRoot.Value, nativeIrohaHash([]byte(scope+":external-entrypoint-root")))
	header.DAProofPoliciesHash.Present = 1
	set32(&header.DAProofPoliciesHash.Value, nativeIrohaHash([]byte(scope+":da-proof-policies")))
	header.DACommitmentsHash.Present = 0
	header.DAPinIntentsHash.Present = 1
	set32(&header.DAPinIntentsHash.Value, nativeIrohaHash([]byte(scope+":da-pin-intents")))
	header.NPoSEffectsHash.Present = 1
	set32(&header.NPoSEffectsHash.Value, nativeIrohaHash([]byte(scope+":npos-effects")))
	header.ExecutionContextHash.Present = 1
	set32(&header.ExecutionContextHash.Value, nativeIrohaHash([]byte(scope+":execution-context")))
	header.CreationTimeMilliseconds = 1_700_000_000_002
	header.ViewChangeIndex = finality.BlockHeaderView
	header.HasConfidentialFeatures = 1
	header.ConfidentialFeatures.RulesVersion.Present = 1
	header.ConfidentialFeatures.RulesVersion.Value = 1
	header.ConfidentialFeatures.ZKPolicyHash.Present = 1
	policy := [32]byte{
		0xed, 0x13, 0xe7, 0xdb, 0x7c, 0xfb, 0xf0, 0x92,
		0xc1, 0x9a, 0x26, 0xef, 0x4a, 0x03, 0x9d, 0x09,
		0x1c, 0xb6, 0x6e, 0x04, 0xca, 0x78, 0x5e, 0xb8,
		0xc3, 0xed, 0xa4, 0xb9, 0xa0, 0x27, 0xc5, 0x5c,
	}
	set32(&header.ConfidentialFeatures.ZKPolicyHash.Value, policy)
}

func nativeHeaderProjectionMaximumFixture() BlockHeaderProjectionWitness {
	var header BlockHeaderProjectionWitness
	initializeBlockHeaderProjection(&header)
	header.Height = 2
	for index, optional := range []*OptionalHeaderHashWitness{
		&header.PreviousBlockHash,
		&header.ExternalEntrypointRoot,
		&header.DAProofPoliciesHash,
		&header.DACommitmentsHash,
		&header.DAPinIntentsHash,
		&header.NPoSEffectsHash,
		&header.ExecutionContextHash,
	} {
		optional.Present = 1
		var digest [32]byte
		binary.LittleEndian.PutUint32(digest[:4], uint32(index+1))
		digest[31] = 1
		set32(&optional.Value, digest)
	}
	header.CreationTimeMilliseconds = 1
	header.ViewChangeIndex = 1
	header.HasConfidentialFeatures = 1
	for index, optional := range []*OptionalHeaderHashWitness{
		&header.ConfidentialFeatures.VKSetHash,
		&header.ConfidentialFeatures.ZKPolicyHash,
	} {
		optional.Present = 1
		var digest [32]byte
		digest[index] = 1
		set32(&optional.Value, digest)
	}
	for index, optional := range []*OptionalHeaderU32Witness{
		&header.ConfidentialFeatures.PoseidonParams,
		&header.ConfidentialFeatures.PedersenParams,
		&header.ConfidentialFeatures.RulesVersion,
	} {
		optional.Present = 1
		optional.Value = index + 1
	}
	return header
}
