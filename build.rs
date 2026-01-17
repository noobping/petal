#[cfg(target_os = "windows")]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data");

    glib_build_tools::compile_resources(
        &["data"],
        &format!("data/resources.xml"),
        "compiled.gresource",
    );

    let ico_path = std::path::Path::new("data").join(format!("{}.ico", env!("CARGO_PKG_NAME")));
    println!("cargo:rerun-if-changed={}", ico_path.display());
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_string_lossy().as_ref());
    res.compile().expect("Failed to compile Windows resources");
}

#[cfg(target_os = "linux")]
fn main() {}
