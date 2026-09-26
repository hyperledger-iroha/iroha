"""Original V1 journal-lag admission rejects mistyped bounds before native work."""

from dataclasses import replace

import pytest

import scaling_native_facts as native
import scaling_native_load as load
from scaling_native_facts_test import owner, pipeline
from scaling_native_load_test import load_setup
from scaling_readiness_fixture import ready_setup


@pytest.mark.parametrize('bound', (0, 62_500_000))
def test_original_journal_lag_boundary_admits_before_native_work(pipeline, bound):
    selected = replace(pipeline.plan, submission_lag_bound_ns=bound)
    pipeline.plan = selected
    value = owner(pipeline)
    try:
        assert value._phase == 'admitted'
        assert value._plan is not selected
        assert value._plan[7] == bound
        assert pipeline.c.commands.calls == []
        assert list(pipeline.outputs.directory.iterdir()) == []
    finally:
        value.close()


@pytest.mark.parametrize('bound', (None, True, 1.0, -1, 1 << 63, '1'))
def test_original_journal_lag_requires_a_bounded_exact_integer_before_child(
    pipeline, bound,
):
    pipeline.plan = replace(pipeline.plan, submission_lag_bound_ns=bound)
    with pytest.raises(native.NativeFactsError):
        owner(pipeline)
    assert pipeline.c.commands.calls == []
    assert list(pipeline.outputs.directory.iterdir()) == []


@pytest.mark.parametrize('bound', (0, 125_000_000))
def test_original_load_lag_boundary_admits_before_native_work(load_setup, bound):
    selected = replace(load_setup.kwargs['plan'], submission_lag_ns=bound)
    value = load_setup.create(plan=selected)
    assert value._phase == 'admitted'
    assert value._plan_snapshot[7] == bound
    assert load_setup.factory.calls == []


@pytest.mark.parametrize('bound', (None, True, 1.0, -1, 1 << 63, '1'))
def test_original_load_lag_requires_a_bounded_exact_integer_before_child(
    load_setup, bound,
):
    selected = replace(load_setup.kwargs['plan'], submission_lag_ns=bound)
    with pytest.raises(load.NativeLoadError, match='^native_load_failed$'):
        load_setup.create(plan=selected)
    assert load_setup.factory.calls == []
    assert not load_setup.paths.resource_config.exists()
