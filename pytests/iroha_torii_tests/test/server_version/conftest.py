import allure
import pytest
import requests


from ...common.settings import BASE_URL


@pytest.fixture(scope="module")
def GIVEN_get_request_to_server_version_endpoint_is_sent():
    with allure.step("GIVEN GET request to /server_version is sent"):
        return requests.get(f"{BASE_URL}/server_version")


@pytest.fixture(scope="module")
def GIVEN_get_request_with_unexpected_param_to_server_version_enpoint_is_sent():
    with allure.step("GIVEN GET request with unexpected param to /server_version is sent"):
        return requests.get(f"{BASE_URL}/server_version", params={"unexpected": "param"})
