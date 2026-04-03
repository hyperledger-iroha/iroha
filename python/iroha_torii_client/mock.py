"""Lightweight Torii mock server for typed Torii API smoke tests."""

from __future__ import annotations

import argparse
import json
import signal
import sys
import threading
import time
from dataclasses import dataclass, field
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Dict, Iterable, List, Mapping, Optional
from urllib.parse import parse_qs, unquote, urlparse

__all__ = ["ToriiMockServer", "main"]


@dataclass
class _Response:
    status: int
    body: bytes = b""
    headers: Dict[str, str] = field(default_factory=dict)


class _ToriiHTTPServer(ThreadingHTTPServer):
    allow_reuse_address = True

    def __init__(self, server_address, RequestHandlerClass, state: "_MockState"):
        super().__init__(server_address, RequestHandlerClass)
        self.mock_state = state


class _ToriiHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, format: str, *args) -> None:  # noqa: D401 - silence default logging
        return

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        self._dispatch("GET")

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        self._dispatch("POST")

    def do_DELETE(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        self._dispatch("DELETE")

    def _dispatch(self, method: str) -> None:
        parsed = urlparse(self.path)
        params = parse_qs(parsed.query)
        length = int(self.headers.get("Content-Length", "0")) if method in {"POST", "PUT"} else 0
        body = self.rfile.read(length) if length > 0 else b""
        try:
            response = self.server.mock_state.handle_request(  # type: ignore[attr-defined]
                method,
                parsed.path,
                params,
                body,
                self.headers,
            )
        except KeyError:
            self.send_error(HTTPStatus.NOT_FOUND, "not found")
            return
        except ValueError as err:
            self.send_error(HTTPStatus.BAD_REQUEST, str(err))
            return

        self.send_response(response.status)
        for key, value in response.headers.items():
            self.send_header(key, value)
        self.send_header("Content-Length", str(len(response.body)))
        self.end_headers()
        if response.body:
            self.wfile.write(response.body)


class _MockState:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._attachment_seq = 0
        self._report_seq = 0
        self.attachments: Dict[str, Dict[str, Any]] = {}
        self.prover_reports: Dict[str, Dict[str, Any]] = {}
        self.sumeragi_status: Dict[str, Any] = {}
        self.sumeragi_leader: Dict[str, Any] = {}
        self.sumeragi_telemetry: Dict[str, Any] = {}
        self.sumeragi_rbc_status: Dict[str, Any] = {}
        self.sumeragi_rbc_sessions: Dict[str, Any] = {}
        self.pipeline_sequences: Dict[str, Dict[str, Any]] = {}
        self.pipeline_next_plan: Optional[Dict[str, Any]] = None
        self.pipeline_scenario = "success"
        self._pipeline_submit_seq = 0
        self.accounts: Dict[str, Dict[str, Any]] = {}
        self.gov_referenda: Dict[str, Dict[str, Any]] = {}
        self.gov_council_current: Dict[str, Any] = {}
        self.gov_council_audit: Dict[str, Any] = {}
        self.gov_council_derive_result: Dict[str, Any] = {}
        self.gov_council_persist_result: Dict[str, Any] = {}
        self.gov_contracts: Dict[str, Dict[str, Any]] = {}
        self.contract_manifests: Dict[str, Dict[str, Any]] = {}
        self.contract_code_bytes: Dict[str, Dict[str, Any]] = {}
        self.gov_proposals: Dict[str, Dict[str, Any]] = {}
        self.gov_propose_deploy_response: Dict[str, Any] = {}
        self.gov_finalize_response: Dict[str, Any] = {}
        self.gov_enact_response: Dict[str, Any] = {}
        self.gov_protected_namespaces: Dict[str, Any] = {}
        self.gov_locks: Dict[str, Dict[str, Any]] = {}
        self.gov_tallies: Dict[str, Dict[str, Any]] = {}
        self.gov_unlock_stats: Dict[str, Any] = {}
        self.reset()

    # ------------------------------------------------------------------
    # Public helpers called by server
    # ------------------------------------------------------------------
    def handle_request(
        self,
        method: str,
        path: str,
        params: Mapping[str, List[str]],
        body: bytes,
        headers: Mapping[str, str],
    ) -> _Response:
        if method == "POST" and path == "/v1/zk/attachments":
            return self._attachment_post(body, headers)
        if method == "GET" and path == "/v1/zk/attachments":
            return self._attachment_list()
        if method == "GET" and path.startswith("/v1/zk/attachments/"):
            attachment_id = path.split("/")[-1]
            return self._attachment_get(attachment_id)
        if method == "DELETE" and path.startswith("/v1/zk/attachments/"):
            attachment_id = path.split("/")[-1]
            return self._attachment_delete(attachment_id)
        if method == "GET" and path == "/v1/zk/prover/reports":
            return self._prover_list(params)
        if method == "GET" and path == "/v1/zk/prover/reports/count":
            return self._prover_count(params)
        if method == "GET" and path.startswith("/v1/zk/prover/reports/"):
            report_id = path.split("/")[-1]
            return self._prover_get(report_id)
        if method == "DELETE" and path.startswith("/v1/zk/prover/reports/"):
            report_id = path.split("/")[-1]
            return self._prover_delete(report_id)
        if method == "DELETE" and path == "/v1/zk/prover/reports":
            return self._prover_delete_filtered(params)
        if method == "POST" and path in {"/transaction", "/v1/pipeline/transactions", "/v1/transactions"}:
            return self._pipeline_submit(body)
        if method == "GET" and path in {"/v1/pipeline/transactions/status", "/v1/transactions/status"}:
            return self._pipeline_status(params)
        if method == "GET" and path.startswith("/v1/accounts/"):
            account_id = unquote(path.rsplit("/", 1)[-1])
            return self._account_get(account_id)
        if method == "POST" and path == "/v1/gov/proposals/deploy-contract":
            return self._gov_propose_deploy(body)
        if method == "POST" and path == "/v1/gov/finalize":
            return self._gov_finalize(body)
        if method == "POST" and path == "/v1/gov/enact":
            return self._gov_enact(body)
        if method == "POST" and path == "/v1/gov/protected-namespaces":
            return self._gov_protected_set(body)
        if method == "GET" and path == "/v1/gov/protected-namespaces":
            return self._gov_protected_get()
        if method == "GET" and path.startswith("/v1/gov/contracts/"):
            contract_address = unquote(path.rsplit("/", 1)[-1])
            return self._gov_contract_get(contract_address)
        if method == "GET" and path.startswith("/v1/gov/locks/"):
            referendum_id = path.split("/")[-1]
            return self._gov_locks_get(referendum_id)
        if method == "GET" and path.startswith("/v1/gov/referenda/"):
            referendum_id = path.split("/")[-1]
            return self._gov_referendum_get(referendum_id)
        if method == "POST" and path == "/v1/gov/ballots/plain":
            return self._gov_ballot_plain(body)
        if method == "POST" and path == "/v1/gov/ballots/zk":
            return self._gov_ballot_zk(body)
        if method == "GET" and path == "/v1/gov/council/current":
            return _json_response(HTTPStatus.OK, self.gov_council_current)
        if method == "GET" and path == "/v1/gov/council/audit":
            epoch = _parse_int(params.get("epoch"))
            return self._gov_council_audit(epoch)
        if method == "POST" and path == "/v1/gov/council/derive-vrf":
            return self._gov_council_derive(body)
        if method == "POST" and path == "/v1/gov/council/persist":
            return self._gov_council_persist(body)
        if method == "GET" and path.startswith("/v1/contracts/code-bytes/"):
            code_hash = path.split("/")[-1]
            return self._contracts_code_bytes(code_hash)
        if method == "GET" and path.startswith("/v1/contracts/code/"):
            code_hash = path.split("/")[-1]
            return self._contracts_manifest_get(code_hash)
        if method == "GET" and path.startswith("/v1/gov/tally/"):
            referendum_id = path.split("/")[-1]
            return self._gov_tally_get(referendum_id)
        if method == "GET" and path.startswith("/v1/gov/proposals/"):
            proposal_id = path.split("/")[-1]
            return self._gov_proposals_get(proposal_id)
        if method == "GET" and path == "/v1/gov/unlocks/stats":
            return self._gov_unlock_stats()
        if method == "GET" and path == "/v1/sumeragi/status":
            return _json_response(HTTPStatus.OK, self.sumeragi_status)
        if method == "GET" and path == "/v1/sumeragi/leader":
            return _json_response(HTTPStatus.OK, self.sumeragi_leader)
        if method == "GET" and path == "/v1/sumeragi/telemetry":
            return _json_response(HTTPStatus.OK, self.sumeragi_telemetry)
        if method == "GET" and path == "/v1/sumeragi/rbc":
            return _json_response(HTTPStatus.OK, self.sumeragi_rbc_status)
        if method == "GET" and path == "/v1/sumeragi/rbc/sessions":
            return _json_response(HTTPStatus.OK, self.sumeragi_rbc_sessions)
        if method == "GET" and path == "/v1/node/capabilities":
            return _json_response(HTTPStatus.OK, self.node_capabilities)
        if method == "POST" and path == "/__mock__/pipeline/config":
            return self._pipeline_config(body)
        if method == "POST" and path == "/__mock__/accounts/config":
            return self._account_config(body)
        if method == "POST" and path == "/__mock__/gov/config":
            return self._gov_config(body)
        if method == "POST" and path == "/__mock__/reset":
            self.reset()
            return _Response(HTTPStatus.OK, body=b"{}", headers={"Content-Type": "application/json"})
        raise KeyError(path)

    def reset(self) -> None:
        with self._lock:
            self.attachments.clear()
            self.prover_reports.clear()
            self._attachment_seq = 0
            self._report_seq = 0
            self.pipeline_sequences.clear()
            self.pipeline_next_plan = None
            self.pipeline_scenario = "success"
            self._pipeline_submit_seq = 0
            self.accounts.clear()
            self.gov_referenda.clear()
            self.gov_council_current = {"epoch": 0, "members": []}
            self.gov_council_audit = {
                "epoch": 0,
                "seed_hex": "00" * 32,
                "chain_id": "00000000-0000-0000-0000-000000000000",
                "beacon_hex": "00" * 32,
            }
            self.gov_council_derive_result = {"ok": True, "epoch": 0, "members": []}
            self.gov_council_persist_result = {"ok": True, "epoch": 0, "members": []}
            self.gov_contracts.clear()
            self.contract_manifests.clear()
            self.contract_code_bytes.clear()
            self.gov_proposals.clear()
            self.gov_propose_deploy_response = {"ok": True, "proposal_id": "mock-proposal"}
            self.gov_finalize_response = {"ok": True, "tx_instructions": []}
            self.gov_enact_response = {"ok": True, "tx_instructions": []}
            self.gov_protected_namespaces = {"found": False, "namespaces": []}
            self.gov_locks.clear()
            self.gov_tallies.clear()
            self.gov_unlock_stats = {
                "height_current": 0,
                "expired_locks_now": 0,
                "referenda_with_expired": 0,
                "last_sweep_height": 0,
            }
            self.node_capabilities = {
                "abi_version": 1,
                "data_model_version": 1,
            }
            self._seed_reports()
            self._seed_sumeragi()

    # ------------------------------------------------------------------
    # Governance endpoints
    # ------------------------------------------------------------------
    def _gov_config(self, body: bytes) -> _Response:
        try:
            payload = json.loads(body.decode("utf-8") or "{}")
        except json.JSONDecodeError as err:
            raise ValueError(f"invalid governance config: {err}") from err
        referenda = payload.get("referenda", [])
        if not isinstance(referenda, list):
            raise ValueError("referenda must be a list")
        new_state: Dict[str, Dict[str, Any]] = {}
        for entry in referenda:
            if not isinstance(entry, dict):
                raise ValueError("referendum entry must be an object")
            referendum_data = entry.get("referendum") or {}
            if not isinstance(referendum_data, dict):
                raise ValueError("referendum field must be an object")
            referendum_id = entry.get("id") or referendum_data.get("id")
            if not referendum_id or not isinstance(referendum_id, str):
                raise ValueError("referendum entry missing id")
            referendum_payload = dict(referendum_data)
            referendum_payload.setdefault("id", referendum_id)
            mode_value = referendum_payload.get("mode")
            if not isinstance(mode_value, str):
                raise ValueError("referendum.mode must be a string")
            new_state[referendum_id] = {
                "referendum": referendum_payload,
                "ballot_plain": entry.get("ballot_plain_response"),
                "ballot_zk": entry.get("ballot_zk_response"),
            }
        self.gov_referenda = new_state

        council_current = payload.get("council_current")
        if council_current is not None:
            if not isinstance(council_current, dict):
                raise ValueError("council_current must be an object")
            self.gov_council_current = dict(council_current)
        else:
            self.gov_council_current = {"epoch": 0, "members": []}

        council_audit = payload.get("council_audit")
        if council_audit is not None:
            if not isinstance(council_audit, dict):
                raise ValueError("council_audit must be an object")
            self.gov_council_audit = dict(council_audit)
        else:
            self.gov_council_audit = {
                "epoch": 0,
                "seed_hex": "00" * 32,
                "chain_id": "00000000-0000-0000-0000-000000000000",
                "beacon_hex": "00" * 32,
            }

        derive_result = payload.get("council_derive_response")
        if derive_result is not None:
            if not isinstance(derive_result, dict):
                raise ValueError("council_derive_response must be an object")
            self.gov_council_derive_result = dict(derive_result)
        else:
            self.gov_council_derive_result = {"ok": True, "epoch": 0, "members": []}

        persist_result = payload.get("council_persist_response")
        if persist_result is not None:
            if not isinstance(persist_result, dict):
                raise ValueError("council_persist_response must be an object")
            self.gov_council_persist_result = dict(persist_result)
        else:
            self.gov_council_persist_result = {"ok": True, "epoch": 0, "members": []}

        gov_contracts_payload = payload.get("gov_contracts")
        if gov_contracts_payload is not None:
            if not isinstance(gov_contracts_payload, dict):
                raise ValueError("gov_contracts must be an object")
            normalized_contracts: Dict[str, Dict[str, Any]] = {}
            for contract_address, entry in gov_contracts_payload.items():
                if not isinstance(entry, dict):
                    raise ValueError("gov_contracts entry must be an object")
                normalized_contracts[str(contract_address)] = entry
            self.gov_contracts = normalized_contracts
        else:
            self.gov_contracts = {}

        manifests_payload = payload.get("manifests")
        if manifests_payload is not None:
            if not isinstance(manifests_payload, dict):
                raise ValueError("manifests must be an object")
            normalized_manifests: Dict[str, Dict[str, Any]] = {}
            for key, value in manifests_payload.items():
                if not isinstance(value, dict):
                    raise ValueError("manifest entry must be an object")
                normalized_manifests[str(key).lower()] = value
            self.contract_manifests = normalized_manifests
        else:
            self.contract_manifests = {}

        code_bytes_payload = payload.get("code_bytes")
        if code_bytes_payload is not None:
            if not isinstance(code_bytes_payload, dict):
                raise ValueError("code_bytes must be an object")
            normalized_code_bytes: Dict[str, Dict[str, Any]] = {}
            for key, value in code_bytes_payload.items():
                if not isinstance(value, dict):
                    raise ValueError("code_bytes entry must be an object")
                normalized_code_bytes[str(key).lower()] = value
            self.contract_code_bytes = normalized_code_bytes
        else:
            self.contract_code_bytes = {}

        proposals_payload = payload.get("proposals")
        if proposals_payload is not None:
            if not isinstance(proposals_payload, dict):
                raise ValueError("proposals must be an object")
            normalized_proposals: Dict[str, Dict[str, Any]] = {}
            for key, value in proposals_payload.items():
                if not isinstance(value, dict):
                    raise ValueError("proposal entry must be an object")
                normalized_proposals[str(key).lower()] = value
            self.gov_proposals = normalized_proposals
        else:
            self.gov_proposals = {}

        propose_payload = payload.get("propose_deploy_response")
        if propose_payload is not None:
            if not isinstance(propose_payload, dict):
                raise ValueError("propose_deploy_response must be an object")
            self.gov_propose_deploy_response = dict(propose_payload)
        else:
            self.gov_propose_deploy_response = {"ok": True, "proposal_id": "mock-proposal"}

        finalize_payload = payload.get("finalize_response")
        if finalize_payload is not None:
            if not isinstance(finalize_payload, dict):
                raise ValueError("finalize_response must be an object")
            self.gov_finalize_response = dict(finalize_payload)
        else:
            self.gov_finalize_response = {"ok": True, "tx_instructions": []}

        enact_payload = payload.get("enact_response")
        if enact_payload is not None:
            if not isinstance(enact_payload, dict):
                raise ValueError("enact_response must be an object")
            self.gov_enact_response = dict(enact_payload)
        else:
            self.gov_enact_response = {"ok": True, "tx_instructions": []}

        protected_payload = payload.get("protected_namespaces")
        if protected_payload is not None:
            if not isinstance(protected_payload, dict):
                raise ValueError("protected_namespaces must be an object")
            namespaces_value = protected_payload.get("namespaces", [])
            if namespaces_value is None:
                namespaces_list: List[str] = []
            else:
                if not isinstance(namespaces_value, list):
                    raise ValueError("protected_namespaces.namespaces must be a list")
                namespaces_list = []
                for raw in namespaces_value:
                    if not isinstance(raw, str):
                        raise ValueError("protected_namespaces entries must be strings")
                    namespaces_list.append(raw)
            found_value = protected_payload.get("found")
            if found_value is None:
                found = bool(namespaces_list)
            elif isinstance(found_value, bool):
                found = found_value
            else:
                raise ValueError("protected_namespaces.found must be a boolean")
            self.gov_protected_namespaces = {
                "found": found,
                "namespaces": namespaces_list,
            }
        else:
            self.gov_protected_namespaces = {"found": False, "namespaces": []}

        locks_payload = payload.get("locks")
        if locks_payload is not None:
            if not isinstance(locks_payload, dict):
                raise ValueError("locks must be an object")
            normalized_locks: Dict[str, Dict[str, Any]] = {}
            for key, value in locks_payload.items():
                if not isinstance(value, dict):
                    raise ValueError("locks entry must be an object")
                normalized_locks[str(key)] = value
            self.gov_locks = normalized_locks
        else:
            self.gov_locks = {}

        tallies_payload = payload.get("tallies")
        if tallies_payload is not None:
            if not isinstance(tallies_payload, dict):
                raise ValueError("tallies must be an object")
            normalized_tallies: Dict[str, Dict[str, Any]] = {}
            for key, value in tallies_payload.items():
                if not isinstance(value, dict):
                    raise ValueError("tally entry must be an object")
                normalized_tallies[str(key)] = value
            self.gov_tallies = normalized_tallies
        else:
            self.gov_tallies = {}

        unlock_payload = payload.get("unlock_stats")
        if unlock_payload is not None:
            if not isinstance(unlock_payload, dict):
                raise ValueError("unlock_stats must be an object")
            def _expect_int(name: str) -> int:
                value = unlock_payload.get(name, 0)
                if isinstance(value, bool):
                    return int(value)
                if isinstance(value, (int, float)):
                    return int(value)
                if isinstance(value, str):
                    value_str = value.strip()
                    if value_str == "":
                        return 0
                    return int(value_str)
                raise ValueError(f"unlock_stats.{name} must be an integer")
            self.gov_unlock_stats = {
                "height_current": _expect_int("height_current"),
                "expired_locks_now": _expect_int("expired_locks_now"),
                "referenda_with_expired": _expect_int("referenda_with_expired"),
                "last_sweep_height": _expect_int("last_sweep_height"),
            }
        else:
            self.gov_unlock_stats = {
                "height_current": 0,
                "expired_locks_now": 0,
                "referenda_with_expired": 0,
                "last_sweep_height": 0,
            }

        return _json_response(HTTPStatus.OK, {"configured": len(new_state)})

    def _gov_propose_deploy(self, body: bytes) -> _Response:
        if body:
            try:
                payload = json.loads(body.decode("utf-8") or "{}")
            except json.JSONDecodeError as err:
                raise ValueError(f"invalid propose-deploy payload: {err}") from err
            if not isinstance(payload, dict):
                raise ValueError("propose-deploy payload must be an object")
            if ("contract_address" in payload) == ("contract_alias" in payload):
                raise ValueError("propose-deploy payload must include exactly one of contract_address or contract_alias")
            for key in ("code_hash", "abi_hash"):
                if key not in payload:
                    raise ValueError(f"propose-deploy payload missing '{key}'")
        response = json.loads(json.dumps(self.gov_propose_deploy_response))
        return _json_response(HTTPStatus.OK, response)

    def _gov_finalize(self, body: bytes) -> _Response:
        try:
            payload = json.loads(body.decode("utf-8") or "{}")
        except json.JSONDecodeError as err:
            raise ValueError(f"invalid finalize payload: {err}") from err
        if not isinstance(payload, dict):
            raise ValueError("finalize payload must be an object")
        referendum_id = payload.get("referendum_id")
        proposal_id = payload.get("proposal_id")
        if not isinstance(referendum_id, str):
            raise ValueError("referendum_id must be provided")
        if not isinstance(proposal_id, str):
            raise ValueError("proposal_id must be provided")
        response = json.loads(json.dumps(self.gov_finalize_response))
        return _json_response(HTTPStatus.OK, response)

    def _gov_enact(self, body: bytes) -> _Response:
        try:
            payload = json.loads(body.decode("utf-8") or "{}")
        except json.JSONDecodeError as err:
            raise ValueError(f"invalid enact payload: {err}") from err
        if not isinstance(payload, dict):
            raise ValueError("enact payload must be an object")
        proposal_id = payload.get("proposal_id")
        if not isinstance(proposal_id, str):
            raise ValueError("proposal_id must be provided")
        response = json.loads(json.dumps(self.gov_enact_response))
        return _json_response(HTTPStatus.OK, response)

    def _gov_protected_set(self, body: bytes) -> _Response:
        try:
            payload = json.loads(body.decode("utf-8") or "{}")
        except json.JSONDecodeError as err:
            raise ValueError(f"invalid protected-namespaces payload: {err}") from err
        if not isinstance(payload, dict):
            raise ValueError("protected-namespaces payload must be an object")
        namespaces = payload.get("namespaces")
        if not isinstance(namespaces, list):
            raise ValueError("namespaces must be a list")
        trimmed: List[str] = []
        for entry in namespaces:
            if not isinstance(entry, str):
                raise ValueError("namespaces must contain strings")
            name = entry.strip()
            if name:
                trimmed.append(name)
        with self._lock:
            self.gov_protected_namespaces = {"found": True, "namespaces": list(trimmed)}
        return _json_response(HTTPStatus.OK, {"ok": True, "applied": len(trimmed)})

    def _gov_protected_get(self) -> _Response:
        with self._lock:
            payload = dict(self.gov_protected_namespaces)
        namespaces_value = payload.get("namespaces")
        if isinstance(namespaces_value, list):
            payload["namespaces"] = list(namespaces_value)
        else:
            payload["namespaces"] = []
        payload.setdefault("found", False)
        return _json_response(HTTPStatus.OK, payload)

    def _gov_locks_get(self, referendum_id: str) -> _Response:
        with self._lock:
            entry = self.gov_locks.get(referendum_id)
        if entry is None:
            payload: Dict[str, Any] = {
                "found": False,
                "referendum_id": referendum_id,
            }
        else:
            payload = dict(entry)
            payload.setdefault("referendum_id", referendum_id)
            payload.setdefault("found", True)
        return _json_response(HTTPStatus.OK, payload)

    def _gov_tally_get(self, referendum_id: str) -> _Response:
        with self._lock:
            entry = self.gov_tallies.get(referendum_id)
        if entry is None:
            payload: Dict[str, Any] = {
                "referendum_id": referendum_id,
                "approve": 0,
                "reject": 0,
                "abstain": 0,
            }
        else:
            payload = dict(entry)
            payload.setdefault("referendum_id", referendum_id)
            payload.setdefault("approve", 0)
            payload.setdefault("reject", 0)
            payload.setdefault("abstain", 0)
        return _json_response(HTTPStatus.OK, payload)

    def _gov_unlock_stats(self) -> _Response:
        with self._lock:
            payload = dict(self.gov_unlock_stats)
        payload.setdefault("height_current", 0)
        payload.setdefault("expired_locks_now", 0)
        payload.setdefault("referenda_with_expired", 0)
        payload.setdefault("last_sweep_height", 0)
        return _json_response(HTTPStatus.OK, payload)

    def _gov_referendum_get(self, referendum_id: str) -> _Response:
        entry = self.gov_referenda.get(referendum_id)
        if entry is None:
            return _json_response(HTTPStatus.OK, {"found": False})
        return _json_response(
            HTTPStatus.OK,
            {"found": True, "referendum": dict(entry["referendum"])},
        )

    def _gov_ballot_plain(self, body: bytes) -> _Response:
        try:
            payload = json.loads(body.decode("utf-8") or "{}")
        except json.JSONDecodeError as err:
            raise ValueError(f"invalid plain ballot payload: {err}") from err
        referendum_id = payload.get("referendum_id")
        if not isinstance(referendum_id, str):
            raise ValueError("referendum_id must be provided")
        entry = self.gov_referenda.get(referendum_id)
        if entry is None or entry.get("ballot_plain") is None:
            raise KeyError("governance plain ballot not configured")
        return _json_response(HTTPStatus.OK, entry["ballot_plain"])

    def _gov_ballot_zk(self, body: bytes) -> _Response:
        try:
            payload = json.loads(body.decode("utf-8") or "{}")
        except json.JSONDecodeError as err:
            raise ValueError(f"invalid zk ballot payload: {err}") from err
        election_id = payload.get("election_id")
        if not isinstance(election_id, str):
            raise ValueError("election_id must be provided")
        public_inputs = payload.get("public")
        if public_inputs is not None:
            if not isinstance(public_inputs, Mapping):
                raise ValueError("public inputs must be a JSON object")
            has_owner = public_inputs.get("owner") is not None
            has_amount = public_inputs.get("amount") is not None
            has_duration = public_inputs.get("duration_blocks") is not None
            if (has_owner or has_amount or has_duration) and not (
                has_owner and has_amount and has_duration
            ):
                raise ValueError(
                    "lock hints must include owner, amount, duration_blocks"
                )
            _ensure_governance_owner_canonical(
                public_inputs.get("owner"),
                context="zk ballot public inputs",
            )
        entry = self.gov_referenda.get(election_id)
        if entry is None or entry.get("ballot_zk") is None:
            raise KeyError("governance zk ballot not configured")
        return _json_response(HTTPStatus.OK, entry["ballot_zk"])

    def _gov_contract_get(self, contract_address: str) -> _Response:
        entry = self.gov_contracts.get(contract_address)
        if entry is None:
            return _json_response(
                HTTPStatus.OK,
                {"found": False, "contract_address": contract_address, "dataspace": None, "code_hash_hex": None},
            )
        payload = dict(entry)
        payload.setdefault("found", True)
        payload.setdefault("contract_address", contract_address)
        return _json_response(HTTPStatus.OK, payload)

    def _gov_council_audit(self, epoch: Optional[int]) -> _Response:
        payload = dict(self.gov_council_audit)
        if epoch is not None and "epoch" not in payload:
            payload["epoch"] = epoch
        return _json_response(HTTPStatus.OK, payload)

    def _gov_council_derive(self, body: bytes) -> _Response:
        if body:
            try:
                json.loads(body.decode("utf-8") or "{}")
            except json.JSONDecodeError as err:
                raise ValueError(f"invalid council derive payload: {err}") from err
        return _json_response(HTTPStatus.OK, self.gov_council_derive_result)

    def _gov_council_persist(self, body: bytes) -> _Response:
        if body:
            try:
                json.loads(body.decode("utf-8") or "{}")
            except json.JSONDecodeError as err:
                raise ValueError(f"invalid council persist payload: {err}") from err
        return _json_response(HTTPStatus.OK, self.gov_council_persist_result)

    def _contracts_manifest_get(self, code_hash: str) -> _Response:
        key = code_hash.lower()
        payload = self.contract_manifests.get(key)
        if payload is None:
            return _json_response(HTTPStatus.NOT_FOUND, {"error": "manifest not found"})
        return _json_response(HTTPStatus.OK, payload)

    def _contracts_code_bytes(self, code_hash: str) -> _Response:
        key = code_hash.lower()
        payload = self.contract_code_bytes.get(key)
        if payload is None:
            return _json_response(HTTPStatus.NOT_FOUND, {"error": "code bytes not found"})
        return _json_response(HTTPStatus.OK, payload)

    def _gov_proposals_get(self, proposal_id: str) -> _Response:
        key = proposal_id.lower()
        payload = self.gov_proposals.get(key)
        if payload is None:
            return _json_response(HTTPStatus.OK, {"found": False})
        return _json_response(HTTPStatus.OK, payload)

    # ------------------------------------------------------------------
    # Pipeline endpoints
    # ------------------------------------------------------------------
    def _pipeline_submit(self, body: bytes) -> _Response:  # noqa: ARG002 - future body inspection
        plan = self._make_pipeline_plan()
        statuses = [dict(entry) for entry in plan["statuses"]]
        with self._lock:
            hash_value = plan["hash"] or self._next_pipeline_hash_locked()
            sequence = {
                "remaining": statuses,
                "repeat_last": plan["repeat_last"],
                "last": None,
            }
            self.pipeline_sequences[hash_value] = sequence
        response_body = {
            "payload": {
                "tx_hash": hash_value,
                "submitted_at_ms": 0,
                "submitted_at_height": 0,
                "signer": "mock-signer",
            },
            "signature": "mock-signature",
        }
        return _json_response(plan["submit_status"], response_body)

    def _pipeline_status(self, params: Mapping[str, List[str]]) -> _Response:
        hashes = params.get("hash")
        if not hashes:
            raise ValueError("missing hash query parameter")
        hash_value = str(hashes[0])
        with self._lock:
            sequence = self.pipeline_sequences.get(hash_value)
            if sequence is None:
                raise KeyError("pipeline status")
            remaining = sequence["remaining"]
            if remaining:
                current = dict(remaining.pop(0))
                sequence["last"] = current
            else:
                current = dict(sequence.get("last") or {"kind": "Queued", "content": None})
                sequence["last"] = current
                if not sequence["repeat_last"]:
                    self.pipeline_sequences.pop(hash_value, None)
        payload = self._make_status_payload(hash_value, current)
        return _json_response(HTTPStatus.OK, payload)

    def _pipeline_config(self, body: bytes) -> _Response:
        if body:
            try:
                raw = json.loads(body.decode("utf-8"))
            except json.JSONDecodeError as err:  # pragma: no cover - defensive
                raise ValueError(f"invalid JSON: {err}") from err
            if not isinstance(raw, dict):
                raise ValueError("pipeline config must be a JSON object")
            payload: Dict[str, Any] = raw
        else:
            payload = {}

        scenario_value = payload.get("scenario")
        if scenario_value is not None:
            scenario_str = str(scenario_value)
            if scenario_str not in {"success", "failure", "timeout"}:
                raise ValueError("invalid pipeline scenario")
            with self._lock:
                self.pipeline_scenario = scenario_str

        statuses_override = None
        if "statuses" in payload:
            statuses_value = payload["statuses"]
            if not isinstance(statuses_value, list):
                raise ValueError("statuses must be a list")
            statuses_override = [self._normalize_status_entry(item) for item in statuses_value]

        overrides: Dict[str, Any] = {
            "hash": payload.get("hash"),
            "submit_status": payload.get("submit_status"),
            "accepted": payload.get("accepted"),
            "repeat_last": payload.get("repeat_last"),
            "statuses": statuses_override,
        }
        if scenario_value is not None:
            overrides["scenario"] = scenario_value

        with self._lock:
            self.pipeline_next_plan = overrides
            if (
                isinstance(overrides.get("hash"), str)
                and overrides["hash"]
                and statuses_override is not None
            ):
                self.pipeline_sequences[overrides["hash"]] = {
                    "remaining": [dict(entry) for entry in statuses_override],
                    "repeat_last": bool(payload.get("repeat_last", True)),
                    "last": None,
                }

        return _json_response(HTTPStatus.OK, {"configured": True, "scenario": self.pipeline_scenario})

    def _account_config(self, body: bytes) -> _Response:
        try:
            payload = json.loads(body.decode("utf-8") or "{}")
        except json.JSONDecodeError as err:
            raise ValueError(f"invalid account config: {err}") from err
        if not isinstance(payload, dict):
            raise ValueError("account config must be a JSON object")
        entries = payload.get("accounts", [])
        if not isinstance(entries, list):
            raise ValueError("accounts must be a list")

        configured: Dict[str, Dict[str, Any]] = {}
        for entry in entries:
            if not isinstance(entry, dict):
                raise ValueError("account entry must be an object")
            account_id = entry.get("account_id")
            if not isinstance(account_id, str) or not account_id:
                raise ValueError("account entry missing account_id")
            configured[account_id] = json.loads(json.dumps(entry))

        with self._lock:
            self.accounts = configured

        return _json_response(HTTPStatus.OK, {"configured": len(configured)})

    def _account_get(self, account_id: str) -> _Response:
        with self._lock:
            payload = self.accounts.get(account_id)
            if payload is None:
                raise KeyError("account read")
            response = json.loads(json.dumps(payload))
        return _json_response(HTTPStatus.OK, response)

    def _make_pipeline_plan(self) -> Dict[str, Any]:
        with self._lock:
            overrides = self.pipeline_next_plan
            self.pipeline_next_plan = None
            scenario = self.pipeline_scenario
            if overrides and overrides.get("scenario") is not None:
                scenario = str(overrides["scenario"])

        base = self._scenario_plan(scenario)
        hash_override = overrides.get("hash") if overrides else None
        if hash_override in ("", None):
            hash_value = None
        elif isinstance(hash_override, str):
            hash_value = hash_override
        else:
            hash_value = str(hash_override)

        submit_status = overrides.get("submit_status") if overrides and overrides.get("submit_status") is not None else base["submit_status"]
        accepted = overrides.get("accepted") if overrides and overrides.get("accepted") is not None else base["accepted"]
        repeat_last = overrides.get("repeat_last") if overrides and overrides.get("repeat_last") is not None else base["repeat_last"]
        statuses_source = overrides.get("statuses") if overrides and overrides.get("statuses") is not None else base["statuses"]

        if statuses_source is None:
            statuses_source = []
        submit_status_value = submit_status if submit_status is not None else HTTPStatus.ACCEPTED
        statuses = [self._normalize_status_entry(entry) for entry in statuses_source]
        plan = {
            "hash": hash_value,
            "submit_status": int(submit_status_value),
            "accepted": bool(accepted),
            "repeat_last": bool(repeat_last),
            "statuses": statuses,
        }
        return plan

    def _scenario_plan(self, scenario: Optional[str]) -> Dict[str, Any]:
        if scenario == "failure":
            statuses: List[Dict[str, Any]] = [
                {"kind": "Queued", "content": None},
                {"kind": "Rejected", "content": "mock rejection"},
            ]
        elif scenario == "timeout":
            statuses = [{"kind": "Queued", "content": None}]
        else:
            statuses = [
                {"kind": "Queued", "content": None},
                {"kind": "Approved", "content": None},
                {"kind": "Committed", "content": None},
            ]
        return {
            "hash": None,
            "submit_status": HTTPStatus.ACCEPTED,
            "accepted": True,
            "repeat_last": True,
            "statuses": statuses,
        }

    @staticmethod
    def _normalize_status_entry(entry: object) -> Dict[str, Any]:
        if isinstance(entry, str):
            return {"kind": entry, "content": None}
        if isinstance(entry, Mapping):
            if "kind" not in entry:
                raise ValueError("status entry missing 'kind'")
            kind = str(entry["kind"])
            content = entry.get("content")
            if isinstance(content, (bytes, bytearray)):
                content_value: object = content.decode("utf-8", errors="ignore")
            elif content is None or isinstance(content, (str, int, float, bool)):
                content_value = content
            else:
                content_value = str(content)
            block_height = entry.get("block_height")
            if not isinstance(block_height, int):
                block_height = None
            rejection_reason = entry.get("rejection_reason")
            if not isinstance(rejection_reason, Mapping):
                rejection_reason = None
            scope = entry.get("scope")
            if scope is not None:
                scope = str(scope)
            resolved_from = entry.get("resolved_from")
            if resolved_from is not None:
                resolved_from = str(resolved_from)
            return {
                "kind": kind,
                "content": content_value,
                "block_height": block_height,
                "rejection_reason": rejection_reason,
                "scope": scope,
                "resolved_from": resolved_from,
            }
        raise ValueError("invalid status entry")

    @staticmethod
    def _make_status_payload(hash_value: str, entry: Mapping[str, Any]) -> Dict[str, Any]:
        kind = str(entry.get("kind", "Queued"))
        block_height = entry.get("block_height")
        if block_height is None:
            content = entry.get("content")
            if isinstance(content, int) and kind in {"Committed", "Applied"}:
                block_height = content
        if not isinstance(block_height, int):
            block_height = None

        rejection_reason = entry.get("rejection_reason")
        if not isinstance(rejection_reason, dict):
            rejection_reason = None
        return {
            "hash": hash_value,
            "status": {
                "kind": kind,
                "block_height": block_height,
                "rejection_reason": rejection_reason,
            },
            "scope": str(entry.get("scope", "auto")),
            "resolved_from": str(entry.get("resolved_from", "state")),
        }

    def _next_pipeline_hash_locked(self) -> str:
        self._pipeline_submit_seq += 1
        return f"mock-pipeline-hash-{self._pipeline_submit_seq:04d}"

    # ------------------------------------------------------------------
    # Attachments
    # ------------------------------------------------------------------
    def _attachment_post(self, body: bytes, headers: Mapping[str, str]) -> _Response:
        content_type = headers.get("Content-Type", "application/octet-stream")
        meta = self._register_attachment(body, content_type)
        return _json_response(HTTPStatus.CREATED, meta)

    def _attachment_list(self) -> _Response:
        with self._lock:
            items = [self._attachment_meta(rec) for rec in self.attachments.values()]
        items.sort(key=lambda item: item.get("created_ms", 0))
        return _json_response(HTTPStatus.OK, items)

    def _attachment_get(self, attachment_id: str) -> _Response:
        with self._lock:
            record = self.attachments.get(attachment_id)
            if record is None:
                raise KeyError("attachment")
            body_value = record.get("bytes")
            if not isinstance(body_value, (bytes, bytearray)):
                raise KeyError("attachment bytes")
            body = bytes(body_value)
            ct = str(record.get("content_type", "application/octet-stream"))
        return _Response(HTTPStatus.OK, body=body, headers={"Content-Type": ct})

    def _attachment_delete(self, attachment_id: str) -> _Response:
        with self._lock:
            if attachment_id not in self.attachments:
                raise KeyError("attachment")
            del self.attachments[attachment_id]
        return _Response(HTTPStatus.NO_CONTENT)

    def _register_attachment(self, body: bytes, content_type: str) -> Dict[str, Any]:
        with self._lock:
            self._attachment_seq += 1
            attachment_id = f"att-{self._attachment_seq:04d}"
            created_ms = self._timestamp_ms(self._attachment_seq)
            record: Dict[str, Any] = {
                "id": attachment_id,
                "content_type": content_type,
                "size": len(body),
                "created_ms": created_ms,
                "bytes": body,
            }
            self.attachments[attachment_id] = record
            return self._attachment_meta(record)

    @staticmethod
    def _attachment_meta(record: Mapping[str, Any]) -> Dict[str, Any]:
        return {
            "id": record.get("id", ""),
            "content_type": record.get("content_type", "application/octet-stream"),
            "size": record.get("size", 0),
            "created_ms": record.get("created_ms", 0),
        }

    # ------------------------------------------------------------------
    # Prover reports
    # ------------------------------------------------------------------
    def _prover_list(self, params: Mapping[str, List[str]]) -> _Response:
        reports = self._filter_reports(params)
        if _is_true(params.get("ids_only")):
            payload = [{"id": r["id"]} for r in reports]
        elif _is_true(params.get("messages_only")):
            payload = [{"id": r["id"], "error": r.get("error", "") or ""} for r in reports]
        else:
            payload = [self._report_public_fields(r) for r in reports]
        return _json_response(HTTPStatus.OK, payload)

    def _prover_count(self, params: Mapping[str, List[str]]) -> _Response:
        reports = self._filter_reports(params)
        return _json_response(HTTPStatus.OK, {"count": len(reports)})

    def _prover_get(self, report_id: str) -> _Response:
        with self._lock:
            record = self.prover_reports.get(report_id)
            if record is None:
                raise KeyError("report")
            return _json_response(HTTPStatus.OK, self._report_public_fields(record))

    def _prover_delete(self, report_id: str) -> _Response:
        with self._lock:
            if report_id not in self.prover_reports:
                raise KeyError("report")
            del self.prover_reports[report_id]
        return _Response(HTTPStatus.NO_CONTENT)

    def _prover_delete_filtered(self, params: Mapping[str, List[str]]) -> _Response:
        reports = self._filter_reports(params)
        deleted = 0
        with self._lock:
            for report in reports:
                rid = str(report.get("id", ""))
                if rid and rid in self.prover_reports:
                    del self.prover_reports[rid]
                    deleted += 1
        return _json_response(HTTPStatus.OK, {"deleted": deleted})

    def _filter_reports(self, params: Mapping[str, List[str]]) -> List[Dict[str, Any]]:
        with self._lock:
            reports = [self._report_public_fields(r) for r in self.prover_reports.values()]
        reports.sort(key=lambda r: r.get("processed_ms", 0))
        if params.get("order", ["asc"])[0].lower() == "desc":
            reports.reverse()

        def matches(report: Mapping[str, Any]) -> bool:
            if _is_true(params.get("ok_only")) and not report.get("ok", False):
                return False
            if _is_true(params.get("failed_only")) and report.get("ok", False):
                return False
            if _is_true(params.get("errors_only")) and report.get("ok", False):
                return False
            rid = params.get("id", [None])[0]
            if rid and report.get("id") != rid:
                return False
            ct = params.get("content_type", [None])[0]
            if ct and ct not in str(report.get("content_type", "")):
                return False
            tag = params.get("has_tag", [None])[0]
            if tag:
                tags = report.get("zk1_tags") or []
                if tag not in tags:
                    return False
            since = _parse_int(params.get("since_ms"))
            if since is not None and report.get("processed_ms", 0) < since:
                return False
            before = _parse_int(params.get("before_ms"))
            if before is not None and report.get("processed_ms", 0) > before:
                return False
            return True

        filtered = [r for r in reports if matches(r)]
        if _is_true(params.get("latest")) and filtered:
            filtered = [filtered[-1]]
        offset = _parse_int(params.get("offset")) or 0
        if offset:
            filtered = filtered[offset:]
        limit = _parse_int(params.get("limit"))
        if limit is not None:
            filtered = filtered[:limit]
        return filtered

    def _report_public_fields(self, record: Mapping[str, Any]) -> Dict[str, Any]:
        allowed = {
            "id",
            "ok",
            "error",
            "content_type",
            "size",
            "created_ms",
            "processed_ms",
            "latency_ms",
            "zk1_tags",
        }
        return {key: record.get(key) for key in allowed if key in record}

    def _seed_reports(self) -> None:
        now = self._timestamp_ms(0)
        samples = [
            {
                "ok": True,
                "content_type": "application/json",
                "size": 128,
                "error": None,
                "zk1_tags": ["TEST"],
            },
            {
                "ok": False,
                "content_type": "application/octet-stream",
                "size": 256,
                "error": "prover failed",
                "zk1_tags": ["IPAK"],
            },
        ]
        for sample in samples:
            self._report_seq += 1
            rid = f"rep-{self._report_seq:04d}"
            record = dict(sample)
            record.update(
                {
                    "id": rid,
                    "created_ms": now + self._report_seq,
                    "processed_ms": now + 100 + self._report_seq,
                    "latency_ms": 5,
                }
            )
            self.prover_reports[rid] = record

    @staticmethod
    def _timestamp_ms(seq: int) -> int:
        return int(time.time() * 1000) + seq

    def _seed_sumeragi(self) -> None:
        self.sumeragi_status = {
            "leader_index": 2,
            "highest_qc": {
                "height": 12,
                "view": 4,
                "subject_block_hash": "abcdef1234567890",
            },
            "locked_qc": {
                "height": 10,
                "view": 3,
                "subject_block_hash": "deadbeefcafefeed",
            },
            "gossip_fallback_total": 1,
            "block_created_dropped_by_lock_total": 2,
            "block_created_hint_mismatch_total": 3,
            "block_created_proposal_mismatch_total": 4,
            "tx_queue": {
                "depth": 12,
                "capacity": 128,
                "saturated": False,
            },
            "epoch": {
                "length_blocks": 3600,
                "commit_deadline_offset": 120,
                "reveal_deadline_offset": 160,
            },
            "vrf_penalty_epoch": 7,
            "vrf_committed_no_reveal_total": 5,
            "vrf_no_participation_total": 6,
            "vrf_late_reveals_total": 8,
            "rbc_store": {
                "sessions": 9,
                "bytes": 1000,
                "persist_drops_total": 2,
                "evictions_total": 10,
                "pressure_level": 11,
                "recent_evictions": [
                    {
                        "block_hash": "0123456789abcdef",
                        "height": 13,
                        "view": 5,
                    }
                ],
            },
        }
        self.sumeragi_leader = {
            "leader_index": 3,
            "prf": {
                "height": 20,
                "view": 2,
                "epoch_seed": "feedfacecafebeef",
            },
        }
        self.sumeragi_telemetry = {
            "availability": {
                "total_votes_ingested": 123,
                "collectors": [1, 2, 3],
            },
            "rbc_backlog": {"pending_sessions": 4},
            "vrf": {
                "epoch": 5,
                "finalized": True,
                "reveals_total": 6,
                "late_reveals_total": 7,
                "committed_no_reveal_total": 8,
                "no_participation_total": 9,
            },
        }
        self.sumeragi_rbc_status = {
            "sessions_active": 3,
            "sessions_pruned_total": 2,
            "ready_broadcasts_total": 5,
            "deliver_broadcasts_total": 7,
            "payload_bytes_delivered_total": 99,
        }
        self.sumeragi_rbc_sessions = {
            "sessions_active": 2,
            "items": [
                {
                    "block_hash": "feedfacecafebeef",
                    "height": 42,
                    "view": 8,
                    "ready_count": 3,
                    "total_chunks": 10,
                    "received_chunks": 9,
                    "delivered": True,
                    "invalid": False,
                },
                {
                    "block_hash": "deadbeefcafefeed",
                    "height": 41,
                    "view": 7,
                    "ready_count": 2,
                    "total_chunks": 8,
                    "received_chunks": 8,
                    "delivered": True,
                    "invalid": False,
                },
            ],
        }


def _is_true(values: Optional[Iterable[str]]) -> bool:
    if not values:
        return False
    value = next(iter(values))
    return value is not None and value.lower() in {"1", "true", "yes"}


def _parse_int(values: Optional[Iterable[str]]) -> Optional[int]:
    if not values:
        return None
    value = next(iter(values))
    if value in (None, ""):
        return None
    try:
        return int(value)
    except ValueError:
        return None


def _json_response(status: int, payload: object) -> _Response:
    body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    return _Response(status, body=body, headers={"Content-Type": "application/json"})


def _ensure_governance_owner_canonical(owner: Any, *, context: str) -> None:
    if owner is None:
        return
    if not isinstance(owner, str):
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    trimmed = owner.strip()
    if not trimmed or trimmed != owner:
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    if any(ch.isspace() for ch in trimmed):
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    if "@" in trimmed:
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    if trimmed.lower().startswith("0x"):
        raise ValueError(f"{context}.owner must be a canonical I105 account id")


class ToriiMockServer:
    """Embedded HTTP server used by tests in multiple languages."""

    def __init__(self, host: str = "127.0.0.1", port: int = 0) -> None:
        self._state = _MockState()
        self._server = _ToriiHTTPServer((host, port), _ToriiHandler, self._state)
        self._thread: Optional[threading.Thread] = None

    @property
    def base_url(self) -> str:
        host, port = self._server.server_address[:2]
        host_value = host.decode("utf-8") if isinstance(host, (bytes, bytearray)) else host
        return f"http://{host_value}:{port}/"

    def start(self) -> "ToriiMockServer":
        if self._thread is None:
            self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
            self._thread.start()
        return self

    def stop(self) -> None:
        self._server.shutdown()
        if self._thread is not None:
            self._thread.join(timeout=1.0)
            self._thread = None
        self._server.server_close()

    def reset(self) -> None:
        self._state.reset()

    def serve_forever(self) -> None:
        try:
            self._server.serve_forever()
        finally:
            self._server.server_close()


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Run the Torii mock server")
    parser.add_argument("--host", default="127.0.0.1", help="Listen host (default: 127.0.0.1)")
    parser.add_argument("--port", type=int, default=0, help="Listen port (default: auto)")
    parser.add_argument(
        "--stdio",
        action="store_true",
        help="Emit base URL as JSON to stdout and wait for termination",
    )
    args = parser.parse_args(argv)

    server = ToriiMockServer(args.host, args.port)

    def _graceful_shutdown(signum, frame):  # noqa: D401 - simple signal handler
        _ = (signum, frame)
        server.stop()
        sys.exit(0)

    signal.signal(signal.SIGTERM, _graceful_shutdown)
    signal.signal(signal.SIGINT, _graceful_shutdown)

    if args.stdio:
        server.start()
        print(json.dumps({"base_url": server.base_url}), flush=True)
        try:
            server.serve_forever()
        except SystemExit:
            raise
        except Exception:  # pragma: no cover - unexpected runtime failure
            server.stop()
            raise
        return 0

    print(f"Torii mock server listening on {server.base_url}")
    try:
        server.serve_forever()
    except SystemExit:
        raise
    except Exception:  # pragma: no cover - unexpected runtime failure
        server.stop()
        raise
    return 0


if __name__ == "__main__":
    sys.exit(main())
