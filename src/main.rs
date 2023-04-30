use bootloader::DiskImageBuilder;
use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    // set by cargo for the kernel artifact dependency
    let kernel_path = PathBuf::from(env!("CARGO_BIN_FILE_KERNEL"));
    let disk_builder = DiskImageBuilder::new(kernel_path);

    // place the disk image files under target/debug or target/release
    let target_dir = env::current_exe()?;

    let uefi_path = target_dir.with_file_name("blog_os-uefi.img");
    disk_builder.create_uefi_image(&uefi_path)?;
    println!("Created UEFI disk image at {}", uefi_path.display());

    let bios_path = target_dir.with_file_name("blog_os-bios.img");
    disk_builder.create_bios_image(&bios_path)?;
    println!("Created BIOS disk image at {}", bios_path.display());

    Ok(())
}
