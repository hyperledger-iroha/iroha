package circuit

import (
	"encoding/binary"
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
	"github.com/consensys/gnark/test"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

// transferWireTestCircuit isolates canonical transfer parsing from finality.
type transferWireTestCircuit struct {
	Payload       [MaxCanonicalPayloadBytes]uints.U8
	PayloadLength frontend.Variable
	SenderI105    I105Witness
	cfg           profile.Config
}

func (c *transferWireTestCircuit) Define(api frontend.API) error {
	message := MessageCircuit{Payload: c.Payload, PayloadLength: c.PayloadLength, SenderI105: c.SenderI105, cfg: c.cfg}
	return message.constrainTransfer(api)
}

func TestTransferParserAcceptsCanonicalWireIDsAndRejectsCompactedAliases(t *testing.T) {
	for _, lane := range []struct {
		name     string
		domain   uint32
		codec    byte
		oldCodec byte
		curve    ecc.ID
	}{
		{"ethereum", 1, 2, 1, ecc.BN254},
		{"bsc", 2, 2, 1, ecc.BN254},
		{"tron", 5, 5, 2, ecc.BN254},
		{"ton", 4, 7, 3, ecc.BLS12_381},
	} {
		t.Run(lane.name, func(t *testing.T) {
			cfg, err := profile.ByID("sccp-final-v1-" + lane.name + "-mainnet-message")
			if err != nil {
				t.Fatal(err)
			}
			base := transferWireTestCircuit{cfg: cfg}
			zeroU8s(base.Payload[:])
			sender, err := populateI105Witness(&base.SenderI105, katI105CanonicalAccount[:])
			if err != nil {
				t.Fatal(err)
			}
			payload := canonicalKATPayload(cfg, sender)
			// Expected bytes come from the canonical Rust wire namespace, not cfg.
			payload[0] = 2
			binary.LittleEndian.PutUint32(payload[6:10], lane.domain)
			recipientOffset := 55 + len(sender)
			routeOffset := recipientOffset + 5 + cfg.RecipientLength
			payload[26], payload[50], payload[recipientOffset], payload[routeOffset] = 1, 1, lane.codec, 1
			copyU8(base.Payload[:], payload)
			base.PayloadLength = len(payload)
			definition := &transferWireTestCircuit{cfg: cfg}
			if err := test.IsSolved(definition, &base, lane.curve.ScalarField()); err != nil {
				t.Fatalf("canonical Rust wire identifiers rejected: %v", err)
			}
			mutations := []struct {
				name   string
				offset int
				value  byte
			}{
				{"payload tag", 0, 0}, {"asset codec", 26, 0}, {"sender codec", 50, 0},
				{"recipient codec", recipientOffset, lane.oldCodec}, {"route codec", routeOffset, 0},
			}
			if lane.name == "tron" {
				mutations = append(mutations, struct {
					name   string
					offset int
					value  byte
				}{"TRON domain", 6, 3})
			}
			for _, mutation := range mutations {
				t.Run(mutation.name, func(t *testing.T) {
					candidate := base
					candidate.Payload[mutation.offset] = uints.NewU8(mutation.value)
					if err := test.IsSolved(definition, &candidate, lane.curve.ScalarField()); err == nil {
						t.Fatalf("retired compact %s accepted", mutation.name)
					}
				})
			}
		})
	}
}
