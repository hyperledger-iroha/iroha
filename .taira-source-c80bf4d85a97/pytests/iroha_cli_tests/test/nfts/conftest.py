import allure  # type: ignore
import pytest


@pytest.fixture(scope="function", autouse=True)
def nft_test_setup():
    allure.dynamic.feature("Nfts")
    allure.dynamic.label("permission", "no_permission_required")
