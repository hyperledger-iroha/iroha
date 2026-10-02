"""Bounded interval data from the actual retained Native parent clock.

This type checks numbers only. Constructing or decoding it grants no clock,
root, release, elapsed reference, signer or monetary authority.
"""
from dataclasses import dataclass
from .attestation import require

@dataclass(frozen=True)
class NativeTimeInterval:
    lower_at_ms: int
    upper_at_ms: int

    def validate(self):
        require(type(self.lower_at_ms) is int and type(self.upper_at_ms) is int
                and 0 < self.lower_at_ms <= self.upper_at_ms < (1 << 64),
                "Native clock interval malformed")
        return self

    def endpoints(self):
        self.validate()
        return (self.lower_at_ms, self.upper_at_ms)

    def require_window(self, issued_at_ms: int, expires_at_ms: int):
        self.validate()
        require(type(issued_at_ms) is int and type(expires_at_ms) is int
                and 0 < issued_at_ms <= self.lower_at_ms <= self.upper_at_ms < expires_at_ms < (1 << 64),
                "Native interval is future or expired")

    def check_both(self, check):
        require(callable(check), "interval check absent")
        return tuple(check(now) for now in self.endpoints())
