# Torii API Testing Project

This project automates testing of the Torii API exposed by Iroha 3.

## Getting Started

These instructions will get you a copy of the project up and running on your local machine for development and testing purposes.

### Prerequisites

- Python 3.11+
- Poetry for Python package management

### Installation

First, use Poetry to install the dependencies:

```bash
poetry install --no-root
```

This command will create a virtual environment and install all the necessary dependencies.

### Running the Tests

From the repository root, prepare the four real peers with the same binaries used by CI:

```bash
./scripts/test_env.py setup
```

Then, from `pytests/iroha_torii_tests`, run:

```bash
poetry run pytest
```

The suite reads all four generated peers on ports 8080–8083. It checks `/health`,
`/v1/api/version` after genesis, `/status`, and `/v1/schema` using the current
response contracts. Missing peers, invalid responses, and empty collection fail;
there is no mock fallback. These are API smoke checks, not release qualification.
The separate `test/test_smoke_contracts.py` exercises the assertions and bounded
startup wait against controlled HTTP responses without starting a node.

From the repository root, stop the generated network after testing:

```bash
./scripts/test_env.py cleanup
```
