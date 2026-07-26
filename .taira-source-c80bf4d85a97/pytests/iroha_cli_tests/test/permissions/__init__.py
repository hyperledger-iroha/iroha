import allure  # type: ignore
import pytest


@pytest.fixture(scope="function", autouse=True)
def domain_test_setup():
    allure.dynamic.feature("Permissions")
