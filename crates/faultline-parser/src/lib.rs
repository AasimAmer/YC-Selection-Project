//! Grammar selection for the supported source extensions.
use std::path::Path;
use tree_sitter::{LanguageError, Parser};

pub fn parser_for(path: &Path) -> Result<Option<Parser>, LanguageError> {
    let language = match path.extension().and_then(|ext| ext.to_str()) {
        Some("ts" | "mts" | "cts") => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Some("tsx") => tree_sitter_typescript::LANGUAGE_TSX,
        Some("js" | "jsx" | "mjs" | "cjs") => tree_sitter_javascript::LANGUAGE,
        _ => return Ok(None),
    };
    let mut parser = Parser::new();
    parser.set_language(&language.into())?;
    Ok(Some(parser))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_real_sources_in_each_dialect() {
        for (path, source) in [
            ("a.ts", "export const n: number = 1;"),
            ("a.tsx", "export const A = (p: Props) => <p/>;"),
            ("a.js", "export function f() { return 1; }"),
            ("a.jsx", "export const A = () => <p/>;"),
        ] {
            let mut parser = parser_for(Path::new(path)).unwrap().unwrap();
            let tree = parser.parse(source, None).unwrap();
            assert!(!tree.root_node().has_error(), "{path}");
            assert_eq!(tree.root_node().end_byte(), source.len());
        }
        assert!(parser_for(Path::new("a.txt")).unwrap().is_none());
    }
}
