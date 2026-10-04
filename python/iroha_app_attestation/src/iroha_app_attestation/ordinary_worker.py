"""Persistent private worker owned by authenticated Native Core startup.

There is no HTTP listener, credential path argument or environment selection.
The parent retains the complete current release and performs a fresh Native
recheck over private pipes before verification, Google decoding and signing.
Returned originals are independently authenticated again by Native Core.
"""
from __future__ import annotations

import base64
import ctypes
import hashlib
import json
import os
import select
import stat
import sys
import time
from pathlib import Path

from .attestation import AttestationRejected, VerificationUnavailable, require
from .native_time_interval import NativeTimeInterval
from .google_oauth import GoogleServiceAccountTokenProvider, _json
from .native_policy_projection import decode_native_policy_projection
from .ordinary_issuance import (CanonicalOrdinaryCredentialEncoder, CanonicalRawAppAdmissionEncoder,
                                DurableOrdinaryCredentialIssuer)
from .ordinary_provider import GovernedOrdinaryEvidenceProvider
from .ordinary_service import OrdinaryCredentialService, PATH, RAW_PATH, REFRESH_PATH, MAX_BODY_BYTES
from .ordinary_refresh_issuance import DurableOrdinaryIntegrityRefreshIssuer
from .play_integrity import GooglePlayIntegrityVerifier
from .hardware_evidence_worker import NativeHardwareEvidenceVerifier
from .service import _decode_base64, _decode_hex32
from .private_process import (close_unrelated_worker_descriptors, disable_core_dumps,
                              protect_darwin_process, require_worker_role_originals)

STARTUP_SCHEMA = "iroha.kagemusha.ordinary-issuer-worker-startup.v1"
REQUEST_SCHEMA = "iroha.kagemusha.ordinary-issuer-worker-request.v1"
MAX_PACKET_BYTES = 3*1024*1024


def protect_private_process() -> None:
    """Set and verify protection in this image, since exec resets dumpability."""
    require(sys.platform in ("linux", "darwin"), "private issuer process protection unavailable")
    disable_core_dumps()
    if sys.platform == "darwin":
        protect_darwin_process()
        return
    process = ctypes.CDLL(None, use_errno=True)
    prctl = process.prctl
    prctl.argtypes = [ctypes.c_int, ctypes.c_ulong, ctypes.c_ulong,
                      ctypes.c_ulong, ctypes.c_ulong]
    prctl.restype = ctypes.c_int
    require(prctl(4, 0, 0, 0, 0) == 0 and prctl(3, 0, 0, 0, 0) == 0,
            "private issuer process protection unavailable")


class NativeParentChannel:
    """Bounded private FD transport; decoding packets is never custody admission."""
    def __init__(self, input_fd: int, output_fd: int):
        require(type(input_fd) is int and type(output_fd) is int and min(input_fd,output_fd) >= 3
                and input_fd != output_fd, "Native parent channel absent")
        self._input = os.dup(input_fd); self._output = os.dup(output_fd)
        os.set_blocking(self._input,False); os.set_blocking(self._output,False)
        self._request = None; self._sequence = 0; self._time = 0; self._interval = None; self._projection = None
        self._deadline = time.monotonic()+60

    def _transfer(self, *, writing: bool, value: bytes | int) -> bytes:
        count = len(value) if writing else value
        result = bytearray(); offset = 0
        while offset < count:
            remaining = None if self._deadline is None else self._deadline-time.monotonic()
            require(remaining is None or remaining > 0,"Native parent channel deadline exceeded")
            readable,writable,_ = select.select([] if writing else [self._input],
                                               [self._output] if writing else [],[],remaining)
            require(bool(writable if writing else readable),"Native parent channel deadline exceeded")
            try:
                if writing:
                    size=os.write(self._output,value[offset:]); require(size>0,"Native parent channel closed")
                else:
                    chunk=os.read(self._input,count-offset); require(bool(chunk),"Native parent channel closed")
                    size=len(chunk); result.extend(chunk)
                offset+=size
            except (BlockingIOError,InterruptedError):
                continue
        return bytes(result)

    def receive(self) -> dict:
        width=int.from_bytes(self._transfer(writing=False,value=4),"little")
        require(0 < width <= MAX_PACKET_BYTES,"Native parent packet outside bound")
        return _json(self._transfer(writing=False,value=width),MAX_PACKET_BYTES,"Native parent packet")

    def send(self,value:dict) -> None:
        original=json.dumps(value,sort_keys=True,separators=(",",":"),allow_nan=False).encode("utf-8")
        require(0 < len(original) <= MAX_PACKET_BYTES,"Native worker packet outside bound")
        self._transfer(writing=True,value=len(original).to_bytes(4,"little")+original)

    def begin(self,request_id:str,projection_sha256:bytes) -> None:
        _decode_hex32(request_id,"Native worker request ID")
        require(type(projection_sha256) is bytes and len(projection_sha256)==32,"Native projection pin absent")
        self._request=request_id;self._projection=projection_sha256;self._sequence=0;self._interval=None
        self._deadline=time.monotonic()+60

    def recheck(self) -> None:
        require(self._request is not None,"Native operation absent")
        self._sequence+=1
        self.send({"kind":"recheck","request_id":self._request,"sequence":self._sequence})
        value=self.receive()
        require(set(value)=={"kind","request_id","sequence","lower_at_ms","upper_at_ms","projection_sha256"}
                and value["kind"]=="current" and value["request_id"]==self._request
                and type(value["sequence"]) is int and value["sequence"]==self._sequence
                and _decode_hex32(value["projection_sha256"],"current Native projection")==self._projection
                and type(value["lower_at_ms"]) is int and type(value["upper_at_ms"]) is int
                and 0 < value["lower_at_ms"] <= value["upper_at_ms"] < (1<<64)
                and value["lower_at_ms"] >= self._time,
                "Native current release/clock response differs")
        self._interval=NativeTimeInterval(value["lower_at_ms"],value["upper_at_ms"]).validate()
        self._time=self._interval.lower_at_ms

    def trusted_time_interval(self) -> NativeTimeInterval:
        require(type(self._interval) is NativeTimeInterval,"Native interval absent")
        return self._interval.validate()

    def close(self) -> None:
        for name in ("_input","_output"):
            descriptor=getattr(self,name,-1)
            if descriptor>=0:
                os.close(descriptor);setattr(self,name,-1)


def _store_directory(path_value,descriptor:int) -> Path:
    require(type(path_value) is str,"Native private store path absent")
    path=Path(path_value)
    require(path.is_absolute() and not path.is_symlink(),"Native private store path differs")
    held=os.fstat(descriptor); named=path.lstat()
    require(stat.S_ISDIR(held.st_mode) and held.st_uid==os.getuid() and held.st_mode & 0o077==0
            and (held.st_dev,held.st_ino,held.st_mode,held.st_uid)==
                (named.st_dev,named.st_ino,named.st_mode,named.st_uid),
            "Native private store directory original changed")
    return path


def _command_path(command: dict) -> str:
    """Select a closed purpose only inside the held Native parent channel.

    This structural check grants no transport, release or signing authority.
    An old command lacking the mandatory phase never reaches any issuer.
    """
    require(type(command) is dict
            and set(command)=={"schema","request_id","phase","body_base64"}
            and command["schema"]==REQUEST_SCHEMA
            and command["phase"] in ("raw","credential","refresh","hardware_raw","hardware_integrity"),
            "Native worker request layout differs")
    return {"raw":RAW_PATH,"credential":PATH,"refresh":REFRESH_PATH}.get(command["phase"])


def _hardware_result(hardware, phase: str, body: bytes) -> tuple[int, bytes]:
    """Run one hardware phase and map its outcome to the parent's status.

    A Google revocation, decoder or OAuth outage is retryable (503); other
    evidence failures are rejections (400). Any other fault propagates and
    ends the worker, which the parent observes as EOF and refuses.
    """
    try:
        return 200, hardware.handle(phase, body)
    except VerificationUnavailable:
        return 503, b'{"error":"issuer_unavailable"}'
    except AttestationRejected:
        return 400, b'{"error":"hardware evidence rejected"}'


def serve_native_parent(channel:NativeParentChannel, roles:frozenset[int]) -> None:
    oauth=None;encoder=None;raw_encoder=None;hardware=None
    try:
        startup=channel.receive()
        require(set(startup)=={"schema","version","request_id","projection_base64","projection_sha256",
            "encoder_sha256","raw_encoder_sha256","authority_public_key","credential_owner_uid","google_credential_present","store_directory","hardware_projection"}
            and startup["schema"]==STARTUP_SCHEMA and type(startup["version"]) is int and startup["version"]==1
            and type(startup["credential_owner_uid"]) is int and startup["credential_owner_uid"]==0
            and type(startup["google_credential_present"]) is bool,
            "Native worker startup layout differs")
        require_worker_role_originals(roles,startup["google_credential_present"])
        raw_projection=_decode_base64(startup["projection_base64"],"Native policy projection",2*1024*1024)
        pin=_decode_hex32(startup["projection_sha256"],"Native policy projection pin")
        require(hashlib.sha256(raw_projection).digest()==pin,"Native policy projection original changed")
        channel.begin(startup["request_id"],pin);channel.recheck()
        projection=decode_native_policy_projection(raw_projection)
        authority=_decode_hex32(startup["authority_public_key"],"Native app authority public key")
        require(all(policy.authority_public_key==authority for policy in projection.policies),
                "Native protected signer differs from admitted profiles")
        store=_store_directory(startup["store_directory"],17)
        require(bool(projection.google_public_originals)==startup["google_credential_present"],
                "Native Google credential role differs")
        openssl=Path("/usr/bin/openssl")
        if projection.google_public_originals:
            policy,original=projection.google_public_originals[0]
            oauth=GoogleServiceAccountTokenProvider(public_policy_original=original,native_policy=policy,
                credential_fd=13,trusted_time_interval=channel.trusted_time_interval,openssl_path=openssl,credential_owner_uid=0)
        encoder=CanonicalOrdinaryCredentialEncoder(encoder_fd=14,
            encoder_sha256=_decode_hex32(startup["encoder_sha256"],"Native encoder pin"),
            authority_key_fd=12,authority_public_key=authority,credential_owner_uid=0)
        raw_encoder=CanonicalRawAppAdmissionEncoder(encoder_fd=18,
            encoder_sha256=_decode_hex32(startup["raw_encoder_sha256"],"Native raw encoder pin"),
            authority_key_fd=12,authority_public_key=authority,credential_owner_uid=0)
        provider=GovernedOrdinaryEvidenceProvider(policies=projection.policies,trusted_time_interval=channel.trusted_time_interval,
            recheck_native_policy=channel.recheck,openssl_path=openssl,
            play_integrity=GooglePlayIntegrityVerifier(oauth) if oauth is not None else None)
        issuer=DurableOrdinaryCredentialIssuer(path=store/"ordinary-app-attempts.sqlite",provider=provider,
            encoder=encoder,raw_encoder=raw_encoder)
        transport_owner=object()
        refresh=DurableOrdinaryIntegrityRefreshIssuer(issuer)
        service=OrdinaryCredentialService(issuer=issuer,refresh_issuer=refresh,
            authorize_core_call=lambda offered:offered is transport_owner)
        if startup["hardware_projection"] is not None:
            require(startup["google_credential_present"], "hardware Google credential custody absent")
            hardware=NativeHardwareEvidenceVerifier(startup["hardware_projection"],channel,credential_fd=13)
        channel.recheck();channel.send({"kind":"ready","request_id":startup["request_id"],"projection_sha256":pin.hex()})
        while True:
            # An idle worker holds no current financial capability. Only an
            # actual parent request starts the bounded operation deadline.
            channel._deadline=None
            command=channel.receive()
            path=_command_path(command)
            channel.begin(command["request_id"],pin)
            body=_decode_base64(command["body_base64"],"Core original request",MAX_BODY_BYTES)
            channel.recheck();_store_directory(startup["store_directory"],17)
            if path is None:
                require(hardware is not None,"hardware issuer source absent")
                status,result=_hardware_result(hardware,command["phase"],body)
            else:
                status,result=service.handle(method="POST",path=path,
                                             body=body,content_type="application/json",
                                             transport_context=transport_owner)
            channel.recheck();_store_directory(startup["store_directory"],17)
            channel.send({"kind":"result","request_id":command["request_id"],"status":status,
                          "body_base64":base64.b64encode(result).decode("ascii")})
    finally:
        if encoder is not None:encoder.close()
        if raw_encoder is not None:raw_encoder.close()
        if oauth is not None:oauth.close()
        if hardware is not None:hardware.close()


def main() -> int:
    if len(sys.argv)!=1 or sys.platform not in ("linux","darwin") or sys.version_info<(3,10):
        return 78
    channel=None
    try:
        protect_private_process()
        require(sys.flags.isolated and sys.flags.ignore_environment and sys.flags.dont_write_bytecode,
                "private Native worker requires its installed isolated Python launch")
        roles=close_unrelated_worker_descriptors()
        channel=NativeParentChannel(9,10)
        serve_native_parent(channel,roles)
        return 0
    except Exception:
        # No diagnostics containing private credentials, request originals or
        # signer material cross stdout/stderr. Parent receives EOF and refuses.
        return 78
    finally:
        if channel is not None:channel.close()
