use std::collections::BTreeSet;

use syn::visit::{self, Visit};
use syn::{Item, ItemUse, UseTree, Visibility};

use crate::model::DynResult;

/// AST-derived source-shape measurements for one Rust compilation unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Metrics {
    pub(super) physical_lines: usize,
    pub(super) item_count: usize,
    pub(super) public_item_count: usize,
    pub(super) import_count: usize,
    pub(super) function_count: usize,
    pub(super) decision_points: usize,
    pub(super) match_arms: usize,
}

impl Metrics {
    pub(super) fn from_source(source: &str) -> DynResult<Self> {
        let file = syn::parse_file(source)
            .map_err(|error| format!("cannot parse maintained Rust source: {error}"))?;
        let mut visitor = ShapeVisitor::default();
        visitor.visit_file(&file);
        Ok(Self {
            physical_lines: source.lines().count(),
            item_count: file.items.len(),
            public_item_count: file
                .items
                .iter()
                .filter(|item| item_is_public(item))
                .count(),
            import_count: file
                .items
                .iter()
                .filter(|item| matches!(item, Item::Use(_)))
                .count(),
            function_count: visitor.function_count,
            decision_points: visitor.decision_points,
            match_arms: visitor.match_arms,
        })
    }
}

/// Extracts direct internal crate-module dependencies from Rust use declarations.
///
/// A `use crate::{PublicType, model::Thing}` declaration carries two different kinds of
/// relationship. `PublicType` is consumed through the crate's public facade, while
/// `model::Thing` names an internal module boundary. The gate records the former as `crate` and
/// the latter as `model`; this avoids mistaking individual types for architectural layers while
/// retaining fail-closed checks on explicit module paths.
pub(super) fn measured_internal_dependencies(source: &str) -> DynResult<BTreeSet<String>> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("cannot parse maintained Rust source: {error}"))?;
    let mut dependencies = BTreeSet::new();
    for item in &file.items {
        let Item::Use(ItemUse { tree, .. }) = item else {
            continue;
        };
        collect_crate_dependencies(tree, &mut dependencies);
    }
    Ok(dependencies)
}

/// Counts direct Rust struct-literal construction of one named type.
///
/// This intentionally operates on the parsed Rust AST rather than textual fragments, so comments,
/// documentation, type declarations, and method calls cannot produce a false structural finding.
pub(super) fn direct_struct_construction_count(source: &str, type_name: &str) -> DynResult<usize> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("cannot parse maintained Rust source: {error}"))?;
    let mut visitor = StructConstructionVisitor {
        type_name,
        count: 0,
    };
    visitor.visit_file(&file);
    Ok(visitor.count)
}

fn item_is_public(item: &Item) -> bool {
    let visibility = match item {
        Item::Const(item) => Some(&item.vis),
        Item::Enum(item) => Some(&item.vis),
        Item::ExternCrate(item) => Some(&item.vis),
        Item::Fn(item) => Some(&item.vis),
        Item::Mod(item) => Some(&item.vis),
        Item::Static(item) => Some(&item.vis),
        Item::Struct(item) => Some(&item.vis),
        Item::Trait(item) => Some(&item.vis),
        Item::TraitAlias(item) => Some(&item.vis),
        Item::Type(item) => Some(&item.vis),
        Item::Union(item) => Some(&item.vis),
        Item::Use(item) => Some(&item.vis),
        _ => None,
    };
    visibility.is_some_and(|visibility| matches!(visibility, Visibility::Public(_)))
}

fn collect_crate_dependencies(tree: &UseTree, dependencies: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) if path.ident == "crate" => {
            collect_crate_dependency_tail(&path.tree, dependencies)
        }
        UseTree::Path(path) => collect_crate_dependencies(&path.tree, dependencies),
        UseTree::Group(group) => {
            for item in &group.items {
                collect_crate_dependencies(item, dependencies);
            }
        }
        UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => {}
    }
}

fn collect_crate_dependency_tail(tree: &UseTree, dependencies: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => {
            dependencies.insert(path.ident.to_string());
        }
        UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => {
            dependencies.insert("crate".to_owned());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_crate_dependency_tail(item, dependencies);
            }
        }
    }
}

#[derive(Default)]
struct ShapeVisitor {
    function_count: usize,
    decision_points: usize,
    match_arms: usize,
}

struct StructConstructionVisitor<'a> {
    type_name: &'a str,
    count: usize,
}

impl<'ast> Visit<'ast> for StructConstructionVisitor<'_> {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if node
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == self.type_name)
        {
            self.count += 1;
        }
        visit::visit_expr_struct(self, node);
    }
}

impl<'ast> Visit<'ast> for ShapeVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.function_count += 1;
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.function_count += 1;
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        self.function_count += 1;
        visit::visit_trait_item_fn(self, node);
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.decision_points += 1;
        visit::visit_expr_if(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.decision_points += 1;
        visit::visit_expr_for_loop(self, node);
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.decision_points += 1;
        visit::visit_expr_loop(self, node);
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.decision_points += 1;
        visit::visit_expr_while(self, node);
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.decision_points += node.arms.len();
        self.match_arms += node.arms.len();
        visit::visit_expr_match(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            self.decision_points += 1;
        }
        visit::visit_expr_binary(self, node);
    }
}
