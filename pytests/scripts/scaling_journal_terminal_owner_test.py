"""Terminal journal rows are checked by the current V1 resource replay owner."""

import copy

import pytest

import resource_replay as replay
from resource_replay_test import Fixture


@pytest.mark.parametrize(
    'event,field,replacement,reason',
    (
        ('request_final', 'submission_finished', 1, 'transaction_final_invalid'),
        ('request_final', 'failure', 'rejected', 'transaction_final_invalid'),
        ('collection_finished', 'passed', 1, 'collection_failed'),
        ('collection_finished', 'failure', 'unavailable', 'collection_failed'),
    ),
)
def test_rehashed_terminal_journal_cannot_convert_failure_to_completed_run(
    tmp_path, event, field, replacement, reason,
):
    """Rehashing a failed terminal cannot supply a successful original replay."""
    fixture = Fixture(tmp_path)
    original = fixture.run()
    assert len(original.applied_requests) == len(original.signed_requests) == 1
    terminal = next(row for row in fixture.events if row['event'] == event)
    before = copy.deepcopy(terminal)
    terminal[field] = replacement
    assert type(terminal[field]) is not type(before[field]) or terminal[field] != before[field]
    with pytest.raises(replay.ReplayError, match=reason):
        fixture.run()
