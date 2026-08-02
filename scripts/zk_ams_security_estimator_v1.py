"""Reproduce the frozen ZK-AMS v1 concrete-security transcript.

Run this file with the SageMath interpreter, not CPython.  The release
certificate consumes only a transcript produced from the exact estimator and
Sage artifacts pinned below.  A failed environment, source-tree, parameter, or
attack check exits before emitting a transcript.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import plistlib
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any

import estimator
from estimator import LWE, ND, conf
from sage.all import RealField, floor, oo
from sage.version import version as sage_version

SCHEMA_V1 = "iroha.zk-ams.v1.mkhe.security-estimator-transcript"
SECURITY_GUIDELINES_V1 = (
    "doi:10.62056/anxra69p1:section-5.1:"
    "primal-usvp+primal-bdd+dual-hybrid:hybrid-bdd-only-through-2^14"
)
EXPECTED_ESTIMATOR_COMMIT_V1 = "3e48ef421ec256afddb3e7d2249a77eab6e9ba12"
EXPECTED_SAGE_VERSION_V1 = "10.9"
EXPECTED_SAGE_MACHINE_V1 = "arm64"
EXPECTED_SAGE_DMG_SHA256_V1 = (
    "84f78143db3fb7c251f6eea906c6efb7793d26e96a3fbdb2104c1f9bb4b1827e"
)
EXPECTED_SAGE_MOUNT_V1 = Path("/Volumes/SageMath-10.9")
EXPECTED_SAGE_RUNTIME_ROOT_V1 = EXPECTED_SAGE_MOUNT_V1 / (
    "SageMath-10-9.app/Contents/Frameworks/Sage.framework/Versions/10.9"
)

RING_DEGREE_V1 = 131_072
MAX_SAMPLES_PER_SECRET_EPOCH_V1 = 67_108_864
TARGET_SECURITY_BITS_V1 = 128
RELEASE_MODULI_V1 = (
    1_152_921_504_606_584_833,
    1_152_921_504_598_720_513,
    1_152_921_504_592_429_057,
    1_152_921_504_581_419_009,
    1_152_921_504_580_894_721,
    1_152_921_504_578_273_281,
    1_152_921_504_577_748_993,
    1_152_921_504_577_486_849,
    1_152_921_504_568_836_097,
    1_152_921_504_565_166_081,
    1_152_921_504_563_331_073,
    1_152_921_504_556_515_329,
    1_152_921_504_555_466_753,
    1_152_921_504_554_156_033,
    1_152_921_504_552_583_169,
    1_152_921_504_542_883_841,
    1_152_921_504_538_951_681,
    1_152_921_504_537_378_817,
    1_152_921_504_531_873_793,
    1_152_921_504_521_650_177,
    1_152_921_504_509_853_697,
    1_152_921_504_508_280_833,
    1_152_921_504_506_970_113,
    1_152_921_504_495_697_921,
    1_152_921_504_491_241_473,
    1_152_921_504_488_620_033,
    1_152_921_504_479_444_993,
    1_152_921_504_470_794_241,
    1_152_921_504_468_172_801,
    1_152_921_504_462_929_921,
    1_152_921_504_462_667_777,
    1_152_921_504_455_589_889,
    1_152_921_504_447_987_713,
    1_152_921_504_442_482_689,
    1_152_921_504_436_191_233,
    1_152_921_504_427_278_337,
    1_152_921_504_419_414_017,
    1_152_921_504_409_190_401,
)

ROUGH_ATTACKS_V1 = ("usvp", "dual_hybrid")
GUIDELINE_ATTACKS_V1 = (
    "usvp",
    "bdd",
    "dual",
    "dual_hybrid",
)


def _file_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _sha256_file(path: Path) -> str:
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
    ):
        raise RuntimeError(f"security evidence is not one regular file: {path}")
    flags = os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    digest = hashlib.sha256()
    try:
        opened = os.fstat(descriptor)
        if _file_identity(opened) != _file_identity(before):
            raise RuntimeError(f"security evidence changed while opening: {path}")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        if _file_identity(os.fstat(descriptor)) != _file_identity(before):
            raise RuntimeError(f"security evidence changed while hashing: {path}")
    finally:
        os.close(descriptor)
    if _file_identity(path.lstat()) != _file_identity(before):
        raise RuntimeError(f"security evidence path changed after hashing: {path}")
    return digest.hexdigest()


def _git_output(root: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ("git", "-C", str(root), *arguments),
        capture_output=True,
        check=True,
        text=True,
    )
    return completed.stdout.strip()


def _verify_environment(estimator_root: Path, sage_dmg: Path) -> dict[str, str]:
    if not estimator_root.is_dir() or not (estimator_root / ".git").exists():
        raise RuntimeError("estimator root is not a Git worktree")
    estimator_commit = _git_output(estimator_root, "rev-parse", "HEAD")
    if estimator_commit != EXPECTED_ESTIMATOR_COMMIT_V1:
        raise RuntimeError("unexpected lattice-estimator revision")
    if _git_output(estimator_root, "status", "--porcelain=v1", "--untracked-files=no"):
        raise RuntimeError("lattice-estimator has tracked worktree changes")
    expected_estimator_init = (estimator_root / "estimator/__init__.py").resolve(
        strict=True
    )
    if Path(estimator.__file__).resolve(strict=True) != expected_estimator_init:
        raise RuntimeError("loaded lattice-estimator differs from --estimator-root")
    if sage_version != EXPECTED_SAGE_VERSION_V1:
        raise RuntimeError("unexpected SageMath version")
    if platform.machine() != EXPECTED_SAGE_MACHINE_V1:
        raise RuntimeError("unexpected SageMath machine architecture")
    if not sage_dmg.is_file():
        raise RuntimeError("SageMath DMG is not a regular file")
    sage_dmg_sha256 = _sha256_file(sage_dmg)
    if sage_dmg_sha256 != EXPECTED_SAGE_DMG_SHA256_V1:
        raise RuntimeError("unexpected SageMath DMG digest")
    image_info = subprocess.run(
        ("hdiutil", "info", "-plist"),
        capture_output=True,
        check=True,
    )
    mounted_from_dmg = False
    for image in plistlib.loads(image_info.stdout).get("images", []):
        image_path = image.get("image-path")
        if not isinstance(image_path, str):
            continue
        if Path(image_path).resolve(strict=True) != sage_dmg:
            continue
        mount_points = {
            entity.get("mount-point")
            for entity in image.get("system-entities", [])
            if isinstance(entity, dict)
        }
        mounted_from_dmg = str(EXPECTED_SAGE_MOUNT_V1) in mount_points
        if mounted_from_dmg:
            break
    if not mounted_from_dmg:
        raise RuntimeError("verified SageMath DMG is not mounted at the frozen volume")
    runtime_root = EXPECTED_SAGE_RUNTIME_ROOT_V1.resolve(strict=True)
    if not Path(sys.executable).resolve(strict=True).is_relative_to(runtime_root):
        raise RuntimeError("running SageMath interpreter is not from the verified DMG")
    return {
        "estimator_commit": estimator_commit,
        "sage_dmg_sha256": sage_dmg_sha256,
        "sage_distribution": "SageMath-10.9_arm64.dmg",
        "sage_machine": platform.machine(),
        "sage_version": sage_version,
    }


def _finite_attack_record(name: str, result: Any) -> dict[str, Any]:
    rop = result["rop"]
    if rop == oo:
        raise RuntimeError(f"attack {name!r} returned an unbounded operation count")
    log2_rop = RealField(256)(rop).log2()
    if log2_rop.is_infinity() or log2_rop.is_NaN():
        raise RuntimeError(f"attack {name!r} returned a non-finite operation count")
    return {
        "attack": name,
        "result_repr": repr(result),
        "rop_log2": str(log2_rop.n(digits=50)),
        "rop_log2_floor": int(floor(log2_rop)),
    }


def _run_mode(parameters: Any, mode: str) -> list[dict[str, Any]]:
    if mode == "rough":
        expected = ROUGH_ATTACKS_V1
        results = LWE.estimate.rough(
            parameters,
            jobs=1,
            catch_exceptions=False,
            quiet=True,
        )
    elif mode == "guideline":
        expected = GUIDELINE_ATTACKS_V1
        results = {
            "usvp": LWE.primal_usvp(
                parameters,
                red_cost_model=conf.red_cost_model,
                red_shape_model=conf.red_shape_model,
            ),
            "bdd": LWE.primal_bdd(
                parameters,
                red_cost_model=conf.red_cost_model,
                red_shape_model=conf.red_shape_model,
            ),
            # The guidelines require dual-hybrid.  The non-hybrid dual result
            # is additionally retained so a future dominance regression is
            # visible rather than silently hidden by the high-level helper.
            "dual": LWE.dual(
                parameters,
                red_cost_model=conf.red_cost_model,
            ),
            "dual_hybrid": LWE.dual_hybrid(
                parameters,
                red_cost_model=conf.red_cost_model,
            ),
        }
    else:  # pragma: no cover - argparse prevents this path.
        raise RuntimeError("unknown estimator mode")
    if set(results) != set(expected):
        raise RuntimeError(
            f"{mode} estimator attack set mismatch: "
            f"expected {expected!r}, received {tuple(sorted(results))!r}"
        )
    return [_finite_attack_record(name, results[name]) for name in expected]


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--estimator-root",
        required=True,
        type=Path,
        help="clean checkout of the pinned lattice-estimator revision",
    )
    parser.add_argument(
        "--sage-dmg",
        required=True,
        type=Path,
        help="official SageMath 10.9 arm64 DMG",
    )
    parser.add_argument(
        "--mode",
        choices=("rough", "guideline", "both"),
        default="both",
        help="both is required for a release certificate",
    )
    return parser.parse_args()


def main() -> int:
    arguments = _parse_args()
    runner_path = Path(__file__).resolve(strict=True)
    runner_sha256 = _sha256_file(runner_path)
    estimator_root = arguments.estimator_root.resolve(strict=True)
    environment = _verify_environment(
        estimator_root, arguments.sage_dmg.resolve(strict=True)
    )
    modulus = 1
    for prime in RELEASE_MODULI_V1:
        modulus *= prime
    if modulus.bit_length() != 2_280:
        raise RuntimeError("frozen ciphertext modulus does not have 2280 bits")
    reduction_model = (
        f"{type(conf.red_cost_model).__module__}."
        f"{type(conf.red_cost_model).__qualname__}"
    )
    shape_model = (
        f"{conf.red_shape_model.__module__}.{conf.red_shape_model.__qualname__}"
    )
    if reduction_model != "estimator.reduction.MATZOV":
        raise RuntimeError("unexpected full-estimate reduction cost model")
    if shape_model != "estimator.simulator.GSA":
        raise RuntimeError("unexpected full-estimate reduction shape model")

    parameters = LWE.Parameters(
        n=RING_DEGREE_V1,
        q=modulus,
        Xs=ND.Uniform(-1, 1),
        Xe=ND.CenteredBinomial(2),
        m=MAX_SAMPLES_PER_SECRET_EPOCH_V1,
        tag="iroha-zk-ams-v1",
    )
    modes = (
        ("rough", "guideline")
        if arguments.mode == "both"
        else (arguments.mode,)
    )
    attacks = {mode: _run_mode(parameters, mode) for mode in modes}
    minimum_bits = min(
        record["rop_log2_floor"]
        for mode_records in attacks.values()
        for record in mode_records
    )
    if minimum_bits < TARGET_SECURITY_BITS_V1:
        raise RuntimeError("frozen parameters do not meet the security target")

    transcript = {
        "attack_scope": {
            "guideline": SECURITY_GUIDELINES_V1,
            "included": {
                "guideline": list(GUIDELINE_ATTACKS_V1),
                "rough": list(ROUGH_ATTACKS_V1),
            },
            "not_in_guideline_certificate": {
                "arora-gb": "not in the section-5.1 table-generation attack set",
                "bdd_hybrid": "section 5.1 limits hybrid BDD to N <= 2^14",
                "bdd_mitm_hybrid": "section 5.1 limits hybrid BDD to N <= 2^14",
                "bkw": "not in the section-5.1 table-generation attack set",
            },
        },
        "attacks": attacks,
        "environment": environment,
        "estimator_models": {
            "guideline_reduction_cost_model": reduction_model,
            "guideline_reduction_shape_model": shape_model,
            "rough_model": "upstream-commit-pinned-ADPS16-GSA",
        },
        "input": {
            "ciphertext_modulus": str(modulus),
            "ciphertext_modulus_bits": modulus.bit_length(),
            "error_distribution": "centered-binomial-eta-2",
            "max_samples_per_secret_epoch": MAX_SAMPLES_PER_SECRET_EPOCH_V1,
            "release_moduli": [str(prime) for prime in RELEASE_MODULI_V1],
            "ring_degree": RING_DEGREE_V1,
            "secret_distribution": "dense-uniform-ternary-minus-one-zero-one",
            "target_security_bits": TARGET_SECURITY_BITS_V1,
        },
        "minimum_rop_log2_floor": minimum_bits,
        "runner_sha256": runner_sha256,
        "schema": SCHEMA_V1,
    }
    if _sha256_file(runner_path) != runner_sha256:
        raise RuntimeError("security estimator runner changed during execution")
    if _git_output(estimator_root, "rev-parse", "HEAD") != EXPECTED_ESTIMATOR_COMMIT_V1:
        raise RuntimeError("lattice-estimator revision changed during execution")
    if _git_output(estimator_root, "status", "--porcelain=v1", "--untracked-files=no"):
        raise RuntimeError("lattice-estimator changed during execution")
    canonical_transcript = json.dumps(
        transcript,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    )
    envelope = {
        "transcript": transcript,
        "transcript_sha256": hashlib.sha256(
            canonical_transcript.encode("ascii")
        ).hexdigest(),
    }
    print(
        json.dumps(
            envelope,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
