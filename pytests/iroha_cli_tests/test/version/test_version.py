import allure  # type: ignore

from ...src.iroha_cli import iroha_cli, have


def test_version():
    with allure.step("WHEN iroha_cli get version"):
        iroha = iroha_cli.version()

    with allure.step("THEN Iroha should return multiple lines of version info"):
        allure.attach(
            iroha.stdout,
            name="version_stdout",
            attachment_type=allure.attachment_type.TEXT,
        )
        iroha.should(have.lines_count(4))
        iroha.should(have.line_starts_with("Client git SHA: ", 0))
        iroha.should(have.line_starts_with("Client version: 2.0.0", 1))
        iroha.should(have.line_starts_with("Server git SHA: ", 2))
        iroha.should(have.line_starts_with("Server version: 2.0.0", 3))
