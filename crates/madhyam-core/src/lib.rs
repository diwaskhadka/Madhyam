//! # madhyam_core
//!
//! The platform-independent heart of an IPTV player. It knows how to:
//!
//! * parse **M3U / M3U8** playlists ([`m3u`]),
//! * parse **XMLTV** electronic programme guides, streaming ([`xmltv`]),
//! * talk to **Xtream Codes** panels ([`xtream`]),
//! * keep everything in a fast, searchable, in-memory **catalog** ([`catalog`]).
//!
//! ## Design: "sans-IO"
//!
//! The core never opens a socket. Each platform already has the best HTTP stack
//! for itself (OkHttp on Android, `HttpClient` on Windows, `URLSession` on
//! Apple), with the right proxy, certificate, and power-management behaviour.
//! So the core *describes* requests (URLs) and *consumes* responses (bytes).
//! See [`xtream::XtreamClient`] and the [`fetch::Fetcher`] trait.
//!
//! ## Zero dependencies
//!
//! Only `std` is used. That keeps cross-compiling to Android and Windows
//! trivial and makes every parser readable as learning material.

pub mod catalog;
pub mod error;
pub mod fetch;
pub mod json;
pub mod m3u;
pub mod model;
pub mod textutil;
pub mod time;
pub mod xml;
pub mod xmltv;
pub mod xtream;

pub use catalog::Catalog;
pub use error::{Error, Result};
pub use model::*;
