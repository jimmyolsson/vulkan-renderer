use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    handle_shaders();
    handle_sdl3();
}
fn handle_sdl3() {
    println!("cargo:rustc-link-search=native=vendor/SDL3/lib/x64");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    println!("OUT_DIR: {}", out_dir);
    let target_dir = Path::new(&out_dir).ancestors().nth(3).unwrap();
    println!("Target dir: {}", target_dir.display());

    let _ = fs::copy("vendor/SDL3/bin/SDL3.dll", target_dir.join("SDL3.dll"));
}

fn handle_shaders() {
    let shader_dir = Path::new("shaders");
    println!("Starting compiling shaders");

    for entry in fs::read_dir(shader_dir).expect("Failed to read shaders directory") {
        let entry = entry.expect("Invalid directory entry");
        let path = entry.path();

        // Only process .slang files
        if path.extension().and_then(|e| e.to_str()) == Some("slang") {
            compile_shader(&path);
        }
    }
    println!("Compiling shaders DONE!")
}

fn compile_shader(path: &Path) {
    let output_path = path.with_extension("spv");

    println!("Compiling {:?}", path);

    let status = Command::new(r"C:\VulkanSDK\1.4.328.0\Bin\slangc.exe")
        .args([
            path.to_str().unwrap(),
            "-target",
            "spirv",
            "-profile",
            "spirv_1_4",
            "-emit-spirv-directly",
            "-fvk-use-entrypoint-name",
            "-entry",
            "vertMain",
            "-entry",
            "fragMain",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run slangc");

    if !status.success() {
        panic!("Shader compilation failed for {:?}", path);
    }
}
