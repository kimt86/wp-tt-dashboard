//! Library surface of the extractor, so integration tests can exercise the
//! parse/upsert/run-log paths without the binary.

pub mod baseline;
pub mod db;
pub mod kpis;
pub mod params;
pub mod runner;
pub mod shift;
pub mod transform;
pub mod vessel;
