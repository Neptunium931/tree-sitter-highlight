#![cfg_attr(not(any(test, doctest)), doc = include_str!("../README.md"))]

pub mod highlight;
pub mod input;
pub mod logger;
pub mod query_testing;
pub mod tags;
pub mod test;
pub mod test_highlight;
pub mod test_tags;
pub mod util;
pub mod version;
pub mod wasm;

#[cfg(test)]
mod tests;

#[cfg(doctest)]
mod tests;
