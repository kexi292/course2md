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
}
