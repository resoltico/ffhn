use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

#[derive(Clone, Copy, Default)]
pub(super) struct LineCoverage {
    pub(super) executable: bool,
    pub(super) covered: bool,
}

#[derive(Clone)]
pub(super) struct SourceMetadata {
    pub(super) line_count: usize,
    pub(super) requires_line_coverage: bool,
    pub(super) lines: Vec<String>,
}

pub(super) fn accumulate_line_coverage(
    line_coverage: &mut BTreeMap<u64, LineCoverage>,
    segments: &[(u64, u64, u64, bool, bool, bool)],
    metadata: &SourceMetadata,
) {
    let line_count = metadata.line_count;
    let max_line = u64::try_from(line_count).expect("usize line count fits in u64");
    let eof_segment = (max_line + 1, 1, 0, false, false, false);
    for (index, current) in segments.iter().enumerate() {
        let next = segments.get(index + 1).unwrap_or(&eof_segment);
        if !current.4 || current.5 {
            continue;
        }
        if !same_line_segment_is_executable(metadata, *current, *next) {
            continue;
        }

        if current.0 > max_line {
            continue;
        }
        let start_line = current.0;
        let end_line = covered_line_interval_end(*current, *next, max_line);

        for line in start_line..=end_line {
            let coverage = line_coverage.entry(line).or_default();
            coverage.executable = true;
            coverage.covered |= current.2 > 0;
        }
    }
}

fn covered_line_interval_end(
    current: (u64, u64, u64, bool, bool, bool),
    next: (u64, u64, u64, bool, bool, bool),
    max_line: u64,
) -> u64 {
    if current.0 == next.0 {
        return current.0;
    }

    let inclusive_end = if next.1 <= 1 {
        next.0.saturating_sub(1)
    } else {
        next.0
    };
    inclusive_end.min(max_line)
}

fn same_line_segment_is_executable(
    metadata: &SourceMetadata,
    current: (u64, u64, u64, bool, bool, bool),
    next: (u64, u64, u64, bool, bool, bool),
) -> bool {
    if current.0 != next.0 {
        return true;
    }
    if current.1 >= next.1 {
        return false;
    }

    #[cfg(target_pointer_width = "64")]
    {
        let Some(line) = metadata.lines.get(
            usize::try_from(current.0.saturating_sub(1))
                .expect("u64 line index fits in usize on 64-bit targets"),
        ) else {
            return false;
        };
        let start = usize::try_from(current.1.saturating_sub(1))
            .expect("u64 column index fits in usize on 64-bit targets");
        let end = usize::try_from(next.1.saturating_sub(1))
            .expect("u64 column index fits in usize on 64-bit targets");
        let span = &line[start.min(line.len())..end.min(line.len())];
        span.chars().any(is_substantive_rust_span_char)
    }

    #[cfg(not(target_pointer_width = "64"))]
    {
        let Ok(line_index) = usize::try_from(current.0.saturating_sub(1)) else {
            return false;
        };
        let Some(line) = metadata.lines.get(line_index) else {
            return false;
        };
        let Ok(start) = usize::try_from(current.1.saturating_sub(1)) else {
            return false;
        };
        let Ok(end) = usize::try_from(next.1.saturating_sub(1)) else {
            return false;
        };
        let span = &line[start.min(line.len())..end.min(line.len())];
        span.chars().any(is_substantive_rust_span_char)
    }
}

fn is_substantive_rust_span_char(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '"' | '\'')
}

pub(super) fn source_metadata(
    cache: &mut BTreeMap<PathBuf, SourceMetadata>,
    path: &Path,
) -> DynResult<SourceMetadata> {
    if let Some(metadata) = cache.get(path).cloned() {
        return Ok(metadata);
    }

    let source = fs::read_to_string(path)?;
    let lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let metadata = SourceMetadata {
        line_count: lines.len(),
        requires_line_coverage: rust_source_requires_line_coverage(path, &source)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?,
        lines,
    };
    cache.insert(path.to_path_buf(), metadata.clone());
    Ok(metadata)
}

fn rust_source_requires_line_coverage(_path: &Path, source: &str) -> Result<bool, syn::Error> {
    let file = syn::parse_file(source)?;
    Ok(items_require_line_coverage(&file.items))
}

fn items_require_line_coverage(items: &[syn::Item]) -> bool {
    items.iter().any(item_requires_line_coverage)
}

fn item_requires_line_coverage(item: &syn::Item) -> bool {
    match item {
        syn::Item::Fn(_) => true,
        syn::Item::Impl(item) => item.items.iter().any(impl_item_requires_line_coverage),
        syn::Item::Trait(item) => item.items.iter().any(trait_item_requires_line_coverage),
        syn::Item::Mod(item) => item
            .content
            .as_ref()
            .is_some_and(|(_, items)| items_require_line_coverage(items)),
        syn::Item::Verbatim(_) => true,
        _ => false,
    }
}

fn impl_item_requires_line_coverage(item: &syn::ImplItem) -> bool {
    matches!(item, syn::ImplItem::Fn(_) | syn::ImplItem::Verbatim(_))
}

fn trait_item_requires_line_coverage(item: &syn::TraitItem) -> bool {
    match item {
        syn::TraitItem::Fn(item) => item.default.is_some(),
        syn::TraitItem::Verbatim(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_source_requires_line_coverage_distinguishes_contract_bodies_from_barrels() {
        assert!(
            rust_source_requires_line_coverage(Path::new("demo.rs"), "fn run() {}\n")
                .expect("parse function")
        );
        assert!(
            rust_source_requires_line_coverage(
                Path::new("demo.rs"),
                "trait Demo { fn run(&self) {} }\n"
            )
            .expect("parse trait default")
        );
        assert!(
            !rust_source_requires_line_coverage(
                Path::new("demo.rs"),
                "mod app;\npub use app::run;\n"
            )
            .expect("parse barrel")
        );
    }

    #[test]
    fn coverage_shape_helpers_handle_impls_modules_verbatim_and_zero_width_segments() {
        let impl_item =
            syn::parse_str::<syn::Item>("impl Demo { fn run(&self) {} }").expect("parse impl");
        assert!(item_requires_line_coverage(&impl_item));

        let nested_module =
            syn::parse_str::<syn::Item>("mod demo { fn run() {} }").expect("parse module");
        assert!(item_requires_line_coverage(&nested_module));

        assert!(item_requires_line_coverage(&syn::Item::Verbatim(
            Default::default()
        )));
        assert!(impl_item_requires_line_coverage(&syn::ImplItem::Verbatim(
            Default::default(),
        )));
        assert!(!impl_item_requires_line_coverage(
            &syn::parse_str::<syn::ImplItem>("const VALUE: usize = 1;")
                .expect("parse associated constant"),
        ));
        assert!(!impl_item_requires_line_coverage(
            &syn::parse_str::<syn::ImplItem>("type Output = usize;")
                .expect("parse associated type"),
        ));
        assert!(trait_item_requires_line_coverage(
            &syn::TraitItem::Verbatim(Default::default(),)
        ));

        let metadata = SourceMetadata {
            line_count: 1,
            requires_line_coverage: true,
            lines: vec!["    )?;".to_owned()],
        };
        let mut line_coverage = BTreeMap::new();
        accumulate_line_coverage(&mut line_coverage, &[], &metadata);
        accumulate_line_coverage(
            &mut line_coverage,
            &[
                (1, 1, 1, false, true, true),
                (1, 5, 0, false, true, false),
                (1, 7, 0, false, false, false),
                (2, 1, 1, false, true, false),
            ],
            &metadata,
        );
        assert!(line_coverage.is_empty());
        assert_eq!(
            covered_line_interval_end(
                (1, 1, 1, false, true, false),
                (1, 2, 0, false, false, false),
                2
            ),
            1
        );
        assert_eq!(
            covered_line_interval_end(
                (1, 1, 1, false, true, false),
                (2, 3, 0, false, false, false),
                2
            ),
            2
        );

        let metadata = SourceMetadata {
            line_count: 1,
            requires_line_coverage: true,
            lines: vec!["fn tracked() {}".to_owned()],
        };
        let mut out_of_range_coverage = BTreeMap::new();
        accumulate_line_coverage(
            &mut out_of_range_coverage,
            &[
                (2, 1, 1, false, true, false),
                (3, 1, 0, false, false, false),
            ],
            &metadata,
        );
        assert!(out_of_range_coverage.is_empty());

        let zero_line_metadata = SourceMetadata {
            line_count: 0,
            requires_line_coverage: true,
            lines: Vec::new(),
        };
        accumulate_line_coverage(
            &mut out_of_range_coverage,
            &[(1, 1, 1, false, true, false)],
            &zero_line_metadata,
        );
        assert!(out_of_range_coverage.is_empty());

        assert!(!same_line_segment_is_executable(
            &metadata,
            (2, 1, 0, false, true, false),
            (2, 2, 0, false, false, false),
        ));
        assert!(!trait_item_requires_line_coverage(
            &syn::parse_str::<syn::TraitItem>("fn run(&self);").expect("parse trait signature"),
        ));
        assert!(!trait_item_requires_line_coverage(
            &syn::parse_str::<syn::TraitItem>("type Output;").expect("parse trait type"),
        ));
    }

    #[test]
    fn substantive_span_characters_match_rust_tokens_not_punctuation_or_spacing() {
        for character in ['a', 'Z', '7', '_', '"', '\''] {
            assert!(
                is_substantive_rust_span_char(character),
                "expected {character:?} to be substantive"
            );
        }
        for character in [' ', '\t', '(', ')', '?', ';'] {
            assert!(
                !is_substantive_rust_span_char(character),
                "expected {character:?} to be non-substantive"
            );
        }

        let metadata = SourceMetadata {
            line_count: 1,
            requires_line_coverage: true,
            lines: vec!["fn tracked() {}".to_owned()],
        };
        assert!(same_line_segment_is_executable(
            &metadata,
            (1, 1, 0, false, true, false),
            (1, 3, 0, false, false, false),
        ));
        assert!(!same_line_segment_is_executable(
            &metadata,
            (1, 1, 0, false, true, false),
            (1, 1, 0, false, false, false),
        ));
        assert!(!same_line_segment_is_executable(
            &metadata,
            (1, 3, 0, false, true, false),
            (1, 1, 0, false, false, false),
        ));
    }

    #[test]
    fn final_source_line_is_accumulated_and_covered_at_the_exact_boundary() {
        let metadata = SourceMetadata {
            line_count: 2,
            requires_line_coverage: true,
            lines: vec!["fn first() {}".to_owned(), "fn final_line() {}".to_owned()],
        };
        let mut coverage = BTreeMap::new();
        accumulate_line_coverage(&mut coverage, &[(2, 1, 1, false, true, false)], &metadata);
        assert_eq!(coverage.len(), 1);
        let final_line = coverage.get(&2).expect("final source line");
        assert!(final_line.executable);
        assert!(final_line.covered);
    }
}
