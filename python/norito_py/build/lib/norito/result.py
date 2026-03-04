# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Result helpers mirroring Rust's `Result<T, E>`."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Generic, TypeVar

T = TypeVar("T")
E = TypeVar("E")


@dataclass
class Ok(Generic[T]):
    """Wrapper representing a successful `Result` value."""

    value: T


@dataclass
class Err(Generic[E]):
    """Wrapper representing an error `Result` value."""

    error: E


__all__ = ["Ok", "Err"]
