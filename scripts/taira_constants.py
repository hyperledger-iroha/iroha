#!/usr/bin/env python3
"""Canonical first-release public Taira identity and validator projection."""

from __future__ import annotations


NETWORK_NAME = "taira"
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
NETWORK_ID = "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
CHAIN_DISCRIMINANT = 369
PEER_COUNT = 4
SLUGS = tuple(f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1))
