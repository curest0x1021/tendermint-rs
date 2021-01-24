//! ABCI framework for building applications with Tendermint.

mod application;
#[cfg(any(
    feature = "client",
    feature = "runtime-tokio",
    feature = "runtime-async-std",
    feature = "runtime-std",
))]
mod codec;
mod result;
pub mod runtime;
mod server;

// Client exports
#[cfg(feature = "client")]
mod client;

#[cfg(feature = "client")]
pub use client::Client;

// Example applications
#[cfg(feature = "echo-app")]
pub use application::echo::EchoApp;

// Common exports
pub use application::Application;
pub use result::{Error, Result};
pub use server::Server;

// Runtime-specific exports
#[cfg(all(feature = "async", feature = "client", feature = "runtime-tokio"))]
pub type TokioClient = Client<runtime::tokio::Tokio>;
#[cfg(all(feature = "async", feature = "client", feature = "runtime-async-std"))]
pub type AsyncStdClient = Client<runtime::async_std::AsyncStd>;
#[cfg(all(not(feature = "async"), feature = "client", feature = "runtime-std"))]
pub type StdClient = Client<runtime::std::Std>;

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
pub type TokioServer<A> = Server<A, runtime::tokio::Tokio>;
#[cfg(all(feature = "async", feature = "runtime-async-std"))]
pub type AsyncStdServer<A> = Server<A, runtime::async_std::AsyncStd>;
#[cfg(all(not(feature = "async"), feature = "runtime-std"))]
pub type StdServer<A> = Server<A, runtime::std::Std>;