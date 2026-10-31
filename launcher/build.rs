/*
    written by: @corescripts
*/

// Embeds the XBLOX: REWIND icon and version info into XbloxRewind.exe.
fn main() {
    println!("cargo:rerun-if-changed=assets/branding/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/branding/app.ico");
    res.set("ProductName", "XBLOX: REWIND");
    res.set("FileDescription", "XBLOX: REWIND");
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not embed the exe icon: {e}");
    }
}

// >> end << //