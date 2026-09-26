# Multilane script-suite collection boundary

The current checkout was exercised on `optimizations` with the repository's
script requirements installed under the ignored `target/py312-venv` directory.
The system `python3` is 3.9 and cannot collect scripts that use
`dataclass(slots=True)`. Python 3.12 plus the pinned requirements reaches
collection, but direct `pytest pytests/scripts` still exits before tests run:
`sumeragi_v2_release_scaling_preflight_test.py` imports its isolated collector,
which requires actual `-I -B -S` flags. Re-running collection under those
flags exposes other ordinary-suite import errors and a required isolated
preflight context. Neither attempt is a passing script suite.

One independent first-release interface drift is directly visible:
`scripts/nexus/validate_multilane_scaling_evidence.py` imports the retired
`RunReplayInput` from `resource_experiment.py`, while the canonical resource
owner exposes `RunReplayScope` only. The validator also constructs
`ResourceExperiment(...)` directly, whose constructor now rejects independent
borrowers; the original completed experiment must create that borrower. An
alias for `RunReplayInput` would conceal the owner mismatch and violate the
single-V1 release contract. The validator, its caller and tests need one
original completed-experiment authority before scaling evidence can qualify.

These are release-tooling blockers, separate from the still-required five
paired measured runs and the full candidate-specific validation matrix.
