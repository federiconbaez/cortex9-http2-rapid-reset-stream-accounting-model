//! Discrete-time model of HTTP/2 server stream accounting (RFC 9113 §5.1,
//! §5.1.2, §6.4) versus handler work, for studying the Rapid Reset pattern
//! (CVE-2023-44487). The "fixed" scheduler follows golang.org/x/net commit
//! b225e7ca6dde (CVE-2023-39325 in Go's numbering).
//!
//! No sockets, no frames on a wire, no HTTP parsing. Verdicts are computed
//! from counters produced by the state transitions.
pub mod model;
