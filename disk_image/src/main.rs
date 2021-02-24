use std::{fs, io, path::Path};

fn main() {
    println!("Hello, world!");
}

fn create_fat_filesystem(fat_path: &Path, efi_file: &Path) {
    // retrieve size of `.efi` file and round it up
    let efi_size = fs::metadata(&efi_file).unwrap().len();
    let mb = 1024 * 1024; // size of a megabyte
                          // round it to next megabyte
    let efi_size_rounded = ((efi_size - 1) / mb + 1) * mb;

    // create new filesystem image file at the given path and set its length
    let fat_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&fat_path)
        .unwrap();
    fat_file.set_len(efi_size_rounded).unwrap();

    // create new FAT file system and open it
    let format_options = fatfs::FormatVolumeOptions::new();
    fatfs::format_volume(&fat_file, format_options).unwrap();
    let filesystem = fatfs::FileSystem::new(&fat_file, fatfs::FsOptions::new()).unwrap();

    // copy EFI file to FAT filesystem
    let root_dir = filesystem.root_dir();
    root_dir.create_dir("efi").unwrap();
    root_dir.create_dir("efi/boot").unwrap();
    let mut bootx64 = root_dir.create_file("efi/boot/bootx64.efi").unwrap();
    bootx64.truncate().unwrap();
    io::copy(&mut fs::File::open(&efi_file).unwrap(), &mut bootx64).unwrap();
}
