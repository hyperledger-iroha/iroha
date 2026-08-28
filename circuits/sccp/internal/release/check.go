// Package release validates fail-closed SCCP circuit release closure.
package release

import (
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"reflect"
	"slices"
	"strings"
	"time"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

const (
	// PinnedBuilderDigest is the linux/amd64 platform manifest for the immutable builder.
	PinnedBuilderDigest = "sha256:58259daf0a27c150118663ef7452aa94d66a86d55e73b3443386146623f5364d"
	goVersion           = "go1.25.7"
	gnarkVersion        = "v0.16.3"
	coverageRule        = "No R1CS, proving key, verification key, verifier, deployment, or release evidence produced from this source is production-admissible while production_ready is false or any blocking_missing_constraints entry remains."
	// semanticImplementationComplete is deliberately a source constant, not a
	// manifest field or build flag. This revision cannot produce a production
	// closure while any exact consensus/cryptographic constraint is absent.
	semanticImplementationComplete = false
)

var requiredArtifacts = []string{
	"source_archive",
	"vendor_inventory",
	"toolchain_inventory",
	"sbom",
	"r1cs",
	"proving_key",
	"verifying_key",
	"phase1_transcript",
	"phase2_transcript",
	"witness_compiler",
	"prover",
	"fixed_key_verifier",
	"unique_kat",
}

var globallySharedArtifactRoles = []string{
	"source_archive",
	"vendor_inventory",
	"toolchain_inventory",
	"sbom",
}

// Coverage is the checked-in semantic constraint status.
type Coverage struct {
	Schema                     string          `json:"schema"`
	Version                    int             `json:"version"`
	ProductionReady            bool            `json:"production_ready"`
	Implemented                map[string]bool `json:"implemented"`
	BlockingMissingConstraints []string        `json:"blocking_missing_constraints"`
	Rule                       string          `json:"rule"`
}

// Artifact is one immutable content-addressed release input.
type Artifact struct {
	SHA256 string `json:"sha256"`
	Size   uint64 `json:"size"`
}

// Contribution is one independently signed MPC contribution.
type Contribution struct {
	ContributorID string    `json:"contributor_id"`
	ContributedAt time.Time `json:"contributed_at"`
	Receipt       Artifact  `json:"signed_receipt"`
}

// Beacon is a publicly announced future entropy beacon.
type Beacon struct {
	Announcement Artifact  `json:"signed_announcement"`
	AnnouncedAt  time.Time `json:"announced_at"`
	Reveal       Artifact  `json:"reveal"`
	RevealedAt   time.Time `json:"revealed_at"`
	EntropyBytes uint32    `json:"entropy_bytes"`
}

// Ceremony records exact Phase-1 or circuit-specific Phase-2 inputs.
type Ceremony struct {
	ID            string         `json:"id"`
	Contributions []Contribution `json:"contributions"`
	Beacon        *Beacon        `json:"future_beacon,omitempty"`
}

// CircuitClosure binds one fixed circuit to its unique keys and artifacts.
type CircuitClosure struct {
	ID               string              `json:"id"`
	IndependentKeyID string              `json:"independent_key_id"`
	Phase1           Ceremony            `json:"phase1"`
	Phase2           Ceremony            `json:"phase2"`
	Artifacts        map[string]Artifact `json:"artifacts"`
}

// Audit is one independent signed audit with no unresolved medium-or-higher findings.
type Audit struct {
	Role               string   `json:"role"`
	AuditorID          string   `json:"auditor_id"`
	SignedReport       Artifact `json:"signed_report"`
	UnresolvedCritical uint32   `json:"unresolved_critical"`
	UnresolvedHigh     uint32   `json:"unresolved_high"`
	UnresolvedMedium   uint32   `json:"unresolved_medium"`
}

// Closure is the only production release input shape accepted by this module.
type Closure struct {
	Schema               string           `json:"schema"`
	Version              int              `json:"version"`
	GoVersion            string           `json:"go_version"`
	GnarkVersion         string           `json:"gnark_version"`
	BuilderDigest        string           `json:"builder_digest"`
	GitCommit            string           `json:"git_commit"`
	SignedGitCommitProof Artifact         `json:"signed_git_commit_proof"`
	Circuits             []CircuitClosure `json:"circuits"`
	Audits               []Audit          `json:"audits"`
}

// CheckProduction validates semantic coverage and a complete externally
// supplied ceremony/audit closure. It never accepts a caller-selected raw VK.
func CheckProduction(coveragePath, closurePath string) error {
	if !semanticImplementationComplete {
		return errors.New("semantic circuit implementation is fail-closed in this source revision")
	}
	coverageBytes, err := os.ReadFile(coveragePath)
	if err != nil {
		return fmt.Errorf("read semantic coverage: %w", err)
	}
	var coverage Coverage
	if err := decodeStrict(coverageBytes, &coverage); err != nil {
		return fmt.Errorf("decode semantic coverage: %w", err)
	}
	if coverage.Schema != "sccp-semantic-coverage-final-v1" || coverage.Version != 1 {
		return errors.New("semantic coverage is not strict final-V1")
	}
	if !coverage.ProductionReady || len(coverage.BlockingMissingConstraints) != 0 {
		return fmt.Errorf("semantic circuit production is fail-closed: unresolved constraints: %s", strings.Join(coverage.BlockingMissingConstraints, ", "))
	}
	if coverage.Rule != coverageRule {
		return errors.New("semantic coverage does not carry the final-V1 fail-closed rule")
	}
	for name, complete := range coverage.Implemented {
		if !complete {
			return fmt.Errorf("semantic circuit production is fail-closed: coverage %q is incomplete", name)
		}
	}

	closureBytes, err := os.ReadFile(closurePath)
	if err != nil {
		return fmt.Errorf("read production closure: %w", err)
	}
	var closure Closure
	if err := decodeStrict(closureBytes, &closure); err != nil {
		return fmt.Errorf("decode production closure: %w", err)
	}
	return validateClosure(&closure)
}

func validateClosure(closure *Closure) error {
	if closure.Schema != "sccp-circuit-release-closure-final-v1" || closure.Version != 1 {
		return errors.New("circuit closure is not strict final-V1")
	}
	if closure.GoVersion != goVersion || closure.GnarkVersion != gnarkVersion {
		return errors.New("circuit closure toolchain does not match the pinned Go/gnark versions")
	}
	if closure.BuilderDigest != PinnedBuilderDigest {
		return errors.New("circuit closure builder is not the pinned linux/amd64 image")
	}
	if !isLowerHex(closure.GitCommit, 20) && !isLowerHex(closure.GitCommit, 32) {
		return errors.New("circuit closure Git commit is not a canonical full hash")
	}
	if err := validateArtifact("signed_git_commit_proof", closure.SignedGitCommitProof); err != nil {
		return err
	}
	configs := profile.All()
	if len(closure.Circuits) != len(configs) {
		return fmt.Errorf("circuit closure has %d profiles; expected %d", len(closure.Circuits), len(configs))
	}
	byID := make(map[string]CircuitClosure, len(closure.Circuits))
	keyDigests := make(map[string]string)
	phase1ByID := make(map[string]Ceremony, 2)
	phase1ArtifactByID := make(map[string]Artifact, 2)
	phase1IDByArtifactDigest := make(map[string]string, 2)
	sharedArtifacts := make(map[string]Artifact, len(globallySharedArtifactRoles))
	for _, circuit := range closure.Circuits {
		if _, duplicate := byID[circuit.ID]; duplicate {
			return fmt.Errorf("duplicate circuit closure %q", circuit.ID)
		}
		byID[circuit.ID] = circuit
	}
	for _, cfg := range configs {
		circuit, ok := byID[cfg.ID]
		if !ok {
			return fmt.Errorf("missing circuit closure %q", cfg.ID)
		}
		expectedKeyID := hex.EncodeToString(cfg.IndependentKeyID[:])
		if circuit.IndependentKeyID != expectedKeyID {
			return fmt.Errorf("circuit %q independent key domain mismatch", cfg.ID)
		}
		if err := validateCeremony(circuit.Phase1, cfg.Phase1CeremonyID, true); err != nil {
			return fmt.Errorf("circuit %q phase 1: %w", cfg.ID, err)
		}
		if previous, ok := phase1ByID[cfg.Phase1CeremonyID]; ok {
			if !reflect.DeepEqual(previous, circuit.Phase1) {
				return fmt.Errorf("circuit %q supplies a conflicting shared phase-1 ceremony", cfg.ID)
			}
		} else {
			phase1ByID[cfg.Phase1CeremonyID] = circuit.Phase1
		}
		if err := validateCeremony(circuit.Phase2, cfg.Phase2CeremonyID, false); err != nil {
			return fmt.Errorf("circuit %q phase 2: %w", cfg.ID, err)
		}
		for index, contribution := range circuit.Phase2.Contributions {
			if !contribution.ContributedAt.After(circuit.Phase1.Beacon.RevealedAt) {
				return fmt.Errorf("circuit %q phase-2 contribution %d does not follow the completed phase-1 beacon", cfg.ID, index)
			}
		}
		if len(circuit.Artifacts) != len(requiredArtifacts) {
			return fmt.Errorf("circuit %q artifact inventory is not exact", cfg.ID)
		}
		artifactRolesByDigest := make(map[string]string, len(requiredArtifacts))
		for _, role := range requiredArtifacts {
			artifact, ok := circuit.Artifacts[role]
			if !ok {
				return fmt.Errorf("circuit %q is missing artifact %q", cfg.ID, role)
			}
			if err := validateArtifact(role, artifact); err != nil {
				return fmt.Errorf("circuit %q: %w", cfg.ID, err)
			}
			if previousRole, reused := artifactRolesByDigest[artifact.SHA256]; reused {
				return fmt.Errorf("circuit %q reuses one artifact digest for %q and %q", cfg.ID, previousRole, role)
			}
			artifactRolesByDigest[artifact.SHA256] = role
		}
		for _, role := range globallySharedArtifactRoles {
			artifact := circuit.Artifacts[role]
			if previous, found := sharedArtifacts[role]; found && previous != artifact {
				return fmt.Errorf("circuit %q supplies a conflicting globally shared %s", cfg.ID, role)
			}
			sharedArtifacts[role] = artifact
		}
		phase1Artifact := circuit.Artifacts["phase1_transcript"]
		if previous, ok := phase1ArtifactByID[cfg.Phase1CeremonyID]; ok {
			if previous != phase1Artifact {
				return fmt.Errorf("circuit %q supplies a conflicting shared phase-1 transcript", cfg.ID)
			}
		} else {
			phase1ArtifactByID[cfg.Phase1CeremonyID] = phase1Artifact
		}
		if previousID, reused := phase1IDByArtifactDigest[phase1Artifact.SHA256]; reused && previousID != cfg.Phase1CeremonyID {
			return fmt.Errorf("curve phase-1 ceremonies %q and %q reuse one transcript digest", previousID, cfg.Phase1CeremonyID)
		}
		phase1IDByArtifactDigest[phase1Artifact.SHA256] = cfg.Phase1CeremonyID
		for _, role := range []string{"proving_key", "verifying_key", "r1cs", "fixed_key_verifier", "phase2_transcript", "unique_kat"} {
			digest := circuit.Artifacts[role].SHA256
			if previous, reused := keyDigests[digest]; reused {
				return fmt.Errorf("circuits %q and %q reuse %s digest", previous, cfg.ID, role)
			}
			keyDigests[digest] = cfg.ID
		}
	}
	return validateAudits(closure.Audits)
}

func validateCeremony(ceremony Ceremony, expectedID string, phase1 bool) error {
	if ceremony.ID != expectedID {
		return fmt.Errorf("ceremony id %q does not match %q", ceremony.ID, expectedID)
	}
	if len(ceremony.Contributions) != 8 {
		return fmt.Errorf("ceremony has %d contributions; exactly 8 required", len(ceremony.Contributions))
	}
	seen := make(map[string]struct{}, 8)
	seenReceipts := make(map[string]struct{}, 8)
	var previousContributionTime time.Time
	for index, contribution := range ceremony.Contributions {
		if contribution.ContributorID == "" {
			return fmt.Errorf("contribution %d has no independently assigned identity", index)
		}
		if _, duplicate := seen[contribution.ContributorID]; duplicate {
			return fmt.Errorf("contributor %q is duplicated", contribution.ContributorID)
		}
		seen[contribution.ContributorID] = struct{}{}
		if contribution.ContributedAt.IsZero() {
			return fmt.Errorf("contribution %d has no signed contribution time", index)
		}
		if index > 0 && !contribution.ContributedAt.After(previousContributionTime) {
			return fmt.Errorf("contribution %d does not strictly follow the preceding contribution", index)
		}
		previousContributionTime = contribution.ContributedAt
		if err := validateArtifact("signed_contribution_receipt", contribution.Receipt); err != nil {
			return err
		}
		if _, duplicate := seenReceipts[contribution.Receipt.SHA256]; duplicate {
			return fmt.Errorf("contribution %d reuses another signed receipt", index)
		}
		seenReceipts[contribution.Receipt.SHA256] = struct{}{}
	}
	if phase1 {
		if ceremony.Beacon == nil {
			return errors.New("phase-1 ceremony has no future beacon")
		}
		if ceremony.Beacon.AnnouncedAt.IsZero() || ceremony.Beacon.RevealedAt.IsZero() {
			return errors.New("future beacon has no authenticated announcement or reveal time")
		}
		if !ceremony.Beacon.RevealedAt.After(ceremony.Beacon.AnnouncedAt) {
			return errors.New("future beacon reveal does not follow its public announcement")
		}
		if ceremony.Beacon.EntropyBytes < 32 {
			return errors.New("future beacon reveal carries fewer than 32 declared entropy bytes")
		}
		for index, contribution := range ceremony.Contributions {
			if !contribution.ContributedAt.After(ceremony.Beacon.AnnouncedAt) {
				return fmt.Errorf("contribution %d does not strictly follow the public future-beacon announcement", index)
			}
			if !ceremony.Beacon.RevealedAt.After(contribution.ContributedAt) {
				return fmt.Errorf("future beacon was not revealed after contribution %d", index)
			}
		}
		if err := validateArtifact("future_beacon_announcement", ceremony.Beacon.Announcement); err != nil {
			return err
		}
		if err := validateArtifact("future_beacon_reveal", ceremony.Beacon.Reveal); err != nil {
			return err
		}
	} else if ceremony.Beacon != nil {
		return errors.New("phase-2 ceremony must use the curve phase-1 beacon, not inject another beacon")
	}
	return nil
}

func validateAudits(audits []Audit) error {
	required := []string{"destination-integration", "reproducibility-ceremony", "semantic-cryptographic"}
	if len(audits) != len(required) {
		return fmt.Errorf("audit count is %d; expected 3", len(audits))
	}
	seenRoles := make([]string, 0, len(audits))
	seenAuditors := make(map[string]struct{}, len(audits))
	seenReports := make(map[string]string, len(audits))
	for _, audit := range audits {
		if audit.AuditorID == "" {
			return errors.New("audit has no independently assigned auditor identity")
		}
		if _, duplicate := seenAuditors[audit.AuditorID]; duplicate {
			return fmt.Errorf("auditor %q fills multiple independent roles", audit.AuditorID)
		}
		seenAuditors[audit.AuditorID] = struct{}{}
		seenRoles = append(seenRoles, audit.Role)
		if audit.UnresolvedCritical != 0 || audit.UnresolvedHigh != 0 || audit.UnresolvedMedium != 0 {
			return fmt.Errorf("audit %q has unresolved medium-or-higher findings", audit.Role)
		}
		if err := validateArtifact("signed_audit_report", audit.SignedReport); err != nil {
			return err
		}
		if previousRole, duplicate := seenReports[audit.SignedReport.SHA256]; duplicate {
			return fmt.Errorf("audit roles %q and %q reuse one signed report", previousRole, audit.Role)
		}
		seenReports[audit.SignedReport.SHA256] = audit.Role
	}
	slices.Sort(seenRoles)
	if !slices.Equal(seenRoles, required) {
		return errors.New("circuit closure does not contain the three exact independent audit roles")
	}
	return nil
}

func validateArtifact(role string, artifact Artifact) error {
	if !isLowerHex(artifact.SHA256, 32) || strings.Trim(artifact.SHA256, "0") == "" || artifact.Size == 0 {
		return fmt.Errorf("artifact %q has no canonical nonzero SHA-256 and positive size", role)
	}
	return nil
}

func isLowerHex(value string, bytes int) bool {
	if len(value) != bytes*2 || strings.ToLower(value) != value {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func decodeStrict(data []byte, target any) error {
	decoder := json.NewDecoder(strings.NewReader(string(data)))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return errors.New("trailing JSON values")
		}
		return fmt.Errorf("decode trailing JSON: %w", err)
	}
	return nil
}
