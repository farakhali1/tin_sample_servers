//! Compiled gRPC types for the TIN sample servers.
//!
//! Schemas live next to this crate (`auth.proto`, `block_engine.proto`, …).
//! Wire message names are unchanged so validators stay compatible.

pub mod auth {
    tonic::include_proto!("auth");
}

pub mod block_engine {
    tonic::include_proto!("block_engine");
}

pub mod bundle {
    tonic::include_proto!("bundle");
}

pub mod packet {
    tonic::include_proto!("packet");
}

pub mod shared {
    tonic::include_proto!("shared");
}
