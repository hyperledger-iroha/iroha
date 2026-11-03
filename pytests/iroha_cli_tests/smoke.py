"""Current CLI smoke-test inputs from the disposable four-validator localnet."""

from contextlib import contextmanager
from dataclasses import dataclass
import os
from pathlib import Path
import subprocess
import tempfile
from typing import Iterator, Mapping

import tomlkit


@dataclass(frozen=True)
class Localnet:
    """Existing native executable and generated client configuration, never credentials."""

    binary: Path
    config: Path
    ports: tuple[int, ...]


def load_localnet(environ: Mapping[str, str], cwd: Path) -> Localnet:
    """Require the generated files and exactly four consecutive local peer ports."""
    root = (cwd / environ["TMP_DIR"]).resolve(strict=True)
    binary = (root / environ["IROHA_CLI_BINARY"]).resolve(strict=True)
    config = (root / environ["IROHA_CLI_CONFIG"]).resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("IROHA_CLI_BINARY must name an executable regular file")
    if not config.is_file():
        raise ValueError("IROHA_CLI_CONFIG must name the generated client TOML file")
    first = int(environ.get("TORII_API_PORT_MIN", "8080"))
    last = int(environ.get("TORII_API_PORT_MAX", "8083"))
    if not 1 <= first <= last <= 65535 or last - first != 3:
        raise ValueError("CLI smoke tests require exactly four consecutive peer ports")
    return Localnet(binary, config, tuple(range(first, last + 1)))


@contextmanager
def peer_config(localnet: Localnet, peer: int) -> Iterator[Path]:
    """Change only Torii's port in a private sibling of the generated TOML.

    Keeping the file beside its original preserves relative network-identity and
    credential paths. The original bytes and all account fields remain untouched.
    """
    if not 0 <= peer < len(localnet.ports):
        raise ValueError("peer is outside the generated four-validator committee")
    source = tomlkit.parse(localnet.config.read_text(encoding="utf-8"))
    if source["torii_url"] != f"http://127.0.0.1:{localnet.ports[0]}/":
        raise ValueError("client.toml must select the generated first loopback peer")
    if source["http_transport_policy"] != "direct_loopback":
        raise ValueError("the generated direct_loopback transport policy is required")
    source["torii_url"] = f"http://127.0.0.1:{localnet.ports[peer]}/"
    descriptor, name = tempfile.mkstemp(
        prefix=".cli-smoke-", suffix=".toml", dir=localnet.config.parent
    )
    path = Path(name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            output.write(tomlkit.dumps(source))
        yield path
    finally:
        path.unlink()


def run_cli(binary: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Execute the actual native CLI with bounded time and retain its exit status."""
    return subprocess.run(
        [str(binary), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=30,
        check=False,
    )
