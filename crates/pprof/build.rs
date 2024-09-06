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
}
