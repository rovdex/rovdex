fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/Rovdex.ico");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set("ProductName", "Rovdex");
        res.set("FileDescription", "Rovdex AI coding tool");
        res.set("CompanyName", "Rovdex");
        res.set("LegalCopyright", "Copyright (c) Rovdex");
        res.set("OriginalFilename", "Rovdex.exe");
        res.set("InternalName", "Rovdex");
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set_icon("../../assets/icons/Rovdex.ico");
        if let Err(error) = res.compile() {
            panic!("failed to compile Windows resources: {error}");
        }
    }
}
