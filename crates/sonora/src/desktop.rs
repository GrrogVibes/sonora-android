//! The desktop entry point.
//!
//! Everything here is the desktop preamble: the widevine host, the command
//! line, logging, the single-instance socket and the `gpui_platform`
//! application. The startup that follows is shared with Android in
//! [`crate::launch`].

use std::process::{ExitCode, exit};
use std::sync::Arc;

use cli::commands::CommandType;

use crate::assets;
use crate::console;
use crate::http;
use crate::launch::{self, Launch};
use crate::logging;
use crate::memory;
use crate::single;

pub fn main() -> ExitCode {
    memory::tune();
    if let Some(code) = webview::probed() {
        exit(code);
    }
    // The Widevine host is this executable started again by the app. It must not claim the
    // instance socket, write the app's log file or start a runtime or a window.
    if let Some(module) = music::drm::hosted() {
        logging::console();
        if let Err(error) = music::drm::host(&module) {
            log::error!("widevine: the cdm host failed: {error:#}");
            exit(1);
        }
        return ExitCode::SUCCESS;
    }
    // A command for the running Sonora is sent and answered here, without a log file, a
    // runtime or a window. Older macOS adds a `-psn_` argument to a Finder launch.
    let args = std::env::args_os()
        .filter(|arg| !arg.to_string_lossy().starts_with("-psn_"))
        .collect();
    let command = cli::parse(args, state::version());
    if !matches!(command, Ok(CommandType::Launch(_))) {
        console::attach();
    }
    let args = match command {
        Ok(CommandType::Launch(items)) => items,
        Ok(CommandType::Local(local)) => return cli::local(local),
        Ok(CommandType::Remote(request, format)) => {
            return match single::connect(request.streams()) {
                Ok(mut stream) => cli::run(&mut stream, &request, format),
                Err(_) => cli::not_running(),
            };
        }
        Err(refusal) => return refusal.exit(),
    };
    logging::init();

    let (sender, links) = tokio::sync::mpsc::unbounded_channel::<Vec<String>>();
    let (asked, commands) = tokio::sync::mpsc::unbounded_channel::<single::Asked>();
    match single::claim(&args, sender.clone(), asked) {
        single::Instance::First => {}
        single::Instance::Running => return ExitCode::SUCCESS,
        single::Instance::Failed => exit(1),
    }
    // Only the instance that owns the socket may replace Sonora, never a hand-off to one that runs.
    if state::start_staged(!args.is_empty()) {
        return ExitCode::SUCCESS;
    }
    // reqwest is built without a crypto provider, and opensubsonic still compiles aws-lc-rs in
    // beside ring, so rustls cannot pick one by itself. Every client uses ring from here on. A
    // second install only means another crate got there first.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let io = match state::Io::new() {
        Ok(io) => io,
        Err(error) => {
            eprintln!("sonora: cannot start runtime: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    let app = gpui_platform::application()
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(http::Client::new(io.handle())));
    let opened_urls_sender = sender.clone();
    app.on_open_urls(move |opened| {
        opened_urls_sender.send(opened).ok();
    });
    app.on_reopen(launch::show_window);

    launch::start(
        app,
        Launch {
            io,
            args,
            links: Some((sender, links)),
            commands: Some(commands),
        },
    );
    ExitCode::SUCCESS
}
