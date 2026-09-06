package circuit

import (
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
	"github.com/consensys/gnark/test"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

type blockHeaderBindingTestCircuit struct {
	Header         BlockHeaderProjectionWitness
	Root           [32]uints.U8
	SubjectParent  [32]uints.U8
	ExpectedHash   [32]uints.U8
	FinalityHeight frontend.Variable
	FinalityView   frontend.Variable
}

func (c *blockHeaderBindingTestCircuit) Define(api frontend.API) error {
	message := &MessageCircuit{BlockHeader: c.Header}
	message.RawSignals[3] = c.Root
	message.Finality.Height = c.FinalityHeight
	message.Finality.BlockHeaderView = c.FinalityView
	message.Finality.SubjectParentBlockHash = c.SubjectParent
	message.Finality.BlockHeaderHash = c.ExpectedHash
	return message.constrainBlockHeaderCommitment(api)
}

func TestCanonicalBlockHeaderProjectionBindsSCCPRootToFinalityHash(t *testing.T) {
	definition := &blockHeaderBindingTestCircuit{}
	assignment := blockHeaderBindingAssignment(t)
	field := ecc.BN254.ScalarField()
	if err := test.IsSolved(definition, assignment, field); err != nil {
		t.Fatalf("valid canonical block-header projection rejected: %v", err)
	}

	mutations := []struct {
		name   string
		mutate func(*blockHeaderBindingTestCircuit)
	}{
		{"SCCP root", func(value *blockHeaderBindingTestCircuit) { flipHeaderByte(&value.Root[4]) }},
		{"signed header hash", func(value *blockHeaderBindingTestCircuit) { flipHeaderByte(&value.ExpectedHash[8]) }},
		{"coherent height", func(value *blockHeaderBindingTestCircuit) {
			value.Header.Height = value.Header.Height.(int) + 1
			value.FinalityHeight = value.FinalityHeight.(int) + 1
		}},
		{"finality height linkage", func(value *blockHeaderBindingTestCircuit) {
			value.FinalityHeight = value.FinalityHeight.(int) + 1
		}},
		{"previous hash presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderHash(&value.Header.PreviousBlockHash)
		}},
		{"coherent previous hash", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.Header.PreviousBlockHash.Value[11])
			flipHeaderByte(&value.SubjectParent[11])
		}},
		{"parent hash linkage", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.SubjectParent[12])
		}},
		{"external root presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderHash(&value.Header.ExternalEntrypointRoot)
		}},
		{"external root value", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.Header.ExternalEntrypointRoot.Value[17])
		}},
		{"DA proof-policy presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderHash(&value.Header.DAProofPoliciesHash)
		}},
		{"DA proof-policy value", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.Header.DAProofPoliciesHash.Value[5])
		}},
		{"DA commitment presence", func(value *blockHeaderBindingTestCircuit) {
			setTestOptionalHeaderHash(&value.Header.DACommitmentsHash, "DA commitments")
		}},
		{"DA commitment absent padding", func(value *blockHeaderBindingTestCircuit) {
			value.Header.DACommitmentsHash.Value[0].Val = 1
		}},
		{"DA pin-intent presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderHash(&value.Header.DAPinIntentsHash)
		}},
		{"DA pin-intent value", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.Header.DAPinIntentsHash.Value[6])
		}},
		{"NPoS-effects presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderHash(&value.Header.NPoSEffectsHash)
		}},
		{"NPoS-effects value", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.Header.NPoSEffectsHash.Value[7])
		}},
		{"execution-context presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderHash(&value.Header.ExecutionContextHash)
		}},
		{"execution-context value", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.Header.ExecutionContextHash.Value[8])
		}},
		{"creation time", func(value *blockHeaderBindingTestCircuit) {
			value.Header.CreationTimeMilliseconds = value.Header.CreationTimeMilliseconds.(int) + 1
		}},
		{"coherent view", func(value *blockHeaderBindingTestCircuit) {
			value.Header.ViewChangeIndex = value.Header.ViewChangeIndex.(int) + 1
			value.FinalityView = value.FinalityView.(int) + 1
		}},
		{"finality view linkage", func(value *blockHeaderBindingTestCircuit) {
			value.FinalityView = value.FinalityView.(int) + 1
		}},
		{"confidential outer discriminant", func(value *blockHeaderBindingTestCircuit) {
			clearTestConfidentialFeatureDigest(&value.Header)
		}},
		{"confidential outer non-boolean", func(value *blockHeaderBindingTestCircuit) {
			value.Header.HasConfidentialFeatures = 2
		}},
		{"VK-set presence", func(value *blockHeaderBindingTestCircuit) {
			setTestOptionalHeaderHash(&value.Header.ConfidentialFeatures.VKSetHash, "VK set")
		}},
		{"VK-set absent padding", func(value *blockHeaderBindingTestCircuit) {
			value.Header.ConfidentialFeatures.VKSetHash.Value[0].Val = 1
		}},
		{"Poseidon-parameter presence", func(value *blockHeaderBindingTestCircuit) {
			value.Header.ConfidentialFeatures.PoseidonParams.Present = 1
			value.Header.ConfidentialFeatures.PoseidonParams.Value = 7
		}},
		{"Poseidon-parameter absent value", func(value *blockHeaderBindingTestCircuit) {
			value.Header.ConfidentialFeatures.PoseidonParams.Value = 7
		}},
		{"Pedersen-parameter presence", func(value *blockHeaderBindingTestCircuit) {
			value.Header.ConfidentialFeatures.PedersenParams.Present = 1
			value.Header.ConfidentialFeatures.PedersenParams.Value = 8
		}},
		{"Pedersen-parameter absent value", func(value *blockHeaderBindingTestCircuit) {
			value.Header.ConfidentialFeatures.PedersenParams.Value = 8
		}},
		{"rules-version presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderU32(&value.Header.ConfidentialFeatures.RulesVersion)
		}},
		{"rules-version value", func(value *blockHeaderBindingTestCircuit) {
			value.Header.ConfidentialFeatures.RulesVersion.Value =
				value.Header.ConfidentialFeatures.RulesVersion.Value.(int) + 1
		}},
		{"ZK-policy presence", func(value *blockHeaderBindingTestCircuit) {
			initializeOptionalHeaderHash(&value.Header.ConfidentialFeatures.ZKPolicyHash)
		}},
		{"ZK-policy value", func(value *blockHeaderBindingTestCircuit) {
			flipHeaderByte(&value.Header.ConfidentialFeatures.ZKPolicyHash.Value[3])
		}},
	}
	for _, mutation := range mutations {
		mutation := mutation
		t.Run(mutation.name, func(t *testing.T) {
			candidate := *assignment
			mutation.mutate(&candidate)
			if err := test.IsSolved(definition, &candidate, field); err == nil {
				t.Fatalf("%s substitution was accepted", mutation.name)
			}
		})
	}
}

func flipHeaderByte(value *uints.U8) {
	value.Val = value.Val.(uint8) ^ 1
}

func setTestOptionalHeaderHash(value *OptionalHeaderHashWitness, label string) {
	value.Present = 1
	digest := nativeIrohaHash([]byte("sccp:block-header-mutation:" + label))
	set32(&value.Value, digest)
}

func clearTestConfidentialFeatureDigest(header *BlockHeaderProjectionWitness) {
	header.HasConfidentialFeatures = 0
	initializeOptionalHeaderHash(&header.ConfidentialFeatures.VKSetHash)
	initializeOptionalHeaderU32(&header.ConfidentialFeatures.PoseidonParams)
	initializeOptionalHeaderU32(&header.ConfidentialFeatures.PedersenParams)
	initializeOptionalHeaderU32(&header.ConfidentialFeatures.RulesVersion)
	initializeOptionalHeaderHash(&header.ConfidentialFeatures.ZKPolicyHash)
}

func TestBlockHeaderConsensusProjectionMaximumIsExact(t *testing.T) {
	header := nativeHeaderProjectionMaximumFixture()
	root := [32]byte{0x42}
	encoded := nativeBlockHeaderConsensusProjection(header, root)
	if len(encoded) != maxBlockHeaderConsensusProjectionBytes {
		t.Fatalf(
			"maximum BlockHeaderConsensusProjectionV1 length = %d, exact bound = %d",
			len(encoded),
			maxBlockHeaderConsensusProjectionBytes,
		)
	}
}

func blockHeaderBindingAssignment(t *testing.T) *blockHeaderBindingTestCircuit {
	t.Helper()
	cfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	_, message, err := MessageKAT(cfg)
	if err != nil {
		t.Fatal(err)
	}
	return &blockHeaderBindingTestCircuit{
		Header:         message.BlockHeader,
		Root:           message.RawSignals[3],
		SubjectParent:  message.Finality.SubjectParentBlockHash,
		ExpectedHash:   message.Finality.BlockHeaderHash,
		FinalityHeight: message.Finality.Height,
		FinalityView:   message.Finality.BlockHeaderView,
	}
}
