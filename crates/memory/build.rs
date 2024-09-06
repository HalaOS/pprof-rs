fn gen_gperf() {
    use std::{fs::create_dir, path::Path};

    let out_dir = Path::new("src/proto");

    if !out_dir.exists() {
        create_dir(out_dir).unwrap();
    }

    protobuf_codegen::Codegen::new()
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .includes(&["proto"])
        .input("proto/gperf.proto")
        .out_dir(out_dir)
        .run_from_script();
}

fn main() {
    gen_gperf();

    let mut build = cc::Build::new();

    build
        .cpp(true)
        .static_crt(true)
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17")
        .flag_if_supported("/MD")
        .opt_level(3);

    println!("cargo:rerun-if-changed=src/helper/helper.cpp");
    build.file("src/helper/helper.cpp");
    build.compile("hala_pprof_c");
}
