//! 
//! Implements a mocker for the realtime-server-dedicated-worker-node.
//!

#[tokio::main]
async fn main() {
    fasttravel_rt_server::initialize_and_run_dedicated_node()
        .await
        .ok();
}
