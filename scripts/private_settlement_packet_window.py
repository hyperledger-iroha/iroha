"""Retain actual loopback packets and reduce exact marker-delimited TCP bytes.

Network bytes count each IP packet once, including IP/TCP headers and excluding
the loopback header and this collector's UDP markers. Retransmissions count.
CPU counter brackets are wider than the packet markers; both offsets remain
explicit. Native collection requires permission to open the macOS BPF device.
No snapshot socket counter is a substitute for a complete retained capture.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import select
import signal
import socket
import stat
import struct
import subprocess
import sys
import threading
import time
from typing import Any, Iterable

import private_settlement_capture_split as split
import private_settlement_session_control as control

TCPDUMP = Path('/usr/sbin/tcpdump')
MAX_CAPTURE = 2 * 1024 ** 3
CHUNK_BYTES = 1024 ** 2
MAX_PACKET = 65535 + 4
MARKER_MAGIC = b'BCK26-PACKET-WINDOW\0'


class PacketWindowError(ValueError):
    """No complete packet measurement can be accepted."""


class PacketCaptureIncomplete(PacketWindowError):
    """Retain the actual capture owner even when startup cleanup also fails."""

    def __init__(self, owner):
        self.owner = owner
        super().__init__('packet capture incomplete; actual owner retained')


def require(condition: bool, reason: str) -> None:
    """Reject unsupported, ambiguous or incomplete evidence."""
    if not condition:
        raise PacketWindowError(reason)


def marker_payload(session: dict[str, Any], attempt: dict[str, Any], nonce: bytes,
                   phase: int) -> bytes:
    """Bind one fixed-size marker to an exact session and attempt."""
    control.identity(session)
    control.attempt(attempt)
    require(type(nonce) is bytes and len(nonce) == 32 and type(phase) is int and phase in (0, 1),
            'invalid marker nonce or phase')
    return (MARKER_MAGIC + hashlib.sha256(control.canonical(
        {'session': session, 'attempt': attempt})).digest() + nonce + bytes([phase]))


def capture_filter(groups: dict[str, Any], marker_port: int) -> str:
    """Select only declared TCP listeners and the private marker socket."""
    groups = split.validate_port_manifest(groups)
    ports = sorted({port for row in groups.values() for port in row})
    require(type(marker_port) is int and 0 < marker_port < 65536 and marker_port not in ports,
            'invalid or overlapping marker port')
    return ('ip and src host 127.0.0.1 and dst host 127.0.0.1 and ((tcp and ('
            + ' or '.join('port ' + str(port) for port in ports)
            + ')) or (udp and port ' + str(marker_port) + '))')


class PacketReducer:
    """Incrementally validate an untruncated classic-pcap stream on Darwin lo0.

    The endpoint owner already requires IPv4 loopback. Unsupported link types,
    fragments, foreign packets, duplicate markers and partial tails fail closed.
    At most one bounded packet is buffered, independent of capture duration.
    """

    def __init__(self, groups: dict[str, Any], marker_port: int,
                 begin_payload: bytes, end_payload: bytes):
        self.groups = split.validate_port_manifest(groups)
        capture_filter(groups, marker_port)
        require(type(begin_payload) is bytes and type(end_payload) is bytes
                and begin_payload[:-1] == end_payload[:-1]
                and begin_payload.endswith(b'\0') and end_payload.endswith(b'\1')
                and len(begin_payload) == len(MARKER_MAGIC) + 65,
                'invalid marker pair')
        self.marker_port, self.begin, self.end = marker_port, begin_payload, end_payload
        self.buffer = bytearray()
        self.order: str | None = None
        self.scale = 0
        self.snaplen = 0
        self.phase = 0
        self.total_bytes = 0
        self.packets = 0
        self.tcp_packets = 0
        self.ip_bytes = 0
        self.group_counts = {key: {'packets': 0, 'ip_bytes': 0} for key in self.groups}
        self.markers: list[dict[str, int]] = []
        self.failed = False

    def feed(self, raw: bytes) -> None:
        """Consume exact bytes in bounded chunks; a failure poisons the reducer."""
        require(not self.failed, 'packet reducer is poisoned')
        try:
            require(type(raw) is bytes and 0 < len(raw) <= CHUNK_BYTES, 'invalid capture chunk')
            self.total_bytes += len(raw)
            require(self.total_bytes <= MAX_CAPTURE, 'capture exceeds its registered bound')
            self.buffer.extend(raw)
            if self.order is None:
                if len(self.buffer) < 24:
                    return
                magic = bytes(self.buffer[:4])
                layout = {b'\xd4\xc3\xb2\xa1': ('<', 1000), b'\xa1\xb2\xc3\xd4': ('>', 1000),
                          b'\x4d\x3c\xb2\xa1': ('<', 1), b'\xa1\xb2\x3c\x4d': ('>', 1)}
                require(magic in layout, 'unsupported capture container')
                self.order, self.scale = layout[magic]
                major, minor, zone, sigfigs, self.snaplen, link = struct.unpack(
                    self.order + 'HHiIII', self.buffer[4:24])
                require((major, minor, zone, sigfigs, link) == (2, 4, 0, 0, 0)
                        and MAX_PACKET <= self.snaplen <= 16 * 1024 ** 2,
                        'capture is not full-length Darwin loopback pcap')
                del self.buffer[:24]
            while len(self.buffer) >= 16:
                sec, fraction, length, original = struct.unpack(self.order + 'IIII', self.buffer[:16])
                require(0 < length == original <= MAX_PACKET and length <= self.snaplen
                        and fraction < 10 ** 9 // self.scale, 'invalid or truncated captured packet')
                if len(self.buffer) < 16 + length:
                    break
                packet = bytes(self.buffer[16:16 + length])
                del self.buffer[:16 + length]
                self.packets += 1
                self._packet(packet, sec * 10 ** 9 + fraction * self.scale)
        except BaseException:
            self.failed = True
            raise

    def _packet(self, packet: bytes, timestamp_ns: int) -> None:
        require(len(packet) >= 24 and struct.unpack(self.order + 'I', packet[:4])[0] == 2,
                'packet is not loopback AF_INET')
        ip = packet[4:]
        ihl = (ip[0] & 15) * 4
        length = int.from_bytes(ip[2:4], 'big')
        require(ip[0] >> 4 == 4 and 20 <= ihl <= 60 and len(ip) == length >= ihl
                and int.from_bytes(ip[6:8], 'big') & 0x3FFF == 0
                and ip[12:16] == ip[16:20] == b'\x7f\0\0\1',
                'invalid, fragmented, truncated or foreign IPv4 packet')
        transport = ip[ihl:]
        require(len(transport) >= 4, 'packet lacks transport ports')
        source, destination = struct.unpack('!HH', transport[:4])
        if ip[9] == 17:
            require(len(transport) >= 8 and source == destination == self.marker_port
                    and int.from_bytes(transport[4:6], 'big') == len(transport),
                    'UDP packet is not the exact owned marker socket')
            payload = transport[8:]
            require((self.phase == 0 and payload == self.begin)
                    or (self.phase == 1 and payload == self.end),
                    'unknown, repeated or out-of-order capture marker')
            self.phase += 1
            self.markers.append({'packet_index': self.packets, 'capture_timestamp_ns': timestamp_ns})
            return
        require(ip[9] == 6 and len(transport) >= 20
                and 20 <= (transport[12] >> 4) * 4 <= len(transport), 'invalid TCP segment')
        selected = [key for key, ports in self.groups.items() if source in ports or destination in ports]
        require(bool(selected), 'TCP packet is outside the exact declared endpoint set')
        if self.phase == 1:
            self.tcp_packets += 1
            self.ip_bytes += length
            for key in selected:
                self.group_counts[key]['packets'] += 1
                self.group_counts[key]['ip_bytes'] += length

    def finish(self) -> dict[str, Any]:
        """Return recomputed counts only after exactly two complete markers."""
        require(not self.failed and self.order is not None and not self.buffer and self.phase == 2,
                'capture is incomplete or lacks its unique begin/end markers')
        return {'captured_packets': self.packets, 'captured_bytes': self.total_bytes,
                'window_tcp_packets': self.tcp_packets, 'window_ip_bytes': self.ip_bytes,
                'groups_nonexclusive': self.group_counts, 'markers': self.markers,
                'counting_unit': 'ipv4_packet_bytes_including_ip_tcp_headers',
                'marker_bytes_excluded': True, 'tcp_retransmissions_included': True}


def reduce_chunks(chunks: Iterable[bytes], *, groups: dict[str, Any], marker_port: int,
                  begin_payload: bytes, end_payload: bytes, statistics: bytes) -> dict[str, Any]:
    """Independently replay all retained packets and authenticate final counts."""
    reducer = PacketReducer(groups, marker_port, begin_payload, end_payload)
    for chunk in chunks:
        reducer.feed(chunk)
    result = reducer.finish()
    stats = split.parse_tcpdump_statistics(statistics)
    require(stats['captured_packets'] == result['captured_packets'],
            'native capture statistics differ from retained packet count')
    return {**result, 'statistics': stats}


def _utility() -> dict[str, Any]:
    before = TCPDUMP.stat(follow_symlinks=False)
    require(stat.S_ISREG(before.st_mode) and not TCPDUMP.is_symlink()
            and 0 < before.st_size <= 16 * 1024 ** 2 and before.st_uid == 0
            and stat.S_IMODE(before.st_mode) & 0o022 == 0
            and os.access(TCPDUMP, os.X_OK), 'unsafe capture executable')
    with TCPDUMP.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    fields = ('st_dev', 'st_ino', 'st_size', 'st_mode', 'st_uid', 'st_mtime_ns', 'st_ctime_ns')
    after = TCPDUMP.stat(follow_symlinks=False)
    require(all(getattr(before, key) == getattr(after, key) for key in fields),
            'capture executable changed')
    return {'path': str(TCPDUMP), 'sha256': digest,
            'metadata': {key: getattr(after, key) for key in fields}}


class PacketCaptureOwner:
    """Own exactly one native tcpdump child and its marker-delimited window.

    Only the retained child is interrupted, using tcpdump's documented SIGINT
    termination to flush packets and print statistics. The owner never signals
    a benchmark process. Failure retains an incomplete window without metrics.
    """

    @classmethod
    def begin(cls, *, records: Any, prefix: str, session: dict[str, Any], attempt: dict[str, Any],
              ready_marker: dict[str, Any], ports_reference: dict[str, Any],
              listener_before: dict[str, Any], resource_baseline: dict[str, Any],
              deadline_ns: int) -> PacketCaptureOwner:
        """Establish capture and observe its start marker before releasing work."""
        require(sys.platform == 'darwin', 'native packet collection requires Darwin')
        owner = cls()
        owner.child = owner.thread = owner.socket = None
        owner.closed = False
        owner.start_error = owner.cleanup_error = None
        owner.cleanup_attempted = False
        owner.incomplete_reference = None
        try:
            owner.records, owner.prefix = records, prefix
            owner.session, owner.attempt = control.identity(session), control.attempt(attempt)
            owner.deadline = deadline_ns
            require(type(deadline_ns) is int and time.monotonic_ns() < deadline_ns,
                    'packet window deadline has expired')
            owner.refs = {'ready_marker': ready_marker, 'ports': ports_reference,
                          'listener_before': listener_before, 'resource_baseline': resource_baseline}
            owner.bound = {key: records.read(ref) for key, ref in owner.refs.items()}
            ports = control.decode(owner.bound['ports'])
            require(all(ports.get(key) == value for key, value in owner.session.items()),
                    'packet ports belong to another session')
            split.validate_port_manifest(ports['groups'])
            owner.groups = ports['groups']
            owner.utility = _utility()
            owner.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            owner.socket.bind(('127.0.0.1', 0))
            owner.socket.settimeout(1)
            owner.port = owner.socket.getsockname()[1]
            owner.nonce = os.urandom(32)
            owner.payloads = [marker_payload(owner.session, owner.attempt, owner.nonce, phase)
                              for phase in (0, 1)]
            owner.reducer = PacketReducer(owner.groups, owner.port, *owner.payloads)
            owner.chunks: list[dict[str, Any]] = []
            owner.events: list[dict[str, Any]] = []
            owner.stderr = bytearray()
            owner.failure: BaseException | None = None
            owner.child: subprocess.Popen | None = None
            owner.thread: threading.Thread | None = None
            owner.closed = False
            owner.ended = False
            owner.finished_marker = None
            command = [str(TCPDUMP), '-n', '-U', '-i', 'lo0', '-s', '0', '-w', '-',
                       capture_filter(owner.groups, owner.port)]
            owner.start = records.publish(prefix + '/capture-start.json', control.canonical({
                **owner.session, **owner.attempt, 'command': command, 'utility': owner.utility,
                'references': owner.refs, 'marker_port': owner.port, 'nonce_hex': owner.nonce.hex(),
                'deadline_monotonic_ns': deadline_ns, 'started_monotonic_ns': time.monotonic_ns()}))
            owner.child = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, close_fds=True,
                env={'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LC_ALL': 'C'})
            owner.process = records.publish(prefix + '/capture-process.json', control.canonical({
                'start': owner.start, 'pid': owner.child.pid, 'parent_pid': os.getpid(),
                'spawned_monotonic_ns': time.monotonic_ns()}))
            owner.thread = threading.Thread(target=owner._read, name='aps-native-packets', daemon=True)
            owner.thread.start()
            owner._wait(lambda: owner.reducer.order is not None)
            owner._marker(0)
            return owner
        except BaseException as error:
            owner.start_error = error
            try:
                if hasattr(owner, 'start'):
                    owner.cleanup_attempted = True
                    owner.incomplete_reference = owner.abort('capture_start_failed')
                elif owner.socket is not None:
                    owner.socket.close()
            except BaseException as cleanup_error:
                # Preserve the original child/readers and both failures.
                # The adapter must not silently retry this cleanup operation.
                owner.cleanup_error = cleanup_error
            raise PacketCaptureIncomplete(owner) from error

    def _read(self) -> None:
        pending = bytearray()
        try:
            streams = {self.child.stdout.fileno(): False, self.child.stderr.fileno(): True}
            while streams:
                require(time.monotonic_ns() < self.deadline, 'capture exceeded attempt deadline')
                ready, _, _ = select.select(list(streams), [], [], 0.1)
                for fd in ready:
                    raw = os.read(fd, 65536)
                    if not raw:
                        del streams[fd]
                    elif streams[fd]:
                        require(len(self.stderr) + len(raw) <= 65536, 'capture diagnostics exceed bound')
                        self.stderr.extend(raw)
                    else:
                        pending.extend(raw)
                        self.reducer.feed(raw)
                        if len(pending) >= CHUNK_BYTES:
                            self._chunk(bytes(pending[:CHUNK_BYTES]))
                            del pending[:CHUNK_BYTES]
            if pending:
                self._chunk(bytes(pending))
        except BaseException as error:
            self.failure = error
            # Even a malformed/truncated capture retains the received prefix.
            if pending:
                try:
                    self._chunk(bytes(pending))
                except BaseException:
                    pass

    def _chunk(self, raw: bytes) -> None:
        require(len(self.chunks) < MAX_CAPTURE // CHUNK_BYTES + 1, 'capture chunk inventory exceeds bound')
        self.chunks.append(self.records.publish(
            self.prefix + f'/capture-{len(self.chunks):06d}.pcap-part', raw))

    def _wait(self, predicate: Any) -> None:
        while not predicate():
            require(self.failure is None and self.thread.is_alive()
                    and time.monotonic_ns() < self.deadline, 'native capture ended before its boundary')
            threading.Event().wait(0.005)

    def _marker(self, phase: int) -> None:
        before = time.monotonic_ns()
        require(before < self.deadline, 'marker missed its attempt deadline')
        payload = self.payloads[phase]
        require(self.socket.sendto(payload, ('127.0.0.1', self.port)) == len(payload),
                'marker send was incomplete')
        after = time.monotonic_ns()
        received, source = self.socket.recvfrom(len(payload) + 1)
        require(received == payload and source == ('127.0.0.1', self.port), 'marker echo differs')
        self._wait(lambda: self.reducer.phase == phase + 1)
        self.events.append({'phase': phase, 'send_started_monotonic_ns': before,
                            'send_finished_monotonic_ns': after,
                            'observed_monotonic_ns': time.monotonic_ns()})

    def end(self, finished_marker: dict[str, Any]) -> None:
        """Place the end marker before resource-finalization or endpoint queries."""
        require(not self.closed and not self.ended, 'capture window already ended')
        self.finished_marker = control.reference(finished_marker)
        self._marker(1)
        self.ended = True
        self.records.read(finished_marker)

    def _stop(self) -> dict[str, Any]:
        require(not self.closed, 'capture owner already closed')
        self.closed = True
        code = None
        if self.child is not None:
            if self.child.poll() is None:
                self.child.send_signal(signal.SIGINT)
            try:
                code = self.child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                # Never kill a process to manufacture a completed capture.
                self.failure = PacketWindowError('owned tcpdump did not terminate')
            if self.thread is not None:
                self.thread.join(timeout=10)
                if self.thread.is_alive():
                    self.failure = PacketWindowError('capture drain did not terminate')
            if code is not None:
                self.child.stdout.close()
                self.child.stderr.close()
        self.socket.close()
        return {'exit_code': code, 'pid': self.child.pid if self.child else None,
                'reader_stopped': self.thread is None or not self.thread.is_alive(),
                'finished_monotonic_ns': time.monotonic_ns()}

    def _seal(self, terminal: dict[str, Any], *, reason: str | None,
              listener_after: dict[str, Any] | None = None,
              resource_window: dict[str, Any] | None = None) -> dict[str, Any]:
        if not hasattr(self, 'stderr_ref'):
            self.stderr_ref = self.records.publish(self.prefix + '/capture-stderr.json',
                control.canonical({'raw_hex': self.stderr.hex()}))
        stderr_ref = self.stderr_ref
        common = {'version': 1, 'kind': 'benchmark_packet_window', **self.session, **self.attempt,
                  'start': getattr(self, 'start', None), 'process': getattr(self, 'process', None),
                  'chunks': self.chunks, 'stderr': stderr_ref, 'terminal': terminal,
                  'events': self.events, 'finished_marker': self.finished_marker,
                  'listener_after': listener_after}
        if reason is not None:
            return self.records.publish(self.prefix + '/packet-window.json', control.canonical({
                **common, 'outcome': {'kind': 'incomplete', 'reason': reason}}))
        require(terminal['exit_code'] == 0 and terminal['reader_stopped']
                and self.failure is None and self.ended, 'capture did not close successfully')
        require(_utility() == self.utility, 'native capture executable changed')
        require(all(self.records.read(self.refs[key]) == raw for key, raw in self.bound.items()),
                'capture inputs changed')
        self.records.read(listener_after)
        require(resource_window['baseline'] == self.refs['resource_baseline']
                and resource_window['outcome']['kind'] == 'succeeded', 'resource boundary differs')
        measured = reduce_chunks((self.records.read(ref) for ref in self.chunks), groups=self.groups,
            marker_port=self.port, begin_payload=self.payloads[0], end_payload=self.payloads[1],
            statistics=bytes(self.stderr))
        baseline = control.decode(self.bound['resource_baseline'])
        final = resource_window['outcome']
        require(baseline['finished_monotonic_ns'] <= self.events[0]['send_started_monotonic_ns']
                <= self.events[1]['send_started_monotonic_ns']
                <= final['final_started_monotonic_ns'], 'packet markers exceed resource brackets')
        offsets = {'baseline_to_start_marker_ns': self.events[0]['send_started_monotonic_ns']
                   - baseline['finished_monotonic_ns'],
                   'end_marker_to_final_counter_ns': final['final_started_monotonic_ns']
                   - self.events[1]['send_started_monotonic_ns']}
        return self.records.publish(self.prefix + '/packet-window.json', control.canonical({
            **common, 'outcome': {'kind': 'succeeded', **measured, 'resource_offsets': offsets}}))

    def finish(self, listener_after: dict[str, Any], resource_window: dict[str, Any]) -> dict[str, Any]:
        """Drain, retain and independently reduce the complete native window."""
        terminal = self._stop()
        try:
            return self._seal(terminal, reason=None, listener_after=listener_after,
                              resource_window=resource_window)
        except BaseException:
            return self._seal(terminal, reason='capture_validation_failed', listener_after=listener_after)

    def abort(self, reason: str) -> dict[str, Any]:
        """Preserve incomplete capture evidence; never synthesize end or metrics."""
        require(type(reason) is str and 0 < len(reason) <= 128, 'invalid capture abort reason')
        terminal = self._stop()
        return self._seal(terminal, reason=reason)


def validate_packet_window(binding: dict[str, Any], *, records: Any,
                           session: dict[str, Any], attempt: dict[str, Any],
                           ready_marker: dict[str, Any], finished_marker: dict[str, Any],
                           ports_reference: dict[str, Any], listener_before: dict[str, Any],
                           listener_after: dict[str, Any], resource_window: dict[str, Any],
                           deadline_ns: int, expected_utility: dict[str, Any]) -> dict[str, Any]:
    """Reconstruct packet counts from raw chunks before accepting a measurement.

    The adapter independently validates listener and CPU/RSS records. This
    reducer binds exactly those same references and brackets, plus the actual
    capture process terminal and raw native loss/count statistics.
    """
    raw = records.read(binding)
    window = control.decode(raw)
    expected = {**control.identity(session), **control.attempt(attempt)}
    control.exact(window, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
        'kind', 'start', 'process', 'chunks', 'stderr', 'terminal', 'events',
        'finished_marker', 'listener_after', 'outcome'}, 'packet window')
    require(all(window.get(key) == value for key, value in expected.items())
            and window.get('kind') == 'benchmark_packet_window'
            and window.get('outcome', {}).get('kind') == 'succeeded', 'packet window is not complete')
    prefix = binding['path'].removesuffix('/packet-window.json')
    require(binding['path'] == prefix + '/packet-window.json', 'packet-window locator differs')
    for key, suffix in (('start', 'capture-start.json'), ('process', 'capture-process.json'),
                        ('stderr', 'capture-stderr.json')):
        control.reference(window[key])
        require(window[key]['path'] == prefix + '/' + suffix, 'capture record locator differs')
    start_raw = records.read(window['start'])
    start = control.decode(start_raw)
    control.exact(start, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
        'command', 'utility', 'references', 'marker_port', 'nonce_hex',
        'deadline_monotonic_ns', 'started_monotonic_ns'}, 'packet capture start')
    require(control.canonical(start['utility']) == control.canonical(expected_utility),
            'capture utility differs from its admitted image identity')
    refs = {'ready_marker': ready_marker, 'ports': ports_reference,
            'listener_before': listener_before, 'resource_baseline': resource_window['baseline']}
    require(all(start.get(key) == value for key, value in expected.items())
            and start['references'] == refs and window['finished_marker'] == finished_marker
            and window['listener_after'] == listener_after
            and start['deadline_monotonic_ns'] == deadline_ns,
            'packet window identity, references or deadline differ')
    bound = {key: records.read(ref) for key, ref in refs.items()}
    for ref in (finished_marker, listener_after):
        records.read(ref)
    ports = control.decode(bound['ports'])
    require(all(ports.get(key) == value for key, value in session.items()), 'foreign session ports')
    nonce = bytes.fromhex(start['nonce_hex'])
    begin, end = [marker_payload(session, attempt, nonce, phase) for phase in (0, 1)]
    require(start['command'] == [str(TCPDUMP), '-n', '-U', '-i', 'lo0', '-s', '0', '-w', '-',
            capture_filter(ports['groups'], start['marker_port'])], 'capture command scope differs')
    process = control.decode(records.read(window['process']))
    terminal = window['terminal']
    control.exact(process, {'start', 'pid', 'parent_pid', 'spawned_monotonic_ns'}, 'capture process')
    control.exact(terminal, {'exit_code', 'pid', 'reader_stopped', 'finished_monotonic_ns'},
                  'capture terminal')
    require(process['start'] == window['start'] and type(process['pid']) is int
            and process['pid'] > 1 and type(process['parent_pid']) is int and process['parent_pid'] > 1
            and type(terminal['exit_code']) is int and terminal['exit_code'] == 0
            and terminal['pid'] == process['pid'] and terminal['reader_stopped'] is True,
            'capture process did not terminate and drain')
    require(type(window['chunks']) is list and 1 <= len(window['chunks']) <= MAX_CAPTURE // CHUNK_BYTES + 1,
            'capture chunk inventory is empty or unbounded')
    for index, ref in enumerate(window['chunks']):
        control.reference(ref)
        require(ref['path'] == prefix + f'/capture-{index:06d}.pcap-part'
                and 0 < ref['bytes'] <= CHUNK_BYTES
                and (index == len(window['chunks']) - 1 or ref['bytes'] == CHUNK_BYTES),
                'capture chunk order, size or locator differs')
    stderr = control.decode(records.read(window['stderr']))
    control.exact(stderr, {'raw_hex'}, 'native capture diagnostics')
    statistics = bytes.fromhex(stderr['raw_hex'])
    require(len(statistics) <= 65536, 'capture statistics exceed bound')
    measured = reduce_chunks((records.read(ref) for ref in window['chunks']), groups=ports['groups'],
        marker_port=start['marker_port'], begin_payload=begin, end_payload=end, statistics=statistics)
    require(resource_window['outcome']['kind'] == 'succeeded', 'resource window failed')
    baseline = control.decode(bound['resource_baseline'])
    final = resource_window['outcome']
    events = window['events']
    require(type(events) is list and len(events) == 2, 'marker send observations are incomplete')
    for index, event in enumerate(events):
        control.exact(event, {'phase', 'send_started_monotonic_ns', 'send_finished_monotonic_ns',
                              'observed_monotonic_ns'}, 'marker observation')
        require(all(type(value) is int and value >= 0 for value in event.values())
                and event['phase'] == index and event['send_started_monotonic_ns']
                <= event['send_finished_monotonic_ns'] <= event['observed_monotonic_ns'] <= deadline_ns,
                'marker observation order or deadline differs')
    require(baseline['finished_monotonic_ns'] <= start['started_monotonic_ns']
            <= process['spawned_monotonic_ns'] <= events[0]['send_started_monotonic_ns']
            <= events[0]['observed_monotonic_ns'] <= events[1]['send_started_monotonic_ns']
            <= events[1]['observed_monotonic_ns'] <= final['final_started_monotonic_ns']
            <= terminal['finished_monotonic_ns'], 'resource and capture process order differs')
    offsets = {'baseline_to_start_marker_ns': events[0]['send_started_monotonic_ns']
               - baseline['finished_monotonic_ns'],
               'end_marker_to_final_counter_ns': final['final_started_monotonic_ns']
               - events[1]['send_started_monotonic_ns']}
    result = {'kind': 'succeeded', **measured, 'resource_offsets': offsets}
    require(control.canonical(window['outcome']) == control.canonical(result),
            'claimed packet metrics differ from replay')
    require(records.read(binding) == raw and records.read(window['start']) == start_raw
            and all(records.read(refs[key]) == value for key, value in bound.items()),
            'packet inputs changed during replay')
    return result
