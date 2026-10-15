//! PCX pictures (to be filled in by the screen reader).
//!
//! Menu backgrounds and full-screen pictures are 256-colour PCX files:
//! a 128-byte header, run-length encoded rows and, at the end, an optional
//! palette. The reader will decode them into an indexed image and the
//! palette it carries, for the menu and end-of-level screens. Like every
//! reader in this crate it will be bounded, never panic and treat its input
//! as untrusted.
