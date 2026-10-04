"""Typed Torii HTTP client: collection queries, typed errors and KAGEMUSHA V1 helpers."""

from . import client as _client
from . import collection as _collection
from . import errors as _errors
from . import governance_proposals as _governance_proposals
from . import list_query as _list_query
from . import parliament_api as _parliament_api
from . import private_settlement_client as _private_settlement_client
from . import transaction_submission as _transaction_submission

_MODULES = (
    _client,
    _list_query,
    _collection,
    _errors,
    _governance_proposals,
    _parliament_api,
    _private_settlement_client,
    _transaction_submission,
)

__all__ = list(dict.fromkeys(name for module in _MODULES for name in module.__all__))
for _module in _MODULES:
    for _name in _module.__all__:
        globals()[_name] = getattr(_module, _name)
