fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rerun-if-changed=../ios/ListenerNative/QQLogin.m");
        cc::Build::new()
            .file("../ios/ListenerNative/QQLogin.m")
            .flag("-fobjc-arc")
            .compile("listener_ios");
        println!("cargo:rustc-link-lib=framework=WebKit");
    }
    tauri_build::build()
}
