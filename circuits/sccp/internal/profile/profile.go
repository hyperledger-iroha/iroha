// Package profile defines the closed SCCP final-V1 circuit catalogue.
package profile

import (
	"crypto/sha256"
	"fmt"
)

// Role is the immutable semantic role of a circuit.
type Role string

const (
	// Message authenticates one outbound transfer and its finality proof.
	Message Role = "message"
	// EpochAnchorUpdate authenticates one exact epoch transition and anchor activation.
	EpochAnchorUpdate Role = "epoch-anchor-update"
)

// Curve is the Groth16 curve used by a destination runtime.
type Curve string

const (
	// BN254 is used by EVM, BSC, and TRON destination runtimes.
	BN254 Curve = "bn254"
	// BLS12381 is used by the TON destination runtime.
	BLS12381 Curve = "bls12-381"
)

// SignalHash selects the final-V1 public-signal construction.
type SignalHash string

const (
	// KeccakSignal is keccak256(keccak256(label) || value) reduced in the outer field.
	KeccakSignal SignalHash = "keccak256"
	// SHA256Signal is sha256(sha256(label) || value) reduced in the outer field.
	SHA256Signal SignalHash = "sha256"
)

// Final-V1 canonical network tags. These bytes are part of the authenticated
// network encodings and deliberately do not reuse the retired compact 1..5
// assignments. TargetDomain is a separately typed public-signal value and
// must never be substituted for one of these tags.
const (
	SoraNetworkTag     byte = 0x40
	EthereumNetworkTag byte = 0x41
	BSCNetworkTag      byte = 0x42
	TRONNetworkTag     byte = 0x43
	TONNetworkTag      byte = 0x44
)

// Final-V1 domain identifiers. These are the exact u32 values encoded in the
// canonical SCCP payload and exposed as public signals.
const (
	SoraDomain     uint32 = 0
	EthereumDomain uint32 = 1
	BSCDomain      uint32 = 2
	TRONDomain     uint32 = 3
	TONDomain      uint32 = 4
)

// Final-V1 canonical payload codec identifiers.
const (
	CanonicalTextCodec byte = 0
	EVMAddress20Codec  byte = 1
	TRONAddress21Codec byte = 2
	TONAccount36Codec  byte = 3
)

// Final-V1 closed union tags used by the canonical payload and hub commitment.
const (
	TransferPayloadDiscriminant byte = 0
	TransferHubMessageKind      byte = 0
)

// Config is compile-time configuration for one fixed circuit.
type Config struct {
	ID               string
	Lane             string
	Role             Role
	Curve            Curve
	SignalHash       SignalHash
	TargetDomain     uint32
	TargetNetworkTag byte
	BackendTag       byte
	RecipientCodec   byte
	RecipientLength  int
	RouteID          string
	Phase1CeremonyID string
	Phase2CeremonyID string
	IndependentKeyID [32]byte
}

var catalogue = buildCatalogue()

func buildCatalogue() map[string]Config {
	lanes := []Config{
		{Lane: "ethereum-mainnet", Curve: BN254, SignalHash: KeccakSignal, TargetDomain: EthereumDomain, TargetNetworkTag: EthereumNetworkTag, BackendTag: 0, RecipientCodec: EVMAddress20Codec, RecipientLength: 20, RouteID: "taira_eth_xor", Phase1CeremonyID: "sccp-final-v1-bn254-phase1"},
		{Lane: "bsc-mainnet", Curve: BN254, SignalHash: KeccakSignal, TargetDomain: BSCDomain, TargetNetworkTag: BSCNetworkTag, BackendTag: 0, RecipientCodec: EVMAddress20Codec, RecipientLength: 20, RouteID: "taira_bsc_xor", Phase1CeremonyID: "sccp-final-v1-bn254-phase1"},
		{Lane: "tron-mainnet", Curve: BN254, SignalHash: KeccakSignal, TargetDomain: TRONDomain, TargetNetworkTag: TRONNetworkTag, BackendTag: 1, RecipientCodec: TRONAddress21Codec, RecipientLength: 21, RouteID: "taira_tron_xor", Phase1CeremonyID: "sccp-final-v1-bn254-phase1"},
		{Lane: "ton-mainnet", Curve: BLS12381, SignalHash: SHA256Signal, TargetDomain: TONDomain, TargetNetworkTag: TONNetworkTag, BackendTag: 3, RecipientCodec: TONAccount36Codec, RecipientLength: 36, RouteID: "taira_ton_xor", Phase1CeremonyID: "sccp-final-v1-bls12-381-phase1"},
	}
	out := make(map[string]Config, 8)
	for _, lane := range lanes {
		for _, role := range []Role{Message, EpochAnchorUpdate} {
			cfg := lane
			cfg.Role = role
			cfg.ID = fmt.Sprintf("sccp-final-v1-%s-%s", lane.Lane, role)
			cfg.Phase2CeremonyID = cfg.ID + "-phase2"
			cfg.IndependentKeyID = sha256.Sum256([]byte("sccp:groth16:independent-key:v1\x00" + cfg.ID))
			out[cfg.ID] = cfg
		}
	}
	return out
}

// All returns the eight fixed profiles in deterministic ID order.
func All() []Config {
	ids := []string{
		"sccp-final-v1-bsc-mainnet-epoch-anchor-update",
		"sccp-final-v1-bsc-mainnet-message",
		"sccp-final-v1-ethereum-mainnet-epoch-anchor-update",
		"sccp-final-v1-ethereum-mainnet-message",
		"sccp-final-v1-ton-mainnet-epoch-anchor-update",
		"sccp-final-v1-ton-mainnet-message",
		"sccp-final-v1-tron-mainnet-epoch-anchor-update",
		"sccp-final-v1-tron-mainnet-message",
	}
	out := make([]Config, 0, len(ids))
	for _, id := range ids {
		out = append(out, catalogue[id])
	}
	return out
}

// ByID resolves an exact closed profile. Caller-provided curve, role, and lane
// combinations are deliberately unsupported.
func ByID(id string) (Config, error) {
	cfg, ok := catalogue[id]
	if !ok {
		return Config{}, fmt.Errorf("unknown SCCP final-V1 circuit profile %q", id)
	}
	return cfg, nil
}

// ValidateClosed rejects a configuration that does not byte-for-byte match
// the immutable catalogue entry named by its ID. Constructors call this after
// receiving a Config so a caller cannot copy an entry and drift its curve,
// lane, constants, or independent key domain.
func ValidateClosed(cfg Config) error {
	canonical, err := ByID(cfg.ID)
	if err != nil {
		return err
	}
	if cfg != canonical {
		return fmt.Errorf("SCCP final-V1 circuit profile %q differs from the closed catalogue", cfg.ID)
	}
	return nil
}
