// Embeds the icon, version info and manifest into DockX.exe (GNU toolchain via windres).
fn main() {
    println!("cargo:rerun-if-changed=res");
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows-gnu") {
        return;
    }
    let out = std::env::var("OUT_DIR").unwrap();
    let obj = format!("{out}/dockx_res.o");
    for tool in ["x86_64-w64-mingw32-windres", "windres"] {
        let ok = std::process::Command::new(tool)
            .args(["--input-format=rc", "-O", "coff", "-c", "65001", "-i", "res/dockx.rc", "-o", &obj])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            println!("cargo:rustc-link-arg-bin=DockX={obj}");
            return;
        }
    }
    println!("cargo:warning=windres not found: DockX.exe will have no icon");
}
