package release

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

func TestCheckedInCoverageFailsClosed(t *testing.T) {
	coverage := filepath.Join("..", "..", "manifests", "semantic-coverage-final-v1.json")
	err := CheckProduction(coverage, filepath.Join(t.TempDir(), "absent-closure.json"))
	if err == nil || !strings.Contains(err.Error(), "fail-closed") {
		t.Fatalf("expected fail-closed semantic coverage error, got %v", err)
	}
}

func TestCheckedInSemanticCoverageManifestIsStrictAndIncomplete(t *testing.T) {
	path := filepath.Join("..", "..", "manifests", "semantic-coverage-final-v1.json")
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var coverage Coverage
	if err := decodeStrict(encoded, &coverage); err != nil {
		t.Fatal(err)
	}
	if coverage.Schema != "sccp-semantic-coverage-final-v1" || coverage.Version != 1 || coverage.ProductionReady {
		t.Fatalf("semantic coverage has an unsafe header: %#v", coverage)
	}
	if len(coverage.BlockingMissingConstraints) == 0 || coverage.Rule != coverageRule {
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
		ID                   string        `json:"id"`
		Curve                profile.Curve `json:"curve"`
		Contributions        int           `json:"contributions"`
		FutureBeaconRequired bool          `json:"future_beacon_required"`
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
	path := filepath.Join("..", "..", "manifests", "ceremony-policy-final-v1.json")
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := decodeStrict(encoded, &policy); err != nil {
		t.Fatal(err)
	}
	if policy.Schema != "sccp-ceremony-policy-final-v1" || policy.Version != 1 || len(policy.Phase1) != 2 {
		t.Fatalf("ceremony policy header/phase-1 set mismatch: %#v", policy)
	}
	expectedPhase1 := map[string]profile.Curve{
		"sccp-final-v1-bn254-phase1":     profile.BN254,
		"sccp-final-v1-bls12-381-phase1": profile.BLS12381,
	}
	for _, phase1 := range policy.Phase1 {
		curve, ok := expectedPhase1[phase1.ID]
		if !ok || phase1.Curve != curve || phase1.Contributions != 8 || !phase1.FutureBeaconRequired {
			t.Fatalf("invalid phase-1 policy: %#v", phase1)
		}
		delete(expectedPhase1, phase1.ID)
	}
	if len(expectedPhase1) != 0 || len(policy.Phase2) != len(profile.All()) {
		t.Fatalf("ceremony policy omits a phase-1 or phase-2 profile")
	}
	phase2ByProfile := make(map[string]phase2Policy, len(policy.Phase2))
	for _, phase2 := range policy.Phase2 {
		if _, duplicate := phase2ByProfile[phase2.Profile]; duplicate {
			t.Fatalf("duplicate phase-2 profile %q", phase2.Profile)
		}
		phase2ByProfile[phase2.Profile] = phase2
	}
	for _, cfg := range profile.All() {
		phase2, ok := phase2ByProfile[cfg.ID]
		if !ok || phase2.ID != cfg.Phase2CeremonyID || phase2.Contributions != 8 {
			t.Fatalf("missing or invalid phase-2 policy for %q: %#v", cfg.ID, phase2)
		}
	}
	if !policy.Beacon.AnnouncementBeforeLast || !policy.Beacon.RevealAfterLast || policy.Beacon.MinimumEntropyBytes < 32 ||
		!policy.KeySeparation.Phase2PerProfile || !policy.KeySeparation.ProvingVerifyingNoReuse ||
		policy.MaximumUnresolvedSeverity != "low" || len(policy.RequiredIndependentAudits) != 3 || len(policy.Invalidation) < 6 {
		t.Fatalf("ceremony policy weakens final-V1 closure requirements: %#v", policy)
	}
}

func TestPinnedOfflineBuilderPolicyMatchesExecutableSources(t *testing.T) {
	var policy struct {
		Schema                  string   `json:"schema"`
		Version                 int      `json:"version"`
		Platform                string   `json:"platform"`
		GOAMD64                 string   `json:"goamd64"`
		BaseImage               string   `json:"base_image"`
		PlatformManifestSHA256  string   `json:"base_image_platform_manifest_sha256"`
		MultiarchIndexSHA256    string   `json:"base_image_multiarch_index_sha256"`
		GoVersion               string   `json:"go_version"`
		OfficialArchiveSHA256   string   `json:"official_go_linux_amd64_archive_sha256"`
		GnarkVersion            string   `json:"gnark_version"`
		GnarkModuleH1           string   `json:"gnark_module_h1"`
		Network                 string   `json:"network"`
		ModuleMode              string   `json:"module_mode"`
		GOENV                   string   `json:"goenv"`
		GOWORK                  string   `json:"gowork"`
		CGO                     bool     `json:"cgo"`
		SourceDateEpoch         int64    `json:"source_date_epoch"`
		TestTimeout             string   `json:"test_timeout"`
		BuildFlags              []string `json:"build_flags"`
		ProductionKeyGeneration bool     `json:"production_key_generation"`
		ProductionVKInjection   bool     `json:"production_verifying_key_injection"`
	}
	policyBytes, err := os.ReadFile(filepath.Join("..", "..", "builder", "policy-final-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	if err := decodeStrict(policyBytes, &policy); err != nil {
		t.Fatal(err)
	}
	if policy.Schema != "sccp-circuit-builder-policy-final-v1" || policy.Version != 1 ||
		policy.Platform != "linux/amd64" || policy.GOAMD64 != "v1" ||
		policy.BaseImage != "golang:1.25.7-bookworm" ||
		policy.PlatformManifestSHA256 != strings.TrimPrefix(PinnedBuilderDigest, "sha256:") ||
		len(policy.MultiarchIndexSHA256) != 64 || len(policy.OfficialArchiveSHA256) != 64 ||
		policy.GoVersion != goVersion || policy.GnarkVersion != gnarkVersion ||
		policy.GnarkModuleH1 == "" || policy.Network != "none" || policy.ModuleMode != "vendor" ||
		policy.GOENV != "off" || policy.GOWORK != "off" || policy.CGO ||
		policy.SourceDateEpoch <= 0 || policy.TestTimeout != "2h" ||
		policy.ProductionKeyGeneration || policy.ProductionVKInjection {
		t.Fatalf("unsafe or drifting builder policy: %#v", policy)
	}
	expectedFlags := []string{"-trimpath", "-buildvcs=false", "-ldflags=-buildid="}
	if !slices.Equal(policy.BuildFlags, expectedFlags) {
		t.Fatalf("builder flags drifted: %#v", policy.BuildFlags)
	}

	dockerfile, err := os.ReadFile(filepath.Join("..", "..", "builder", "Dockerfile"))
	if err != nil {
		t.Fatal(err)
	}
	for _, required := range []string{
		"FROM --platform=linux/amd64 " + policy.BaseImage + "@" + PinnedBuilderDigest + " AS build",
		"RUN --network=none",
		"GOAMD64=v1",
		"GOENV=off",
		"GOFLAGS=-mod=vendor",
		"GOPROXY=off",
		"GOSUMDB=off",
		"GOTOOLCHAIN=local",
		"GOWORK=off",
		"go test -count=1 -timeout=2h ./...",
		"go build -trimpath -buildvcs=false -ldflags=\"-buildid=\"",
		"sha256sum sccp-circuits > sccp-circuits.sha256",
		"COPY builder/policy-final-v1.json /metadata/builder-policy-final-v1.json",
	} {
		if !bytes.Contains(dockerfile, []byte(required)) {
			t.Fatalf("Dockerfile omits pinned builder invariant %q", required)
		}
	}
	buildScript, err := os.ReadFile(filepath.Join("..", "..", "builder", "build.sh"))
	if err != nil {
		t.Fatal(err)
	}
	for _, required := range []string{
		"--network none",
		"--no-cache",
		"--platform linux/amd64",
		"--provenance=false",
		"--sbom=false",
	} {
		if !bytes.Contains(buildScript, []byte(required)) {
			t.Fatalf("build script omits pinned builder invariant %q", required)
		}
	}
}

func TestPhase1CeremonyRequiresFutureBeaconAndIndependentReceipts(t *testing.T) {
	announcement := time.Date(2026, 8, 1, 0, 0, 0, 0, time.UTC)
	ceremony := Ceremony{
		ID: "phase1",
		Beacon: &Beacon{
			Announcement: testArtifact(0xa0),
			AnnouncedAt:  announcement,
			Reveal:       testArtifact(0xa1),
			RevealedAt:   announcement.Add(10 * time.Hour),
			EntropyBytes: 32,
		},
	}
	for index := 0; index < 8; index++ {
		ceremony.Contributions = append(ceremony.Contributions, Contribution{
			ContributorID: fmt.Sprintf("contributor-%d", index),
			ContributedAt: announcement.Add(time.Duration(index+1) * time.Hour),
			Receipt:       testArtifact(byte(index + 1)),
		})
	}
	if err := validateCeremony(ceremony, "phase1", true); err != nil {
		t.Fatalf("valid phase-1 ceremony rejected: %v", err)
	}

	shortEntropy := ceremony
	shortBeacon := *ceremony.Beacon
	shortBeacon.EntropyBytes = 31
	shortEntropy.Beacon = &shortBeacon
	if err := validateCeremony(shortEntropy, "phase1", true); err == nil {
		t.Fatal("short future-beacon entropy was accepted")
	}

	notFuture := ceremony
	notFuture.Contributions = append([]Contribution(nil), ceremony.Contributions...)
	notFuture.Contributions[0].ContributedAt = announcement
	if err := validateCeremony(notFuture, "phase1", true); err == nil {
		t.Fatal("contribution simultaneous with beacon announcement was accepted")
	}

	duplicateReceipt := ceremony
	duplicateReceipt.Contributions = append([]Contribution(nil), ceremony.Contributions...)
	duplicateReceipt.Contributions[1].Receipt = duplicateReceipt.Contributions[0].Receipt
	if err := validateCeremony(duplicateReceipt, "phase1", true); err == nil {
		t.Fatal("duplicate signed contribution receipt was accepted")
	}

	nonMonotonic := ceremony
	nonMonotonic.Contributions = append([]Contribution(nil), ceremony.Contributions...)
	nonMonotonic.Contributions[1].ContributedAt = nonMonotonic.Contributions[0].ContributedAt
	if err := validateCeremony(nonMonotonic, "phase1", true); err == nil {
		t.Fatal("non-monotonic contribution sequence was accepted")
	}
}

func TestValidateClosureAcceptsExactIndependentInventoryAndRejectsKATReuse(t *testing.T) {
	closure := validTestClosure(t)
	if err := validateClosure(&closure); err != nil {
		t.Fatalf("valid exact closure rejected: %v", err)
	}
	closure.Circuits[1].Artifacts["unique_kat"] = closure.Circuits[0].Artifacts["unique_kat"]
	if err := validateClosure(&closure); err == nil {
		t.Fatal("cross-profile KAT reuse was accepted")
	}
}

func validTestClosure(t *testing.T) Closure {
	t.Helper()
	base := time.Date(2026, 8, 1, 0, 0, 0, 0, time.UTC)
	phase1ByID := make(map[string]Ceremony, 2)
	for _, cfg := range profile.All() {
		if _, found := phase1ByID[cfg.Phase1CeremonyID]; !found {
			phase1ByID[cfg.Phase1CeremonyID] = testCeremony(cfg.Phase1CeremonyID, "phase1:"+cfg.Phase1CeremonyID, base, true)
		}
	}
	closure := Closure{
		Schema:               "sccp-circuit-release-closure-final-v1",
		Version:              1,
		GoVersion:            goVersion,
		GnarkVersion:         gnarkVersion,
		BuilderDigest:        PinnedBuilderDigest,
		GitCommit:            strings.Repeat("a", 40),
		SignedGitCommitProof: namedArtifact("signed-git-commit-proof"),
	}
	for _, cfg := range profile.All() {
		artifacts := make(map[string]Artifact, len(requiredArtifacts))
		for _, role := range requiredArtifacts {
			label := cfg.ID + ":" + role
			for _, sharedRole := range globallySharedArtifactRoles {
				if role == sharedRole {
					label = "global:" + role
				}
			}
			if role == "phase1_transcript" {
				label = "phase1-transcript:" + cfg.Phase1CeremonyID
			}
			artifacts[role] = namedArtifact(label)
		}
		closure.Circuits = append(closure.Circuits, CircuitClosure{
			ID:               cfg.ID,
			IndependentKeyID: fmt.Sprintf("%x", cfg.IndependentKeyID),
			Phase1:           phase1ByID[cfg.Phase1CeremonyID],
			Phase2:           testCeremony(cfg.Phase2CeremonyID, "phase2:"+cfg.ID, base.Add(24*time.Hour), false),
			Artifacts:        artifacts,
		})
	}
	for _, role := range []string{"semantic-cryptographic", "reproducibility-ceremony", "destination-integration"} {
		closure.Audits = append(closure.Audits, Audit{
			Role:         role,
			AuditorID:    "auditor:" + role,
			SignedReport: namedArtifact("audit:" + role),
		})
	}
	return closure
}

func testCeremony(id, prefix string, base time.Time, phase1 bool) Ceremony {
	ceremony := Ceremony{ID: id}
	for index := 0; index < 8; index++ {
		ceremony.Contributions = append(ceremony.Contributions, Contribution{
			ContributorID: fmt.Sprintf("%s:contributor:%d", prefix, index),
			ContributedAt: base.Add(time.Duration(index+1) * time.Hour),
			Receipt:       namedArtifact(fmt.Sprintf("%s:receipt:%d", prefix, index)),
		})
	}
	if phase1 {
		ceremony.Beacon = &Beacon{
			Announcement: namedArtifact(prefix + ":beacon-announcement"),
			AnnouncedAt:  base,
			Reveal:       namedArtifact(prefix + ":beacon-reveal"),
			RevealedAt:   base.Add(10 * time.Hour),
			EntropyBytes: 32,
		}
	}
	return ceremony
}

func namedArtifact(name string) Artifact {
	digest := sha256.Sum256([]byte(name))
	return Artifact{SHA256: fmt.Sprintf("%x", digest), Size: uint64(len(name)) + 1}
}

func testArtifact(marker byte) Artifact {
	return Artifact{SHA256: strings.Repeat(fmt.Sprintf("%02x", marker), 32), Size: 1}
}
