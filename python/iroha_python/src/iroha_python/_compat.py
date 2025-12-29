"""Compatibility shims bridging deprecated Python packages to `iroha_python`."""

from __future__ import annotations

import warnings
from typing import Any, Optional

from .client import ToriiClient, create_torii_client

__all__ = [
    "ToriiClient",
    "create_torii_client",
    "warn_deprecated_import",
    "compat_create_torii_client",
    "compat_torii_client",
    "compat_mock_torii",
]


def warn_deprecated_import(
    target: str, replacement: str, *, hint: Optional[str] = None
) -> None:
    """Emit a standardized ``DeprecationWarning`` for deprecated modules."""

    message = (
        f"`{target}` is provided for backwards compatibility and will be removed in a "
        f"future release. Import `{replacement}` instead."
    )
    if hint:
        message = f"{message} {hint}"
    warnings.warn(message, DeprecationWarning, stacklevel=3)


def compat_torii_client(*args: Any, **kwargs: Any) -> ToriiClient:
    """Instantiate :class:`ToriiClient` while flagging the deprecated path."""

    warn_deprecated_import(
        "iroha_torii_client.client.ToriiClient",
        "iroha_python.ToriiClient",
    )
    return ToriiClient(*args, **kwargs)


def compat_create_torii_client(*args: Any, **kwargs: Any) -> ToriiClient:
    """Return the SDK Torii client via the compatibility constructor helper."""

    warn_deprecated_import(
        "iroha_torii_client.client.create_torii_client",
        "iroha_python.create_torii_client",
    )
    return create_torii_client(*args, **kwargs)


def compat_mock_torii(*args: Any, **kwargs: Any) -> Any:
    """Create a Torii mock server while advising callers to migrate."""

    from iroha_torii_client.mock import ToriiMockServer

    warn_deprecated_import(
        "iroha_torii_client.mock.ToriiMockServer",
        "iroha_python.client",
        hint="Prefer `iroha_python.test_utils.MockTorii` when possible.",
    )
    return ToriiMockServer(*args, **kwargs)
