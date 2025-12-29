import allure
import pytest
import requests


from ...common.settings import BASE_URL


@pytest.fixture(scope="module")
def GIVEN_get_request_to_server_version_endpoint_is_sent():
    with allure.step("GIVEN GET request to /server_version is sent"):
        return requests.get(f"{BASE_URL}/server_version")
