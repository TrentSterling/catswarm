// Embeds the kitty icon into KittyParty.exe. Needs rc.exe (Windows SDK) or
// windres (mingw); if neither is around the exe still builds, just with the
// default icon.
fn main() {
    println!("cargo:rerun-if-changed=assets/kittyparty.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/kittyparty.ico");
    res.set("FileDescription", "KittyParty");
    res.set("ProductName", "KittyParty");
    if let Err(e) = res.compile() {
        println!("cargo:warning=skipping exe icon: {e}");
    }
}
