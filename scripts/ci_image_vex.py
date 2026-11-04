#!/usr/bin/env python3
"""Bind reviewed CI findings to an exact image, package inventory and import graph.

Requires Python 3.11+ and evidence copied from that candidate image. No environment
variables or network access are used. Changed evidence refuses; only the explicit
--output file is written, and the high/critical scan gate remains independent.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re


NAMES_ONLY = "github.com/docker/docker/pkg/namesgenerator"


def document(image: str, packages: str, imports: str, build_info: str) -> dict:
    """Refuse changed evidence; describe only three reviewed findings on one image."""
    match = re.fullmatch(r"hyperledger/iroha2-ci@sha256:([a-f0-9]{64})", image)
    if match is None:
        raise ValueError("require the exact candidate repository and digest")
    inventory = {}
    for line in packages.splitlines():
        fields = line.split()
        if len(fields) != 2 or fields[0] in inventory:
            raise ValueError("require one exact version per inventory package")
        inventory[fields[0]] = fields[1]
    if inventory.get("pam") != "1.7.3-1" or inventory.get("libxml2") != "2.15.4-1":
        raise ValueError("Arch package versions changed; review their findings again")
    if any(name in inventory for name in ("docker", "containerd", "docker-buildx")):
        raise ValueError("runtime contains an engine package or an unreviewed buildx")
    imported = set(imports.splitlines())
    docker_imports = {name for name in imported if name.startswith("github.com/docker/docker/")}
    if docker_imports != {NAMES_ONLY}:
        raise ValueError("Docker Engine dependency extends beyond the name generator")
    if any(name.startswith(("github.com/moby/moby/v2/daemon/", "github.com/moby/moby/v2/pkg/authorization")) for name in imported):
        raise ValueError("Docker Engine authorization code is present")
    dependencies = {}
    for line in build_info.splitlines():
        fields = line.split()
        if len(fields) >= 3 and fields[0] == "dep":
            dependencies[fields[1]] = fields[2]
    expected = {
        "github.com/docker/docker": "v28.5.2+incompatible",
        "golang.org/x/net": "v0.60.0",
        "github.com/moby/go-archive": "v0.3.0",
    }
    if any(dependencies.get(name) != version for name, version in expected.items()):
        raise ValueError("compiled dependency versions differ from the reviewed module lock")
    if "go1.27.3" not in build_info.splitlines()[0].split():
        raise ValueError("compiled Go toolchain differs from the reviewed version")
    product = f"pkg:oci/iroha2-ci@sha256:{match[1]}?repository_url=docker.io%2Fhyperledger%2Firoha2-ci"
    findings = [
        (
            "AVG-2901", "pkg:alpm/arch/pam@1.7.3-1", "fixed", None,
            "The exact Arch pam 1.7.3-1 package includes Linux-PAM's fix for CVE-2025-6020, released in 1.7.1. The Arch group still lists 1.7.0-2 with an unknown fixed version. https://github.com/linux-pam/linux-pam/releases/tag/v1.7.1 ; https://security.archlinux.org/AVG-2901",
        ),
        (
            "AVG-2898", "pkg:alpm/arch/libxml2@2.15.4-1", "fixed", None,
            "The exact Arch libxml2 2.15.4-1 package includes the Schematron and interactive-shell security fixes released in 2.14.5 for CVE-2025-49794, CVE-2025-49795, CVE-2025-49796 and CVE-2025-6170. The Arch group still lists 2.14.4-1 with an unknown fixed version. https://raw.githubusercontent.com/GNOME/libxml2/v2.14.5/NEWS ; https://security.archlinux.org/AVG-2898",
        ),
        (
            "GO-2026-4887", "pkg:golang/github.com/docker/docker@v28.5.2%2Bincompatible", "not_affected", "vulnerable_code_not_present",
            "The compiled buildx dependency graph includes only github.com/docker/docker/pkg/namesgenerator from this module. Docker Engine and its AuthZ request-body handler are absent. The exact image has client tools only; no engine package is installed. https://github.com/moby/moby/security/advisories/GHSA-x744-4wpc-v9h2",
        ),
    ]
    statements = []
    for vulnerability, component, status, justification, detail in findings:
        item = {
            "vulnerability": {"name": vulnerability},
            "products": [{"@id": product, "subcomponents": [{"@id": component}]}],
            "status": status,
            "status_notes": detail,
        }
        if justification:
            item.update(justification=justification, impact_statement=detail)
        statements.append(item)
    evidence_hash = hashlib.sha256((image + packages + imports + build_info).encode()).hexdigest()
    return {
        "@context": "https://openvex.dev/ns/v0.2.0",
        "@id": f"https://github.com/hyperledger-iroha/iroha/ci-image-vex/{evidence_hash}",
        "author": "Hyperledger Iroha CI image build",
        "timestamp": "2026-11-04T00:00:00Z",
        "version": 1,
        "statements": statements,
    }


def main() -> None:
    """Read retained image evidence and write the candidate's complete VEX document."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    files = ["arch-packages.txt", "buildx-packages.txt", "buildx-build-info.txt"]
    result = document(args.image, *(args.evidence.joinpath(name).read_text() for name in files))
    args.output.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
