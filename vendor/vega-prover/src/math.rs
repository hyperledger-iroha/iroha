// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Mathematical utility functions for Spartan.
//!
//! This module provides basic mathematical operations needed throughout the codebase,
//! particularly for computing logarithms used in polynomial degree calculations.

/// Trait providing mathematical utility functions.
pub trait Math {
    /// Computes the base-2 logarithm of the value.
    ///
    /// For powers of two, returns the exact logarithm.
    /// For other values, returns the floor of the logarithm.
    ///
    /// # Panics
    /// Panics if the value is zero.
    fn log_2(self) -> usize;
}

impl Math for usize {
    fn log_2(self) -> usize {
        assert_ne!(self, 0);

        if self.is_power_of_two() {
            (1usize.leading_zeros() - self.leading_zeros()) as usize
        } else {
            (0usize.leading_zeros() - self.leading_zeros()) as usize
        }
    }
}
