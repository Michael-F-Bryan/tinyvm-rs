use cc::Build;
use std::path::Path;

fn main() {
    let tinyvm = Path::new("vendor/tinyvm");
    let include = tinyvm.join("include");
    let src = tinyvm.join("libtvm");

    Build::new()
        .warnings(false)
        .debug(true)
        .file(src.join("tvm_file.c"))
        .file(src.join("tvm_lexer.c"))
        .file(src.join("tvm_memory.c"))
        .file(src.join("tvm_parser.c"))
        .file(src.join("tvm_preprocessor.c"))
        .file(src.join("tvm_program.c"))
        .file(src.join("tvm.c"))
        .include(&include)
        .compile("tvm");
}
