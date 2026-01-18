import allure  # type: ignore
import pytest


@pytest.fixture(scope="function", autouse=True)
def version_test_setup():
    allure.dynamic.feature("Version")
    allure.dynamic.label("permission", "no_permission_required")
