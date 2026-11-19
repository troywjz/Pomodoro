fn main() {
    slint_build::compile("ui/main.slint").expect("failed to compile the Slint UI");
    println!("cargo:rerun-if-changed=ui/main.slint");
}
