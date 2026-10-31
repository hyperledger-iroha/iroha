"""Load the maintained controller package for pure local DATA/fake tests."""
from pathlib import Path
import sys

import pytest

TESTS = Path(__file__).resolve().parent
CHECKOUT = TESTS.parents[2]
RUNTIME = CHECKOUT / "scripts/kagemusha_network_acceptance"
sys.path[:0] = [str(RUNTIME), str(TESTS)]
repository = next(parent for parent in (CHECKOUT, *CHECKOUT.parents)
                  if (parent / "Cargo.toml").is_file() and (parent / ".git").exists())


@pytest.fixture(autouse=True, scope="session")
def qualification_output_parent():
    """Create the ignored qualification output parent only when this suite runs.

    A fresh clone need not already contain it. Collecting any other suite under
    pytests/scripts imports this conftest, so creating it at import would write
    outside that suite. No compiler, proof, node or payment is started here.
    """
    (repository / "target/qualification").mkdir(mode=0o700, parents=True, exist_ok=True)
