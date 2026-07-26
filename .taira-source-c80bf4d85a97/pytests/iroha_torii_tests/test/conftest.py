try:
    import allure  # type: ignore
except ImportError:  # pragma: no cover - fallback for environments without allure
    import contextlib

    class _AllureStub:
        @staticmethod
        @contextlib.contextmanager
        def step(_name):
            yield

    allure = _AllureStub()
import pytest

from python.iroha_torii_client.mock import ToriiMockServer


@pytest.fixture(scope="session", autouse=True)
def GIVEN_api_up_and_running():
    with allure.step("Given the API is up and running"):
        pass


@pytest.fixture
def torii_mock():
    try:
        server = ToriiMockServer().start()
    except PermissionError:
        pytest.skip("torii mock server cannot bind in this environment")
        return
    server.reset()
    try:
        yield server
    finally:
        server.stop()
