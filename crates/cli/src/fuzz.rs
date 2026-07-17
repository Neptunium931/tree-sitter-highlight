use std::{env, path::PathBuf, sync::LazyLock};

use log::info;
use rand::RngExt;
use regex::Regex;

pub mod corpus_test;
pub mod edits;
pub mod random;
pub mod scope_sequence;

use crate::test::{TestEntry, TestExpectation};

pub static LOG_ENABLED: LazyLock<bool> = LazyLock::new(|| env::var("TREE_SITTER_LOG").is_ok());

pub static LOG_GRAPH_ENABLED: LazyLock<bool> =
    LazyLock::new(|| env::var("TREE_SITTER_LOG_GRAPHS").is_ok());

pub static LANGUAGE_FILTER: LazyLock<Option<String>> =
    LazyLock::new(|| env::var("TREE_SITTER_LANGUAGE").ok());

pub static EXAMPLE_INCLUDE: LazyLock<Option<Regex>> =
    LazyLock::new(|| regex_env_var("TREE_SITTER_EXAMPLE_INCLUDE"));

pub static EXAMPLE_EXCLUDE: LazyLock<Option<Regex>> =
    LazyLock::new(|| regex_env_var("TREE_SITTER_EXAMPLE_EXCLUDE"));

pub static START_SEED: LazyLock<usize> = LazyLock::new(new_seed);

pub const DEFAULT_EDIT_COUNT: usize = 3;
pub static EDIT_COUNT: LazyLock<usize> =
    LazyLock::new(|| int_env_var("TREE_SITTER_EDITS").unwrap_or(DEFAULT_EDIT_COUNT));

pub const DEFAULT_ITERATION_COUNT: usize = 10;
pub static ITERATION_COUNT: LazyLock<usize> =
    LazyLock::new(|| int_env_var("TREE_SITTER_ITERATIONS").unwrap_or(DEFAULT_ITERATION_COUNT));

fn int_env_var(name: &'static str) -> Option<usize> {
    env::var(name).ok().and_then(|e| e.parse().ok())
}

fn regex_env_var(name: &'static str) -> Option<Regex> {
    env::var(name).ok().and_then(|e| Regex::new(&e).ok())
}

#[must_use]
pub fn new_seed() -> usize {
    int_env_var("TREE_SITTER_SEED").unwrap_or_else(|| {
        let mut rng = rand::rng();
        let seed = rng.random_range(0..=usize::MAX);
        info!("Seed: {seed}");
        seed
    })
}

pub struct FuzzOptions {
    pub skipped: Option<Vec<String>>,
    pub subdir: Option<PathBuf>,
    pub edits: usize,
    pub iterations: usize,
    pub include: Option<Regex>,
    pub exclude: Option<Regex>,
    pub log_graphs: bool,
    pub log: bool,
}

pub struct FlattenedTest {
    pub name: String,
    pub input: Vec<u8>,
    pub output: String,
    pub languages: Vec<Box<str>>,
    pub expectation: TestExpectation,
    pub has_fields: bool,
    pub template_delimiters: Option<(&'static str, &'static str)>,
}

#[must_use]
pub fn flatten_tests(
    test: TestEntry,
    include: Option<&Regex>,
    exclude: Option<&Regex>,
) -> Vec<FlattenedTest> {
    fn helper(
        test: TestEntry,
        include: Option<&Regex>,
        exclude: Option<&Regex>,
        is_root: bool,
        prefix: &str,
        result: &mut Vec<FlattenedTest>,
    ) {
        match test {
            TestEntry::Example {
                mut name,
                input,
                output,
                has_fields,
                attributes,
                ..
            } => {
                if !prefix.is_empty() {
                    name.insert_str(0, " - ");
                    name.insert_str(0, prefix);
                }

                if let Some(include) = include {
                    if !include.is_match(&name) {
                        return;
                    }
                } else if let Some(exclude) = exclude
                    && exclude.is_match(&name)
                {
                    return;
                }

                result.push(FlattenedTest {
                    name,
                    input,
                    output,
                    has_fields,
                    languages: attributes.languages,
                    expectation: attributes.expectation,
                    template_delimiters: None,
                });
            }
            TestEntry::Group {
                mut name, children, ..
            } => {
                if !is_root && !prefix.is_empty() {
                    name.insert_str(0, " - ");
                    name.insert_str(0, prefix);
                }
                for child in children {
                    helper(child, include, exclude, false, &name, result);
                }
            }
        }
    }
    let mut result = Vec::new();
    helper(test, include, exclude, true, "", &mut result);
    result
}
