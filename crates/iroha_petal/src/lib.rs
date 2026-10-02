//! Petal Stream: the Sakura-storm optical transport.
//!
//! A Petal frame is a square image whose data lives in three independent
//! lanes: the light/dark polarity of 256 tiles shaped like the SORA `天`
//! (lane `P`), the katakana glyph drawn in each tile (lane `K`) and the dots on
//! three concentric rings (lane `D`). Every lane is one Reed–Solomon codeword,
//! so any lane that reads cleanly yields fountain-coded payload atoms.
// Image geometry mixes pixel counts, lattice indices and floating-point canvas
// coordinates. Every value is bounded by the image dimensions (a few thousand
// pixels) or by the layout constants, so these numeric conversions are exact in
// practice, and fused multiply-add rewrites would make results depend on the
// hardware. Cast and float-style lints are therefore allowed crate-wide.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::suboptimal_flops
)]
pub mod crc;
pub mod decode;
pub mod fountain;
pub mod geometry;
pub mod glyphs;
pub mod image;
pub mod lanes;
pub mod layout;
pub mod locate;
pub mod png;
pub mod prng;
pub mod qualify;
pub mod render;
pub mod rs;
pub mod session;
pub mod sim;
pub mod stream;
