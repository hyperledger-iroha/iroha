//! TON recipient address literals (§7.1 step 2).
//!
//! A minted message cannot be voided, so a mistyped recipient loses the value for good. A
//! recipient is either the raw `0:<64 hex>` form or the 48-character user-friendly form:
//! base64 or base64url of `flags ‖ workchain ‖ account id ‖ CRC-16/XMODEM` (36 bytes, the
//! checksum big-endian over the first 34). The user-friendly form is accepted only with a valid
//! checksum, mainnet bounceable (`0x11`) or non-bounceable (`0x51`) flags and workchain 0. The
//! bounce flag does not reach the SCCP payload, which carries the workchain and account id only.

use core::fmt;

/// Length of a user-friendly address in base64 characters.
pub const FRIENDLY_ADDRESS_CHARS: usize = 48;
/// User-friendly flags of a bounceable mainnet address.
pub const FLAGS_BOUNCEABLE: u8 = 0x11;
/// User-friendly flags of a non-bounceable mainnet address.
pub const FLAGS_NON_BOUNCEABLE: u8 = 0x51;
/// User-friendly flag bit of a testnet-only address.
pub const FLAG_TESTNET_ONLY: u8 = 0x80;

/// Why a TON address literal is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TonAddressError {
    /// Neither a raw `<workchain>:<64 hex>` nor a 48-character user-friendly address.
    Malformed,
    /// The user-friendly address fails its CRC-16 checksum.
    Checksum,
    /// The user-friendly address is marked testnet-only.
    TestnetOnly,
    /// The user-friendly flags are neither bounceable nor non-bounceable.
    Flags(u8),
    /// The address is not on the basechain (workchain 0).
    Workchain(i8),
}

impl fmt::Display for TonAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => f.write_str(
                "a TON address is raw `0:<64 hex>` or a 48-character user-friendly address",
            ),
            Self::Checksum => f.write_str(
                "the TON address fails its CRC-16 checksum; check it for a mistyped character",
            ),
            Self::TestnetOnly => f.write_str("the TON address is marked testnet-only"),
            Self::Flags(flags) => write!(f, "the TON address has unknown flags {flags:#04x}"),
            Self::Workchain(workchain) => write!(
                f,
                "the TON address is on workchain {workchain}; SCCP accounts are on workchain 0"
            ),
        }
    }
}

impl std::error::Error for TonAddressError {}

/// Parse a raw or user-friendly workchain-0 TON address into its 32-byte account id.
///
/// # Errors
/// Returns the [`TonAddressError`] of a malformed literal, a failed checksum, testnet-only or
/// unknown flags, or another workchain.
pub fn parse_ton_account(text: &str) -> Result<[u8; 32], TonAddressError> {
    if let Some((workchain, account)) = text.split_once(':') {
        if workchain != "0" {
            return Err(workchain
                .parse::<i8>()
                .ok()
                .filter(|workchain| *workchain != 0)
                .map_or(TonAddressError::Malformed, TonAddressError::Workchain));
        }
        let mut id = [0_u8; 32];
        hex::decode_to_slice(account, &mut id).map_err(|_| TonAddressError::Malformed)?;
        return Ok(id);
    }
    let bytes = decode_friendly(text)?;
    let checksum = u16::from_be_bytes([bytes[34], bytes[35]]);
    if crc16_xmodem(&bytes[..34]) != checksum {
        return Err(TonAddressError::Checksum);
    }
    let flags = bytes[0];
    if flags & FLAG_TESTNET_ONLY != 0 {
        return Err(TonAddressError::TestnetOnly);
    }
    if flags != FLAGS_BOUNCEABLE && flags != FLAGS_NON_BOUNCEABLE {
        return Err(TonAddressError::Flags(flags));
    }
    let workchain = bytes[1].cast_signed();
    if workchain != 0 {
        return Err(TonAddressError::Workchain(workchain));
    }
    let mut id = [0_u8; 32];
    id.copy_from_slice(&bytes[2..34]);
    Ok(id)
}

/// Decode the 36 bytes of a 48-character base64 or base64url address (no padding).
fn decode_friendly(text: &str) -> Result<[u8; 36], TonAddressError> {
    let digits = text.as_bytes();
    if digits.len() != FRIENDLY_ADDRESS_CHARS {
        return Err(TonAddressError::Malformed);
    }
    let mut bytes = [0_u8; 36];
    for (chunk, out) in digits
        .as_chunks::<4>()
        .0
        .iter()
        .zip(bytes.as_chunks_mut::<3>().0)
    {
        let mut word = 0_u32;
        for digit in chunk {
            word = (word << 6) | u32::from(base64_value(*digit)?);
        }
        out.copy_from_slice(&word.to_be_bytes()[1..]);
    }
    Ok(bytes)
}

/// The value of one base64 or base64url digit.
fn base64_value(digit: u8) -> Result<u8, TonAddressError> {
    match digit {
        b'A'..=b'Z' => Ok(digit - b'A'),
        b'a'..=b'z' => Ok(digit - b'a' + 26),
        b'0'..=b'9' => Ok(digit - b'0' + 52),
        b'+' | b'-' => Ok(62),
        b'/' | b'_' => Ok(63),
        _ => Err(TonAddressError::Malformed),
    }
}

/// CRC-16/XMODEM: polynomial `0x1021`, initial value 0, no reflection.
fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0_u16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    // The TON documentation's example address in its raw and user-friendly forms; the
    // checksums were recomputed with Python's `binascii.crc_hqx`.
    const ACCOUNT: &str = "83dfd552e63729b472fcbcc8c45ebcc6691702558b68ec7527e1ba403a0f31a8";
    const BOUNCEABLE: &str = "EQCD39VS5jcptHL8vMjEXrzGaRcCVYto7HUn4bpAOg8xqB2N";
    const NON_BOUNCEABLE: &str = "UQCD39VS5jcptHL8vMjEXrzGaRcCVYto7HUn4bpAOg8xqEBI";

    fn account() -> [u8; 32] {
        hex::decode(ACCOUNT).unwrap().try_into().unwrap()
    }

    #[test]
    fn crc16_xmodem_matches_its_check_value() {
        assert_eq!(crc16_xmodem(b"123456789"), 0x31c3);
    }

    #[test]
    fn raw_and_user_friendly_forms_name_the_same_account() {
        assert_eq!(parse_ton_account(&format!("0:{ACCOUNT}")), Ok(account()));
        assert_eq!(
            parse_ton_account(&format!("0:{}", ACCOUNT.to_ascii_uppercase())),
            Ok(account())
        );
        assert_eq!(parse_ton_account(BOUNCEABLE), Ok(account()));
        assert_eq!(parse_ton_account(NON_BOUNCEABLE), Ok(account()));
        // The standard and URL-safe alphabets decode alike.
        let account = [0xfb, 0xff].repeat(16);
        let url = "UQD7__v_-__7__v_-__7__v_-__7__v_-__7__v_-__7_8u3";
        let standard = "UQD7//v/+//7//v/+//7//v/+//7//v/+//7//v/+//7/8u3";
        assert_eq!(parse_ton_account(url).unwrap().as_slice(), account);
        assert_eq!(parse_ton_account(standard).unwrap().as_slice(), account);
    }

    #[test]
    fn one_flipped_character_fails_the_checksum() {
        // A changed digit alters at most six adjacent bits, a burst CRC-16 always detects.
        for index in 0..FRIENDLY_ADDRESS_CHARS {
            let mut flipped = BOUNCEABLE.as_bytes().to_vec();
            flipped[index] = if flipped[index] == b'A' { b'B' } else { b'A' };
            let flipped = String::from_utf8(flipped).unwrap();
            assert_eq!(
                parse_ton_account(&flipped),
                Err(TonAddressError::Checksum),
                "character {index} was flipped"
            );
        }
        let mut last = BOUNCEABLE.to_owned();
        last.replace_range(47.., "O");
        assert_eq!(parse_ton_account(&last), Err(TonAddressError::Checksum));
    }

    #[test]
    fn testnet_unknown_flags_other_workchains_and_malformed_literals_are_refused() {
        assert_eq!(
            parse_ton_account("kQCD39VS5jcptHL8vMjEXrzGaRcCVYto7HUn4bpAOg8xqKYH"),
            Err(TonAddressError::TestnetOnly)
        );
        assert_eq!(
            parse_ton_account("EgCD39VS5jcptHL8vMjEXrzGaRcCVYto7HUn4bpAOg8xqKnD"),
            Err(TonAddressError::Flags(0x12))
        );
        assert_eq!(
            parse_ton_account("Ef-D39VS5jcptHL8vMjEXrzGaRcCVYto7HUn4bpAOg8xqOLF"),
            Err(TonAddressError::Workchain(-1))
        );
        assert_eq!(
            parse_ton_account(&format!("-1:{ACCOUNT}")),
            Err(TonAddressError::Workchain(-1))
        );
        for malformed in [
            String::new(),
            format!("0:{}", &ACCOUNT[2..]),
            format!("0:{ACCOUNT}00"),
            format!("0:{}zz", &ACCOUNT[2..]),
            format!("x:{ACCOUNT}"),
            format!("+0:{ACCOUNT}"),
            format!("00:{ACCOUNT}"),
            format!("1000:{ACCOUNT}"),
            BOUNCEABLE[1..].to_owned(),
            format!("{BOUNCEABLE}="),
            BOUNCEABLE.replace('Q', "="),
        ] {
            assert_eq!(
                parse_ton_account(&malformed),
                Err(TonAddressError::Malformed),
                "{malformed}"
            );
        }
        assert!(
            TonAddressError::Checksum
                .to_string()
                .contains("mistyped character")
        );
        assert!(TonAddressError::Flags(0x12).to_string().contains("0x12"));
    }
}
