use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    slint_build::compile("ui/main.slint").expect("failed to compile the Slint UI");
    println!("cargo:rerun-if-changed=ui/main.slint");

    let target = env::var("TARGET").expect("Cargo must set TARGET");
    if target == "x86_64-pc-windows-msvc" {
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));
        let rc = out_dir.join("pomodoro-icon.rc");
        let res = out_dir.join("pomodoro-icon.res");
        let ico = "images/pomodoro_icon_theme_borderless.ico";
        std::fs::write(&rc, format!("1 ICON \"{ico}\"\n"))
            .expect("failed to write icon resource script");
        println!("cargo:rerun-if-changed={ico}");
        println!("cargo:rerun-if-env-changed=RC");
        let compiler = env::var_os("RC").unwrap_or_else(|| "rc.exe".into());
        let status = Command::new(compiler)
            .args(["/nologo", "/fo"])
            .arg(&res)
            .arg(&rc)
            .status()
            .expect("failed to launch Windows SDK rc.exe");
        assert!(status.success(), "Windows icon resource compilation failed");
        println!("cargo:rustc-link-arg-bin=pomodoro={}", res.display());
    }
}
