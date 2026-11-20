//! Owned, unresolved syntax facts extracted from one source file.
use crate::parser_for;
use faultline_contract::SymbolKind;
use std::path::Path;
use tree_sitter::{LanguageError, Node};

/// Half-open UTF-8 byte offsets and one-based endpoint lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}

impl From<Node<'_>> for Span {
    fn from(node: Node<'_>) -> Self {
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// Exact source spelling, including quotes for string literals.
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub parent: Option<usize>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub name: Token,
    pub kind: SymbolKind,
    /// Index into the containing scope in `FileFacts::scopes`.
    pub scope: usize,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportName {
    Default,
    Namespace,
    Named(Token),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportBinding {
    pub imported: ImportName,
    pub local: Token,
    pub type_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub source: Token,
    pub bindings: Vec<ImportBinding>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Declaration(Span),
    DefaultExpression(Token),
    Named { local: Token, alias: Option<Token> },
    All { namespace: Option<Token> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    pub kind: ExportKind,
    pub source: Option<Token>,
    pub is_default: bool,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub target: Token,
    pub scope: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub span: Span,
    pub missing: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileFacts {
    pub scopes: Vec<Scope>,
    pub declarations: Vec<Declaration>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub calls: Vec<Call>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}

#[derive(Debug)]
pub enum ExtractError {
    Language(LanguageError),
    ParseCancelled,
}

/// Extracts static syntax without reading files or resolving references.
/// Returns no facts for unsupported extensions.
pub fn extract(path: &Path, source: &str) -> Result<Option<FileFacts>, ExtractError> {
    let Some(mut parser) = parser_for(path).map_err(ExtractError::Language)? else {
        return Ok(None);
    };
    let tree = parser
        .parse(source, None)
        .ok_or(ExtractError::ParseCancelled)?;
    let mut facts = FileFacts::default();
    let mut pending = vec![(tree.root_node(), None)];
    while let Some((node, parent_scope)) = pending.pop() {
        if node.is_error() || node.is_missing() {
            facts.diagnostics.push(SyntaxDiagnostic {
                span: node.into(),
                missing: node.is_missing(),
            });
        }
        let scope = parent_scope.unwrap_or(0);
        if let Some(declaration) = declaration(node, scope, source) {
            facts.declarations.push(declaration);
        }
        match node.kind() {
            "import_statement" => facts.imports.extend(import(node, source)),
            "export_statement" => exports(node, source, &mut facts.exports),
            "call_expression" => {
                if let Some(target) = node.child_by_field_name("function") {
                    facts.calls.push(Call {
                        target: token(target, source),
                        scope,
                    });
                }
            }
            _ => {}
        }

        let child_scope = if parent_scope.is_none() || introduces_scope(node.kind()) {
            let id = facts.scopes.len();
            facts.scopes.push(Scope {
                parent: parent_scope,
                span: node.into(),
            });
            id
        } else {
            scope
        };
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        pending.extend(
            children
                .into_iter()
                .rev()
                .map(|child| (child, Some(child_scope))),
        );
    }
    Ok(Some(facts))
}

fn token(node: Node<'_>, source: &str) -> Token {
    Token {
        text: source[node.byte_range()].to_owned(),
        span: node.into(),
    }
}

fn named_children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    (0..node.named_child_count()).filter_map(move |index| node.named_child(index))
}

fn has_keyword(node: Node<'_>, keyword: &str) -> bool {
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .any(|child| !child.is_named() && child.kind() == keyword)
}

fn introduces_scope(kind: &str) -> bool {
    matches!(
        kind,
        "statement_block"
            | "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "generator_function"
            | "arrow_function"
            | "method_definition"
            | "class_body"
            | "catch_clause"
            | "for_statement"
            | "for_in_statement"
            | "switch_body"
    )
}

fn declaration(node: Node<'_>, scope: usize, source: &str) -> Option<Declaration> {
    let kind = match node.kind() {
        "function_declaration" | "generator_function_declaration" | "function_signature" => {
            SymbolKind::Function
        }
        "class_declaration" | "abstract_class_declaration" => SymbolKind::Class,
        "interface_declaration" => SymbolKind::Interface,
        "type_alias_declaration" => SymbolKind::Type,
        "method_definition" => SymbolKind::Method,
        "variable_declarator" => {
            match node.child_by_field_name("value").map(|value| value.kind()) {
                Some("arrow_function" | "function_expression" | "generator_function") => {
                    SymbolKind::Function
                }
                Some("class") => SymbolKind::Class,
                _ => SymbolKind::Variable,
            }
        }
        _ => return None,
    };
    let name = node.child_by_field_name("name")?;
    if name.is_missing()
        || matches!(
            name.kind(),
            "object_pattern" | "array_pattern" | "computed_property_name"
        )
    {
        return None;
    }
    Some(Declaration {
        name: token(name, source),
        kind,
        scope,
        span: node.into(),
    })
}

fn import(node: Node<'_>, source: &str) -> Option<Import> {
    let module = node
        .child_by_field_name("source")
        .filter(|module| !module.is_missing())?;
    let mut bindings = Vec::new();
    let type_only = has_keyword(node, "type");
    if let Some(clause) = named_children(node).find(|child| child.kind() == "import_clause") {
        for child in named_children(clause) {
            match child.kind() {
                "identifier" => bindings.push(ImportBinding {
                    imported: ImportName::Default,
                    local: token(child, source),
                    type_only,
                }),
                "namespace_import" => {
                    if let Some(local) =
                        named_children(child).find(|name| name.kind() == "identifier")
                    {
                        bindings.push(ImportBinding {
                            imported: ImportName::Namespace,
                            local: token(local, source),
                            type_only,
                        });
                    }
                }
                "named_imports" => {
                    for specifier in
                        named_children(child).filter(|child| child.kind() == "import_specifier")
                    {
                        if let Some(name) = specifier
                            .child_by_field_name("name")
                            .filter(|name| !name.is_missing())
                        {
                            let local = specifier.child_by_field_name("alias").unwrap_or(name);
                            if !local.is_missing() {
                                bindings.push(ImportBinding {
                                    imported: ImportName::Named(token(name, source)),
                                    local: token(local, source),
                                    type_only: type_only || has_keyword(specifier, "type"),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Some(Import {
        source: token(module, source),
        bindings,
        span: node.into(),
    })
}

fn exports(node: Node<'_>, source: &str, output: &mut Vec<Export>) {
    let module = node
        .child_by_field_name("source")
        .map(|module| token(module, source));
    let is_default = has_keyword(node, "default");
    let mut push = |kind| {
        output.push(Export {
            kind,
            source: module.clone(),
            is_default,
            span: node.into(),
        })
    };
    if let Some(value) = node.child_by_field_name("declaration") {
        push(ExportKind::Declaration(value.into()));
    } else if is_default {
        if let Some(value) = node
            .child_by_field_name("value")
            .filter(|value| !value.is_missing())
        {
            push(ExportKind::DefaultExpression(token(value, source)));
        }
    } else if let Some(clause) = named_children(node).find(|child| child.kind() == "export_clause")
    {
        for specifier in named_children(clause).filter(|child| child.kind() == "export_specifier") {
            if let Some(local) = specifier
                .child_by_field_name("name")
                .filter(|name| !name.is_missing())
            {
                let alias = specifier
                    .child_by_field_name("alias")
                    .map(|alias| token(alias, source));
                push(ExportKind::Named {
                    local: token(local, source),
                    alias,
                });
            }
        }
    } else if has_keyword(node, "*") {
        push(ExportKind::All { namespace: None });
    } else if let Some(namespace) =
        named_children(node).find(|child| child.kind() == "namespace_export")
    {
        push(ExportKind::All {
            namespace: named_children(namespace)
                .find(|name| !name.is_extra())
                .map(|name| token(name, source)),
        });
    }
}
