package profile

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestCatalogueIsClosedAndIndependentlyKeyed(t *testing.T) {
	profiles := All()
	if len(profiles) != 8 {
		t.Fatalf("profile count = %d, want 8", len(profiles))
	}
	seen := make(map[[32]byte]string, len(profiles))
	for _, cfg := range profiles {
		if previous, exists := seen[cfg.IndependentKeyID]; exists {
			t.Fatalf("profiles %q and %q share a key domain", previous, cfg.ID)
		}
		seen[cfg.IndependentKeyID] = cfg.ID
		if cfg.Phase2CeremonyID != cfg.ID+"-phase2" {
			t.Fatalf("profile %q has a non-canonical phase-2 id", cfg.ID)
		}
	}
	if _, err := ByID("caller-selected-profile"); err == nil {
		t.Fatal("unknown profile was accepted")
	}
}

func TestValidateClosedRejectsProfileDrift(t *testing.T) {
	cfg, err := ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	if err := ValidateClosed(cfg); err != nil {
		t.Fatalf("canonical profile rejected: %v", err)
	}
	mutations := []Config{cfg, cfg, cfg, cfg, cfg}
	mutations[0].TargetDomain++
	mutations[1].TargetNetworkTag = 1
	mutations[2].Curve = BLS12381
	mutations[3].RouteID = "attacker-route"
	mutations[4].IndependentKeyID[0] ^= 1
	for index, mutated := range mutations {
		if err := ValidateClosed(mutated); err == nil {
			t.Fatalf("profile mutation %d was accepted", index)
		}
	}
}

func TestFinalV1NetworkTagsAreExactAndNonCompact(t *testing.T) {
	want := map[string]byte{
		"ethereum-mainnet": EthereumNetworkTag,
		"bsc-mainnet":      BSCNetworkTag,
		"tron-mainnet":     TRONNetworkTag,
		"ton-mainnet":      TONNetworkTag,
	}
	if SoraNetworkTag != 0x40 || EthereumNetworkTag != 0x41 || BSCNetworkTag != 0x42 ||
		TRONNetworkTag != 0x43 || TONNetworkTag != 0x44 {
		t.Fatal("final-V1 network-tag constants drifted")
	}
	for _, cfg := range All() {
		if cfg.TargetNetworkTag != want[cfg.Lane] {
			t.Fatalf("profile %q network tag = %#x, want %#x", cfg.ID, cfg.TargetNetworkTag, want[cfg.Lane])
		}
		mutated := cfg
		mutated.TargetNetworkTag = cfg.TargetNetworkTag - 0x40
		if err := ValidateClosed(mutated); err == nil {
			t.Fatalf("profile %q accepted retired compact tag %#x", cfg.ID, mutated.TargetNetworkTag)
		}
	}
}

func TestCheckedInProfileManifestMatchesCatalogue(t *testing.T) {
	type manifestProfile struct {
		ID               string `json:"id"`
		Lane             string `json:"lane"`
		Role             Role   `json:"role"`
		Curve            Curve  `json:"curve"`
		SourceNetworkTag string `json:"source_network_tag"`
		TargetNetworkTag string `json:"target_network_tag"`
		Phase1           string `json:"phase1"`
		Phase2           string `json:"phase2"`
		IndependentKeyID string `json:"independent_key_id"`
	}
	var manifest struct {
		Schema   string            `json:"schema"`
		Version  int               `json:"version"`
		Profiles []manifestProfile `json:"profiles"`
	}
	path := filepath.Join("..", "..", "manifests", "profiles-final-v1.json")
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	decoder := json.NewDecoder(file)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&manifest); err != nil {
		t.Fatal(err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatalf("profile manifest has trailing content: %v", err)
	}
	configs := All()
	if manifest.Schema != "sccp-circuit-profiles-final-v1" || manifest.Version != 1 || len(manifest.Profiles) != len(configs) {
		t.Fatalf("profile manifest header/length mismatch: %#v", manifest)
	}
	for index, cfg := range configs {
		entry := manifest.Profiles[index]
		expectedKeyID := fmt.Sprintf("%x", cfg.IndependentKeyID)
		if entry.ID != cfg.ID || entry.Lane != cfg.Lane || entry.Role != cfg.Role || entry.Curve != cfg.Curve ||
			entry.SourceNetworkTag != fmt.Sprintf("0x%02x", SoraNetworkTag) ||
			entry.TargetNetworkTag != fmt.Sprintf("0x%02x", cfg.TargetNetworkTag) ||
			entry.Phase1 != cfg.Phase1CeremonyID || entry.Phase2 != cfg.Phase2CeremonyID || entry.IndependentKeyID != expectedKeyID {
			t.Fatalf("profile manifest entry %d differs from catalogue: %#v != %#v", index, entry, cfg)
		}
	}
}
