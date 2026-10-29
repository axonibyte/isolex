//! Fonts to and from the outside world (SPEC §10, Fonts): a TrueType,
//! OpenType or WOFF font read into an isolex font, and an isolex font
//! written as TrueType or a UFO. Behind the `font-tools` feature, which
//! the command line and the studio's desktop shell are built with, and the
//! WebAssembly modules without.

pub mod export;
pub mod import;
mod outline;
mod sfnt;
mod ufo;
