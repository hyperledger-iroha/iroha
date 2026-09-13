"""Synthetic packet frames only; never capture or measurement qualification."""
import struct

def ip_packet(payload=b'', *, source=30001, destination=50001, udp=False,
              fragment=0, address=b'\x7f\0\0\1', order='<'):
    if udp:
        transport = struct.pack('!HHHH', source, destination, len(payload) + 8, 0) + payload
    else:
        transport = struct.pack('!HHIIBBHHH', source, destination, 0, 0, 0x50, 0x10, 0, 0, 0) + payload
    ip = struct.pack('!BBHHHBBH4s4s', 0x45, 0, 20 + len(transport), 0, fragment,
                     64, 17 if udp else 6, 0, address, address)
    return struct.pack(order + 'I', 2) + ip + transport


def pcap(packets, order='<', nano=False):
    magic = 0xA1B23C4D if nano else 0xA1B2C3D4
    raw = struct.pack(order + 'IHHiIII', magic, 2, 4, 0, 0, 262144, 0)
    for index, row in enumerate(packets):
        # Deliberately nonmonotonic wall clock: ordered markers delimit the window.
        raw += struct.pack(order + 'IIII', 100 - index, 100, len(row), len(row)) + row
    return raw


def stats(count, drop=0):
    return (f'tcpdump: listening on lo0, link-type NULL (BSD loopback), snapshot length 262144 bytes\n'
            f'{count} packets captured\n{count} packets received by filter\n'
            f'{drop} packets dropped by kernel\n').encode()


# The fixture writes only explicit synthetic records; it never opens capture.
from pathlib import Path
import tempfile
import private_settlement_packet_window as packet
import private_settlement_session_control as control

SESSION = {key: '1' * 64 for key in control.IDENTITY_FIELDS}
SESSION.update(version=1, protocol=control.PROTOCOL, campaign_id='fixture')
ATTEMPT = {key: '2' * 64 for key in control.ATTEMPT_FIELDS}
ATTEMPT['session_attempt_index'] = 0
GROUPS = {'version': 1, 'torii_ports': [30001], 'public_p2p_ports': [30002],
          'restricted_p2p_ports': [30003]}


class PacketRecordFixture:
    """Construct the repository-owned synthetic capture record preconditions."""
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.root.chmod(0o700)
        (self.root / 'capture').mkdir(mode=0o700)
        self.records = control.RecordDirectory(self.root)
        self.ready = self.publish('ready.json', {'kind': 'fixture_ready'})
        self.finished = self.publish('finished.json', {'kind': 'fixture_finished'})
        self.ports = self.publish('ports.json', {**SESSION, 'groups': GROUPS})
        self.before = self.publish('before.json', {'kind': 'fixture_listener_before'})
        self.after = self.publish('after.json', {'kind': 'fixture_listener_after'})
        self.baseline = self.publish('baseline.json', {'started_monotonic_ns': 1,
                                                      'finished_monotonic_ns': 2})
        self.resource = {'baseline': self.baseline, 'outcome': {'kind': 'succeeded',
                        'final_started_monotonic_ns': 100, 'final_finished_monotonic_ns': 101}}
        self.arguments = {'records': self.records, 'session': SESSION, 'attempt': ATTEMPT,
                          'ready_marker': self.ready, 'finished_marker': self.finished,
                          'ports_reference': self.ports, 'listener_before': self.before,
                          'listener_after': self.after, 'resource_window': self.resource,
                          'deadline_ns': 200, 'expected_utility': {'path': str(packet.TCPDUMP),
                                                               'sha256': 'a' * 64}}

    def tearDown(self):
        self.records.close()
        self.temporary.cleanup()

    def publish(self, name, value):
        return self.records.publish(name, control.canonical(value))

