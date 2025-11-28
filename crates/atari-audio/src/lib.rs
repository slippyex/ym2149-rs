//! Atari Audio Library - Rust port
//!
//! Small & accurate ATARI-ST audio emulation
//! Original C++ by Arnaud Carré aka Leonard/Oxygene (@leonard_coder)
//! Rust port for ym2149-rs project

pub mod machine;
pub mod mfp68901;
pub mod ste_dac;
mod tables;
pub mod ym2149;

pub use machine::AtariMachine;
pub use mfp68901::Mfp68901;
pub use ste_dac::SteDac;
pub use ym2149::Ym2149c;
