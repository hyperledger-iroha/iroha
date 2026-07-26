
#!/usr/bin/env python3
"""Summarise SoraNet relay guard capacity metrics and suggest tuning hints."""

import argparse
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Dict

METRIC_KEYS = {
    'success': 'soranet_handshake_success_total',
    'failure': 'soranet_handshake_failure_total',
    'throttled': 'soranet_handshake_throttled_total',
    'capacity': 'soranet_handshake_capacity_reject_total',
}

PATTERN = re.compile(r'^(?P<name>[a-zA-Z0-9_]+)\{[^}]*mode="(?P<mode>[^"]+)"[^}]*\}\s+(?P<value>[0-9.]+)')

@dataclass
class Metrics:
    success: float = 0.0
    failure: float = 0.0
    throttled: float = 0.0
    capacity: float = 0.0

    @property
    def total(self) -> float:
        return self.success + self.failure

    def summary(self) -> Dict[str, float]:
        return {
            'success': self.success,
            'failure': self.failure,
            'throttled': self.throttled,
            'capacity_reject': self.capacity,
        }


def parse_metrics(content: str, mode: str) -> Metrics:
    values: Dict[str, float] = {key: 0.0 for key in METRIC_KEYS}
    for line in content.splitlines():
        match = PATTERN.match(line.strip())
        if not match:
            continue
        name = match.group('name')
        line_mode = match.group('mode')
        if line_mode != mode:
            continue
        for key, metric_name in METRIC_KEYS.items():
            if name == metric_name:
                values[key] = float(match.group('value'))
                break
    return Metrics(**values)


def recommendation(metrics: Metrics, current_limit: int, current_cooldown: int) -> str:
    hints = []
    if metrics.capacity > 0:
        increment = max(1, int(current_limit * 0.25))
        hints.append(
            f"increase max_circuits_per_client by ~{increment} (current {current_limit})"
        )
    if metrics.throttled > metrics.success * 0.05:
        decrease = int(current_cooldown * 0.2)
        hints.append(
            f"reduce handshake_cooldown_millis by ~{decrease} (current {current_cooldown})"
        )
    if not hints:
        return "current settings look healthy; monitor periodically"
    return '; '.join(hints)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('metrics_file', type=Path, help='Path to text metrics output from the relay')
    parser.add_argument('--mode', default='entry', help='Relay mode to analyse (entry/middle/exit)')
    parser.add_argument('--max-circuits', type=int, default=8, help='Current max_circuits_per_client setting')
    parser.add_argument('--handshake-cooldown', type=int, default=200, help='Current handshake_cooldown_millis setting')

    args = parser.parse_args()
    content = args.metrics_file.read_text(encoding='utf-8')
    metrics = parse_metrics(content, args.mode)

    print(f"Mode: {args.mode}")
    for key, value in metrics.summary().items():
        print(f"  {key:<16} {value}")
    print()
    print('Suggestion: ' + recommendation(metrics, args.max_circuits, args.handshake_cooldown))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
