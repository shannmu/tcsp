//! tcsp
#![deny(
    // missing_docs,
    // clippy::exhaustive_enums,
    // clippy::exhaustive_structs,
    // clippy::all,
    // clippy::pedantic,
    absolute_paths_not_starting_with_crate,
    rust_2021_incompatible_closure_captures,
    rust_2021_incompatible_or_patterns,
    rust_2021_prefixes_incompatible_syntax,
    rust_2021_prelude_collisions,
    variant_size_differences, 
    clippy::clone_on_ref_ptr,
    clippy::else_if_without_else,
    clippy::exit,
    clippy::expect_used,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    // clippy::indexing_slicing,
    // clippy::arithmetic_side_effects,
    clippy::shadow_unrelated,
    // clippy::unwrap_in_result,
    clippy::unwrap_used, 
)]
#![cfg_attr(
    test,
    allow(
        clippy::indexing_slicing,
        unused_results,
        clippy::unwrap_used,
        clippy::unwrap_in_result,
        clippy::expect_used,
        clippy::as_conversions,
        clippy::shadow_unrelated,
        clippy::arithmetic_side_effects,
        clippy::let_underscore_untyped,
        clippy::pedantic, 
        clippy::default_numeric_fallback,
        clippy::print_stderr,
    )
)]
use std::sync::Arc;

use application::{Application, EchoCommand, Reboot, TeleMetry, TimeSync};
use server::TcspServer;

use tokio::{self};

pub(crate) mod adaptor;
use adaptor::TyCanProtocol;
mod application;
mod protocol;
mod server;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    env_logger::init();
    #[allow(clippy::unwrap_used)]
    let adaptor = TyCanProtocol::new(0x43, "can0", "can0").unwrap();
    let tel: Arc<dyn Application> = Arc::new(TeleMetry {});
    let echo: Arc<dyn Application> = Arc::new(EchoCommand {});
    let time: Arc<dyn Application> = Arc::new(TimeSync {});
    let reboot: Arc<dyn Application> = Arc::new(Reboot {});
    let applications = [tel, echo, time,reboot].into_iter();
    let server = TcspServer::new_can(adaptor,applications);
    server.listen().await;
}
