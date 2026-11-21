fn main() {
    slint_build::compile("ui/app.slint").expect("slint build failed");

    // Windows shows nothing in a file's Properties without a version resource,
    // and "which build is this" is the first question a beta has to answer.
    // Mirrors the release exe's identity (ProductName/FileDescription/Company)
    // with the crate's own version, so the two shells never claim the same one.
    #[cfg(windows)]
    {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "Quota Tray");
        res.set("FileDescription", "Quota Tray (Slint shell, beta)");
        res.set("CompanyName", "shawnqi");
        res.set("FileVersion", &format!("{version}.0"));
        res.set("ProductVersion", &format!("{version}.0"));
        res.compile().expect("windows version resource");
    }
}
