fn main() {
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icons/icon.ico");
            res.set("ProductName", "Quick Download Manager");
            res.set(
                "FileDescription",
                "Quick Download Manager - Modern Open-Source Downloader",
            );
            res.set("CompanyName", "ShahStudioz");
            res.set("LegalCopyright", "Copyright (C) 2026 ShahStudioz");
            if let Err(e) = res.compile() {
                eprintln!("Warning: Failed to compile Windows resource: {}", e);
            }
        }
    }
}
