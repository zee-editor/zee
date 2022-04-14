use once_cell::sync::Lazy;
use tree_sitter::Language;

pub static BASH: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_bash() });
pub static C: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_c() });
pub static CPP: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_cpp() });
pub static CSS: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_css() });
pub static HTML: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_html() });
pub static JAVASCRIPT: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_javascript() });
pub static JSON: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_json() });
pub static MARKDOWN: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_markdown() });
pub static PYTHON: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_python() });
pub static RUST: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_rust() });
pub static TYPESCRIPT: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_typescript() });
pub static TSX: Lazy<Language> = Lazy::new(|| unsafe { tree_sitter_tsx() });

extern "C" {
    fn tree_sitter_bash() -> Language;
    fn tree_sitter_c() -> Language;
    fn tree_sitter_cpp() -> Language;
    fn tree_sitter_css() -> Language;
    fn tree_sitter_html() -> Language;
    fn tree_sitter_javascript() -> Language;
    fn tree_sitter_json() -> Language;
    fn tree_sitter_markdown() -> Language;
    fn tree_sitter_python() -> Language;
    fn tree_sitter_rust() -> Language;
    fn tree_sitter_typescript() -> Language;
    fn tree_sitter_tsx() -> Language;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instantiate_all_languages() {
        assert!(BASH.node_kind_count() > 0);
        assert!(C.node_kind_count() > 0);
        assert!(CPP.node_kind_count() > 0);
        assert!(CSS.node_kind_count() > 0);
        assert!(HTML.node_kind_count() > 0);
        assert!(JAVASCRIPT.node_kind_count() > 0);
        assert!(JSON.node_kind_count() > 0);
        assert!(MARKDOWN.node_kind_count() > 0);
        assert!(PYTHON.node_kind_count() > 0);
        assert!(RUST.node_kind_count() > 0);
        assert!(TYPESCRIPT.node_kind_count() > 0);
        assert!(TSX.node_kind_count() > 0);
    }
}
