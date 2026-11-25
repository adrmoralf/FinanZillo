fn main() {
    println!("cargo:rerun-if-changed=assets/finanzillo.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/finanzillo.ico");
        res.set("FileDescription", "FinanZillo");
        res.set("ProductName", "FinanZillo");
        res.set("LegalCopyright", "Uso personal");
        if let Err(e) = res.compile() {
            // Sin el compilador de recursos el exe sale sin icono, pero compila.
            println!("cargo:warning=No se pudo incrustar el icono: {e}");
        }
    }
}
