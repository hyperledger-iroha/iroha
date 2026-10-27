"""Load the maintained controller package for pure local DATA/fake tests."""
from pathlib import Path
import sys

TESTS = Path(__file__).resolve().parent
CHECKOUT = TESTS.parents[2]
RUNTIME = CHECKOUT / "scripts/kagemusha_network_acceptance"
sys.path[:0] = [str(RUNTIME), str(TESTS)]
# A fresh clone need not already contain ignored qualification output parents.
# No compiler, proof, node or payment is started by this synthetic suite.
repository = next(parent for parent in (CHECKOUT, *CHECKOUT.parents)
                  if (parent / "Cargo.toml").is_file() and (parent / ".git").exists())
(repository / "target/qualification").mkdir(mode=0o700, parents=True, exist_ok=True)
