"""Anonymous bounded seed transport owned by one fixed generation attempt.

The only payload is 64 lowercase hexadecimal bytes and ready EOF. No path,
argument, environment entry or public receipt contains the development seed.
The caller retains this owner through command failure and original child reap.
"""
import fcntl
import os
import re
import stat


class SeedPipeError(ValueError):
    """Closed transport failure without descriptor details or secret bytes."""


def _require(condition):
    if not condition:
        raise SeedPipeError('fixed_scaling_seed_pipe_failed')


def _identity(info):
    # Pipe size changes as the child consumes bytes (including on Darwin).
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid, info.st_nlink)


def _pin(fd):
    return (_identity(os.fstat(fd)), fcntl.fcntl(fd, fcntl.F_GETFL),
            fcntl.fcntl(fd, fcntl.F_GETFD))


def _writer_pin(fd):
    # Darwin adds a kernel write-history flag after the first pipe write.
    # Keep its descriptor identity, access, blocking mode and inheritance exact.
    info, flags, descriptor_flags = _pin(fd)
    return info, flags & (os.O_ACCMODE | os.O_NONBLOCK), descriptor_flags


class DevelopmentSeedPipe:
    """Original anonymous read end; never reopen, refresh, or retain seed text."""
    __slots__ = ('_reader', '_pin', '_closed')

    def __init__(self, seed: str):
        _require(not hasattr(self, '_reader') and type(seed) is str
                 and re.fullmatch('[a-f0-9]{64}', seed)
                 and seed != '0' * 64)
        self._reader, self._pin, self._closed = None, None, False
        writer, writer_pin = None, None
        try:
            reader, writer = os.pipe()
            self._reader = reader
            self._pin, writer_pin = _pin(reader), _writer_pin(writer)
            _require(3 <= reader <= 65535 and writer > 2
                     and not os.get_inheritable(reader) and not os.get_inheritable(writer))
            os.set_blocking(reader, False)
            self._pin = _pin(reader)
            os.set_blocking(writer, False)
            writer_pin = _writer_pin(writer)
            self.validate()
            _require(os.write(writer, seed.encode('ascii')) == 64)
            _require(_writer_pin(writer) == writer_pin)
            os.close(writer)
            writer, writer_pin = None, None
            self.validate()
        except BaseException as error:
            self.close()
            if isinstance(error, (KeyboardInterrupt, SystemExit, GeneratorExit)):
                raise
            raise SeedPipeError('fixed_scaling_seed_pipe_failed') from None
        finally:
            if writer is not None and writer_pin is not None:
                try:
                    if _writer_pin(writer) == writer_pin:
                        os.close(writer)
                except OSError:
                    pass

    def validate(self):
        """Validate only the original handle; consumption does not refresh its pin."""
        try:
            _require(not self._closed and self._reader is not None and self._pin is not None)
            info = os.fstat(self._reader)
            flags = fcntl.fcntl(self._reader, fcntl.F_GETFL)
            _require(stat.S_ISFIFO(info.st_mode) and info.st_uid == os.geteuid()
                     and 0 <= info.st_size <= 64
                     and flags & os.O_ACCMODE == os.O_RDONLY
                     and flags & os.O_NONBLOCK and _pin(self._reader) == self._pin)
        except (OSError, ValueError, TypeError):
            raise SeedPipeError('fixed_scaling_seed_pipe_failed') from None

    @property
    def fd(self) -> int:
        """The original read descriptor, admitted for one fixed command handoff."""
        self.validate()
        return self._reader

    def close(self):
        """Release this slot once; a reused or changed slot belongs to its other owner."""
        if self._closed:
            return
        self._closed = True
        if self._reader is not None and self._pin is not None:
            try:
                if _pin(self._reader) == self._pin:
                    os.close(self._reader)
            except OSError:
                pass
