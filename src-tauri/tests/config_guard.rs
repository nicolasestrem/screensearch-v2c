use serde_json::Value;

#[test]
fn overlay_window_is_precreated_hidden_and_capture_protected() {
    let raw = std::fs::read_to_string("tauri.conf.json").expect("read tauri.conf.json");
    let config: Value = serde_json::from_str(&raw).expect("parse tauri.conf.json");
    let windows = config["app"]["windows"]
        .as_array()
        .expect("app.windows array");
    let overlay = windows
        .iter()
        .find(|window| window["label"] == "overlay")
        .expect("overlay window exists");

    assert_eq!(overlay["visible"], false, "overlay starts hidden");
    assert_eq!(overlay["alwaysOnTop"], true, "overlay is always-on-top");
    assert_eq!(overlay["skipTaskbar"], true, "overlay skips taskbar");
    assert_eq!(
        overlay["contentProtected"], true,
        "overlay is excluded from OS capture"
    );
}
