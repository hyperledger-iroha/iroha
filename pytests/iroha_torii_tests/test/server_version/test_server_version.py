import allure
import pytest
import re


@pytest.fixture(scope="function", autouse=True)
def setup_server_version():
    allure.dynamic.label("endpoint", "/server_version")
    allure.dynamic.label("method", "GET")
    allure.dynamic.label("status_code", "200")


@allure.id("1458")
def test_server_version_response_plain_text_format(GIVEN_get_request_to_server_version_endpoint_is_sent):
    with allure.step("WHEN I send GET request to /server_version"):
        response = GIVEN_get_request_to_server_version_endpoint_is_sent
    with allure.step("THEN the response should be in plain text format"):
        assert response.text, "Response is not a valid plain text"

@allure.id("1459")
def test_server_version_response_content_type(GIVEN_get_request_to_server_version_endpoint_is_sent):
    with allure.step("WHEN I send GET request to /server_version"):
        response = GIVEN_get_request_to_server_version_endpoint_is_sent
    with allure.step("THEN the Content-Type should be text/plain"):
        assert (
            response.headers["Content-Type"] == "text/plain; charset=utf-8"
        ), "Content-Type is not text/plain; charset=utf-8"
