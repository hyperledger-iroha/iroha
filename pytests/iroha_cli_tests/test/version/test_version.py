import allure  # type: ignore

from ...src.iroha_cli import iroha_cli, have


def test_version():
    def condition():
        with allure.step(f"WHEN iroha_cli get version"):
            iroha = iroha_cli.version()
        with allure.step("THEN Iroha should return multiple lines of version info"):
            allure.attach(
                iroha,
                name="version",
                attachment_type=allure.attachment_type.TEXT,
            )
            return iroha.should(have.version())

    iroha_cli.wait_for(condition)
