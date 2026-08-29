package circuit

import (
	"encoding/binary"
	"fmt"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

const semanticStatementDomainFinalV1 = "sccp:semantic-statement:final-v1"

// StatementWitness contains the non-deployment semantic policy selected by a
// fixed verifier. The verifying-key digest is deliberately not an R1CS
// constant: a circuit cannot commit to its own Phase-2 key without creating a
// cycle. The generated fixed-key wrapper instead binds RawSignals[9] to the
// route configuration containing its compiled key hash and code identity.
type StatementWitness struct {
	SemanticProofProfileHash [32]uints.U8
	VerifierCircuitHash      [32]uints.U8
	ProofProfileCommitment   [32]uints.U8
}

func (c *MessageCircuit) constrainSemanticStatement(api frontend.API) error {
	if err := nonZeroBytes(api, c.Statement.SemanticProofProfileHash[:]); err != nil {
		return err
	}
	if c.cfg.Curve == profile.BLS12381 {
		if err := nonZeroBytes(api, c.Statement.VerifierCircuitHash[:]); err != nil {
			return err
		}
		if err := nonZeroBytes(api, c.Statement.ProofProfileCommitment[:]); err != nil {
			return err
		}
	} else {
		for i := 0; i < 32; i++ {
			api.AssertIsEqual(c.Statement.VerifierCircuitHash[i].Val, 0)
			api.AssertIsEqual(c.Statement.ProofProfileCommitment[i].Val, 0)
		}
	}

	bundleCommitment, err := c.semanticBundleCommitment(api)
	if err != nil {
		return err
	}
	publicInputs, err := c.canonicalPublicInputBytes(api)
	if err != nil {
		return err
	}
	source, target := canonicalNetworkBytes(c.cfg)
	preimage := constants([]byte(semanticStatementDomainFinalV1))
	preimage = append(preimage, uints.NewU8(1), uints.NewU8(c.cfg.BackendTag))
	preimage = appendConstantVec(preimage, source)
	preimage = appendConstantVec(preimage, target)
	preimage = append(preimage, c.RawSignals[8][:]...)
	preimage = append(preimage, c.RawSignals[9][:]...)
	preimage = append(preimage, c.Statement.SemanticProofProfileHash[:]...)
	preimage = append(preimage, c.RawSignals[10][:]...)
	preimage = append(preimage, c.Statement.VerifierCircuitHash[:]...)
	preimage = append(preimage, c.Statement.ProofProfileCommitment[:]...)
	preimage = append(preimage, publicInputs...)
	// RawSignals[1] is already constrained to the domain-separated hash of the
	// exact canonical payload bytes. Committing that digest here avoids an
	// unsound variable-position suffix while preserving byte-for-byte payload
	// binding under BLAKE2b-256 collision resistance.
	preimage = append(preimage, c.RawSignals[1][:]...)
	preimage = append(preimage, bundleCommitment[:]...)

	digest, err := hashFixed(api, c.cfg.SignalHash, preimage)
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, digest, c.RawSignals[7][:]); err != nil {
		return err
	}

	roles := [][]uints.U8{
		c.RawSignals[7][:],
		c.RawSignals[8][:],
		c.RawSignals[9][:],
		c.RawSignals[10][:],
		c.Statement.SemanticProofProfileHash[:],
	}
	if c.cfg.Curve == profile.BLS12381 {
		roles = append(roles, c.Statement.VerifierCircuitHash[:], c.Statement.ProofProfileCommitment[:])
	}
	for left := 0; left < len(roles); left++ {
		for right := left + 1; right < len(roles); right++ {
			if err := assertBytesNotEqual(api, roles[left], roles[right]); err != nil {
				return err
			}
		}
	}
	return nil
}

func (c *MessageCircuit) semanticBundleCommitment(api frontend.API) ([32]uints.U8, error) {
	commitment := []uints.U8{
		uints.NewU8(1), uints.NewU8(5), uints.NewU8(profile.SoraNetworkTag), uints.NewU8(c.cfg.TargetNetworkTag),
	}
	commitment = append(commitment, c.RawSignals[8][:]...)
	commitment = append(commitment, c.RawSignals[9][:]...)
	commitment = append(commitment, c.RawSignals[0][:]...)
	commitment = append(commitment, c.RawSignals[1][:]...)

	depthBytes, err := u32Bytes(api, c.MerkleDepth)
	if err != nil {
		return [32]uints.U8{}, err
	}
	proof := append([]uints.U8{}, depthBytes...)
	for level := 0; level < MaxMerkleDepth; level++ {
		proof = append(proof, c.Siblings[level][:]...)
		proof = append(proof, uints.U8{Val: c.SiblingIsLeft[level]})
	}
	proofLength := api.Add(4, api.Mul(c.MerkleDepth, 33))
	proofDigest, err := hashVariable(api, profile.SHA256Signal, proof, proofLength)
	if err != nil {
		return [32]uints.U8{}, err
	}

	finalityDigest, err := finalitySemanticCommitment(api, &c.Finality)
	if err != nil {
		return [32]uints.U8{}, err
	}
	preimage := constants([]byte("sccp:semantic-bundle:final-v1"))
	preimage = append(preimage, uints.NewU8(1))
	preimage = append(preimage, c.RawSignals[3][:]...)
	preimage = append(preimage, commitment...)
	preimage = append(preimage, proofDigest...)
	preimage = append(preimage, c.RawSignals[1][:]...)
	preimage = append(preimage, finalityDigest[:]...)
	digest, err := hashFixed(api, profile.SHA256Signal, preimage)
	if err != nil {
		return [32]uints.U8{}, err
	}
	if len(digest) != 32 {
		return [32]uints.U8{}, fmt.Errorf("semantic bundle digest has %d bytes", len(digest))
	}
	var out [32]uints.U8
	copy(out[:], digest)
	return out, nil
}

func (c *MessageCircuit) canonicalPublicInputBytes(api frontend.API) ([]uints.U8, error) {
	heightLE, err := u64Bytes(api, c.Finality.Height)
	if err != nil {
		return nil, err
	}
	result := []uints.U8{uints.NewU8(1)}
	result = append(result, c.RawSignals[0][:]...)
	result = append(result, c.RawSignals[1][:]...)
	var target [4]byte
	binary.LittleEndian.PutUint32(target[:], c.cfg.TargetDomain)
	result = append(result, constants(target[:])...)
	result = append(result, c.RawSignals[3][:]...)
	result = append(result, heightLE...)
	result = append(result, c.RawSignals[5][:]...)
	return result, nil
}

func appendConstantVec(destination []uints.U8, value []byte) []uints.U8 {
	destination = append(destination, constants(binary.LittleEndian.AppendUint32(nil, uint32(len(value))))...)
	return append(destination, constants(value)...)
}

func u16Bytes(api frontend.API, value frontend.Variable) ([]uints.U8, error) {
	encoded, err := u32Bytes(api, value)
	if err != nil {
		return nil, err
	}
	api.AssertIsEqual(encoded[2].Val, 0)
	api.AssertIsEqual(encoded[3].Val, 0)
	return encoded[:2], nil
}

func u32Bytes(api frontend.API, value frontend.Variable) ([]uints.U8, error) {
	u32api, err := uints.New[uints.U32](api)
	if err != nil {
		return nil, err
	}
	return u32api.UnpackLSB(u32api.ValueOf(value)), nil
}

func u64Bytes(api frontend.API, value frontend.Variable) ([]uints.U8, error) {
	u64api, err := uints.New[uints.U64](api)
	if err != nil {
		return nil, err
	}
	return u64api.UnpackLSB(u64api.ValueOf(value)), nil
}

func finalitySemanticCommitment(_ frontend.API, finality *FinalityWitness) ([32]uints.U8, error) {
	// constrainFinalityStructure independently derives and authenticates this
	// digest. Reuse the constrained result in the SCCP semantic bundle instead
	// of hashing a second, subtly different projection of the artifact.
	return finality.FinalityArtifactHash, nil
}
