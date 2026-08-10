#!/usr/bin/env python3
"""Canonical first-release public Taira identity and validator projection."""

from __future__ import annotations


NETWORK_NAME = "taira"
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
CHAIN_DISCRIMINANT = 369
PEER_COUNT = 4
SLUGS = tuple(f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1))
