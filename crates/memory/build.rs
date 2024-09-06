fn main() {
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
