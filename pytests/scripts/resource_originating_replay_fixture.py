"""Actual raw publisher files in the current fixed replay path layout.

This fixture supplies no completed native authority. Tests use the separately
identified controlled originating-owner protocol from the borrower test suite.
"""
from dataclasses import replace

import resource_experiment as experiment
import resource_evidence_budget as budget
from resource_publisher_fixture import RawPublishedRuns, RUNS


class ScopedPublishedRuns(RawPublishedRuns):
    """Retain independently constructed raw inputs at canonical run paths."""

    def __init__(self, root):
        super().__init__(root)
        for index, (pair, variant) in enumerate(RUNS):
            target = self.root / 'runs' / f'pair-{pair:02}' / variant / 'collector.jsonl'
            target.parent.mkdir(mode=0o700, parents=True)
            self.journal(index).rename(target)
            label = self.budget.runs[index].collector_journal.label
            self.controls = tuple(replace(row, path=str(target.relative_to(self.root)))
                                  if row.label == label else row for row in self.controls)
        # Fixed file custody requires private evidence directories, including
        # intermediate directories created by the raw publisher test fixture.
        for path in self.root.rglob('*'):
            if path.is_dir():
                path.chmod(0o700)

    def scopes(self):
        """Construct current scopes from selected inputs and retained digests."""
        bindings = {row.label: row for row in self.controls}
        return tuple(experiment.RunReplayScope(row.pair_index, row.variant,
            self.captures(index), self.journal(index),
            bindings[self.budget.runs[index].collector_journal.label].sha256,
            row.peers, row.geometry,
            budget.select_run_budget(self.budget, row.pair_index, row.variant))
            for index, row in enumerate(self.runs))
