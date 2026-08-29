package main

import (
	"os"
	"path/filepath"
	"testing"
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
