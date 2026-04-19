//! Build script for Controller-III
//! Adds extra linker libraries needed for FFmpeg static linking on Windows

fn main() {
    #[cfg(windows)]
    {
        // FFmpeg with MediaFoundation requires these libraries
        println!("cargo:rustc-link-lib=strmiids");
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=uuid");
        println!("cargo:rustc-link-lib=mf");
        println!("cargo:rustc-link-lib=mfuuid");
    }
}
