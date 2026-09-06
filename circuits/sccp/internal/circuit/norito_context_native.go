package circuit

import (
	"encoding/binary"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
)

func nativeCanonicalPublicKeyBody(publicKey [48]byte) []byte {
	body := binary.LittleEndian.AppendUint64(nil, 49)
	body = append(body, nativeNoritoField([]byte{blsNormalAlgorithmTag})...)
	for _, value := range publicKey {
		body = append(body, nativeNoritoField([]byte{value})...)
	}
	return body
}

func nativeCanonicalValidatorPowerBody(publicKey [48]byte) []byte {
	peerBody := nativeNoritoField(nativeCanonicalPublicKeyBody(publicKey))
	body := nativeNoritoField(peerBody)
	return append(body, nativeNoritoU64(1)...)
}

func nativeFinalityRosterVectorBody(finality FinalityWitness, count int) []byte {
	body := binary.LittleEndian.AppendUint64(nil, uint64(count))
	for index := 0; index < count; index++ {
		key := u8Array48(finality.ValidatorPublicKeys[index])
		body = append(body, nativeNoritoField(nativeCanonicalValidatorPowerBody(key))...)
	}
	return body
}

func nativeSnapshotRosterVectorBody(snapshot NextEpochSnapshotWitness, count int) []byte {
	body := binary.LittleEndian.AppendUint64(nil, uint64(count))
	for index := 0; index < count; index++ {
		key := u8Array48(snapshot.ValidatorPublicKeys[index])
		body = append(body, nativeNoritoField(nativeCanonicalValidatorPowerBody(key))...)
	}
	return body
}

func nativeSnapshotPoPVectorBody(snapshot NextEpochSnapshotWitness, count int) []byte {
	body := binary.LittleEndian.AppendUint64(nil, uint64(count))
	for index := 0; index < count; index++ {
		proof := binary.LittleEndian.AppendUint64(nil, canonicalBLSNormalPoPBytes)
		proofBytes := u8Array96(snapshot.ValidatorPoPs[index])
		proof = append(proof, proofBytes[:]...)
		body = append(body, nativeNoritoField(proof)...)
	}
	return body
}

func nativeCanonicalDualQuorumBody(count int) []byte {
	minimum := 2*((count-1)/3) + 1
	body := nativeNoritoU32(uint32(minimum))
	return append(body, nativeNoritoU64(uint64(count))...)
}

func nativeCanonicalNextEpochSnapshotBody(snapshot NextEpochSnapshotWitness) []byte {
	count := snapshot.ValidatorCount.(int)
	body := nativeNoritoU64(uint64(snapshot.Epoch.(int)))
	body = append(body, nativeNoritoU64(uint64(snapshot.EpochEndHeight.(int)))...)
	body = append(body, nativeNoritoU32(uint32(snapshot.Mode.(int)))...)
	body = append(body, nativeNoritoField(nativeSnapshotRosterVectorBody(snapshot, count))...)
	body = append(body, nativeNoritoField(nativeSnapshotPoPVectorBody(snapshot, count))...)
	body = append(body, nativeNoritoField(nativeCanonicalDualQuorumBody(count))...)
	leader := u8Array32(snapshot.LeaderSeed)
	body = append(body, nativeNoritoField(leader[:])...)
	return body
}

func nativeCanonicalParentCommitIdentityBody(finality FinalityWitness) []byte {
	context := u8Array32(finality.ParentContextID)
	body := nativeNoritoField(nativeNoritoHash(context))
	body = append(body, nativeNoritoU64(uint64(finality.ParentHeight.(int)))...)
	body = append(body, nativeNoritoU32(2)...)
	grandparent := u8Array32(finality.ParentSubjectParentBlockHash)
	parentBlock := u8Array32(finality.ParentBlockHash)
	parentPayload := u8Array32(finality.ParentPayloadHash)
	parentOption := append([]byte{1}, nativeNoritoHash(grandparent)...)
	subject := nativeNoritoField(parentOption)
	subject = append(subject, nativeNoritoHash(parentBlock)...)
	subject = append(subject, nativeNoritoHash(parentPayload)...)
	body = append(body, nativeNoritoField(subject)...)
	body = append(
		body,
		nativeNoritoField(nativeCanonicalExecutionCommitmentBody(finality.ParentExecution))...,
	)
	return body
}

func nativeCanonicalConsensusRound(
	context [32]byte,
	height uint64,
	view uint64,
) []byte {
	body := nativeNoritoField(nativeNoritoHash(context))
	body = append(body, nativeNoritoU64(height)...)
	body = append(body, nativeNoritoU64(view)...)
	return body
}

func nativeSignerVectorBody(indices [MaxValidators]frontend.Variable, count int) []byte {
	body := binary.LittleEndian.AppendUint64(nil, uint64(count))
	for index := 0; index < count; index++ {
		body = append(body, nativeNoritoU32(uint32(indices[index].(int)))...)
	}
	return body
}

func nativeSignatureVectorBody(signature [96]uints.U8) []byte {
	body := binary.LittleEndian.AppendUint64(nil, 96)
	value := u8Array96(signature)
	return append(body, value[:]...)
}

func nativeQuorumCertificateBody(finality FinalityWitness, parent bool) []byte {
	var context [32]byte
	var height, roundView, proposalView uint64
	var subject []byte
	var execution ExecutionCommitmentWitness
	var indices [MaxValidators]frontend.Variable
	var signerCount int
	var aggregate [96]uints.U8
	if parent {
		context = u8Array32(finality.ParentContextID)
		height = uint64(finality.ParentHeight.(int))
		roundView = uint64(finality.ParentRoundView.(int))
		proposalView = uint64(finality.ParentProposalView.(int))
		grandparent := u8Array32(finality.ParentSubjectParentBlockHash)
		parentBlock := u8Array32(finality.ParentBlockHash)
		parentPayload := u8Array32(finality.ParentPayloadHash)
		parentOption := append([]byte{1}, nativeNoritoHash(grandparent)...)
		subject = nativeNoritoField(parentOption)
		subject = append(subject, nativeNoritoHash(parentBlock)...)
		subject = append(subject, nativeNoritoHash(parentPayload)...)
		execution = finality.ParentExecution
		indices = finality.ParentSignerIndices
		signerCount = finality.ParentSignerCount.(int)
		aggregate = finality.ParentAggregateSignature
	} else {
		context = u8Array32(finality.HeightContextID)
		height = uint64(finality.Height.(int))
		roundView = uint64(finality.RoundView.(int))
		proposalView = uint64(finality.ProposalView.(int))
		parentBlock := u8Array32(finality.SubjectParentBlockHash)
		block := u8Array32(finality.BlockHeaderHash)
		payload := u8Array32(finality.SubjectPayloadHash)
		parentOption := append([]byte{1}, nativeNoritoHash(parentBlock)...)
		subject = nativeNoritoField(parentOption)
		subject = append(subject, nativeNoritoHash(block)...)
		subject = append(subject, nativeNoritoHash(payload)...)
		execution = finality.Execution
		indices = finality.SignerIndices
		validatorCount := finality.ValidatorCount.(int)
		signerCount = 2*((validatorCount-1)/3) + 1
		aggregate = finality.AggregateSignature
	}
	body := nativeNoritoField(nativeCanonicalConsensusRound(context, height, roundView))
	body = append(
		body,
		nativeNoritoField(nativeCanonicalConsensusRound(context, height, proposalView))...,
	)
	body = append(body, nativeNoritoU32(2)...)
	body = append(body, nativeNoritoField(subject)...)
	body = append(body, nativeNoritoField(nativeCanonicalExecutionCommitmentBody(execution))...)
	body = append(body, nativeNoritoField(nativeSignerVectorBody(indices, signerCount))...)
	body = append(body, nativeNoritoField(nativeSignatureVectorBody(aggregate))...)
	return body
}

func nativeCanonicalDataAvailabilityLayoutBody(layout DataAvailabilityLayoutWitness) []byte {
	body := nativeNoritoU32(0)
	body = append(body, nativeNoritoU32(uint32(layout.ChunkSizeBytes.(int)))...)
	body = append(body, nativeNoritoU16(uint16(layout.DataShards.(int)))...)
	body = append(body, nativeNoritoU16(uint16(layout.ParityShards.(int)))...)
	body = append(body, nativeNoritoU64(uint64(layout.MaxPayloadSizeBytes.(int)))...)
	body = append(body, nativeNoritoU32(uint32(layout.MaxChunkCount.(int)))...)
	return body
}

func nativeHeightContextIdentity(finality FinalityWitness) [32]byte {
	body := nativeNoritoU16(heightContextIdentityVersion)
	body = append(body, nativeNoritoHash(tairaFinalityNetworkID)...)
	body = append(body, nativeNoritoU16(sumeragiProtocolVersion)...)
	body = append(body, nativeNoritoU64(uint64(finality.Height.(int)))...)
	body = append(body, nativeNoritoU64(uint64(finality.Epoch.(int)))...)
	body = append(body, nativeNoritoU64(uint64(finality.EpochEndHeight.(int)))...)
	if finality.HasNextEpochSnapshot.(int) == 1 {
		snapshot := nativeCanonicalNextEpochSnapshotBody(finality.NextEpochSnapshot)
		option := append([]byte{1}, nativeNoritoField(snapshot)...)
		body = append(body, nativeNoritoField(option)...)
	} else {
		body = append(body, nativeNoritoField([]byte{0})...)
	}
	body = append(body, nativeNoritoU32(uint32(finality.Mode.(int)))...)
	parent := nativeCanonicalParentCommitIdentityBody(finality)
	body = append(body, nativeNoritoField(append([]byte{1}, nativeNoritoField(parent)...))...)
	// SCCP accepts only native v4 parent QCs, never a legacy snapshot bootstrap.
	body = append(body, nativeNoritoField([]byte{0})...)
	count := finality.ValidatorCount.(int)
	body = append(body, nativeNoritoField(nativeFinalityRosterVectorBody(finality, count))...)
	body = append(body, nativeNoritoField(nativeCanonicalDualQuorumBody(count))...)
	for _, digest := range [][32]uints.U8{
		finality.NexusAMXContextHash,
		finality.ExecutionPolicyHash,
	} {
		value := u8Array32(digest)
		body = append(body, nativeNoritoHash(value)...)
	}
	body = append(body, nativeNoritoField(nativeCanonicalDataAvailabilityLayoutBody(finality.DALayout))...)
	leader := u8Array32(finality.LeaderSeed)
	body = append(body, nativeNoritoField(leader[:])...)
	return nativeIrohaHash(body)
}

func nativeHeightContextBody(finality FinalityWitness) []byte {
	body := nativeNoritoHash(tairaFinalityNetworkID)
	body = append(body, nativeNoritoU16(sumeragiProtocolVersion)...)
	body = append(body, nativeNoritoU64(uint64(finality.Height.(int)))...)
	body = append(body, nativeNoritoU64(uint64(finality.Epoch.(int)))...)
	body = append(body, nativeNoritoU64(uint64(finality.EpochEndHeight.(int)))...)
	if finality.HasNextEpochSnapshot.(int) == 1 {
		snapshot := nativeCanonicalNextEpochSnapshotBody(finality.NextEpochSnapshot)
		option := append([]byte{1}, nativeNoritoField(snapshot)...)
		body = append(body, nativeNoritoField(option)...)
	} else {
		body = append(body, nativeNoritoField([]byte{0})...)
	}
	body = append(body, nativeNoritoU32(uint32(finality.Mode.(int)))...)
	parent := nativeQuorumCertificateBody(finality, true)
	body = append(body, nativeNoritoField(append([]byte{1}, nativeNoritoField(parent)...))...)
	body = append(body, nativeNoritoField([]byte{0})...)
	count := finality.ValidatorCount.(int)
	body = append(body, nativeNoritoField(nativeFinalityRosterVectorBody(finality, count))...)
	body = append(body, nativeNoritoField(nativeCanonicalDualQuorumBody(count))...)
	for _, digest := range [][32]uints.U8{
		finality.NexusAMXContextHash,
		finality.ExecutionPolicyHash,
	} {
		value := u8Array32(digest)
		body = append(body, nativeNoritoHash(value)...)
	}
	body = append(body, nativeNoritoField(nativeCanonicalDataAvailabilityLayoutBody(finality.DALayout))...)
	leader := u8Array32(finality.LeaderSeed)
	body = append(body, nativeNoritoField(leader[:])...)
	return body
}

func nativeCurrentPoPVectorBody(finality FinalityWitness) []byte {
	count := finality.ValidatorCount.(int)
	body := binary.LittleEndian.AppendUint64(nil, uint64(count))
	for index := 0; index < count; index++ {
		proof := binary.LittleEndian.AppendUint64(nil, 96)
		value := u8Array96(finality.ValidatorPoPs[index])
		proof = append(proof, value[:]...)
		body = append(body, nativeNoritoField(proof)...)
	}
	return body
}

func nativeFinalityArtifactBytes(finality FinalityWitness) []byte {
	body := nativeNoritoU16(sumeragiFinalityFormatVersion)
	body = append(body, nativeNoritoU16(sumeragiProtocolVersion)...)
	body = append(body, nativeNoritoU64(uint64(finality.Height.(int)))...)
	body = append(body, nativeNoritoField(nativeHeightContextBody(finality))...)
	parent := u8Array32(finality.SubjectParentBlockHash)
	block := u8Array32(finality.BlockHeaderHash)
	payload := u8Array32(finality.SubjectPayloadHash)
	parentOption := append([]byte{1}, nativeNoritoHash(parent)...)
	subject := nativeNoritoField(parentOption)
	subject = append(subject, nativeNoritoHash(block)...)
	subject = append(subject, nativeNoritoHash(payload)...)
	body = append(body, nativeNoritoField(subject)...)
	body = append(body, nativeNoritoHash(block)...)
	body = append(body, nativeNoritoField(nativeQuorumCertificateBody(finality, false))...)
	body = append(body, nativeNoritoField(nativeCurrentPoPVectorBody(finality))...)
	return body
}

func nativeFinalityArtifactHash(finality FinalityWitness) [32]byte {
	return nativeIrohaHash(nativeFinalityArtifactBytes(finality))
}

func u8Array48(source [48]uints.U8) [48]byte {
	var out [48]byte
	for index := range source {
		out[index] = byte(source[index].Val.(uint8))
	}
	return out
}

func u8Array96(source [96]uints.U8) [96]byte {
	var out [96]byte
	for index := range source {
		out[index] = byte(source[index].Val.(uint8))
	}
	return out
}
