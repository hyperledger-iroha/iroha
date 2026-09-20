#!/usr/bin/env python3
"""Exchange one fixed request over the designated inherited release gate socket.

The protected bootstrap owns every process and the actual collector observation.
This helper imports only the standard library, spawns nothing, and receives no
seed, launch input, arbitrary command, evidence path or claimed passing report.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import socket
import stat
import sys
import time


class HandoffError(ValueError):
    """Closed protocol rejection without argument or private data disclosure."""


def _require(value):
    if not value:
        raise HandoffError('fixed_scaling_handoff_failed')


def _canonical(value):
    return (json.dumps(value,sort_keys=True,separators=(',',':'))+'\n').encode('ascii')


def _digest(value):
    _require(type(value) is str and re.fullmatch('[a-f0-9]{64}',value))
    return value


def _identity(fd):
    info=os.fstat(fd)
    _require(stat.S_ISSOCK(info.st_mode))
    return info.st_dev,info.st_ino


def _receive(endpoint, frame_timeout_seconds):
    # The collector may naturally run for its full admitted scope plus cleanup.
    # Do not start a short helper deadline before the first response byte.
    endpoint.settimeout(None)
    first=endpoint.recv(1)
    _require(bool(first))
    end=time.monotonic()+frame_timeout_seconds
    raw=bytearray(first)
    while True:
        remaining=end-time.monotonic()
        _require(remaining>0)
        endpoint.settimeout(remaining)
        chunk=endpoint.recv(4101-len(raw))
        if not chunk: break
        raw.extend(chunk)
        _require(len(raw)<=4100)
        if len(raw)>=4:
            size=int.from_bytes(raw[:4],'big')
            _require(0<size<=4096 and len(raw)<=size+4)
    _require(time.monotonic()<=end and len(raw)>=4
        and 0<int.from_bytes(raw[:4],'big')<=4096
        and len(raw)==int.from_bytes(raw[:4],'big')+4)
    return bytes(raw[4:])


def _decode_response(raw, invocation_sha256, challenge):
    _require(0<len(raw)<=4096 and raw.isascii())
    def pairs(items):
        value={}
        for key,item in items:
            _require(key not in value);value[key]=item
        return value
    try:
        value=json.loads(raw,object_pairs_hook=pairs)
        _require(type(value) is dict and set(value)=={'operation','invocation_sha256',
            'challenge','gate_status','process_returncode','manifest_sha256','report_sha256'})
        _require(_canonical(value)==raw and value['operation']=='fixed-scaling'
            and value['invocation_sha256']==invocation_sha256 and value['challenge']==challenge)
        status=value['gate_status'];code=value['process_returncode']
        _require(type(status) is int and status in (0,2)
            and (code is None or type(code) is int and -255<=code<=255))
        if status==0:
            _require(code==0)
            _digest(value['manifest_sha256']);_digest(value['report_sha256'])
        else:
            _require(value['manifest_sha256'] is None and value['report_sha256'] is None)
        return value
    except (ValueError,TypeError,RecursionError) as error:
        raise HandoffError('fixed_scaling_handoff_failed') from error


def exchange(gate_fd: int, invocation_sha256: str, challenge: str,
             *, frame_timeout_seconds: int=30) -> dict:
    """Consume the one inherited endpoint and return the exact parent response."""
    _require(type(gate_fd) is int and 3<=gate_fd<(1<<20))
    _digest(invocation_sha256);_digest(challenge)
    _require(type(frame_timeout_seconds) is int and 0<frame_timeout_seconds<=300)
    pin=_identity(gate_fd)
    endpoint=socket.socket(fileno=gate_fd)
    try:
        _require(endpoint.family==socket.AF_UNIX and endpoint.type==socket.SOCK_STREAM)
        endpoint.set_inheritable(False)
        endpoint.settimeout(frame_timeout_seconds)
        raw=_canonical({'operation':'fixed-scaling','invocation_sha256':invocation_sha256,
            'challenge':challenge})
        endpoint.sendall(len(raw).to_bytes(4,'big')+raw)
        endpoint.shutdown(socket.SHUT_WR)
        response=_decode_response(_receive(endpoint,frame_timeout_seconds),
            invocation_sha256,challenge)
        _require(_identity(gate_fd)==pin)
        return response
    finally:
        number=endpoint.detach()
        try:
            if _identity(number)==pin: os.close(number)
        except (OSError,HandoffError): pass


class _Parser(argparse.ArgumentParser):
    def error(self, _message):
        self.exit(2,'fixed scaling handoff failed: invalid arguments\n')


class _Once(argparse.Action):
    def __call__(self, parser, namespace, values, option_string=None):
        if getattr(namespace,self.dest,None) is not None: parser.error('duplicate input')
        setattr(namespace,self.dest,values)


def parse_args(argv=None):
    """Accept only an inherited channel and its exact original invocation binding."""
    parser=_Parser(description=__doc__,allow_abbrev=False)
    for name in ('gate-fd','invocation-sha256','challenge'):
        parser.add_argument('--'+name,required=True,action=_Once)
    parser.add_argument('--frame-timeout-seconds',action=_Once)
    args=parser.parse_args(argv)
    try:
        _require(re.fullmatch('[0-9]{1,7}',args.gate_fd))
        args.gate_fd=int(args.gate_fd)
        _require(3<=args.gate_fd<(1<<20))
        _digest(args.invocation_sha256);_digest(args.challenge)
        raw=args.frame_timeout_seconds
        _require(raw is None or re.fullmatch('[0-9]{1,3}',raw))
        args.frame_timeout_seconds=30 if raw is None else int(raw)
        _require(0<args.frame_timeout_seconds<=300)
    except (TypeError,ValueError): parser.error('invalid input')
    return args


def main(argv=None) -> int:
    """Exit with the parent's gate outcome; print no report or authority claim."""
    args=parse_args(argv)
    try:
        return exchange(args.gate_fd,args.invocation_sha256,args.challenge,
            frame_timeout_seconds=args.frame_timeout_seconds)['gate_status']
    except (Exception,KeyboardInterrupt):
        print('fixed scaling handoff failed',file=sys.stderr)
        return 2


if __name__=='__main__':
    raise SystemExit(main())
