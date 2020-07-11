use cc::Build;
use std::path::PathBuf;

fn compile_tree_sitter_typescript() {
    println!("cargo:rustc-link-lib=static=tree-sitter-typescript");

    let dir: PathBuf = ["languages", "tree-sitter-typescript", "typescript", "src"]
        .iter()
        .collect();
    Build::new()
        .include(&dir)
        .flag("-w")
        .file(dir.join("parser.c"))
        .file(dir.join("scanner.c"))
        .compile("tree-sitter-typescript");
}

fn compile_tree_sitter_tsx() {
    println!("cargo:rustc-link-lib=static=tree-sitter-tsx");

    let dir: PathBuf = ["languages", "tree-sitter-typescript", "tsx", "src"]
        .iter()
        .collect();
    Build::new()
        .include(&dir)
        .flag("-w")
        .file(dir.join("parser.c"))
        .file(dir.join("scanner.c"))
        .compile("tree-sitter-tsx");
}

fn compile_tree_sitter_c_lib_no_scanner(name: &str) {
    println!("cargo:rustc-link-lib=static={}", name);

    let dir: PathBuf = ["languages", name, "src"].iter().collect();
    Build::new()
        .include(&dir)
        .flag("-w")
        .file(dir.join("parser.c"))
        .compile(name);
}

fn compile_tree_sitter_c_lib(name: &str) {
    println!("cargo:rustc-link-lib=static={}", name);

    let dir: PathBuf = ["languages", name, "src"].iter().collect();
    Build::new()
        .include(&dir)
        .flag("-w")
        .file(dir.join("parser.c"))
        .file(dir.join("scanner.c"))
        .compile(name);
}

fn compile_tree_sitter_cpp_lib(name: &str) {
    let scanner_lib = format!("{}-scanner", name);
    let parser_lib = format!("{}-parser", name);
    println!("cargo:rustc-link-lib=static={}", scanner_lib);
    println!("cargo:rustc-link-lib=static={}", parser_lib);

    let dir: PathBuf = ["languages", name, "src"].iter().collect();
    Build::new()
        .cpp(true)
        .flag("-w")
        .include(&dir)
        .file(dir.join("scanner.cc"))
        .compile(&scanner_lib);
    Build::new()
        .flag("-w")
        .include(&dir)
        .file(dir.join("parser.c"))
        .compile(&parser_lib);
}

#[allow(warnings)]
fn main() {
    compile_tree_sitter_c_lib("tree-sitter-css");
    compile_tree_sitter_c_lib("tree-sitter-javascript");
    compile_tree_sitter_c_lib("tree-sitter-rust");
    compile_tree_sitter_c_lib_no_scanner("tree-sitter-c");
    compile_tree_sitter_c_lib_no_scanner("tree-sitter-json");
    compile_tree_sitter_cpp_lib("tree-sitter-bash");
    compile_tree_sitter_cpp_lib("tree-sitter-cpp");
    compile_tree_sitter_cpp_lib("tree-sitter-html");
    compile_tree_sitter_cpp_lib("tree-sitter-markdown");
    compile_tree_sitter_cpp_lib("tree-sitter-python");
    compile_tree_sitter_typescript();
    compile_tree_sitter_tsx();
}
