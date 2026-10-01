"""Private RSA operations in the protected worker's already loaded TLS library.

No private PEM crosses an exec, child pipe or temporary file. This uses OpenSSL's
documented EVP interface, with explicit SHA256/PKCS1 padding and bounded output:
https://docs.openssl.org/3.0/man3/EVP_DigestSignInit/
"""
from __future__ import annotations

import _ssl
import ctypes as c
import stat
import sys
from pathlib import Path

from .attestation import AttestationRejected, require


def _crypto():
    # _ssl is already loaded by the fixed isolated Python runtime. Reuse its
    # dependency symbols; do not search for a library through PATH or the env.
    original = Path(_ssl.__file__)
    require(original.is_absolute() and original.resolve() == original,
            "TLS crypto module original unavailable")
    if sys.platform == "linux":
        for path in (original, *original.parents):
            metadata = path.lstat()
            require(metadata.st_uid == 0 and not metadata.st_mode & 0o022
                    and not stat.S_ISLNK(metadata.st_mode),
                    "TLS crypto module custody unavailable")
    library = c.CDLL(str(original))
    signatures = {
        'OpenSSL_version_num': (c.c_ulong, []),
        'BIO_new_mem_buf': (c.c_void_p, [c.c_void_p, c.c_int]),
        'BIO_free': (c.c_int, [c.c_void_p]),
        'PEM_read_bio_PrivateKey': (c.c_void_p, [c.c_void_p, c.c_void_p, c.c_void_p, c.c_void_p]),
        'EVP_PKEY_free': (None, [c.c_void_p]),
        'i2d_PUBKEY': (c.c_int, [c.c_void_p, c.POINTER(c.c_void_p)]),
        'EVP_MD_CTX_new': (c.c_void_p, []),
        'EVP_MD_CTX_free': (None, [c.c_void_p]),
        'EVP_sha256': (c.c_void_p, []),
        'EVP_DigestSignInit': (c.c_int, [c.c_void_p, c.POINTER(c.c_void_p), c.c_void_p,
                                       c.c_void_p, c.c_void_p]),
        'EVP_PKEY_CTX_set_rsa_padding': (c.c_int, [c.c_void_p, c.c_int]),
        'EVP_DigestSign': (c.c_int, [c.c_void_p, c.c_void_p, c.POINTER(c.c_size_t),
                                   c.c_void_p, c.c_size_t]),
        'OPENSSL_cleanse': (None, [c.c_void_p, c.c_size_t]),
    }
    for name, (result, arguments) in signatures.items():
        function = getattr(library, name)
        function.restype = result; function.argtypes = arguments
    require(0x30000000 <= library.OpenSSL_version_num() < 0x40000000,
            "TLS crypto requires maintained OpenSSL 3")
    return library


def private_rsa_operation(private_pem: bytes, *, public_only: bool,
                          message: bytes = b'') -> bytes:
    """Encode the public SPKI or sign one exact bounded SHA256/PKCS1 message."""
    require(type(private_pem) is bytes and 0 < len(private_pem) <= 8192
            and private_pem.startswith(b'-----BEGIN PRIVATE KEY-----\n')
            and private_pem.endswith(b'-----END PRIVATE KEY-----\n')
            and type(public_only) is bool and type(message) is bytes
            and (not message if public_only else 0 < len(message) <= 4096),
            "Google OAuth signing input outside bound")
    library = None; private = None; bio = None; key = None; context = None
    try:
        library = _crypto()
        private = c.create_string_buffer(private_pem)
        bio = library.BIO_new_mem_buf(private, len(private_pem))
        require(bool(bio), "Google OAuth signing operation failed")
        key = library.PEM_read_bio_PrivateKey(bio, None, None, None)
        require(bool(key), "Google OAuth signing operation failed")
        if public_only:
            size = library.i2d_PUBKEY(key, None)
            require(0 < size <= 4096, "Google OAuth public key outside bound")
            output = c.create_string_buffer(size)
            position = c.c_void_p(c.addressof(output))
            require(library.i2d_PUBKEY(key, c.byref(position)) == size
                    and position.value == c.addressof(output) + size,
                    "Google OAuth public key encoding failed")
            return output.raw
        context = library.EVP_MD_CTX_new()
        require(bool(context), "Google OAuth signing operation failed")
        signing = c.c_void_p()
        require(library.EVP_DigestSignInit(context, c.byref(signing), library.EVP_sha256(),
                                         None, key) == 1
                and bool(signing.value)
                and library.EVP_PKEY_CTX_set_rsa_padding(signing, 1) == 1,
                "Google OAuth RS256 signing unavailable")
        payload = c.create_string_buffer(message)
        size = c.c_size_t()
        require(library.EVP_DigestSign(context, None, c.byref(size), payload, len(message)) == 1
                and 256 <= size.value <= 512, "Google OAuth signature outside bound")
        output = c.create_string_buffer(size.value)
        expected = size.value
        require(library.EVP_DigestSign(context, output, c.byref(size), payload, len(message)) == 1
                and size.value == expected, "Google OAuth signing operation failed")
        return output.raw
    except AttestationRejected:
        raise
    except Exception:
        raise AttestationRejected('Google OAuth signing operation failed') from None
    finally:
        if library is not None:
            if context: library.EVP_MD_CTX_free(context)
            if key: library.EVP_PKEY_free(key)
            if bio: library.BIO_free(bio)
            if private is not None: library.OPENSSL_cleanse(private, c.sizeof(private))
