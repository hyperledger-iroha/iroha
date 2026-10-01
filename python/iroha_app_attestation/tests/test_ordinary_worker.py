"""Actual private-pipe framing, independent of production Native startup."""
import hashlib
import json
import os
import threading
import unittest
from unittest.mock import Mock, patch
from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation import ordinary_worker as worker
from iroha_app_attestation.ordinary_worker import NativeParentChannel


class OrdinaryWorkerChannelTests(unittest.TestCase):
    def setUp(self):
        self.input_read,self.input_write=os.pipe();self.output_read,self.output_write=os.pipe()
        self.channel=NativeParentChannel(self.input_read,self.output_write)
        self.addCleanup(self.channel.close)
        for descriptor in (self.input_read,self.input_write,self.output_read,self.output_write):self.addCleanup(os.close,descriptor)
        self.pin=hashlib.sha256(b'private channel fixture projection').digest()

    def packet(self,value):
        raw=json.dumps(value,separators=(',',':')).encode();os.write(self.input_write,len(raw).to_bytes(4,'little')+raw)

    def reply(self,**changes):
        value={'kind':'current','request_id':'11'*32,'sequence':1,'trusted_time_ms':1000,'projection_sha256':self.pin.hex()}
        value.update(changes);return value

    def test_actual_pipe_recheck_binds_sequence_operation_projection_and_trusted_time(self):
        self.channel.begin('11'*32,self.pin);self.packet(self.reply());self.channel.recheck()
        self.assertEqual(self.channel.trusted_time_ms(),1000)
        width=int.from_bytes(os.read(self.output_read,4),'little');request=json.loads(os.read(self.output_read,width))
        self.assertEqual(request,{'kind':'recheck','request_id':'11'*32,'sequence':1})
        self.packet(self.reply(sequence=2,trusted_time_ms=999))
        with self.assertRaises(AttestationRejected):self.channel.recheck()

    def test_wrong_request_sequence_projection_and_extra_custody_claims_fail(self):
        for change in ({'request_id':'22'*32},{'sequence':2},{'projection_sha256':'33'*32},
                       {'trusted_time_ms':True},{'authority':True}):
            self.channel.begin('11'*32,self.pin);self.packet(self.reply(**change))
            with self.assertRaises(AttestationRejected):self.channel.recheck()

    def test_duplicate_json_and_oversized_private_frame_fail_before_decode(self):
        raw=b'{"kind":"current","kind":"current"}'
        os.write(self.input_write,len(raw).to_bytes(4,'little')+raw)
        with self.assertRaises(AttestationRejected):self.channel.receive()
        os.write(self.input_write,(3*1024*1024+1).to_bytes(4,'little'))
        with self.assertRaises(AttestationRejected):self.channel.receive()


class OrdinaryWorkerProtectionTests(unittest.TestCase):
    def protect(self, results):
        process = Mock();process.prctl.side_effect = results
        with patch.object(worker.sys, 'platform', 'linux'), patch.object(worker.ctypes, 'CDLL', return_value=process):
            worker.protect_private_process()
        return process

    def test_linux_protection_is_set_and_independently_read_back(self):
        process=self.protect([0,0])
        self.assertEqual(process.prctl.call_args_list,
                         [unittest.mock.call(4,0,0,0,0),unittest.mock.call(3,0,0,0,0)])

    def test_failed_protection_or_dumpable_readback_rejects(self):
        for result in ([-1],[0,1],[0,2], [0,-1]):
            with self.subTest(result=result),self.assertRaises(AttestationRejected):self.protect(result)

    def test_failed_protection_precedes_channel_or_private_input_intake(self):
        with patch.object(worker.sys,'platform','linux'),patch.object(worker.sys,'argv',['worker']), \
                patch.object(worker,'protect_private_process',side_effect=AttestationRejected('closed')), \
                patch.object(worker,'NativeParentChannel') as channel:
            self.assertEqual(worker.main(),78)
            channel.assert_not_called()

    def test_darwin_failed_kernel_probe_precedes_private_channel_or_role_intake(self):
        with patch.object(worker.sys,'platform','darwin'),patch.object(worker.sys,'argv',['worker']), \
                patch.object(worker,'protect_darwin_process',side_effect=AttestationRejected('closed')), \
                patch.object(worker,'close_unrelated_worker_descriptors') as roles, \
                patch.object(worker,'NativeParentChannel') as channel:
            self.assertEqual(worker.main(),78)
            roles.assert_not_called()
            channel.assert_not_called()

    def test_unsupported_platform_does_not_select_a_protection_fallback(self):
        with patch.object(worker.sys,'platform','unsupported'),patch.object(worker.sys,'argv',['worker']), \
                patch.object(worker,'protect_private_process') as protection, \
                patch.object(worker,'NativeParentChannel') as channel:
            self.assertEqual(worker.main(),78)
            protection.assert_not_called()
            channel.assert_not_called()
