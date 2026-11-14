use almahrya_api_types::{SessionKindDto, export_all_to};
use std::path::Path;

#[test]
fn ticket_kind_serializes_tagged() {
    let k = SessionKindDto::Ticket {
        key: "PROJ-123".into(),
    };
    assert_eq!(
        serde_json::to_string(&k).unwrap(),
        r#"{"type":"ticket","key":"PROJ-123"}"#
    );
}

fn read_dir(dir: &Path) -> Vec<(String, String)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("read bindings dir")
        .map(|e| {
            let e = e.expect("dir entry");
            let text = std::fs::read_to_string(e.path()).expect("read binding");
            (e.file_name().to_string_lossy().into_owned(), text)
        })
        .collect();
    v.sort();
    v
}

fn committed() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bindings")
}

#[test]
fn typescript_bindings_are_current() {
    let tmp = std::env::temp_dir().join(format!("almahrya-bindings-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    export_all_to(&tmp).unwrap();
    let fresh = read_dir(&tmp);
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(fresh, read_dir(&committed()));
}

#[test]
#[ignore = "writes the committed bindings"]
fn export_bindings() {
    let dir = committed();
    let _ = std::fs::remove_dir_all(&dir);
    export_all_to(&dir).unwrap();
}
