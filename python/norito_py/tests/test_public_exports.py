"""Public Norito imports must retain their maintained model owner."""

import norito
from norito import FeedbackHintFrame
from norito.streaming import FeedbackHintFrame as StreamingFeedbackHintFrame


def test_feedback_hint_public_import_is_the_streaming_model() -> None:
    assert FeedbackHintFrame is StreamingFeedbackHintFrame
    assert norito.FeedbackHintFrame is StreamingFeedbackHintFrame
    assert "FeedbackHintFrame" in norito.__all__


def test_every_declared_public_export_is_bound() -> None:
    missing = [name for name in norito.__all__ if not hasattr(norito, name)]
    assert not missing, missing
