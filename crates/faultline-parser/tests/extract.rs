use faultline_contract::SymbolKind;
use faultline_parser::{ExportKind, FileFacts, ImportName, Token, extract};
use std::path::Path;

fn parse(path: &str, source: &str) -> FileFacts {
    extract(Path::new(path), source).unwrap().unwrap()
}

fn assert_token(source: &str, token: &Token) {
    assert_eq!(
        token.text,
        source[token.span.start_byte..token.span.end_byte]
    );
    for (byte, line) in [
        (token.span.start_byte, token.span.start_line),
        (token.span.end_byte, token.span.end_line),
    ] {
        assert_eq!(
            line,
            1 + source.as_bytes()[..byte]
                .iter()
                .filter(|&&b| b == b'\n')
                .count()
        );
    }
}

#[test]
fn extracts_aliases_type_imports_and_export_forms() {
    let source = concat!(
        "import Main, { type Item as Local, run as go } from './dep';\n",
        "import type { Shape } from './types';\n",
        "import * as lib from './lib';\n",
        "import './effects';\n",
        "export default function launch() { go(); }\n",
        "export class Service {}\n",
        "export { go as execute };\n",
        "export { v as value } from './mod';\n",
        "export * from './all';\n",
        "export * as methods from './methods';\n",
    );
    let facts = parse("main.ts", source);
    assert!(facts.diagnostics.is_empty());
    assert_eq!(facts.imports.len(), 4);
    assert_eq!(facts.imports[0].source.text, "'./dep'");
    let bindings = &facts.imports[0].bindings;
    assert_eq!(bindings[0].imported, ImportName::Default);
    assert!(matches!(&bindings[1].imported, ImportName::Named(name) if name.text == "Item"));
    assert_eq!(bindings[1].local.text, "Local");
    assert!(bindings[1].type_only);
    assert!(!bindings[2].type_only);
    assert!(facts.imports[1].bindings[0].type_only);
    assert_eq!(facts.imports[2].bindings[0].imported, ImportName::Namespace);
    assert!(facts.imports[3].bindings.is_empty());
    assert_eq!(facts.exports.len(), 6);
    assert!(facts.exports[0].is_default);
    assert!(
        matches!(facts.exports[0].kind, ExportKind::Declaration(span) if span == facts.declarations[0].span)
    );
    assert_eq!(facts.declarations[1].kind, SymbolKind::Class);
    assert!(
        matches!(&facts.exports[2].kind, ExportKind::Named { local, alias: Some(alias) } if local.text == "go" && alias.text == "execute")
    );
    assert_eq!(facts.exports[3].source.as_ref().unwrap().text, "'./mod'");
    assert!(matches!(
        &facts.exports[4].kind,
        ExportKind::All { namespace: None }
    ));
    assert!(
        matches!(&facts.exports[5].kind, ExportKind::All { namespace: Some(name) } if name.text == "methods")
    );
    assert_token(source, &bindings[1].local);
    assert_token(source, &facts.imports[0].source);
}

#[test]
fn tracks_shadowed_declarations_and_call_scopes_with_unicode_and_crlf() {
    let source = "type Value = string;\r\nconst café = () => 1;\r\nexport function outer() {\r\n  const café = () => 2;\r\n  return café();\r\n}\r\ncafé();\r\n";
    let facts = parse("main.ts", source);
    assert!(facts.diagnostics.is_empty());
    assert_eq!(facts.declarations[0].kind, SymbolKind::Type);
    let outer = &facts.declarations[1];
    let inner = &facts.declarations[3];
    assert_eq!(outer.name.text, inner.name.text);
    assert_eq!(outer.kind, SymbolKind::Function);
    assert_ne!(outer.scope, inner.scope);
    assert_eq!(facts.calls[0].scope, inner.scope);
    assert_eq!(facts.calls[1].scope, outer.scope);
    for name in [
        &outer.name,
        &inner.name,
        &facts.calls[0].target,
        &facts.calls[1].target,
    ] {
        assert_token(source, name);
    }
    assert_eq!(facts.calls[0].target.span.start_line, 5);
    assert_eq!(facts.calls[1].target.span.start_line, 7);
}

#[test]
fn supports_jsx_tsx_and_same_dialect_renames() {
    for (original, renamed, source) in [
        ("a.jsx", "b.jsx", "export default () => <div />;"),
        (
            "a.tsx",
            "b.tsx",
            "export default (props: Props) => <div />;",
        ),
        ("a.js", "b.mjs", "export default () => 42;"),
    ] {
        let facts = parse(original, source);
        assert!(facts.diagnostics.is_empty(), "{original}");
        assert!(
            matches!(&facts.exports[0].kind, ExportKind::DefaultExpression(value) if value.text.contains("=>"))
        );
        assert_eq!(facts, parse(renamed, source));
        assert!(extract(Path::new("data.json"), source).unwrap().is_none());
    }
}

#[test]
fn preserves_valid_facts_alongside_broken_syntax() {
    let source = "export function ok() {}\nconst broken = ;\nok();";
    let facts = parse("broken.js", source);
    assert_eq!(facts.declarations[0].name.text, "ok");
    assert_eq!(facts.calls.last().unwrap().target.text, "ok");
    assert!(!facts.diagnostics.is_empty());
    assert!(
        facts
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.span.end_byte <= source.len())
    );
}
