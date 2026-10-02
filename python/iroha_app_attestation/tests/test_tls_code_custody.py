from iroha_app_attestation.native_time_interval import NativeTimeInterval
"""Actual public-file/dependency custody tests; no credential or issuer launch.

The selected Homebrew runtime is deliberately unapproved and must fail the real
Root guard. A Root-owned SDK header tests FD/digest mechanics only, not runtime
approval. Existing Google primitive tests separately label their ownership mock.
"""
import _ssl
import ctypes as c
import errno
import hashlib
import os
import pwd
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation import google_oauth as oauth
from iroha_app_attestation import openssl_private_rsa as crypto
from iroha_app_attestation.attestation import AttestationRejected
from test_google_oauth import ORIGINAL, POLICY

ROOT_HEADER=Path('/Library/Developer/CommandLineTools/SDKs/MacOSX27.0.sdk/usr/include/dlfcn.h')


@unittest.skipUnless(sys.platform=='darwin','actual Darwin custody component')
class DarwinTlsCodeCustodyTests(unittest.TestCase):
    def test_actual_root_public_original_holds_fd_digest_and_rejects_closed_fd(self):
        held=crypto._HeldRootCodeOriginal(ROOT_HEADER)
        try:
            self.assertEqual(os.fstat(held.fd).st_uid,0)
            self.assertFalse(os.get_inheritable(held.fd))
            self.assertEqual(held.digest,hashlib.sha256(ROOT_HEADER.read_bytes()).digest())
            self.assertEqual(held._digest(),held.digest)
            held.recheck()
        finally:held.close()
        held.close()
        with self.assertRaises(AttestationRejected):held.recheck()

    def test_actual_root_original_denies_digest_and_metadata_substitution(self):
        held=crypto._HeldRootCodeOriginal(ROOT_HEADER)
        try:
            held.digest=b'\x00'*32
            with self.assertRaisesRegex(AttestationRejected,'digest changed'):held.recheck()
            held.digest=held._digest()
            held.original=tuple(0 if index==1 else value for index,value in enumerate(held.original))
            with self.assertRaisesRegex(AttestationRejected,'original or ancestor changed'):held.recheck()
        finally:held.close()

    def test_public_user_original_and_symbolic_sdk_alias_cannot_claim_root_custody(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/'public-code-fixture';path.write_bytes(b'public synthetic code')
            path.chmod(0o444)
            with self.assertRaises(AttestationRejected):crypto._HeldRootCodeOriginal(path)
        alias=Path('/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/dlfcn.h')
        self.assertNotEqual(alias,alias.resolve(strict=True))
        with self.assertRaisesRegex(AttestationRejected,'physical original'):crypto._HeldRootCodeOriginal(alias)

    def test_actual_acl_presence_is_denied_and_absence_requires_an_existing_object(self):
        crypto._no_acl(ROOT_HEADER)
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/'public-acl-fixture';path.write_bytes(b'public ACL fixture')
            result=subprocess.run(['/bin/chmod','+a',f'user:{pwd.getpwuid(os.getuid()).pw_name} allow read',str(path)],
                                  capture_output=True,check=True)
            self.assertEqual(result.returncode,0)
            with self.assertRaisesRegex(AttestationRejected,'extended ACL'):crypto._no_acl(path)
            fd=os.open(path,os.O_RDONLY)
            try:
                with self.assertRaisesRegex(AttestationRejected,'extended ACL'):crypto._no_acl(path,fd)
            finally:os.close(fd)
        with self.assertRaises(FileNotFoundError):crypto._no_acl(path)

    def test_actual_dladdr_selects_module_crypto_and_tls_from_fixed_symbols(self):
        original=Path(_ssl.__file__)
        library=c.CDLL(str(original))
        self.assertEqual(crypto._image(library.PyInit__ssl),original)
        selected=crypto._image(library.OpenSSL_version_num)
        for name in ('BIO_new_mem_buf','BIO_free','PEM_read_bio_PrivateKey','EVP_PKEY_free',
                     'i2d_PUBKEY','EVP_MD_CTX_new','EVP_MD_CTX_free','EVP_sha256',
                     'EVP_DigestSignInit','EVP_PKEY_CTX_set_rsa_padding','EVP_DigestSign','OPENSSL_cleanse'):
            with self.subTest(name=name):self.assertEqual(crypto._image(getattr(library,name)),selected)
        tls=crypto._image(library.SSL_CTX_new)
        self.assertNotEqual(selected,tls)
        self.assertEqual(selected.name,'libcrypto.3.dylib')
        self.assertEqual(tls.name,'libssl.3.dylib')
        self.assertTrue(all(path.is_absolute() and path==path.resolve(strict=True)
                            for path in (original,selected,tls)))

    def test_real_current_homebrew_is_rejected_before_loading_for_private_crypto(self):
        original=Path(_ssl.__file__)
        self.assertEqual(original.stat().st_uid,os.getuid())
        self.assertNotEqual(original.stat().st_uid,0)
        with patch.object(crypto.c,'CDLL',side_effect=AssertionError('must reject before loading')) as load:
            with self.assertRaisesRegex(AttestationRejected,'ancestor custody'):crypto.acquire_crypto_originals()
        load.assert_not_called()

    def test_real_unapproved_runtime_rejects_before_google_fd_duplication_or_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/'public-not-a-credential.json';path.write_bytes(b'public fixture; must not read')
            path.chmod(0o600);fd=os.open(path,os.O_RDONLY)
            try:
                with patch.object(oauth.os,'dup') as duplicate, patch.object(
                        oauth.GoogleServiceAccountTokenProvider,'_read') as read:
                    with self.assertRaisesRegex(AttestationRejected,'ancestor custody'):
                        oauth.GoogleServiceAccountTokenProvider(public_policy_original=ORIGINAL,
                            native_policy=POLICY,credential_fd=fd,trusted_time_interval=lambda:NativeTimeInterval(1_800_000_000_000,1_800_000_000_000),
                            openssl_path=Path('/usr/bin/openssl'))
                    duplicate.assert_not_called();read.assert_not_called()
            finally:os.close(fd)

    def test_real_isolated_child_rejects_current_runtime_with_no_private_intake(self):
        package=Path(crypto.__file__).resolve().parents[1]
        code=("import sys;sys.path.insert(0,"+repr(str(package))+");"
              "from iroha_app_attestation.openssl_private_rsa import acquire_crypto_originals;"
              "from iroha_app_attestation.attestation import AttestationRejected;"
              "exec('try:\\n acquire_crypto_originals()\\nexcept AttestationRejected:\\n print(\\\"UNAPPROVED_CODE_REJECTED\\\")\\nelse:\\n raise SystemExit(9)')")
        result=subprocess.run([sys.executable,'-I','-B','-c',code],capture_output=True,timeout=5)
        self.assertEqual(result.returncode,0,result.stderr.decode())
        self.assertEqual(result.stdout,b'UNAPPROVED_CODE_REJECTED\n')
        self.assertEqual(result.stderr,b'')


class LinuxAclCustodyTests(unittest.TestCase):
    def test_linux_actual_acl_names_still_fail_closed(self):
        # Only the OS-specific ACL branch is selected here; no runtime or issuer
        # is acquired, and no Root/file observation is replaced in production.
        with patch.object(crypto.sys,'platform','linux'), patch.object(crypto.os,'listxattr',return_value=[],create=True):
            crypto._no_acl(Path('/public-fixture'))
        for name in ('system.posix_acl_access','system.posix_acl_default'):
            with patch.object(crypto.sys,'platform','linux'), patch.object(crypto.os,'listxattr',return_value=[name],create=True):
                with self.assertRaises(AttestationRejected):crypto._no_acl(Path('/public-fixture'))


if __name__=='__main__':unittest.main()
