from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
STALE_DEPLOY_RESPONSE = (
    '{ "ok": true, "contract_alias": "router::universal", '
    '"contract_address": "tairac1…", "previous_contract_address": "tairac1…"? , '
    '"upgraded": false, "dataspace": "universal", "deploy_nonce": 0, '
    '"tx_hash_hex": "…", "code_hash_hex": "…", "abi_hash_hex": "…" }'
)
STALE_CONTRACT_DEPLOYMENT_RESPONSE = (
    "{ ok, contract_alias, contract_address, previous_contract_address?, "
    "upgraded, dataspace, deploy_nonce, tx_hash_hex, code_hash_hex, abi_hash_hex }"
)


def _target_docs() -> list[Path]:
    patterns = (
        "docs/source/governance_api*.md",
        "docs/source/contract_deployment*.md",
        "docs/portal/docs/governance/api*.md",
        "docs/portal/i18n/*/docusaurus-plugin-content-docs/current/governance/api*.md",
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
    governance = (REPO_ROOT / "docs/source/governance_api.md").read_text(encoding="utf-8")
    contract_deployment = (
        REPO_ROOT / "docs/source/contract_deployment.md"
    ).read_text(encoding="utf-8")
    assert "DeployContractBundleReceiptDto" in governance
    assert "contracts[]" in governance
    assert "DeployContractBundleReceiptDto" in contract_deployment
    assert "contracts[]" in contract_deployment
