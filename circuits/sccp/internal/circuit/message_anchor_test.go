package circuit

import (
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
	"github.com/consensys/gnark/test"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

// messageAnchorTestCircuit isolates checkpoint authorization from the larger
// finality circuit so each redundant identity field is tested independently.
type messageAnchorTestCircuit struct {
	Anchor       AnchorWitness
	Finality     FinalityWitness
	ExpectedHash [32]uints.U8
}

func (c *messageAnchorTestCircuit) Define(api frontend.API) error {
	message := MessageCircuit{Anchor: c.Anchor, Finality: c.Finality}
	message.RawSignals[10] = c.ExpectedHash
	return message.constrainAnchor(api)
}

func TestMessageAnchorBindsSameHeightCheckpointIdentity(t *testing.T) {
	for _, lane := range []struct {
		profile string
		curve   ecc.ID
	}{
		{"sccp-final-v1-ton-mainnet-message", ecc.BLS12_381},
		{"sccp-final-v1-ethereum-mainnet-message", ecc.BN254},
	} {
		t.Run(lane.profile, func(t *testing.T) {
			cfg, err := profile.ByID(lane.profile)
			if err != nil {
				t.Fatal(err)
			}
			_, message, err := MessageKAT(cfg)
			if err != nil {
				t.Fatal(err)
			}
			definition := &messageAnchorTestCircuit{}
			base := messageAnchorTestCircuit{
				Anchor:       message.Anchor,
				Finality:     message.Finality,
				ExpectedHash: message.RawSignals[10],
			}
			field := lane.curve.ScalarField()
			if err := test.IsSolved(definition, &base, field); err != nil {
				t.Fatalf("earlier checkpoint must authorize its epoch roster: %v", err)
			}

			base.Anchor.CheckpointHeight = base.Finality.Height
			base.Anchor.CheckpointBlockHash = base.Finality.BlockHeaderHash
			base.Anchor.CheckpointContextID = base.Finality.HeightContextID
			base.Anchor.CheckpointFinalityArtifactHash = base.Finality.FinalityArtifactHash
			set32(&base.ExpectedHash, nativeAnchorHash(
				concreteKATUint64(base.Anchor.CheckpointHeight),
				concreteKATUint64(base.Anchor.Epoch),
				concreteKATUint64(base.Anchor.EpochEndHeight),
				u8Array32(base.Anchor.RosterCommitment),
				u8Array32(base.Anchor.CheckpointBlockHash),
				u8Array32(base.Anchor.CheckpointContextID),
				u8Array32(base.Anchor.CheckpointFinalityArtifactHash),
			))
			if err := test.IsSolved(definition, &base, field); err != nil {
				t.Fatalf("matching same-height checkpoint must be accepted: %v", err)
			}

			for _, mutation := range []struct {
				name   string
				mutate func(*FinalityWitness)
			}{
				{"block hash", func(finality *FinalityWitness) {
					finality.BlockHeaderHash[0].Val = finality.BlockHeaderHash[0].Val.(uint8) ^ 1
				}},
				{"context identity", func(finality *FinalityWitness) {
					finality.HeightContextID[0].Val = finality.HeightContextID[0].Val.(uint8) ^ 1
				}},
				{"finality artifact hash", func(finality *FinalityWitness) {
					finality.FinalityArtifactHash[0].Val = finality.FinalityArtifactHash[0].Val.(uint8) ^ 1
				}},
			} {
				t.Run(mutation.name, func(t *testing.T) {
					candidate := base
					mutation.mutate(&candidate.Finality)
					if err := test.IsSolved(definition, &candidate, field); err == nil {
						t.Fatalf("same-height checkpoint accepted mismatched %s", mutation.name)
					}
				})
			}
		})
	}
}
