fn main() {
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");
    let _guard = _rt.enter();

    silvani_ui::run();
}
