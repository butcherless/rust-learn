//! Lists the open pull requests of a set of GitHub repositories.
//!
//! Hexagonal layout with dependencies pointing inward: `adapter → service → domain`.

pub mod adapter;
pub mod domain;
pub mod service;
