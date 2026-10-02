"""Private RSA operations in the protected worker's already loaded TLS library.

Actual Root-owned physical module, crypto and TLS originals are held and
rechecked before private intake. These local custody observations do not admit
a signed runtime or construct Native issuer authority; the installed owner must
still admit the exact runtime and loaded dependencies before launch.

No private PEM crosses an exec, child pipe or temporary file. This uses OpenSSL's
documented EVP interface, with explicit SHA256/PKCS1 padding and bounded output:
https://docs.openssl.org/3.0/man3/EVP_DigestSignInit/
"""
from __future__ import annotations

import _ssl
import ctypes as c
import errno
import hashlib
import os
import stat
import sys
from pathlib import Path

from .attestation import AttestationRejected, require

_MAX_CODE_BYTES = 64 * 1024 * 1024


def _identity(value):
    return (value.st_dev,value.st_ino,value.st_size,value.st_uid,value.st_gid,
            value.st_mode,value.st_nlink,value.st_mtime_ns,value.st_ctime_ns,
            getattr(value,'st_flags',0))


def _no_acl(path: Path, descriptor: int | None = None) -> None:
    if sys.platform == 'darwin':
        api=c.CDLL(None,use_errno=True)
        api.acl_get_file.argtypes=[c.c_char_p,c.c_int];api.acl_get_file.restype=c.c_void_p
        api.acl_get_fd.argtypes=[c.c_int];api.acl_get_fd.restype=c.c_void_p
        api.acl_free.argtypes=[c.c_void_p];api.acl_free.restype=c.c_int
        c.set_errno(0)
        acl=(api.acl_get_file(os.fsencode(path),0x100) if descriptor is None else api.acl_get_fd(descriptor))
        error=c.get_errno()
        if acl:
            api.acl_free(acl)
            raise AttestationRejected('TLS code extended ACL is not admitted')
        # Darwin reports an absent extended ACL as NULL/ENOENT. Require the
        # actual object still exists; caller additionally binds its identity.
        require(error==errno.ENOENT,'TLS code ACL observation unavailable')
        (path.lstat() if descriptor is None else os.fstat(descriptor))
    else:
        require(not {'system.posix_acl_access','system.posix_acl_default'} & set(os.listxattr(path if descriptor is None else descriptor)),
                'TLS code extended ACL is not admitted')


def _root_ancestors(original: Path) -> tuple:
    require(sys.platform in ('linux','darwin') and original.is_absolute()
            and original.resolve(strict=True)==original,'TLS code physical original unavailable')
    observed=[]
    for path in original.parents:
        value=path.lstat()
        require(stat.S_ISDIR(value.st_mode) and value.st_uid==0
                and not value.st_mode & 0o022,'TLS code ancestor custody unavailable')
        _no_acl(path)
        observed.append((str(path),value.st_dev,value.st_ino,value.st_uid,value.st_gid,
                         value.st_mode,getattr(value,'st_flags',0)))
    return tuple(observed)


class _HeldRootCodeOriginal:
    """Actual public-code FD/digest custody; no issuer, signer or Native source capability."""
    def __init__(self,path:Path):
        self.path=path;self.fd=-1
        self.ancestors=_root_ancestors(path)
        named=path.lstat()
        require(stat.S_ISREG(named.st_mode) and named.st_uid==0 and named.st_nlink==1
                and not named.st_mode & 0o6022 and 0<named.st_size<=_MAX_CODE_BYTES,
                'TLS code original custody unavailable')
        _no_acl(path)
        try:
            self.fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC)
            self.original=_identity(os.fstat(self.fd))
            require(self.original==_identity(named) and not os.get_inheritable(self.fd),
                    'TLS code held original differs')
            _no_acl(path,self.fd)
            self.digest=self._digest()
            self.recheck()
        except Exception:
            self.close();raise

    def _digest(self):
        value=hashlib.sha256();offset=0
        while offset<self.original[2]:
            raw=os.pread(self.fd,min(64*1024,self.original[2]-offset),offset)
            require(bool(raw),'TLS code original changed length')
            value.update(raw);offset+=len(raw)
        require(not os.pread(self.fd,1,offset),'TLS code original exceeds admitted length')
        return value.digest()

    def recheck(self):
        require(self.fd>=0 and self.ancestors==_root_ancestors(self.path)
                and _identity(os.fstat(self.fd))==self.original
                and _identity(self.path.lstat())==self.original
                and not os.get_inheritable(self.fd),'TLS code original or ancestor changed')
        _no_acl(self.path);_no_acl(self.path,self.fd)
        require(self._digest()==self.digest,'TLS code original digest changed')

    def close(self):
        if self.fd>=0:os.close(self.fd);self.fd=-1


class _DlInfo(c.Structure):
    _fields_=[('filename',c.c_char_p),('base',c.c_void_p),('symbol',c.c_char_p),('address',c.c_void_p)]


def _image(function) -> Path:
    api=c.CDLL(None,use_errno=True);api.dladdr.argtypes=[c.c_void_p,c.POINTER(_DlInfo)];api.dladdr.restype=c.c_int
    info=_DlInfo()
    require(api.dladdr(c.cast(function,c.c_void_p),c.byref(info))!=0 and info.filename,
            'TLS loaded code image unavailable')
    path=Path(os.fsdecode(info.filename))
    require(path.is_absolute() and path.resolve(strict=True)==path,'TLS loaded code image is not exact physical data')
    return path


class _CryptoCodeOriginals:
    """Retains the actual loaded module/crypto/TLS code, without admitting a runtime policy."""
    def __init__(self,library,originals,crypto_path,tls_path,functions):
        self.library=library;self.originals=originals
        self.crypto_path=crypto_path;self.tls_path=tls_path;self.functions=functions

    def recheck(self):
        for original in self.originals:original.recheck()
        require(_image(self.library.PyInit__ssl)==self.originals[0].path
                and all(_image(getattr(self.library,name))==self.crypto_path for name in self.functions)
                and _image(self.library.SSL_CTX_new)==self.tls_path,'TLS loaded dependency selection changed')

    def close(self):
        for original in self.originals:original.close()


def acquire_crypto_originals():
    # _ssl is already loaded by the fixed isolated Python runtime. Reuse its
    # dependency symbols; do not search for a library through PATH or the env.
    original = Path(_ssl.__file__)
    held=[]
    try:
        # Acquire before CDLL and before any private RSA/TLS intake. No offered
        # path, digest, ownership flag or runtime-profile constructor is accepted.
        held.append(_HeldRootCodeOriginal(original))
        library = c.CDLL(str(original))
    except Exception:
        for item in held:item.close()
        raise
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
    try:
        for name, (result, arguments) in signatures.items():
            function = getattr(library, name)
            function.restype = result; function.argtypes = arguments
        crypto=_image(library.OpenSSL_version_num);tls=_image(library.SSL_CTX_new)
        require(_image(library.PyInit__ssl)==original
                and all(_image(getattr(library,name))==crypto for name in signatures),
                'TLS loaded crypto functions differ from one actual image')
        for path in dict.fromkeys((crypto,tls)):
            if path!=original:held.append(_HeldRootCodeOriginal(path))
        owner=_CryptoCodeOriginals(library,held,crypto,tls,tuple(signatures))
        owner.recheck()
        require(0x30000000 <= library.OpenSSL_version_num() < 0x40000000,
                "TLS crypto requires maintained OpenSSL 3")
        return owner
    except Exception:
        for item in held:item.close()
        raise


def private_rsa_operation(private_pem: bytes, *, public_only: bool,
                          message: bytes = b'') -> bytes:
    """Encode the public SPKI or sign one exact bounded SHA256/PKCS1 message."""
    require(type(private_pem) is bytes and 0 < len(private_pem) <= 8192
            and private_pem.startswith(b'-----BEGIN PRIVATE KEY-----\n')
            and private_pem.endswith(b'-----END PRIVATE KEY-----\n')
            and type(public_only) is bool and type(message) is bytes
            and (not message if public_only else 0 < len(message) <= 4096),
            "Google OAuth signing input outside bound")
    library = None; custody=None; private = None; bio = None; key = None; context = None
    try:
        custody=acquire_crypto_originals();custody.recheck();library=custody.library
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
            custody.recheck()
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
        custody.recheck()
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
        if custody is not None:custody.close()
