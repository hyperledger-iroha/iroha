#!/usr/bin/env python3
"""Synthetic adversarial tests for the smoke evidence validator and serial driver.

These fixtures are explicitly unmeasured and contain no valid BLS proof. Tests
never run Cargo, Git signing, validator binaries, or a network. All files live
in disposable owner-only temporary directories; none are release evidence.
"""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[1] / "private_settlement_smoke_campaign.py"
SPEC = importlib.util.spec_from_file_location("smoke_campaign_under_test", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
COMMIT = "a" * 40


def hash_literal(number: int, *, mark: bool = True) -> str:
    """Mirror Hash::prehashed's low marker bit and JSON checksum for synthetic data."""
    body = f"{number | 1 if mark else number:064X}"
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return f"hash:{body}#{crc:04X}"


def observation(peer: int, finalized: bool, *, staged: bool = False) -> dict:
    """Build bound synthetic raw state bytes with the current two-input/three-output counts."""
    counts = {name: 0 for name in M.release_runner.FAULT_STATE_COUNT_FIELDS}
    counts.update(governance=3, pools=3, roots=3, commitments=6)
    if finalized:
        for name, delta in {"roots": 3, "nullifiers": 6, "commitments": 9, "encrypted_outputs": 9,
                            "replay_markers": 1, "receipts": 1}.items():
            counts[name] += delta
    if staged:
        counts.update(staged_pool_heads=1, staged_nullifiers=2, staged_output_commitments=3,
                      staged_locks=6, replicated_staged_locks=28)
    response = {"format_version": 1, "height": 306 if finalized else 302,
                "commitment": hash_literal(24 if finalized else 23),
                "ledger_commitment": hash_literal(12 if finalized else 11),
                "replicated_staged_lock_commitment": hash_literal(14 if staged else 13),
                "staged_lock_commitment": hash_literal(16 if staged else 15), "counts": counts}
    raw = M.canonical(response)
    return {"peer_index": peer, "response_sha256": M.sha(raw), "response_hex": raw.hex(),
            **{name: value for name, value in response.items() if name != "format_version"}}


def continuous(peer: int, bundle: bytes) -> dict:
    """Build an independent response/phase hash-chain fixture with one live baseline poll."""
    observations = [observation(peer, False), observation(peer, False, staged=True),
                    observation(peer, True), observation(peer, True)]
    classes = ["baseline", "baseline", "finalized", "finalized"]
    response_chain = hashlib.sha256(b"iroha:aps-fault-continuous-observation:v1\0" + bundle + struct.pack("<Q", peer))
    for row in observations:
        response_chain.update(bytes.fromhex(row["response_sha256"]))
    phases = []
    for index, (name, allowed, positions, checkpoint) in enumerate((
        ("preflight", False, (0, 1), 1), ("finalization", True, (2,), 0), ("terminal", True, (3,), 0)
    )):
        phase_chain = hashlib.sha256(b"iroha:aps-fault-continuous-observation-phase:v1\0" + bundle
            + struct.pack("<QQQ", peer, index, len(name)) + name.encode() + bytes((0, allowed)))
        attempts = []
        for position in positions:
            kind = classes[position]
            attempts.append({"class": kind, "evidence": observations[position]["response_hex"], "repetitions": 1})
            phase_chain.update(bytes((1 if kind == "baseline" else 2,)))
            phase_chain.update(bytes.fromhex(observations[position]["response_sha256"]))
        phase_chain.update(b"checkpoint\0" + struct.pack("<Q", checkpoint)
                           + b"checkpoint-controls\0" + struct.pack("<Q", 0))
        phases.append({"phase": name, "expected_unavailable": False, "finalization_allowed": allowed,
            "successful_observations": len(positions), "poll_failures": 0,
            "baseline_observations": sum(classes[position] == "baseline" for position in positions),
            "finalized_observations": sum(classes[position] == "finalized" for position in positions),
            "checkpoint_attempt": checkpoint, "checkpoint_control_bindings": [],
            "attempt_chain_sha256": phase_chain.hexdigest(), "attempts": attempts})
    return {"summary": {"peer_index": peer, "check_count": 4, "poll_failure_count": 0,
        "first_response_sha256": observations[0]["response_sha256"], "last_response_sha256": observations[-1]["response_sha256"],
        "response_chain_sha256": response_chain.hexdigest(), "baseline_observations": 2,
        "finalized_observations": 2, "phase_coverage": phases}, "observations": observations}


def finality(network: object, identities: list[str]) -> dict:
    """Construct synthetic finality structure; signatures deliberately have no cryptographic validity."""
    block_hash = hash_literal(777)
    context = {name: None for name in ("next_epoch_snapshot", "parent_commit_qc", "snapshot_bootstrap")}
    context.update(network_id=network, protocol_version=4, height=306, epoch=1,
        kagemusha_mint_finality_epoch_id=[1] * 32, kagemusha_mint_finality_epoch_roster={"synthetic": True},
        epoch_end_height=1000, mode={"mode": "permissioned", "details": None},
        roster=[{"validator": peer, "power": 1} for peer in identities[:4]],
        quorum={"min_signers": 3, "total_power": 4}, nexus_amx_context_hash=hash_literal(778),
        execution_policy_hash=hash_literal(779), leader_seed=[1] * 32,
        da_layout={"encoding": {"encoding": "reed_solomon16", "details": None}, "chunk_size_bytes": 262144,
                   "data_shards": 4, "parity_shards": 2, "max_payload_size_bytes": 16777216, "max_chunk_count": 1024})
    subject = {"block_hash": block_hash}
    round_value = {"context_id": [hash_literal(780)], "height": 306, "view": 0}
    qc = {"round": round_value, "proposal_round": copy.deepcopy(round_value),
          "phase": {"phase": "commit", "details": None}, "subject": subject,
          "execution_commitment": {"synthetic": True}, "signers": [0, 1, 2], "aggregate_signature": [1] * 96}
    header = {name: None for name in ("prev_block_hash", "merkle_root", "result_merkle_root", "da_proof_policies_hash",
        "da_commitments_hash", "da_pin_intents_hash", "npos_effects_hash", "sccp_commitment_root",
        "confidential_features", "execution_context_hash")}
    header.update(height=306, creation_time_ms=123456, view_change_index=0)
    return {"version": 2, "block_header": header, "finality_artifact": {"format_version": 4, "protocol_version": 4,
        "height": 306, "height_context": context, "subject": subject, "block_hash": block_hash,
        "commit_qc": qc, "validator_set_pops": [[1] * 96 for _ in range(4)]}}


def request(index: int) -> dict:
    """Generate deterministic distinct test-only requests."""
    value = {"version": 1, "protocol": M.PROTOCOL, "kind": "smoke", "commit": COMMIT,
             "seed": index + 1, "run": index, "invocation_nonce": f"{index + 1:064x}"}
    value["request_id"] = M.sha(M.canonical(value))
    return value


def evidence_fixture(index: int, validator_sha: str) -> tuple[dict, dict]:
    """Build the 80-file contract entirely from explicitly synthetic test values."""
    req = request(index)
    identities = [f"synthetic-validator-{index}-{peer:02}" for peer in range(16)]
    network = [hash_literal(900 + 2 * index)]
    manifest = {"version": 1, "bundle_id": hash_literal(1000 + 2 * index), "network_id": network,
                "authority_context_height": 302, "expiry_height": 1000, "legs": []}
    authorities, rosters, deltas, prepares, commits, legs = [], [], [], [], [], []
    prepared_digest = hash_literal(3000 + index)
    for ordinal in range(3):
        route = {"lane_id": ordinal + 1, "dataspace_id": ordinal + 1, "lane_incarnation": hash_literal(3100 + ordinal)}
        delta = {"leg_ordinal": ordinal, "route": route}
        authority = {"route": route, "validator_set_hash": hash_literal(3200 + ordinal),
            "validators": identities[(ordinal+1)*4:(ordinal+2)*4], "validator_pops": [[1]*96 for _ in range(4)]}
        body = {"network_id": network, "bundle_id": manifest["bundle_id"], "manifest_digest": hash_literal(3300),
            "leg_ordinal": ordinal, "route": route, "delta_digest": hash_literal(3400 + ordinal),
            "authority_digest": hash_literal(3500 + ordinal), "authority_context_height": 302, "expiry_height": 1000}
        certificates = []
        for phase in ("prepare", "commit"):
            certificates.append({"body": {**body, "phase": {"phase": phase, "value": None},
                "prepared_bundle_digest": hash_literal(0) if phase == "prepare" else prepared_digest},
                "authority_catalog_index": ordinal, "signers_bitmap": 7, "aggregate_signature": [1]*96})
        authorities.append(authority)
        rosters.append({key: value for key, value in authority.items() if key != "route"})
        deltas.append(delta)
        prepares.append(certificates[0])
        commits.append(certificates[1])
        legs.append({"delta": delta, "prepare": certificates[0], "commit": certificates[1]})
        manifest["legs"].append({"ordinal": ordinal, "route": route, "delta_digest": body["delta_digest"]})
    catalog = {"rosters": rosters, "leg_roster_indices": [0, 1, 2]}
    inventory = [{"peer_index": peer, "peer_id": identity, "committee_index": peer // 4,
        "validator_index": peer % 4, "pid": 100 + peer, "executable_sha256": validator_sha,
        "configuration_sha256": M.sha(identity.encode())} for peer, identity in enumerate(identities)]
    after = [{**row, "pid": row["pid"] + 100} for row in inventory]
    evidence = {"request.json": req, "processes-before.json": inventory, "processes-after.json": after,
        "authorities.json": authorities, "prepare-barrier.json": {"version": 1, "manifest": manifest,
            "authority_catalog": catalog, "deltas": deltas, "prepare_certificates": prepares,
            "prepared_bundle_digest": prepared_digest}, "commit-certificates.json": commits,
        "receipt.json": {"version": 1, "manifest": manifest, "authority_catalog": catalog,
                         "legs": legs, "finalized_height": 306},
        "restarts.json": [{"peer_index": peer, "before_pid": 100+peer, "after_pid": 200+peer} for peer in range(16)]}
    for phase in M.STATE_PHASES:
        evidence[f"state-{phase}.json"] = {"label": f"smoke-{phase}",
            "validators": [observation(peer, phase in ("finalized", "replay")) for peer in range(16)]}
    for peer in range(16):
        evidence[f"state-restarted-{peer:02}.json"] = {"label": "smoke-restarted",
            "validators": [observation(other, True) for other in range(16)]}
        evidence[f"continuous-{peer:02}.json"] = continuous(peer, ((1000+2*index) | 1).to_bytes(32, "big"))
        for phase in ("before", "after"):
            evidence[f"finality-{phase}-{peer:02}.json"] = finality(network, identities)
    result = {"version": 1, "protocol": M.PROTOCOL, "kind": "smoke", "request": req,
        "request_sha256": M.sha(M.canonical(req)+b"\n"), "network_id": network, "participants": 3,
        "processes": 16, "restarted": 16, "activation_height": 301, "authority_context_height": 302,
        "finalized_height": 306, "signed_rs16_observations": 16, "continuous_checks": 64, "passed": True,
        "artifacts": []}
    return evidence, result


def put(path: Path, value: object) -> None:
    """Write or mutate only disposable test fixture bytes."""
    path.write_bytes(M.canonical(value)+b"\n")
    path.chmod(0o600)


def store_evidence(directory: Path, evidence: dict, result: dict) -> None:
    """Retain a synthetic fixture with fresh artifact digest bindings."""
    (directory / "evidence").mkdir(mode=0o700, exist_ok=True)
    result["artifacts"] = []
    for name, value in sorted(evidence.items()):
        put(directory / "evidence" / name, value)
        raw = (directory / "evidence" / name).read_bytes()
        result["artifacts"].append({"name": name, "bytes": len(raw), "sha256": M.sha(raw)})
    put(directory / "request.json", result["request"])
    put(directory / "rust-result.json", result)


class SmokeEvidenceTests(unittest.TestCase):
    """Mutate bound evidence adversarially, including recomputed outer file hashes."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="synthetic-smoke-validator-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.root.chmod(0o700)
        self.sha = "b" * 64
        self.evidence, self.result = evidence_fixture(0, self.sha)

    def validate(self) -> dict:
        store_evidence(self.root, self.evidence, self.result)
        return M.validate_run(self.root, request(0), self.sha)

    def test_complete_synthetic_contract_and_live_staged_observation(self) -> None:
        self.assertEqual(self.validate()["continuous_checks"], 64)
        self.assertEqual(len(M.EVIDENCE_NAMES), 80)

    def test_financial_stage_mutations_rejected_even_with_rehashed_artifacts(self) -> None:
        for phase in ("collecting", "audited", "prepared", "registered", "commit-certified"):
            with self.subTest(phase=phase):
                self.evidence, self.result = evidence_fixture(0, self.sha)
                self.evidence[f"state-{phase}.json"]["validators"][9] = observation(9, True)
                with self.assertRaisesRegex(M.CampaignError, "partial financial"):
                    self.validate()

    def test_every_restart_snapshot_is_checked(self) -> None:
        self.evidence["state-restarted-15.json"]["validators"][15] = observation(15, False)
        with self.assertRaisesRegex(M.CampaignError, "predates finality|restart changed"):
            self.validate()

    def test_process_identity_pid_config_and_executable_substitutions(self) -> None:
        mutations = (
            lambda e: e["processes-before.json"][8].update(peer_id=e["processes-before.json"][0]["peer_id"]),
            lambda e: e["processes-after.json"][15].update(pid=115),
            lambda e: e["processes-after.json"][3].update(configuration_sha256="c"*64),
            lambda e: e["processes-after.json"][0].update(executable_sha256="c"*64),
            lambda e: e["processes-before.json"][2].update(pid=100),
        )
        for mutate in mutations:
            self.evidence, self.result = evidence_fixture(0, self.sha)
            mutate(self.evidence)
            with self.assertRaises(M.CampaignError):
                self.validate()

    def test_continuous_rollback_partial_counts_retention_race_and_chain_tampering(self) -> None:
        mutations = (
            lambda row: row["observations"].__setitem__(3, observation(0, False)),
            lambda row: row["observations"].append(observation(0, True)),
            lambda row: row["summary"].update(response_chain_sha256="c"*64),
            lambda row: row["summary"]["phase_coverage"][0].update(finalization_allowed=True),
            lambda row: row["summary"]["phase_coverage"][1].update(checkpoint_attempt=1),
            lambda row: row["summary"]["phase_coverage"][1]["attempts"][0].update(evidence=observation(0, False)["response_hex"]),
        )
        for mutate in mutations:
            self.evidence, self.result = evidence_fixture(0, self.sha)
            mutate(self.evidence["continuous-00.json"])
            with self.assertRaises(M.release_runner.RunnerError):
                self.validate()

    def test_finality_signature_presence_height_roster_rs16_and_consistency(self) -> None:
        mutations = (
            lambda p: p["finality_artifact"]["commit_qc"].update(aggregate_signature=[]),
            lambda p: p["finality_artifact"]["height_context"]["roster"][0].update(validator="synthetic-substitution"),
            lambda p: p["finality_artifact"]["height_context"]["da_layout"].update(data_shards=3),
            lambda p: p["block_header"].update(height=307),
            lambda p: p["block_header"].update(creation_time_ms=123457),
            lambda p: p["finality_artifact"]["commit_qc"].update(signers=[0, 0, 1]),
        )
        for mutate in mutations:
            self.evidence, self.result = evidence_fixture(0, self.sha)
            mutate(self.evidence["finality-after-15.json"])
            with self.assertRaises(M.CampaignError):
                self.validate()

    def test_semantic_finality_allows_equivalent_parent_qc_signer_subsets(self) -> None:
        parent = copy.deepcopy(self.evidence["finality-before-00.json"]["finality_artifact"]["commit_qc"])
        for name, proof in self.evidence.items():
            if name.startswith("finality-"):
                proof["finality_artifact"]["height_context"]["parent_commit_qc"] = copy.deepcopy(parent)
        alternate = self.evidence["finality-after-15.json"]["finality_artifact"]["height_context"]["parent_commit_qc"]
        alternate["signers"] = [1, 2, 3]
        alternate["aggregate_signature"] = [2] * 96
        alternate["round"]["view"] = alternate["proposal_round"]["view"] = 1
        self.validate()
        alternate["round"]["context_id"] = [hash_literal(999)]
        with self.assertRaisesRegex(M.CampaignError, "disagree"):
            self.validate()

    def test_notice_barrier_authority_and_missing_artifact_failures(self) -> None:
        self.result["activation_height"] = 300
        with self.assertRaises(M.release_runner.RunnerError):
            self.validate()
        self.evidence, self.result = evidence_fixture(0, self.sha)
        self.evidence["commit-certificates.json"][2]["body"]["prepared_bundle_digest"] = hash_literal(0)
        with self.assertRaisesRegex(M.CampaignError, "bypasses"):
            self.validate()
        self.evidence, self.result = evidence_fixture(0, self.sha)
        del self.evidence["finality-before-15.json"]
        # Previous subcases left the file on disk; both inventory count and extras are rejected.
        with self.assertRaises(M.CampaignError):
            self.validate()

    def test_prepare_empty_hash_is_canonical_marker_and_commit_never_empty(self) -> None:
        self.assertEqual(M.release_runner.canonical_iroha_hash_body(hash_literal(0), "test hash"), "0"*63+"1")
        self.validate()
        self.evidence["prepare-barrier.json"]["prepare_certificates"][0]["body"]["prepared_bundle_digest"] = hash_literal(0, mark=False)
        with self.assertRaisesRegex(M.CampaignError, "marker"):
            self.validate()
        for empty in (hash_literal(0), hash_literal(0, mark=False)):
            self.evidence, self.result = evidence_fixture(0, self.sha)
            self.evidence["prepare-barrier.json"]["prepared_bundle_digest"] = empty
            for cert in self.evidence["commit-certificates.json"]:
                cert["body"]["prepared_bundle_digest"] = empty
            with self.assertRaisesRegex(M.CampaignError, "bypasses|marker"):
                self.validate()

    def test_protocol_hash_marker_empty_and_boolean_versions_are_rejected(self) -> None:
        for number in (0, 2, 1000):
            with self.assertRaisesRegex(M.CampaignError, "marker"):
                M.protocol_hash(hash_literal(number, mark=False), "synthetic")
        with self.assertRaisesRegex(M.CampaignError, "reserved empty"):
            M.protocol_hash(hash_literal(0), "synthetic")
        self.assertEqual(M.protocol_hash(hash_literal(0), "synthetic", allow_empty=True), M.EMPTY_PROTOCOL_HASH)
        self.assertEqual(M.protocol_hash(hash_literal(1000), "synthetic"), f"{1001:064x}")
        for artifact in ("result", "receipt.json", "prepare-barrier.json"):
            self.evidence, self.result = evidence_fixture(0, self.sha)
            value = self.result if artifact == "result" else self.evidence[artifact]
            value["version"] = True
            with self.assertRaises(M.CampaignError):
                self.validate()

    def test_raw_response_binding_and_owner_only_files(self) -> None:
        store_evidence(self.root, self.evidence, self.result)
        target = self.root / "evidence" / "state-before.json"
        target.chmod(0o644)
        with self.assertRaisesRegex(M.CampaignError, "owner-only"):
            M.validate_run(self.root, request(0), self.sha)
        target.chmod(0o600)
        raw = target.read_bytes()
        target.unlink()
        alternate = self.root / "substitution.json"
        alternate.write_bytes(raw)
        alternate.chmod(0o600)
        target.symlink_to(alternate)
        with self.assertRaises(M.CampaignError):
            M.validate_run(self.root, request(0), self.sha)

    def test_boolean_indices_and_embedded_request_are_not_integer_evidence(self) -> None:
        cases = [
            ("restarts.json", (0, "peer_index")),
            ("continuous-00.json", ("summary", "peer_index")),
            ("receipt.json", ("manifest", "legs", 0, "ordinal")),
            ("prepare-barrier.json", ("deltas", 0, "leg_ordinal")),
            ("commit-certificates.json", (0, "authority_catalog_index")),
            ("commit-certificates.json", (0, "body", "leg_ordinal")),
        ]
        for artifact, path in cases:
            with self.subTest(artifact=artifact, path=path):
                self.evidence, self.result = evidence_fixture(0, self.sha)
                row = self.evidence[artifact]
                for component in path[:-1]:
                    row = row[component]
                row[path[-1]] = False
                with self.assertRaises(M.release_runner.RunnerError):
                    self.validate()
        self.evidence, self.result = evidence_fixture(0, self.sha)
        store_evidence(self.root, self.evidence, self.result)
        for location in ("rust-result.json", "evidence/request.json"):
            with self.subTest(location=location):
                self.evidence, self.result = evidence_fixture(0, self.sha)
                store_evidence(self.root, self.evidence, self.result)
                path = self.root / location
                value = M.read_json(path)
                embedded = value["request"] if location == "rust-result.json" else value
                embedded["version"] = True
                path.write_bytes(M.canonical(value) + b"\n")
                if location == "evidence/request.json":
                    result_path = self.root / "rust-result.json"
                    result = M.read_json(result_path)
                    raw = path.read_bytes()
                    for artifact in result["artifacts"]:
                        if artifact["name"] == "request.json":
                            artifact.update(bytes=len(raw), sha256=M.sha(raw))
                    put(result_path, result)
                with self.assertRaisesRegex(M.CampaignError, "wrong smoke protocol"):
                    M.validate_run(self.root, request(0), self.sha)

    def test_restart_pid_one_is_integer_and_boolean_aliases_are_rejected(self) -> None:
        for phase, field in (("before", "before_pid"), ("after", "after_pid")):
            with self.subTest(phase=phase):
                self.evidence, self.result = evidence_fixture(0, self.sha)
                self.evidence[f"processes-{phase}.json"][0]["pid"] = 1
                self.evidence["restarts.json"][0][field] = 1
                self.validate()
                self.evidence["restarts.json"][0][field] = True
                with self.assertRaisesRegex(M.release_runner.RunnerError, "integer"):
                    self.validate()


class DriverBoundaryTests(unittest.TestCase):
    """Verify strict invocation controls without spawning Git, Cargo, or networks."""

    def test_exact_terminal_and_discovery_reject_zero_ignored_skipped_or_duplicate(self) -> None:
        good = "running 1 test\nAPS smoke completed: synthetic fixture only\n" + (
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 1.0s\n")
        M.terminal_success(good)
        for bad in (good.replace("1 passed", "0 passed"), good.replace("0 ignored", "1 ignored"),
                    good + "network skipped\n", good + good, good.replace("running 1", "running 0")):
            with self.assertRaises(M.CampaignError):
                M.terminal_success(bad)
        listing = f"{M.TEST_NAME}: test\n\n1 test, 0 benchmarks\n"
        M.validate_discovery(listing)
        with self.assertRaises(M.CampaignError):
            M.validate_discovery(listing.replace(M.TEST_NAME, M.TEST_NAME + "_wrong"))

    def test_request_nonce_content_id_commit_range_and_bool_boundaries(self) -> None:
        M.validate_request(request(0), COMMIT, 0)
        for key, value in (("run", True), ("run", 10), ("seed", -1), ("seed", 2**64),
                           ("invocation_nonce", "0"*64), ("commit", "B"*40), ("request_id", "b"*64)):
            candidate = request(0)
            candidate[key] = value
            with self.assertRaises(M.release_runner.RunnerError):
                M.validate_request(candidate, COMMIT, 0)

    def test_environment_drops_network_compiler_loader_and_git_injection(self) -> None:
        injected = {"PATH": "/test/toolchain", "HOME": "/test/home", "IROHA_TEST_REQUIRE_NETWORK": "0",
            "APS_REAL_PROCESS_REQUEST": "/stale", "RUSTFLAGS": "bad", "RUSTC_WRAPPER": "bad",
            "DYLD_INSERT_LIBRARIES": "bad", "LD_PRELOAD": "bad", "GIT_INDEX_FILE": "/stale",
            "IROHA_RELEASE_SOURCE_MANIFEST_SHA256": "stale", "TEST_NETWORK_BIN_IROHAD": "/wrong"}
        with mock.patch.dict(os.environ, injected, clear=True):
            environment = M.sanitized_environment()
        self.assertEqual(environment["PATH"], "/test/toolchain")
        self.assertTrue((set(injected) - {"PATH", "HOME"}).isdisjoint(environment))

    def test_signed_blob_seal_detects_edits_hidden_by_git_status(self) -> None:
        with tempfile.TemporaryDirectory(prefix="synthetic-source-seal-") as temporary:
            root = Path(temporary).resolve()
            source = root / "source.rs"
            original = b"// synthetic signed blob fixture\n"
            source.write_bytes(original)
            source.chmod(0o644)
            blob = hashlib.sha1(f"blob {len(original)}\0".encode()+original).hexdigest()
            responses = {
                ("rev-parse", "--show-toplevel"): str(root).encode()+b"\n",
                ("rev-parse", "HEAD"): COMMIT.encode()+b"\n",
                ("status", "--porcelain=v1", "--untracked-files=all"): b"",
                ("verify-commit", COMMIT): b"",
                ("ls-tree", "-rz", COMMIT): f"100644 blob {blob}\tsource.rs\0".encode(),
                ("rev-parse", f"{COMMIT}^{{tree}}"): b"b"*40+b"\n"}
            with mock.patch.object(M, "git_bytes", side_effect=lambda _repo, args: responses[tuple(args)]) as git, \
                 mock.patch.object(M, "reject_unsigned_cargo_configuration"):
                self.assertEqual(M.source_seal(root, COMMIT)["tracked_files"], 1)
                self.assertIn(mock.call(root, ["verify-commit", COMMIT]), git.call_args_list)
                source.write_bytes(b"// substituted despite clean status\n")
                with self.assertRaisesRegex(M.CampaignError, "signed source bytes"):
                    M.source_seal(root, COMMIT)

    def test_unsigned_cargo_configuration_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="synthetic-cargo-configuration-") as temporary:
            root = Path(temporary).resolve()
            repo = root / "repo"
            (repo / ".cargo").mkdir(parents=True)
            cargo_home = root / "cargo-home"
            cargo_home.mkdir()
            config = repo / ".cargo" / "config.toml"
            config.write_text("# Synthetic configuration fixture only\n")
            with mock.patch.dict(os.environ, {"CARGO_HOME": str(cargo_home)}):
                with self.assertRaisesRegex(M.CampaignError, "unsigned Cargo"):
                    M.reject_unsigned_cargo_configuration(repo, set())
                M.reject_unsigned_cargo_configuration(repo, {".cargo/config.toml"})
                (cargo_home / "config").write_text("# External unsigned configuration\n")
                with self.assertRaisesRegex(M.CampaignError, "unsigned Cargo"):
                    M.reject_unsigned_cargo_configuration(repo, {".cargo/config.toml"})

    def test_signed_config_symlink_cannot_load_unsigned_target_bytes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="synthetic-cargo-symlink-") as temporary:
            root = Path(temporary).resolve()
            repo = root / "repo"
            (repo / ".cargo").mkdir(parents=True)
            outside = root / "unsigned-config.toml"
            outside.write_text("# Unsigned bytes behind a signed link fixture\n")
            config = repo / ".cargo" / "config.toml"
            config.symlink_to(outside)
            with mock.patch.dict(os.environ, {"CARGO_HOME": str(root / "cargo-home")}), self.assertRaisesRegex(
                M.CampaignError, "symlink"
            ):
                M.reject_unsigned_cargo_configuration(repo, {".cargo/config.toml"})

    def test_relative_cargo_home_cannot_hide_configuration_in_build_directory(self) -> None:
        with tempfile.TemporaryDirectory(prefix="synthetic-relative-cargo-home-") as temporary:
            root = Path(temporary).resolve()
            repo = root / "repo"
            cargo_home = repo / "relative-cargo-home"
            cargo_home.mkdir(parents=True)
            (cargo_home / "config.toml").write_text("# Unsigned build-directory configuration\n")
            with mock.patch.dict(os.environ, {"CARGO_HOME": "relative-cargo-home"}), self.assertRaisesRegex(
                M.CampaignError, "absolute and canonical"
            ):
                M.reject_unsigned_cargo_configuration(repo, set())


class SerialCampaignTests(unittest.TestCase):
    """Exercise the real orchestration/reader with mocked commands and synthetic evidence only."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="synthetic-smoke-campaign-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.repo = self.root / "repo"
        self.repo.mkdir()
        (self.repo / "scripts").mkdir()
        self.driver = self.repo / "scripts" / SCRIPT.name
        self.driver.write_text("# Synthetic signed source path; never executed.\n")
        self.target = self.repo / "target" / "synthetic"
        self.validator = self.target / "release" / "iroha3d"
        self.integration = self.target / "release" / "deps" / "nexus_and_streaming-synthetic"
        self.output = self.root / "campaign"
        self.seal = {"commit": COMMIT, "tree": "b"*40, "tracked_files": 1, "source_sha256": "c"*64}
        self.invocations = []
        self.clock = 100
        self.fail_run = None
        self.drift_run = None
        self.source_drift_run = None
        self.patchers = [mock.patch.object(M, "source_seal", side_effect=lambda *_: self.seal.copy()),
                         mock.patch.object(M, "__file__", str(self.driver)),
                         mock.patch.object(M, "new_request", side_effect=lambda _commit, run: request(run)),
                         mock.patch.object(M, "command", side_effect=self.fake_command)]
        for patcher in self.patchers:
            patcher.start()
            self.addCleanup(patcher.stop)

    def fake_command(self, arguments: list[str], _repo: Path, environment: dict, directory: Path,
                     name: str, *, check: bool = True) -> dict:
        """Simulate a command receipt, never spawning the supplied executable."""
        self.invocations.append((name, arguments))
        output = "Synthetic unit-test command output; not release evidence.\n"
        exit_code = 0
        if name == "build-validator":
            (self.target / "release" / "deps").mkdir(parents=True)
            for path in (self.validator, self.integration):
                path.write_text("Synthetic test executable bytes: NEVER EXECUTED.\n")
                path.chmod(0o700)
        elif name == "build-integration":
            output += M.canonical({"reason": "compiler-artifact", "target": {"name": "nexus_and_streaming", "kind": ["test"]},
                                  "executable": str(self.integration)}).decode()+"\n"
        elif name == "discovery":
            output = f"{M.TEST_NAME}: test\n\n1 test, 0 benchmarks\n"
        elif name == "stdout":
            req = M.read_json(directory / "request.json")
            self.assertEqual(environment["IROHA_TEST_REQUIRE_NETWORK"], "1")
            self.assertEqual(environment["IROHA_TEST_NETWORK_START_ATTEMPTS"], "1")
            self.assertEqual(environment["IROHA_TEST_SKIP_BUILD"], "1")
            self.assertEqual(environment["APS_REAL_PROCESS_REQUEST_SHA256"], M.sha((directory/"request.json").read_bytes()))
            self.assertEqual(list((directory/"evidence").iterdir()), [])
            M.owner_path(directory/"evidence", directory=True)
            evidence, result = evidence_fixture(req["run"], M.file_digest(self.validator))
            store_evidence(directory, evidence, result)
            if req["run"] == self.fail_run:
                exit_code = 101
                output = "running 1 test\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out; finished in 1.0s\n"
            else:
                output = "running 1 test\nAPS smoke completed: synthetic test only\n"
                output += "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 1.0s\n"
            if req["run"] == self.drift_run:
                self.validator.write_text("Synthetic mid-run substituted binary.\n")
            if req["run"] == self.source_drift_run:
                self.seal = {**self.seal, "source_sha256": "d"*64}
        M.write_new(directory / f"{name}.log", output.encode())
        self.clock += 10
        record = {"version": 1, "command": arguments, "exit_code": exit_code, "started_ns": self.clock,
                  "finished_ns": self.clock + 1, "log": f"{name}.log", "log_sha256": M.sha(output.encode())}
        M.write_json(directory / f"{name}.json", record)
        if check:
            M.require(exit_code == 0, "synthetic failed command")
        return record

    def run_driver(self) -> dict:
        return M.run_campaign(self.repo, self.output, self.target, COMMIT)

    def test_ten_serial_fresh_runs_one_build_and_readonly_validation(self) -> None:
        campaign = self.run_driver()
        self.assertEqual(len(campaign["runs"]), 10)
        self.assertEqual([name for name, _ in self.invocations[:6]],
                         ["verify-commit", "toolchain-rustc", "toolchain-cargo", "build-validator", "build-integration", "discovery"])
        self.assertEqual([name for name, _ in self.invocations[6:]], ["stdout"]*10)
        self.assertEqual(len(self.invocations), 16)
        before = {str(path.relative_to(self.output)): (path.stat().st_mtime_ns, M.file_digest(path))
                  for path in self.output.rglob("*") if path.is_file()}
        self.assertEqual(M.validate_campaign(self.output, expected_commit=COMMIT), campaign)
        after = {str(path.relative_to(self.output)): (path.stat().st_mtime_ns, M.file_digest(path))
                 for path in self.output.rglob("*") if path.is_file()}
        self.assertEqual(before, after)
        self.assertEqual(len(self.invocations), 16, "read-only validation must never invoke the build/network command helper")
        with self.assertRaisesRegex(M.CampaignError, "another source"):
            M.validate_campaign(self.output, expected_commit="d"*40)

    def test_failure_is_retained_without_retry_and_after_seal_is_recorded(self) -> None:
        self.fail_run = 1
        with self.assertRaisesRegex(M.CampaignError, "run 1 failed"):
            self.run_driver()
        self.assertEqual(sum(name == "stdout" for name, _ in self.invocations), 2)
        self.assertTrue((self.output / "failure.json").is_file())
        self.assertTrue((self.output / "run-01" / "after.json").is_file())
        self.assertFalse((self.output / "run-02").exists())
        self.assertFalse((self.output / "campaign.json").exists())
        with self.assertRaisesRegex(M.CampaignError, "retains a failure"):
            M.validate_campaign(self.output)

    def test_binary_drift_during_first_run_stops_campaign(self) -> None:
        self.drift_run = 0
        with self.assertRaisesRegex(M.CampaignError, "drift during smoke"):
            self.run_driver()
        self.assertEqual(sum(name == "stdout" for name, _ in self.invocations), 1)
        self.assertNotEqual(M.read_json(self.output/"run-00"/"before.json"), M.read_json(self.output/"run-00"/"after.json"))

    def test_source_drift_during_first_run_stops_campaign(self) -> None:
        self.source_drift_run = 0
        with self.assertRaisesRegex(M.CampaignError, "drift during smoke"):
            self.run_driver()
        self.assertEqual(sum(name == "stdout" for name, _ in self.invocations), 1)

    def test_reused_request_nonce_prevents_second_network(self) -> None:
        def reused(_commit: str, run: int) -> dict:
            value = request(run)
            value["invocation_nonce"] = request(0)["invocation_nonce"]
            value["request_id"] = M.sha(M.canonical({key: item for key, item in value.items() if key != "request_id"}))
            return value
        with mock.patch.object(M, "new_request", side_effect=reused):
            with self.assertRaisesRegex(M.CampaignError, "fresh request collision"):
                self.run_driver()
        self.assertEqual(sum(name == "stdout" for name, _ in self.invocations), 1)

    def test_output_inside_repository_and_existing_output_are_rejected(self) -> None:
        with self.assertRaisesRegex(M.CampaignError, "outside the repository"):
            M.run_campaign(self.repo, self.repo/"evidence", self.target, COMMIT)
        self.output.mkdir(mode=0o700)
        with self.assertRaises(FileExistsError):
            self.run_driver()
        self.assertEqual(self.invocations, [])

    def test_existing_build_target_is_rejected_before_any_build(self) -> None:
        self.target.mkdir(parents=True)
        with self.assertRaisesRegex(M.CampaignError, "fresh build target"):
            self.run_driver()
        self.assertEqual(self.invocations, [])


if __name__ == "__main__":
    unittest.main()
