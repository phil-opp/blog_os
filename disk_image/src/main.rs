use std::process::Command;
use anyhow::anyhow;

const TARGET_NAME: &str = "x86_64-blog_os";
const KERNEL_BINARIES: &[&str] = &["blog_os"];

fn main() -> anyhow::Result<()> {
    // build all binaries
    let mut build_cmd = Command::new(env!("CARGO"));
    build_cmd.arg("build");
    build_cmd.arg("--release");
    build_cmd.arg("-Zbuild-std=core");
    build_cmd.arg("--target").arg(format!("{}.json", TARGET_NAME));
    if !build_cmd.status()?.success() {
        return Err(anyhow!("build failed"));
    };

    let kernel_manifest = locate_cargo_manifest::locate_manifest()?;
    let target_dir_root = kernel_manifest.parent().unwrap().join("target");
    let target_dir = target_dir_root.join(TARGET_NAME).join("release");

    for binary_name in KERNEL_BINARIES {
        let binary_path = {
            let path = target_dir.join(binary_name);
            path.canonicalize()?
        };
        
        let disk_image = disk_image::create_disk_image(&binary_path, false)?;

        println!("Created disk image for binary {} at {}", binary_name, disk_image.display());
    }

    Ok(())
}
