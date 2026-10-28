fn main() {
    // the installer's own icon, and its name in file properties
    let mut res = winres::WindowsResource::new();
    res.set_icon("burrow.ico");
    res.set("ProductName", "Burrow Setup");
    res.set("FileDescription", "Burrow Setup");
    res.set("CompanyName", "tuftle");
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not add the icon: {e}");
    }
}
