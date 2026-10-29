fn main() {
    let release: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("../release.json").expect("Read release metadata"),
    )
    .expect("Valid release metadata");
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tauri.conf.json").expect("Read host metadata"),
    )
    .expect("Valid host metadata");
    assert_eq!(
        release["bundle"], config["identifier"],
        "Bundle identity differs between release and host"
    );
    assert_eq!(
        release["version"], config["version"],
        "Release version differs from host"
    );
    println!("cargo:rerun-if-changed=../release.json");
    for (field, variable) in [
        ("bundle", "VAO_BUNDLE_ID"),
        ("version", "VAO_RELEASE_VERSION"),
        ("build", "VAO_BUILD_NUMBER"),
    ] {
        println!(
            "cargo:rustc-env={variable}={}",
            release[field].as_str().expect("String metadata")
        );
    }

    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("../platform/macos/Permissions.m")
            .file("../platform/macos/ShortcutCapture.m")
            .file("../platform/macos/VoiceBar.m")
            .file("../platform/macos/InputActivation.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("vao_permissions");
        for name in [
            "AVFoundation",
            "Foundation",
            "AppKit",
            "ApplicationServices",
            "CoreGraphics",
            "Security",
            "LocalAuthentication",
        ] {
            println!("cargo:rustc-link-lib=framework={name}");
        }
        println!("cargo:rerun-if-changed=../platform/macos/Permissions.m");
        println!("cargo:rerun-if-changed=../platform/macos/ShortcutCapture.m");
        println!("cargo:rerun-if-changed=../platform/macos/VoiceBar.m");
        println!("cargo:rerun-if-changed=../platform/macos/InputActivation.m");
    }
    tauri_build::build()
}
