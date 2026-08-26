"""Typed Torii HTTP client with exact first-release Kagemusha and SCCP helpers."""

from . import client as _client
from . import governance_proposals as _governance_proposals
from . import parliament_api as _parliament_api
from . import sccp as _sccp
from . import transaction_submission as _transaction_submission

__all__ = list(
    dict.fromkeys(
        (
            *_client.__all__,
            *_governance_proposals.__all__,
            *_parliament_api.__all__,
            *_sccp.__all__,
            *_transaction_submission.__all__,
        )
    )
)
for _module in (
    _client,
    _governance_proposals,
    _parliament_api,
    _sccp,
    _transaction_submission,
):
    for _name in _module.__all__:
        globals()[_name] = getattr(_module, _name)
