import pytest
import allure

from test import (
    GIVEN_127_lenght_name,
    GIVEN_129_lenght_name,
    GIVEN_new_one_existence_account,
    GIVEN_new_one_existence_domain,
    GIVEN_fake_name,
    GIVEN_key_with_invalid_character_in_key,
    GIVEN_not_existing_name,
    GIVEN_public_key,
    GIVEN_random_character,
    before_each)

@pytest.fixture(scope="function", autouse=True)
def account_test_setup():
    allure.dynamic.feature('Accounts')
    allure.dynamic.label('permission', 'no_permission_required')
