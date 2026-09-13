"""Execute the vendor build script through Cargo and check native rlib bundling."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest


ROOT = Path(__file__).resolve().parents[2]
BUILD_SCRIPT = ROOT / "vendor/pqcrypto-internals-0.2.11/build.rs"


def test_vendor_build_script_keeps_native_objects_in_rust_archive(tmp_path):
    cargo, rustc, ar = (shutil.which(name) for name in ("cargo", "rustc", "ar"))
    if not all((cargo, rustc, ar)):
        pytest.skip("Cargo, rustc and ar are required for the native linkage regression")

    def run(command, **kwargs):
        result = subprocess.run(command, capture_output=True, text=True, timeout=60, **kwargs)
        assert result.returncode == 0, result.stderr
        return result

    fixture = tmp_path / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "include").mkdir()
    native = tmp_path / "native"
    native.mkdir()
    # Real native objects make the archive inventory check independent of the
    # compiler's diagnostic text. Their bodies are fixtures, not crypto tests.
    for name in ("common", "keccak"):
        source = native / (name + ".rs")
        source.write_text(f'#![no_std]\n#[no_mangle]\npub extern "C" fn {name}_probe() {{}}\n')
        obj = native / (name + ".o")
        run([rustc, "--crate-type=lib", "--emit=obj", str(source), "-o", str(obj)])
        archives = ("pqclean_common",) if name == "common" else ("keccak2x", "keccak4x")
        for archive in archives:
            run([ar, "rcs", str(native / ("lib" + archive + ".a")), str(obj)])

    # The real vendor build.rs runs unchanged. These bounded build-dependency
    # shims stand in for C compilation only and preserve cc's link directives.
    shims = {
        "cc": '''pub struct Build;
impl Build {
 pub fn new() -> Self { Self }
 pub fn include<P: AsRef<std::path::Path>>(&mut self, _: P) -> &mut Self { self }
 pub fn file<P: AsRef<std::path::Path>>(&mut self, _: P) -> &mut Self { self }
 pub fn files<I, P>(&mut self, _: I) -> &mut Self
 where I: IntoIterator<Item=P>, P: AsRef<std::path::Path> { self }
 pub fn flag(&mut self, _: &str) -> &mut Self { self }
 pub fn compile(&self, name: &str) {
  println!("cargo:rustc-link-lib=static={name}");
  println!("cargo:rustc-link-search=native={}", std::env::var("BPNG_BUNDLE_TEST_NATIVE").unwrap());
 }
}''',
        "dunce": "pub use std::fs::canonicalize;",
    }
    for name, source in shims.items():
        directory = fixture / name
        directory.mkdir()
        (directory / "Cargo.toml").write_text(
            f'[package]\nname="{name}"\nversion="0.1.0"\nedition="2021"\n[lib]\npath="lib.rs"\n'
        )
        (directory / "lib.rs").write_text(source)
    (fixture / "Cargo.toml").write_text(
        '[package]\nname="pqcrypto-bundle-link-regression"\nversion="0.1.0"\n'
        'edition="2021"\nlinks="pqcrypto_bundle_link_regression"\n'
        '[build-dependencies]\ncc={path="cc"}\ndunce={path="dunce"}\n[workspace]\n'
    )
    (fixture / "src/lib.rs").write_text("#![no_std]\npub fn fixture_marker() {}\n")
    (fixture / "build.rs").write_bytes(BUILD_SCRIPT.read_bytes())
    env = os.environ.copy()
    env["BPNG_BUNDLE_TEST_NATIVE"] = str(native)
    # Reuse the caller's warm lane, or Cargo's existing repository target.
    env["CARGO_TARGET_DIR"] = os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))
    result = run(
        [cargo, "build", "--manifest-path", str(fixture / "Cargo.toml"), "--offline",
         "--release", "--message-format=json-render-diagnostics"], env=env,
    )
    messages = [json.loads(line) for line in result.stdout.splitlines()]
    artifact = next(row for row in messages if row.get("reason") == "compiler-artifact"
                    and row["target"]["name"] == "pqcrypto_bundle_link_regression")
    library = next(name for name in artifact["filenames"] if name.endswith(".rlib"))
    members = run([ar, "t", library]).stdout.splitlines()
    assert members.count("common.o") == 1
    assert members.count("keccak.o") == 1
