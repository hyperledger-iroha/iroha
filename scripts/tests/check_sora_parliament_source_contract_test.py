"""Mutation coverage for constructor-authenticated Parliament source boundaries."""

from __future__ import annotations

import ast
import inspect
import re

import pytest

from scripts.formal import check_sora_parliament_source_contract as guard


OPAQUE_OWNERS = (
    (
        "crates/iroha_core/src/tle_release.rs",
        "AuthorizedTleReleaseContextV1",
        "/// Authorize a TLE release from one point-in-time committed state view.",
        "opaque release authorization",
        guard.require_opaque_release_authorizations,
    ),
    (
        "crates/iroha_core/src/tle_release.rs",
        "ValidatedTleReleaseProjectionV1",
        "/// Closed failures while validating a public authenticated-broker projection.",
        "validated broker projection",
        guard.require_opaque_release_authorizations,
    ),
    (
        "crates/iroha_core/src/tle_release/casting.rs",
        "AuthorizedTimedOvnCastingContextV1",
        "/// Authorize and replay-validate one public timed-OVN casting context.",
        "opaque casting authorization",
        guard.require_opaque_casting_authorization,
    ),
)


@pytest.mark.parametrize("owner", OPAQUE_OWNERS, ids=lambda owner: owner[1])
def test_opaque_authorization_source_baseline(owner: tuple) -> None:
    """The actual production owner passes before any mutation is considered."""
    path, _, _, _, check = owner
    check(guard.read(path))


@pytest.mark.parametrize("owner", OPAQUE_OWNERS, ids=lambda owner: owner[1])
@pytest.mark.parametrize("trait", ("SerializePayload", "DeserializePayload"))
@pytest.mark.parametrize("implementation", ("derive", "manual"))
def test_opaque_authorizations_reject_payload_codecs(
    owner: tuple, trait: str, implementation: str
) -> None:
    """Neither a derive attribute nor an impl may make authorized state decodable."""
    path, name, end, label, check = owner
    source = guard.read(path)
    check(source)
    if implementation == "derive":
        declaration = f"pub struct {name} {{"
        assert source.count(declaration) == 1
        mutated = source.replace(
            declaration, f"#[derive(norito::{trait})]\n{declaration}", 1
        )
    else:
        assert source.count(end) == 1
        lifetime = "<'_>" if trait == "DeserializePayload" else ""
        mutated = source.replace(
            end, f"impl norito::{trait}{lifetime} for {name} {{}}\n\n{end}", 1
        )
    assert mutated != source
    with pytest.raises(RuntimeError, match=re.escape(f"{path}: {label} exposes {trait!r}")):
        check(mutated)


def test_opaque_checks_are_connected_to_production_entrypoint() -> None:
    """The production check invokes both independently tested authorization gates."""
    body = ast.parse(inspect.getsource(guard.main))
    calls = [node.func.id for node in ast.walk(body) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name)]
    assert calls.count("require_opaque_release_authorizations") == 1
    assert calls.count("require_opaque_casting_authorization") == 1
