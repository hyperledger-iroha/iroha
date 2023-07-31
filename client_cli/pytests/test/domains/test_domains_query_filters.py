import json
import allure

from src.client_cli import iroha, client_cli

def test_filter_by_domain(GIVEN_new_one_existing_domain):
    def condition():
        domain_name = GIVEN_new_one_existing_domain.name
        with allure.step(
                f'WHEN client_cli query domains filtered by name "{domain_name}"'):
            domains = iroha.list_filter(f'{{"Identifiable": {{"Is": "{domain_name}"}}}}').domains()
        with allure.step(
                f'THEN Iroha should return only return domains with "{domain_name}" name'):
            allure.attach(
                json.dumps(domains),
                name='domains',
                attachment_type=allure.attachment_type.JSON)
            return domains and all(domain == domain_name for domain in domains)
    client_cli.wait_for(condition)
