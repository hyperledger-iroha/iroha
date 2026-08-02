from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
STALE_DEPLOY_RESPONSE = (
    '{ "ok": true, "contract_alias": "router::universal", '
    '"contract_address": "irohac1…", "previous_contract_address": "irohac1…"? , '
    '"kaizen": false, "dataspace": "universal", "deploy_nonce": 0, '
    '"tx_hash_hex": "…", "code_hash_hex": "…", "abi_hash_hex": "…" }'
)
STALE_CONTRACT_DEPLOYMENT_RESPONSE = (
    "{ ok, contract_alias, contract_address, previous_contract_address?, "
    "kaizen, dataspace, deploy_nonce, tx_hash_hex, code_hash_hex, abi_hash_hex }"
)


def _target_docs() -> list[Path]:
    patterns = (
        "specs/governance_api*.md",
        "specs/contract_deployment*.md",
    )
    files: list[Path] = []
    for pattern in patterns:
        files.extend(sorted(REPO_ROOT.glob(pattern)))
    return files


def test_targeted_deploy_docs_do_not_describe_flattened_single_contract_response() -> None:
    docs = _target_docs()
    assert docs, "expected targeted deploy/governance docs to exist"
    offenders: list[str] = []
    for path in docs:
        body = path.read_text(encoding="utf-8")
        if STALE_DEPLOY_RESPONSE in body or STALE_CONTRACT_DEPLOYMENT_RESPONSE in body:
            offenders.append(str(path.relative_to(REPO_ROOT)))
    assert not offenders, "flattened deploy response still documented in: " + ", ".join(
        offenders
    )


def test_canonical_english_docs_reference_bundle_receipt_shape() -> None:
    governance = (REPO_ROOT / "specs/governance_api.md").read_text(encoding="utf-8")
    contract_deployment = (
        REPO_ROOT / "specs/contract_deployment.md"
    ).read_text(encoding="utf-8")
    assert "DeployContractBundleReceiptDto" in governance
    assert "contracts[]" in governance
    assert "DeployContractBundleReceiptDto" in contract_deployment
    assert "contracts[]" in contract_deployment
