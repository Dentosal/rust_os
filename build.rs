use std::path::Path;

fn main() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("Missing target_os");

    if target_os != "none" {
        return;
    }

    let linker_args = [
        &format!(
            "--script={}",
            crate_root.join("build_config/linker.ld").display()
        ),
        "-nmagic",
        "-zcommon-page-size=0x1000",
        "-zmax-page-size=0x1000",
        "-zstack-size=0x1000",
        &format!("{}", crate_root.join("build/kernel_entry.o").display()),
    ];

    for arg in linker_args {
        println!("cargo:rustc-link-arg-bins={arg}");
    }
}
