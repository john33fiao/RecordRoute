use recordroute_server::{app, init_tracing, AppState};

#[tokio::main]
async fn main() {
    init_tracing();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
        .await
        .expect("bind listener");

    axum::serve(listener, app(AppState::from_env()))
        .await
        .expect("run server");
}
