import allure  # type: ignore
import pytest

from ...src.iroha_cli import iroha_cli, have, iroha


@pytest.fixture(scope="function", autouse=True)
def story_account_registers_nfts():
    allure.dynamic.story("Account registers an NFT")


@allure.label("sdk_test_id", "register_nft")
def test_register_nft(GIVEN_fake_nft_name, GIVEN_registered_domain):
    with allure.step(
        f'WHEN iroha_cli registers the NFT "{GIVEN_fake_nft_name}" '
        f'in the "{GIVEN_registered_domain.name}" domain'
    ):
        iroha_cli.register().nft(
            nft=GIVEN_fake_nft_name,
            domain=GIVEN_registered_domain.name,
        )
    with allure.step(f'THEN Iroha should have the NFT "{GIVEN_fake_nft_name}"'):
        iroha.should(have.nft(GIVEN_fake_nft_name + "$" + GIVEN_registered_domain.name))
