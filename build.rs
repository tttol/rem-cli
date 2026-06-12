use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=native/macos_speech.m");
    println!("cargo:rerun-if-changed=native/Info.plist");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let object_path = out_dir.join("macos_speech.o");
    let library_path = out_dir.join("librem_speech.a");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("target architecture is not set");
    let clang_arch = match target_arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        unsupported => panic!("unsupported macOS architecture: {unsupported}"),
    };

    run(
        Command::new("clang")
            .args([
                "-fobjc-arc",
                "-fblocks",
                "-mmacosx-version-min=14.0",
                "-arch",
                clang_arch,
                "-c",
                "native/macos_speech.m",
                "-o",
            ])
            .arg(&object_path),
        "compile the macOS speech bridge",
    );
    run(
        Command::new("ar")
            .args(["crus"])
            .arg(&library_path)
            .arg(&object_path),
        "archive the macOS speech bridge",
    );

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=rem_speech");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=Speech");
    println!("cargo:rustc-link-lib=framework=AVFAudio");
    println!("cargo:rustc-link-lib=framework=AVFoundation");

    let info_plist = absolute_path(Path::new("native/Info.plist"));
    println!(
        "cargo:rustc-link-arg-bin=rem=-Wl,-sectcreate,__TEXT,__info_plist,{}",
        info_plist.display()
    );
}

fn run(command: &mut Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to {action}: {error}"));
    assert!(status.success(), "failed to {action}");
}

fn absolute_path(path: &Path) -> PathBuf {
    env::current_dir()
        .expect("current directory is unavailable")
        .join(path)
}
