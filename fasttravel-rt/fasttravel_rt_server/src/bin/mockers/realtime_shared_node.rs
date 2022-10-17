//!
//! Implements a mocker for the realtime-server-shared-worker-node.
//!

#[tokio::main]
async fn main() {
    fasttravel_rt_server::initialize_and_run_shared_node()
        .await
        .ok();
}
