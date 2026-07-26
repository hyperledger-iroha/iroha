"""
This module contains the Nft class.
"""

from dataclasses import dataclass


@dataclass
class Nft:
    """
    Nft class represents an NFT in the Iroha network.

    :param name: The name of the NFT.
    :type name: str
    :param domain: The domain of the NFT.
    :type domain: str
    :param content: The content of the NFT (key-value object encoded as JSON string).
    :type content: str
    """

    name: str
    domain: str
    content: str

    def __repr__(self):
        return f"{self.name}${self.domain}"

    def get_id(self):
        """
        Get the NFT ID.

        :return: The NFT ID.
        :rtype: str
        """
        return f"{self.name}${self.domain}"
