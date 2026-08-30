//! Wire-shape constants shared by the data model and the full Vega engine.

/// Exact canonical COSE `Sig_structure` width in the released Figure 9 relation.
pub const VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1: usize = 368;
/// Exact tagged ISO 18013-5 MSO payload width embedded in the `Sig_structure`.
pub const VEGA_MDL_MSO_PAYLOAD_BYTES_V1: usize = 348;
/// Exact tagged `IssuerSignedItemBytes` width for the private birth date.
pub const VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1: usize = 92;
/// Exact randomizer width inside the birth-date signed item.
pub const VEGA_MDL_BIRTH_RANDOM_BYTES_V1: usize = 16;
/// Exact `YYYY-MM-DD` text width parsed by the released relation.
pub const VEGA_MDL_FULL_DATE_TEXT_BYTES_V1: usize = 10;
/// Exact `YYYY-MM-DDTHH:MM:SSZ` text width parsed by the released relation.
pub const VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1: usize = 20;
/// Lowest trusted UTC presentation year admitted by the released relation.
pub const VEGA_MDL_MIN_PRESENTATION_YEAR_V1: u16 = 1_970;
/// Highest presentation year for which a later four-digit `validUntil` exists.
pub const VEGA_MDL_MAX_PRESENTATION_YEAR_V1: u16 = 9_998;
/// Lowest non-degenerate public age threshold admitted by the released relation.
pub const VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1: u8 = 1;
/// Highest achievable public age threshold admitted by the released relation.
pub const VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1: u8 = 150;
