use std::sync::Arc;

fn main() {
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");
    let _guard = _rt.enter();

    let http_client = Arc::new(
        reqwest_client::ReqwestClient::user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Silvani/1.0",
        )
        .unwrap_or_else(|_| reqwest_client::ReqwestClient::new()),
    );

    silvani_ui::run(http_client);
}
