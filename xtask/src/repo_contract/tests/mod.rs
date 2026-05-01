use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::plan::workspace_version;
use ffhn_core::TargetDocument;

mod catalog;
mod docs;
mod examples;
mod fuzz;
mod git;
mod markdown;
mod parsing;

use catalog::{assert_public_target_example_contract, render_cli_catalog_section};
use git::{git_tracked_relative_paths, repo_root};
use markdown::{
    assert_registered_document_ids, assert_registered_operation_ids, code_segments,
    extract_cli_operation_ids, extract_document_ids, looks_like_contract_document_id,
    markdown_link_targets, marked_section, production_source_text, repo_file_mentions,
    repo_relative_path, resolve_repo_path, string_literals,
};
