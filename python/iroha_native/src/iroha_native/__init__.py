"""The transport-independent Rust cryptographic and identity owner for Iroha SDKs."""

from ._loader import NativeUnavailableError, load_crypto_extension, require_account_codec_v1
from importlib import metadata

try:
    __version__ = metadata.version("iroha-native")
except metadata.PackageNotFoundError:
    __version__ = "0.0.0.dev0"

__all__ = ["NativeUnavailableError", "load_crypto_extension", "require_account_codec_v1", "__version__"]
