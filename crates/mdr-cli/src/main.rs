//! `hairlinectl`: talks to the running Hairline app over its local socket.

mod proto;

use std::process::ExitCode;

use clap::Parser;

use proto::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(text) => {
            eprintln!("{text}");
            ExitCode::FAILURE
        }
    }
}

/// Sends the request and reads the answer; `Err` carries what to print.
#[cfg(unix)]
fn run(cli: &Cli) -> Result<String, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let env_socket = std::env::var("HAIRLINE_SOCKET").ok();
    let path = proto::socket_path(env_socket.as_deref(), &home);
    let stream = UnixStream::connect(&path)
        .map_err(|_| format!("Hairline is not running (no socket at {})", path.display()))?;
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("socket error: {e}"))?;
    writeln!(writer, "{}", proto::request(&cli.command))
        .map_err(|e| format!("could not send to Hairline: {e}"))?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| format!("no answer from Hairline: {e}"))?;
    let answer: serde_json::Value = serde_json::from_str(line.trim())
        .map_err(|_| format!("Hairline answered something unreadable: {}", line.trim()))?;
    let ok = answer["ok"].as_bool() == Some(true);
    let text = if cli.json {
        answer.to_string()
    } else if ok {
        proto::human(&cli.command, &answer)
    } else {
        answer["error"]
            .as_str()
            .unwrap_or("Hairline refused the request")
            .to_string()
    };
    if ok {
        Ok(text)
    } else {
        Err(text)
    }
}

#[cfg(not(unix))]
fn run(_: &Cli) -> Result<String, String> {
    let _ = (proto::request, proto::socket_path, proto::human);
    Err("not supported on Windows yet".to_string())
}
