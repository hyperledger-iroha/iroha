package profile

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"testing"
)

const semanticCoverageRule = "No R1CS, proving key, verification key, verifier, deployment, or release evidence produced from this source is production-admissible while production_ready is false or any blocking_missing_constraints entry remains."

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

func TestCatalogueUsesTheClosedWireDomainAndCodecInventory(t *testing.T) {
	expected := map[string]struct {
		domain  uint32
		backend byte
		codec   byte
	}{
		"ethereum-mainnet": {EthereumDomain, EVMBackendTag, EVMAddress20Codec},
		"bsc-mainnet":      {BSCDomain, EVMBackendTag, EVMAddress20Codec},
		"tron-mainnet":     {TRONDomain, TRONBackendTag, TRONAddress21Codec},
		"ton-mainnet":      {TONDomain, TONBackendTag, TONAccount36Codec},
	}
	if EVMBackendTag != 0 || TRONBackendTag != 1 || TONBackendTag != 2 {
		t.Fatal("final-V1 destination backend tags drifted")
	}
	for _, cfg := range All() {
		wire, ok := expected[cfg.Lane]
		if !ok {
			t.Fatalf("unexpected final-V1 lane %q", cfg.Lane)
		}
		if cfg.TargetDomain != wire.domain || cfg.BackendTag != wire.backend || cfg.RecipientCodec != wire.codec {
			t.Fatalf(
				"profile %q wire identity drifted: domain=%d backend=%d codec=%d, want domain=%d backend=%d codec=%d",
				cfg.ID,
				cfg.TargetDomain,
				cfg.BackendTag,
				cfg.RecipientCodec,
				wire.domain,
				wire.backend,
				wire.codec,
			)
		}
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

func TestCheckedInSemanticCoverageManifestIsStrictAndInformational(t *testing.T) {
	var coverage struct {
		Schema                     string          `json:"schema"`
		Version                    int             `json:"version"`
		ProductionReady            bool            `json:"production_ready"`
		Implemented                map[string]bool `json:"implemented"`
		BlockingMissingConstraints []string        `json:"blocking_missing_constraints"`
		Rule                       string          `json:"rule"`
	}
	decodeStrictManifest(
		t,
		filepath.Join("..", "..", "manifests", "semantic-coverage-final-v1.json"),
		&coverage,
	)
	if coverage.Schema != "sccp-semantic-coverage-final-v1" || coverage.Version != 1 || coverage.ProductionReady {
		t.Fatalf("semantic coverage has an unsafe header: %#v", coverage)
	}
	if len(coverage.BlockingMissingConstraints) == 0 || coverage.Rule != semanticCoverageRule {
		t.Fatalf("semantic coverage omitted fail-closed blockers/rule: %#v", coverage)
	}
	for name, implemented := range coverage.Implemented {
		if !implemented {
			t.Fatalf("implemented coverage entry %q is false instead of moving to blockers", name)
		}
	}
}

func TestCheckedInCeremonyPolicyCoversEveryClosedProfile(t *testing.T) {
	type phase1Policy struct {
		ID                   string `json:"id"`
		Curve                Curve  `json:"curve"`
		Contributions        int    `json:"contributions"`
		FutureBeaconRequired bool   `json:"future_beacon_required"`
	}
	type phase2Policy struct {
		ID            string `json:"id"`
		Profile       string `json:"profile"`
		Contributions int    `json:"contributions"`
	}
	var policy struct {
		Schema  string         `json:"schema"`
		Version int            `json:"version"`
		Phase1  []phase1Policy `json:"phase1"`
		Phase2  []phase2Policy `json:"phase2"`
		Beacon  struct {
			AnnouncementBeforeLast bool `json:"announcement_must_precede_last_contribution"`
			RevealAfterLast        bool `json:"reveal_must_follow_last_contribution"`
			MinimumEntropyBytes    int  `json:"minimum_entropy_bytes"`
		} `json:"future_beacon"`
		KeySeparation struct {
			Phase2PerProfile        bool `json:"phase2_per_profile"`
			ProvingVerifyingNoReuse bool `json:"proving_and_verifying_key_reuse_forbidden"`
		} `json:"key_separation"`
		Invalidation              []string `json:"invalidation"`
		RequiredIndependentAudits []string `json:"required_independent_audits"`
		MaximumUnresolvedSeverity string   `json:"maximum_unresolved_severity"`
	}
	decodeStrictManifest(
		t,
		filepath.Join("..", "..", "manifests", "ceremony-policy-final-v1.json"),
		&policy,
	)
	if policy.Schema != "sccp-ceremony-policy-final-v1" || policy.Version != 1 || len(policy.Phase1) != 2 {
		t.Fatalf("ceremony policy header/phase-1 set mismatch: %#v", policy)
	}
	expectedPhase1 := map[string]Curve{
		"sccp-final-v1-bn254-phase1":     BN254,
		"sccp-final-v1-bls12-381-phase1": BLS12381,
	}
	for _, phase1 := range policy.Phase1 {
		curve, ok := expectedPhase1[phase1.ID]
		if !ok || phase1.Curve != curve || phase1.Contributions != 8 || !phase1.FutureBeaconRequired {
			t.Fatalf("invalid phase-1 policy: %#v", phase1)
		}
		delete(expectedPhase1, phase1.ID)
	}
	if len(expectedPhase1) != 0 || len(policy.Phase2) != len(All()) {
		t.Fatal("ceremony policy omits a phase-1 or phase-2 profile")
	}
	phase2ByProfile := make(map[string]phase2Policy, len(policy.Phase2))
	for _, phase2 := range policy.Phase2 {
		if _, duplicate := phase2ByProfile[phase2.Profile]; duplicate {
			t.Fatalf("duplicate phase-2 profile %q", phase2.Profile)
		}
		phase2ByProfile[phase2.Profile] = phase2
	}
	for _, cfg := range All() {
		phase2, ok := phase2ByProfile[cfg.ID]
		if !ok || phase2.ID != cfg.Phase2CeremonyID || phase2.Contributions != 8 {
			t.Fatalf("missing or invalid phase-2 policy for %q: %#v", cfg.ID, phase2)
		}
	}
	if !policy.Beacon.AnnouncementBeforeLast || !policy.Beacon.RevealAfterLast ||
		policy.Beacon.MinimumEntropyBytes < 32 || !policy.KeySeparation.Phase2PerProfile ||
		!policy.KeySeparation.ProvingVerifyingNoReuse ||
		policy.MaximumUnresolvedSeverity != "low" ||
		len(policy.RequiredIndependentAudits) != 3 || len(policy.Invalidation) < 6 {
		t.Fatalf("ceremony policy weakens final-V1 closure requirements: %#v", policy)
	}
}

func decodeStrictManifest(t *testing.T, path string, value any) {
	t.Helper()
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	decoder := json.NewDecoder(file)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(value); err != nil {
		t.Fatal(err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatalf("manifest has trailing content: %v", err)
	}
}
