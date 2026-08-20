fn main() {
    tauri_build::build();

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("app.manifest");
        res.set_language(0x0409);
        res.compile().unwrap();
    }
}