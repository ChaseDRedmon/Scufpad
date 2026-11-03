mod actor;
mod discovery;
mod gate;
mod hardware;
mod ipc;
mod output;
mod tray;

use anyhow::{Result, bail, ensure};
use scufpad_core::{ControlCommand, ControlRequest, client_request, socket_path};
use std::{
    path::Path,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

const HELP: &str = concat!(
    "scufpad [--daemon [--no-reconnect] [--no-hardware]|--status|--check|--arm|--stop|--help]\n",
    "Default: same-user headless daemon with automatic normal gamepad forwarding.\n",
    "The saved custom profile activates after all six paddles/SAX pass verification.\n",
    "--status  Print daemon state as JSON; armed means virtual output is present.\n",
    "--check   Verify USB discovery without opening input or creating outputs.\n",
    "--arm     Resume outputs after StopAll; custom mappings still require verification.\n",
    "--stop    Neutralize outputs and shut down the daemon.\n",
    "--daemon --no-reconnect  Exit after the first connected controller is lost or invalid.\n",
    "--daemon --no-hardware  Never discover/open input or uinput (also SCUFPAD_NO_HARDWARE=1).\n",
    "StopAll removes every owned output until resumed. Closing the editor leaves the daemon running."
);

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Help,
    Check,
    Client(ControlCommand),
    Daemon { reconnect: bool, hardware: bool },
}

fn parse(args: &[String], no_hardware: bool) -> Result<Mode> {
    if args.len() == 1 {
        match args[0].as_str() {
            "--help" => return Ok(Mode::Help),
            "--check" => return Ok(Mode::Check),
            "--status" => return Ok(Mode::Client(ControlCommand::GetState)),
            "--arm" => return Ok(Mode::Client(ControlCommand::Arm)),
            "--stop" => return Ok(Mode::Client(ControlCommand::Shutdown)),
            _ => {}
        }
    }
    let (mut daemon, mut no_reconnect, mut explicit_no_hardware) = (false, false, false);
    for arg in args {
        match arg.as_str() {
            "--daemon" if !daemon => daemon = true,
            "--no-reconnect" if !no_reconnect => no_reconnect = true,
            "--no-hardware" if !explicit_no_hardware => explicit_no_hardware = true,
            _ => bail!("Unknown or repeated option: {arg}; use --help"),
        }
    }
    ensure!(!no_reconnect || daemon, "--no-reconnect requires --daemon");
    Ok(Mode::Daemon {
        reconnect: !no_reconnect,
        hardware: !(no_hardware || explicit_no_hardware),
    })
}

fn daemon(reconnect: bool, hardware: bool) -> Result<()> {
    // Acquire singleton before reading profiles, starting threads or installing signal handlers.
    let endpoint = ipc::Endpoint::bind(socket_path()?)?;
    let mut actor = actor::Actor::new(hardware)?;
    let snapshot = Arc::new(RwLock::new(actor.snapshot(0)));
    let (send, receive) = mpsc::sync_channel(32);
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = stop.clone();
    ctrlc::set_handler(move || signal_stop.store(true, Ordering::Release))?;
    let server_stop = stop.clone();
    let server_snapshot = snapshot.clone();
    let server = thread::Builder::new()
        .name("scufpad-control".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ipc::serve(endpoint, send, server_snapshot, server_stop.clone())
            }));
            server_stop.store(true, Ordering::Release);
            result.unwrap_or_else(|_| Err(anyhow::anyhow!("Control thread panicked")))
        })?;
    println!("SCUFPAD_RUNTIME Rust native {}", env!("CARGO_PKG_VERSION"));
    println!(
        "Scufpad daemon: automatic gamepad forwarding; custom mappings require six-input verification. hardware={hardware}"
    );
    let tray = tray::Worker::start(snapshot.clone(), stop.clone(), hardware);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        actor.run(receive, snapshot, stop.clone(), reconnect)
    }));
    stop.store(true, Ordering::Release);
    drop(actor);
    tray.shutdown();
    let server_result = server
        .join()
        .map_err(|_| anyhow::anyhow!("Control thread panicked"))?;
    result.map_err(|_| anyhow::anyhow!("Input loop panicked; owned outputs destroyed"))??;
    server_result
}

fn run() -> Result<()> {
    ensure!(cfg!(target_os = "linux"), "Scufpad requires Linux");
    let no_hardware = std::env::var("SCUFPAD_NO_HARDWARE").as_deref() == Ok("1");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match parse(&args, no_hardware)? {
        Mode::Help => println!("{HELP}"),
        Mode::Check => {
            ensure!(
                !no_hardware,
                "Hardware probe disabled by SCUFPAD_NO_HARDWARE=1; no devices opened"
            );
            let found = discovery::discover(Path::new("/sys/class"), Path::new("/dev"))?;
            ensure!(
                found.is_some(),
                "Expected USB 1b1c:3a05 interface 03 with 16-byte report 06 and six owned evdev nodes not found"
            );
            println!("Verified USB discovery; no input devices opened and no outputs created.");
        }
        Mode::Client(command) => {
            let response = client_request(&ControlRequest {
                command,
                profile: None,
            })?;
            println!("{}", serde_json::to_string(&response)?);
            ensure!(
                response.success,
                "{}",
                response
                    .error
                    .as_deref()
                    .unwrap_or("Control request failed")
            );
        }
        Mode::Daemon {
            reconnect,
            hardware,
        } => daemon(reconnect, hardware)?,
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("scufpad: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests {
    use super::*;
    fn args(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn cli_defaults_and_explicit_safety_modes() {
        assert_eq!(
            parse(&[], false).unwrap(),
            Mode::Daemon {
                reconnect: true,
                hardware: true
            }
        );
        assert_eq!(
            parse(&[], true).unwrap(),
            Mode::Daemon {
                reconnect: true,
                hardware: false
            }
        );
        assert_eq!(
            parse(
                &args(&["--daemon", "--no-reconnect", "--no-hardware"]),
                false
            )
            .unwrap(),
            Mode::Daemon {
                reconnect: false,
                hardware: false
            }
        );
        assert_eq!(
            parse(&args(&["--arm"]), false).unwrap(),
            Mode::Client(ControlCommand::Arm)
        );
        assert_eq!(
            parse(&args(&["--stop"]), false).unwrap(),
            Mode::Client(ControlCommand::Shutdown)
        );
        assert_eq!(
            parse(&args(&["--status"]), false).unwrap(),
            Mode::Client(ControlCommand::GetState)
        );
        for options in [
            &["--arm", "--no-hardware"][..],
            &["--legacy"][..],
            &["--no-reconnect"][..],
            &["--daemon", "--daemon"][..],
        ] {
            assert!(parse(&args(options), false).is_err());
        }
    }
}
