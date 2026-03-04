use manyhow::{Result, error_message};
use num_bigint::BigInt;
use proc_macro2::TokenStream;
use quote::quote;

pub fn numeric_impl(input: TokenStream) -> Result<TokenStream> {
    let lit = input.to_string();
    parse_literal(&lit)?;
    let lit_str = lit.trim();

    Ok(quote! {
        <::iroha_primitives::numeric::Numeric as ::core::str::FromStr>::from_str(#lit_str)
            .expect("invalid numeric literal")
    })
}

fn parse_literal(input: &str) -> Result<()> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(error_message!("failed to parse numeric: empty literal").into());
    }
    let negative = trimmed.starts_with('-');
    let digits = trimmed.trim_start_matches(['+', '-']);
    let mut scale = 0u32;
    let mut seen_dot = false;
    let mut mantissa_str = String::new();
    for ch in digits.chars() {
        if ch == '.' {
            if seen_dot {
                return Err(error_message!("failed to parse numeric: multiple dots").into());
            }
            seen_dot = true;
            continue;
        }
        if !ch.is_ascii_digit() {
            return Err(error_message!("failed to parse numeric: invalid character `{ch}`").into());
        }
        mantissa_str.push(ch);
        if seen_dot {
            scale = scale
                .checked_add(1)
                .ok_or_else(|| error_message!("failed to parse numeric: scale overflow"))?;
        }
    }
    if scale > 28 {
        return Err(error_message!("failed to parse numeric: scale too large").into());
    }
    let mut mantissa = BigInt::parse_bytes(mantissa_str.as_bytes(), 10)
        .ok_or_else(|| error_message!("failed to parse numeric: mantissa invalid"))?;
    if negative {
        mantissa = -mantissa;
    }
    if mantissa.bits() > 512 {
        return Err(error_message!("failed to parse numeric: mantissa too large").into());
    }
    Ok(())
}
