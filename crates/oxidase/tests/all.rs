#[path = "frame_test.rs"]
mod frame_test;
#[path = "macro_test.rs"]
mod macro_test;
#[path = "option_b_foundation_parity_test.rs"]
mod option_b_foundation_parity_test;
#[path = "subwave_1b_form_substrate_test.rs"]
mod subwave_1b_form_substrate_test;
#[path = "toast_substrate_parity_test.rs"]
mod toast_substrate_parity_test;
#[path = "track1b_substrate_test.rs"]
mod track1b_substrate_test;

#[cfg(feature = "native")]
#[path = "direct_object_test.rs"]
mod direct_object_test;
#[cfg(feature = "native")]
#[path = "dom_native_test.rs"]
mod dom_native_test;
#[cfg(feature = "native")]
#[path = "floating_cross_host_test.rs"]
mod floating_cross_host_test;
#[cfg(feature = "native")]
#[path = "watcher_substrate_test.rs"]
mod watcher_substrate_test;

#[test]
fn all_test_files_are_included() {
    let tests_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut actual = std::fs::read_dir(tests_dir)
        .expect("read integration test directory")
        .map(|entry| entry.expect("read integration test entry"))
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "all.rs")
        .collect::<Vec<_>>();
    actual.sort();

    let mut expected = [
        "direct_object_test.rs",
        "dom_native_test.rs",
        "floating_cross_host_test.rs",
        "frame_test.rs",
        "macro_test.rs",
        "option_b_foundation_parity_test.rs",
        "subwave_1b_form_substrate_test.rs",
        "toast_substrate_parity_test.rs",
        "track1b_substrate_test.rs",
        "watcher_substrate_test.rs",
    ];
    expected.sort();

    assert_eq!(
        actual, expected,
        "register every top-level integration test module in tests/all.rs"
    );
}
