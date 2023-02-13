fn main() {
    let gexiv2 = pkg_config::Config::new()
        .atleast_version("0.14.0")
        .probe("gexiv2")
        .unwrap();

    // compile c files
    cc::Build::new()
        .file("lib/stream.c")
        .file("lib/add_gps.c")
        .include("lib")
        .includes(gexiv2.include_paths)
        .compile("gps");

    println!("cargo:rerun-if-changed=lib/add_gps.c");
    println!("cargo:rerun-if-changed=lib/stream.h");
    println!("cargo:rerun-if-changed=lib/stream.c");
}
