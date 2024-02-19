"""
This module provides a Config class to manage Iroha network configuration.
"""

import tomlkit
import glob
import json
import os
import random
from urllib.parse import urlparse


class Config:
    """
    Configuration class to handle Iroha network configuration. The class provides methods for loading
    the configuration from a file, accessing the configuration values, and randomising Torii URL
    to access different peers.

    :param port_min: The minimum port number for the TORII_API_URL.
    :type port_min: int
    :param port_max: The maximum port number for the TORII_API_URL.
    :type port_max: int
    """

    def __init__(self, port_min, port_max):
        self._config = None
        self.file = None
        self.port_min = port_min
        self.port_max = port_max
        self._envs = dict()

    def load(self, path_config_client_cli):
        """
        Load the configuration from the given config file.

        :param path_config_client_cli: The path to the configuration file.
        :type path_config_client_cli: str
        :raises IOError: If the file does not exist.
        """
        if not os.path.exists(path_config_client_cli):
            raise IOError(f"No config file found at {path_config_client_cli}")

        if not os.path.isfile(path_config_client_cli):
            raise IOError(f"The path is not a file: {path_config_client_cli}")

        with open(path_config_client_cli, "r", encoding="utf-8") as config_file:
            self._config = tomlkit.load(config_file)
        self.file = path_config_client_cli

    def generate_by_peers(self, peers_configs_dir):
        """
        Generate configuration files for each port in the range from port_min to port_max.
        """
        if self._config is None:
            raise ValueError(
                "No configuration loaded. Use load() method to load the configuration."
            )

        if self.port_min >= self.port_max:
            raise ValueError("port_min must be less than port_max.")

        os.makedirs(peers_configs_dir, exist_ok=True)

        for port in range(self.port_min, self.port_max + 1):
            config_copy = self._config.copy()
            config_copy["TORII_API_URL"] = f"http://localhost:{port}"
            file_name = f"config_to_peer_{port}.json"
            file_path = os.path.join(peers_configs_dir, file_name)
            with open(file_path, "w", encoding="utf-8") as config_file:
                json.dump(config_copy, config_file, indent=4)

    def select_random_peer_config(self):
        """
        Select and load a random configuration file generated by the generate_by_peers method.
        This updates the current configuration to the one chosen.

        :return: None
        """
        peers_configs = glob.glob("path/to/peers/configs/*.json")
        if not peers_configs:
            raise ValueError(
                "Peer configuration files not found. First generate them using generate_by_peers."
            )

        chosen_config_file = random.choice(peers_configs)

        self.load(chosen_config_file)

    def randomise_torii_url(self):
        """
        Update Torii URL.
        Note that in order for update to take effect,
        `self.env` should be used when executing the client cli.

        :return: None
        """
        parsed_url = urlparse(self._config["torii_url"])
        random_port = random.randint(self.port_min, self.port_max)
        self._envs["TORII_URL"] = parsed_url._replace(
            netloc=f"{parsed_url.hostname}:{random_port}"
        ).geturl()

    @property
    def torii_url(self):
        """
        Get the Torii URL set in ENV vars.

        :return: Torii URL
        :rtype: str
        """
        return self._envs["TORII_URL"]

    @property
    def env(self):
        """
        Get the environment variables set to execute the client cli with.

        :return: Dictionary with env vars (mixed with existing OS vars)
        :rtype: dict
        """
        # self.select_random_peer_config()
        # return self._config['TORII_API_URL'
        return {**os.environ, **self._envs}

    @property
    def account_id(self):
        """
        Get the ACCOUNT_ID configuration value.

        :return: The ACCOUNT_ID.
        :rtype: str
        """
        return self._config["account"]["id"]

    @property
    def account_name(self):
        """
        Get the account name from the ACCOUNT_ID configuration value.

        :return: The account name.
        :rtype: str
        """
        return self.account_id.split("@")[0]

    @property
    def account_domain(self):
        """
        Get the account domain from the ACCOUNT_ID configuration value.

        :return: The account domain.
        :rtype: str
        """
        return self.account_id.split("@")[1]

    @property
    def public_key(self):
        """
        Get the PUBLIC_KEY configuration value.

        :return: The public key.
        :rtype: str
        """
        return self._config["account"]["public_key"].split("ed0120")[1]
