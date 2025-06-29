import allure
import pytest
import json
from pathlib import Path
from jsonschema import validate, ValidationError


@pytest.fixture(scope="function", autouse=True)
def setup_server_version():
    allure.dynamic.label("endpoint", "/server_version")
    allure.dynamic.label("method", "GET")
    allure.dynamic.label("status_code", "200")


@allure.id("1458")
def test_server_version_response_json_format(
    GIVEN_get_request_to_server_version_endpoint_is_sent,
):
    with allure.step("WHEN I send GET request to /server_version"):
        response = GIVEN_get_request_to_server_version_endpoint_is_sent
    with allure.step("THEN the response should be in JSON format"):
        assert response.json(), "Response is not a valid JSON object"


@allure.id("1459")
def test_server_version_response_content_type(
    GIVEN_get_request_to_server_version_endpoint_is_sent,
):
    with allure.step("WHEN I send GET request to /server_version"):
        response = GIVEN_get_request_to_server_version_endpoint_is_sent
    with allure.step("THEN the Content-Type should be application/json"):
        assert (
            response.headers["Content-Type"] == "application/json"
        ), "Content-Type is not application/json"


@allure.id("1460")
def test_server_version_response_json_schema(
    GIVEN_get_request_to_server_version_endpoint_is_sent,
):
    schema_file_path = (
        Path(__file__).parents[2]
        / "common"
        / "schemas"
        / "get_server_version_response.json"
    )
    with open(schema_file_path) as schema_file:
        schema = json.load(schema_file)
    with allure.step("WHEN I send a GET request to /server_version"):
        response = GIVEN_get_request_to_server_version_endpoint_is_sent.json()
    with allure.step("THEN the response JSON should match the expected schema"):
        try:
            validate(instance=response, schema=schema)
        except ValidationError as ve:
            assert False, f"Response JSON does not match the expected schema: {ve}"
