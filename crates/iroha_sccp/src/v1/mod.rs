//! Network-free SCCP v1 primitives (spec `specs/sccp.md` §3).
//!
//! This module holds the canonical contract-visible encodings that Taira
//! nodes, destination contracts, wallets and tools must agree on byte for
//! byte: profile identities and lanes, the transfer payload codec and amount
//! rules, payload and message hashes, block commitment and history trees,
//! EIP-712 digests, roster digests, signature checks, the node-local bridge
//! key file, TON cell hashing and EVM ABI helpers. Nothing here performs I/O;
//! network access lives in `iroha_sccp_rpc`.

/// Declare a field-less error enum with `Debug`, `Display` and `std::error::Error`.
macro_rules! unit_error {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $message:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $( $(#[$variant_meta])* $variant, )*
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(match self {
                    $( Self::$variant => $message, )*
                })
            }
        }

        impl std::error::Error for $name {}
    };
}

pub mod amount;
pub mod constants;
pub mod eip712;
pub mod evm_abi;
pub mod hashes;
pub mod history;
pub mod key_file;
pub mod merkle;
pub mod network;
pub mod payload;
pub mod proof;
pub mod roster;
pub mod signature;
pub mod ton_cell;

#[cfg(test)]
mod tests {
    unit_error! {
        /// Macro smoke-test error.
        enum Probe {
            /// First.
            First => "first message",
            /// Second.
            Second => "second message",
        }
    }

    #[test]
    fn unit_error_displays_its_messages() {
        assert_eq!(Probe::First.to_string(), "first message");
        assert_eq!(Probe::Second.to_string(), "second message");
        let boxed: Box<dyn std::error::Error> = Box::new(Probe::Second);
        assert_eq!(boxed.to_string(), "second message");
    }
}
