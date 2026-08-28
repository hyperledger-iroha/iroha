// Command metadata generates deterministic, offline SCCP dependency metadata.
package main

import (
	"bufio"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

const (
	schemaVersion = "SPDX-2.3"
	created       = "2026-08-28T00:00:00Z"
	moduleName    = "github.com/hyperledger-iroha/iroha/circuits/sccp"
)

type module struct {
	Path    string
	Version string
	Sum     string
}

type spdxDocument struct {
	SPDXVersion       string             `json:"spdxVersion"`
	DataLicense       string             `json:"dataLicense"`
	SPDXID            string             `json:"SPDXID"`
	Name              string             `json:"name"`
	DocumentNamespace string             `json:"documentNamespace"`
	CreationInfo      spdxCreationInfo   `json:"creationInfo"`
	Packages          []spdxPackage      `json:"packages"`
	Relationships     []spdxRelationship `json:"relationships"`
}

type spdxCreationInfo struct {
	Created  string   `json:"created"`
	Creators []string `json:"creators"`
}

type spdxPackage struct {
	Name             string            `json:"name"`
	SPDXID           string            `json:"SPDXID"`
	VersionInfo      string            `json:"versionInfo"`
	DownloadLocation string            `json:"downloadLocation"`
	FilesAnalyzed    bool              `json:"filesAnalyzed"`
	LicenseConcluded string            `json:"licenseConcluded"`
	LicenseDeclared  string            `json:"licenseDeclared"`
	CopyrightText    string            `json:"copyrightText"`
	Checksums        []spdxChecksum    `json:"checksums,omitempty"`
	ExternalRefs     []spdxExternalRef `json:"externalRefs,omitempty"`
}

type spdxChecksum struct {
	Algorithm     string `json:"algorithm"`
	ChecksumValue string `json:"checksumValue"`
}

type spdxExternalRef struct {
	ReferenceCategory string `json:"referenceCategory"`
	ReferenceType     string `json:"referenceType"`
	ReferenceLocator  string `json:"referenceLocator"`
}

type spdxRelationship struct {
	SPDXElementID      string `json:"spdxElementId"`
	RelationshipType   string `json:"relationshipType"`
	RelatedSPDXElement string `json:"relatedSpdxElement"`
}

type inventory struct {
	Schema     string          `json:"schema"`
	Version    int             `json:"version"`
	Root       string          `json:"root"`
	Files      []inventoryFile `json:"files"`
	RootSHA256 string          `json:"root_sha256"`
}

type inventoryFile struct {
	Path   string `json:"path"`
	Size   int64  `json:"size"`
	SHA256 string `json:"sha256"`
}

func main() {
	if len(os.Args) < 2 {
		fatal("usage: metadata <sbom|inventory> [flags]")
	}
	switch os.Args[1] {
	case "sbom":
		sbom(os.Args[2:])
	case "inventory":
		fileInventory(os.Args[2:])
	default:
		fatal("unknown metadata command %q", os.Args[1])
	}
}

func sbom(arguments []string) {
	flags := flag.NewFlagSet("sbom", flag.ExitOnError)
	modulesPath := flags.String("modules", "vendor/modules.txt", "vendored module inventory")
	sumsPath := flags.String("sums", "go.sum", "Go checksum database lock")
	output := flags.String("output", "-", "output path or - for stdout")
	mustParse(flags, arguments)

	modules, err := readModules(*modulesPath, *sumsPath)
	if err != nil {
		fatal("build SBOM: %v", err)
	}
	document := makeSPDX(modules)
	if err := encodeExclusive(*output, document); err != nil {
		fatal("write SBOM: %v", err)
	}
}

func fileInventory(arguments []string) {
	flags := flag.NewFlagSet("inventory", flag.ExitOnError)
	root := flags.String("root", "vendor", "directory to inventory")
	output := flags.String("output", "-", "output path or - for stdout")
	mustParse(flags, arguments)

	manifest, err := makeInventory(*root)
	if err != nil {
		fatal("build inventory: %v", err)
	}
	if err := encodeExclusive(*output, manifest); err != nil {
		fatal("write inventory: %v", err)
	}
}

func mustParse(flags *flag.FlagSet, arguments []string) {
	if err := flags.Parse(arguments); err != nil {
		fatal("parse arguments: %v", err)
	}
	if flags.NArg() != 0 {
		fatal("unexpected positional arguments")
	}
}

func readModules(modulesPath, sumsPath string) ([]module, error) {
	sums, err := readSums(sumsPath)
	if err != nil {
		return nil, err
	}
	file, err := os.Open(modulesPath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var modules []module
	seen := make(map[string]struct{})
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 3 || fields[0] != "#" || !strings.HasPrefix(fields[2], "v") {
			continue
		}
		key := fields[1] + " " + fields[2]
		if _, duplicate := seen[key]; duplicate {
			return nil, fmt.Errorf("duplicate vendored module %q", key)
		}
		seen[key] = struct{}{}
		sum, ok := sums[key]
		if !ok {
			return nil, fmt.Errorf("vendored module %q has no go.sum content hash", key)
		}
		modules = append(modules, module{Path: fields[1], Version: fields[2], Sum: sum})
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	if len(modules) == 0 {
		return nil, errors.New("empty vendored module inventory")
	}
	sort.Slice(modules, func(i, j int) bool { return modules[i].Path < modules[j].Path })
	return modules, nil
}

func readSums(path string) (map[string]string, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	result := make(map[string]string)
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) != 3 || strings.HasSuffix(fields[1], "/go.mod") {
			continue
		}
		if !strings.HasPrefix(fields[2], "h1:") {
			return nil, fmt.Errorf("unsupported module checksum for %s %s", fields[0], fields[1])
		}
		result[fields[0]+" "+fields[1]] = fields[2]
	}
	return result, scanner.Err()
}

func makeSPDX(modules []module) spdxDocument {
	identity := sha256.New()
	for _, dependency := range modules {
		fmt.Fprintf(identity, "%s\x00%s\x00%s\n", dependency.Path, dependency.Version, dependency.Sum)
	}
	namespace := "https://hyperledger.org/iroha/sbom/sccp-final-v1/" + hex.EncodeToString(identity.Sum(nil))
	rootID := "SPDXRef-Package-SCCP-Circuits"
	document := spdxDocument{
		SPDXVersion:       schemaVersion,
		DataLicense:       "CC0-1.0",
		SPDXID:            "SPDXRef-DOCUMENT",
		Name:              "iroha-sccp-circuits-final-v1",
		DocumentNamespace: namespace,
		CreationInfo: spdxCreationInfo{
			Created:  created,
			Creators: []string{"Organization: Hyperledger Iroha", "Tool: circuits/sccp/tools/metadata"},
		},
		Packages: []spdxPackage{{
			Name:             moduleName,
			SPDXID:           rootID,
			VersionInfo:      "final-v1-pre-release",
			DownloadLocation: "NOASSERTION",
			FilesAnalyzed:    false,
			LicenseConcluded: "Apache-2.0",
			LicenseDeclared:  "Apache-2.0",
			CopyrightText:    "NOASSERTION",
		}},
		Relationships: []spdxRelationship{{
			SPDXElementID:      "SPDXRef-DOCUMENT",
			RelationshipType:   "DESCRIBES",
			RelatedSPDXElement: rootID,
		}},
	}
	for _, dependency := range modules {
		id := spdxID(dependency.Path, dependency.Version)
		checksum := decodeGoSum(dependency.Sum)
		document.Packages = append(document.Packages, spdxPackage{
			Name:             dependency.Path,
			SPDXID:           id,
			VersionInfo:      dependency.Version,
			DownloadLocation: "NOASSERTION",
			FilesAnalyzed:    false,
			LicenseConcluded: "NOASSERTION",
			LicenseDeclared:  "NOASSERTION",
			CopyrightText:    "NOASSERTION",
			Checksums:        []spdxChecksum{{Algorithm: "SHA256", ChecksumValue: checksum}},
			ExternalRefs: []spdxExternalRef{{
				ReferenceCategory: "PACKAGE-MANAGER",
				ReferenceType:     "purl",
				ReferenceLocator:  "pkg:golang/" + dependency.Path + "@" + dependency.Version,
			}},
		})
		document.Relationships = append(document.Relationships, spdxRelationship{
			SPDXElementID:      rootID,
			RelationshipType:   "DEPENDS_ON",
			RelatedSPDXElement: id,
		})
	}
	return document
}

func decodeGoSum(sum string) string {
	raw, err := base64.StdEncoding.DecodeString(strings.TrimPrefix(sum, "h1:"))
	if err != nil || len(raw) != sha256.Size {
		fatal("invalid Go module h1 checksum %q", sum)
	}
	return hex.EncodeToString(raw)
}

var nonSPDX = regexp.MustCompile(`[^A-Za-z0-9.-]+`)

func spdxID(path, version string) string {
	return "SPDXRef-GoModule-" + nonSPDX.ReplaceAllString(path+"-"+version, "-")
}

func makeInventory(root string) (inventory, error) {
	root = filepath.Clean(root)
	info, err := os.Lstat(root)
	if err != nil {
		return inventory{}, err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return inventory{}, errors.New("inventory root must be a real directory")
	}

	var files []inventoryFile
	err = filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("symlink is forbidden in inventory: %s", path)
		}
		if entry.IsDir() {
			return nil
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("non-regular file is forbidden in inventory: %s", path)
		}
		digest, err := hashFile(path)
		if err != nil {
			return err
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		files = append(files, inventoryFile{Path: filepath.ToSlash(relative), Size: info.Size(), SHA256: digest})
		return nil
	})
	if err != nil {
		return inventory{}, err
	}
	sort.Slice(files, func(i, j int) bool { return files[i].Path < files[j].Path })
	rootHash := sha256.New()
	for _, file := range files {
		fmt.Fprintf(rootHash, "%d:%s:%d:%s\n", len(file.Path), file.Path, file.Size, file.SHA256)
	}
	return inventory{
		Schema:     "sccp-vendor-inventory-final-v1",
		Version:    1,
		Root:       filepath.ToSlash(root),
		Files:      files,
		RootSHA256: hex.EncodeToString(rootHash.Sum(nil)),
	}, nil
}

func hashFile(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	digest := sha256.New()
	if _, err := io.Copy(digest, file); err != nil {
		return "", err
	}
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func encodeExclusive(path string, value any) error {
	var output io.Writer = os.Stdout
	var file *os.File
	if path != "-" {
		var err error
		file, err = os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			return err
		}
		defer file.Close()
		output = file
	}
	encoder := json.NewEncoder(output)
	encoder.SetEscapeHTML(false)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}

func fatal(format string, arguments ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", arguments...)
	os.Exit(1)
}
