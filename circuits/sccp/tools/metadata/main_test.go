package main

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

const (
	pinnedBuilderDigest = "sha256:58259daf0a27c150118663ef7452aa94d66a86d55e73b3443386146623f5364d"
	pinnedGoVersion     = "go1.25.7"
	pinnedGnarkVersion  = "v0.16.3"
)

func TestInventoryIsDeterministicAndRejectsSymlink(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "b"), []byte("two"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "a"), []byte("one"), 0o600); err != nil {
		t.Fatal(err)
	}
	first, err := makeInventory(root)
	if err != nil {
		t.Fatal(err)
	}
	second, err := makeInventory(root)
	if err != nil {
		t.Fatal(err)
	}
	if first.RootSHA256 != second.RootSHA256 || len(first.Files) != 2 || first.Files[0].Path != "a" {
		t.Fatalf("inventory is not stable: %#v %#v", first, second)
	}
	if err := os.Symlink("a", filepath.Join(root, "link")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if _, err := makeInventory(root); err == nil {
		t.Fatal("symlink was accepted")
	}
}

func TestSPDXIncludesEveryModule(t *testing.T) {
	modules := []module{{Path: "example.test/module", Version: "v1.2.3", Sum: "h1:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}}
	document := makeSPDX(modules)
	if len(document.Packages) != 2 || len(document.Relationships) != 2 {
		t.Fatalf("unexpected SPDX closure: %#v", document)
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
	decoder := json.NewDecoder(bytes.NewReader(policyBytes))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&policy); err != nil {
		t.Fatal(err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatalf("builder policy has trailing content: %v", err)
	}
	if policy.Schema != "sccp-circuit-builder-policy-final-v1" || policy.Version != 1 ||
		policy.Platform != "linux/amd64" || policy.GOAMD64 != "v1" ||
		policy.BaseImage != "golang:1.25.7-bookworm" ||
		policy.PlatformManifestSHA256 != strings.TrimPrefix(pinnedBuilderDigest, "sha256:") ||
		len(policy.MultiarchIndexSHA256) != 64 || len(policy.OfficialArchiveSHA256) != 64 ||
		policy.GoVersion != pinnedGoVersion || policy.GnarkVersion != pinnedGnarkVersion ||
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
		"FROM --platform=linux/amd64 " + policy.BaseImage + "@" + pinnedBuilderDigest + " AS build",
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
