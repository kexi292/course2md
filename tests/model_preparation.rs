#[tokio::test]
async fn stopped_model_preparation_preserves_typed_control_errors_without_network() {
    let root = tempfile::tempdir().unwrap();
    let control = root.path().join("control.json");
    let models = root.path().join("models");
    for intent in ["pause", "cancel"] {
        std::fs::write(&control, serde_json::json!({"intent":intent}).to_string()).unwrap();
        let _guard =
            course2md::dispatch::install(root.path(), Some(&control), &Default::default()).unwrap();
        let error = course2md::models::download_models(&models)
            .await
            .unwrap_err();
        assert!(
            error
                .downcast_ref::<course2md::dispatch::Failure>()
                .is_some(),
            "{error:#}"
        );
        assert!(!models.exists());
    }
    std::fs::write(&control, r#"{"intent":"run"}"#).unwrap();
    let _guard =
        course2md::dispatch::install(root.path(), Some(&control), &Default::default()).unwrap();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        course2md::checkpoint::atomic_write(&control, br#"{"intent":"cancel"}"#).unwrap();
    });
    let started = std::time::Instant::now();
    let error =
        course2md::runtime::interruptible_sleep(std::time::Duration::from_secs(30)).unwrap_err();
    writer.join().unwrap();
    assert!(
        error
            .downcast_ref::<course2md::dispatch::Failure>()
            .is_some()
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}
