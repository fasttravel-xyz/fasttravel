**fasttravel-rt-proto**


Package contains the protocol buffer definitions of the fasttravel-rt realtime messages.
Along with the protocol buffer definitions this package also contains:
* The Rust crate that uses [`prost`](https://github.com/tokio-rs/prost) to generate the Rust bindings for the protocol buffers.
* Helper functionalities [`helpers`](./src/helpers.rs) used by both the server and the client-sdk.

