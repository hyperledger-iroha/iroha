package circuit

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
)

// definitionSourceClosure binds the checked-in measurement to its complete
// non-test definition sources and pinned dependency inventory. Each sorted
// entry contains its u32-LE path length, path, u64-LE content length, and SHA-256.
func definitionSourceClosure(root string) (string, error) {
	paths := []string{"go.mod", "go.sum", "vendor/modules.txt", "vendor-inventory-final-v1.json"}
	for _, directory := range []string{"internal/circuit", "internal/profile"} {
		err := filepath.WalkDir(filepath.Join(root, filepath.FromSlash(directory)), func(path string, entry fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if entry.Type()&os.ModeSymlink != 0 {
				return fmt.Errorf("definition source must not be a symlink: %s", path)
			}
			if entry.IsDir() || !strings.HasSuffix(path, ".go") || strings.HasSuffix(path, "_test.go") {
				return nil
			}
			relative, err := filepath.Rel(root, path)
			if err != nil {
				return err
			}
			paths = append(paths, filepath.ToSlash(relative))
			return nil
		})
		if err != nil {
			return "", err
		}
	}
	sort.Strings(paths)
	digest := sha256.New()
	_, _ = digest.Write([]byte("sccp:r1cs-definition-source-closure:v1\x00"))
	for _, relative := range paths {
		path := filepath.Join(root, filepath.FromSlash(relative))
		metadata, err := os.Lstat(path)
		if err != nil {
			return "", err
		}
		if !metadata.Mode().IsRegular() {
			return "", fmt.Errorf("definition source must be a regular file: %s", relative)
		}
		contents, err := os.ReadFile(path)
		if err != nil {
			return "", err
		}
		_, _ = digest.Write(binary.LittleEndian.AppendUint32(nil, uint32(len(relative))))
		_, _ = digest.Write([]byte(relative))
		_, _ = digest.Write(binary.LittleEndian.AppendUint64(nil, uint64(len(contents))))
		fileHash := sha256.Sum256(contents)
		_, _ = digest.Write(fileHash[:])
	}
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func TestDefinitionSourceClosureTracksSourcesAndDependencyPins(t *testing.T) {
	root := t.TempDir()
	for _, directory := range []string{"internal/circuit", "internal/profile", "vendor"} {
		if err := os.MkdirAll(filepath.Join(root, directory), 0o700); err != nil {
			t.Fatal(err)
		}
	}
	included := []string{
		"internal/circuit/message.go", "internal/profile/profile.go",
		"go.mod", "go.sum", "vendor/modules.txt", "vendor-inventory-final-v1.json",
	}
	for _, path := range included {
		if err := os.WriteFile(filepath.Join(root, path), []byte("original\n"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	baseline, err := definitionSourceClosure(root)
	if err != nil {
		t.Fatal(err)
	}
	for _, path := range included {
		t.Run(path, func(t *testing.T) {
			if err := os.WriteFile(filepath.Join(root, path), []byte("changed\n"), 0o600); err != nil {
				t.Fatal(err)
			}
			digest, err := definitionSourceClosure(root)
			if err != nil || digest == baseline {
				t.Fatalf("definition/dependency edit was not detected: digest=%s error=%v", digest, err)
			}
			if err := os.WriteFile(filepath.Join(root, path), []byte("original\n"), 0o600); err != nil {
				t.Fatal(err)
			}
		})
	}
	for _, path := range []string{"internal/circuit/message_test.go", "internal/profile/README.md"} {
		if err := os.WriteFile(filepath.Join(root, path), []byte("test or documentation\n"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	if digest, err := definitionSourceClosure(root); err != nil || digest != baseline {
		t.Fatalf("test/documentation edits changed definition identity: digest=%s error=%v", digest, err)
	}
	addition := filepath.Join(root, "internal/circuit/added.go")
	if err := os.WriteFile(addition, []byte("new definition\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if digest, err := definitionSourceClosure(root); err != nil || digest == baseline {
		t.Fatalf("new definition was not detected: digest=%s error=%v", digest, err)
	}
	if err := os.Remove(addition); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("message.go", addition); err != nil {
		t.Fatal(err)
	}
	if _, err := definitionSourceClosure(root); err == nil {
		t.Fatal("definition symlink was accepted")
	}
}
