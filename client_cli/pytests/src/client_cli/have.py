"""
This module contains functions for checking expected results in tests.
"""

import json
import allure

from src.client_cli import client_cli, iroha, match


def expected_in_actual(expected, actual) -> bool:
    allure.attach(
        json.dumps(actual),
        name='actual',
        attachment_type=allure.attachment_type.JSON)
    allure.attach(
        json.dumps(expected),
        name='expected',
        attachment_type=allure.attachment_type.JSON)

    return expected in actual


def domain(expected):
    """
    Check if the expected domain is present in the list of domains.

    :param expected: The expected domain object.
    :return: True if the domain is present, False otherwise.
    """

    def domain_in_domains() -> bool:
        domains = iroha.list_filter(f'{{"Identifiable": {{"Is": "{expected}"}}}}').domains()
        return expected_in_actual(expected, domains)

    return client_cli.wait_for(domain_in_domains)


def account(expected):
    """
    Check if the expected account is present in the list of accounts.

    :param expected: The expected account object.
    :return: True if the account is present, False otherwise.
    """

    def account_in_accounts() -> bool:
        accounts = iroha.list_filter(f'{{"Identifiable": {{"Is": "{expected}"}}}}').accounts()
        return expected_in_actual(expected, accounts)

    return client_cli.wait_for(account_in_accounts)


def asset_definition(expected):
    """
    Check if the expected asset definition is present in the list of asset definitions.

    :param expected: The expected asset definition object.
    :return: True if the asset definition is present, False otherwise.
    """
    expected_domain = expected.split('#')[1]

    def asset_definition_in_asset_definitions() -> bool:
        asset_definitions = iroha.list_filter(f'{{"Identifiable": {{"Is": "{expected_domain}"}}}}').asset_definitions()
        return expected_in_actual(expected, asset_definitions)

    return client_cli.wait_for(asset_definition_in_asset_definitions)


def asset(expected):
    """
    Check if the expected asset is present in the list of assets.

    :param expected: The expected asset object.
    :return: True if the asset is present, False otherwise.
    """

    def asset_in_assets() -> bool:
        assets = iroha.list_filter(f'{{"Identifiable": {{"Is": "{expected}"}}}}').assets()
        return expected_in_actual(expected, assets)

    return client_cli.wait_for(asset_in_assets)


def asset_has_quantity(asset_id, expected_quantity):
    """
    Check if the expected asset quantity is present in the list of assets.

    :param asset_id: The asset ID.
    :param expected_quantity: The expected quantity of the asset.
    :return: True if the asset quantity matches the expected quantity, False otherwise.
    """

    def check_quantity() -> bool:
        assets = iroha.list_filter(f'{{"Identifiable": {{"Is": "{asset_id}"}}}}').assets()
        if asset_id not in assets:
            raise Exception(f"Asset with ID {asset_id} not found.")
        actual_quantity = iroha.get_quantity(asset_id)
        allure.attach(
            json.dumps(actual_quantity),
            name='actual_quantity',
            attachment_type=allure.attachment_type.JSON)
        allure.attach(
            json.dumps(expected_quantity),
            name='expected_quantity',
            attachment_type=allure.attachment_type.JSON)

        return expected_quantity == actual_quantity

    return client_cli.wait_for(check_quantity)


def asset_has_quantity(asset_id, expected_quantity):
    """
    Check if the expected asset quantity is present in the list of assets.

    :param asset_id: The asset ID.
    :param expected_quantity: The expected quantity of the asset.
    :return: True if the asset quantity matches the expected quantity, False otherwise.
    """

    def check_quantity() -> bool:
        assets = iroha.list_filter(f'{{"Identifiable": {{"Is": "{asset_id}"}}}}').assets()
        if asset_id not in assets:
            raise Exception(f"Asset with ID {asset_id} not found.")
        actual_quantity = iroha.get_quantity(asset_id)
        allure.attach(
            json.dumps(actual_quantity),
            name='actual_quantity',
            attachment_type=allure.attachment_type.JSON)
        allure.attach(
            json.dumps(expected_quantity),
            name='expected_quantity',
            attachment_type=allure.attachment_type.JSON)

        return expected_quantity == actual_quantity

    return client_cli.wait_for(check_quantity)


def asset_has_quantity(expected_asset_id, expected_quantity):
    """
    Check if the expected asset quantity is present in the list of assets.

    :param expected_asset_id: The asset ID.
    :param expected_quantity: The expected quantity of the asset.
    :return: True if the asset quantity matches the expected quantity, False otherwise.
    """

    def check_quantity() -> bool:
        assets = iroha.list_filter(f'{{"Identifiable": {{"Is": "{expected_asset_id}"}}}}').assets()
        actual_quantity = None
        for asset in assets:
            if asset == expected_asset_id:
                actual_quantity = assets.get(expected_asset_id, {})["value"]["Quantity"]
                break

        if actual_quantity is None:
            raise Exception(f"Asset with ID {expected_asset_id} not found.")

        allure.attach(
            json.dumps(actual_quantity),
            name='actual_quantity',
            attachment_type=allure.attachment_type.JSON)
        allure.attach(
            json.dumps(expected_quantity),
            name='expected_quantity',
            attachment_type=allure.attachment_type.JSON)

        return expected_quantity == str(actual_quantity)

    return client_cli.wait_for(check_quantity)

def error(expected):
    """
    Check if the expected error is present in the client_cli stderr.

    :param expected: The expected error message.
    :return: True if the error is present, False otherwise.
    """
    return match.client_cli_have_error(
        expected=expected,
        actual=client_cli.stderr)
