//! isolex studio's core: what the studio does, without a window.
//!
//! The desktop shell (`studio/`) is a thin layer over this crate, which
//! holds everything that can be tested without a webview: the library
//! being edited, its edits and one undo history, saving, compiling in
//! the background and watching the folder (#61–#65). It depends on the
//! compiler, serde (with serde_json) and miniz_oxide only, so it builds and is tested wherever the
//! compiler is, the FreeBSD jail included.
//!
//! The interface it serves is specified in `docs/studio-interface.md`.

use serde::Serialize;

pub mod compile;
pub mod edit;
pub mod export;
pub mod lexicon;
pub mod library;
pub mod outline;
pub mod refactor;
pub mod save;
pub mod session;
pub mod tree;
pub mod watch;

pub use library::{Change, EditError, Library, Patch, Transaction};
pub use watch::{DiskChange, DiskOutcome, PollingWatcher, Watcher};

/// What the studio and the compiler it is built with say they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct About {
    pub studio: &'static str,
    pub isolex: &'static str,
}

/// This build's versions, for the shell's title and about box.
pub fn about() -> About {
    About {
        studio: env!("CARGO_PKG_VERSION"),
        isolex: isolex::VERSION,
    }
}
