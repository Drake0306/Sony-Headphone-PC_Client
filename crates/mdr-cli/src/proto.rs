//! Arguments, the request each one sends, and the human-readable answers.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};

/// The app's bundle identifier; its config directory holds the socket.
const APP_ID: &str = "com.drake0306.hairline";
const SOCKET_NAME: &str = "shdc.sock";

#[derive(Debug, Parser)]
#[command(
    name = "shdcctl",
    version,
    about = "Control the running SHDC app from a terminal",
    long_about = "Control the running SHDC app from a terminal.\n\nTalks to the app over a local socket; SHDC must be running. Set SHDC_SOCKET to use another socket path."
)]
pub struct Cli {
    /// Print the app's raw JSON answer.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Battery, noise, ULT, volume and codec.
    State,
    /// Noise cancelling, ambient sound or off.
    Nc {
        mode: NcArg,
        /// Ambient level, 1 to 20.
        #[arg(long)]
        level: Option<u8>,
        /// Voice focus; ambient only (refused with nc or off).
        #[arg(long)]
        voice: Option<OnOff>,
    },
    /// ULT: 0 (off), 1 or 2.
    Ult { level: u8 },
    /// Volume, 0 to 30.
    Volume { level: u8 },
    /// EQ preset by name ("bass boost") or hex id ("16").
    Eq { preset: String },
    /// Apply a scene by name.
    Scene { name: String },
    /// List the scenes; the active one is marked.
    Scenes,
    /// Time left from the battery history.
    Battery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NcArg {
    Nc,
    Ambient,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OnOff {
    On,
    Off,
}

/// The JSON line `command` sends to the app.
pub fn request(command: &Command) -> Value {
    match command {
        Command::State => json!({"cmd": "state"}),
        Command::Nc { mode, level, voice } => {
            let mode = match mode {
                NcArg::Nc => "nc",
                NcArg::Ambient => "ambient",
                NcArg::Off => "off",
            };
            let mut req = json!({"cmd": "set", "control": "nc", "value": mode});
            if let Some(level) = level {
                req["level"] = json!(level);
            }
            if let Some(voice) = voice {
                req["voice"] = json!(*voice == OnOff::On);
            }
            req
        }
        Command::Ult { level } => json!({"cmd": "set", "control": "ult", "value": level}),
        Command::Volume { level } => json!({"cmd": "set", "control": "volume", "value": level}),
        Command::Eq { preset } => json!({"cmd": "set", "control": "eq_preset", "value": preset}),
        Command::Scene { name } => json!({"cmd": "scene", "name": name}),
        Command::Scenes => json!({"cmd": "scenes"}),
        Command::Battery => json!({"cmd": "battery"}),
    }
}

/// `$SHDC_SOCKET` when set, else `shdc.sock` in the app's config
/// directory (the one Tauri's `app_config_dir` names).
pub fn socket_path(env_socket: Option<&str>, home: &Path) -> PathBuf {
    if let Some(path) = env_socket.filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    let config = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".config"))
    };
    config.join(APP_ID).join(SOCKET_NAME)
}

/// What to print for a successful answer to `command`.
pub fn human(command: &Command, answer: &Value) -> String {
    match command {
        Command::State => format_state(&answer["state"]),
        Command::Battery => format_battery(&answer["battery"]),
        Command::Scenes => format_scenes(answer),
        _ => "ok".to_string(),
    }
}

const BLANK: &str = "--";

/// A tracked value's `value`, when it has one.
fn tracked(state: &Value, key: &str) -> Option<Value> {
    state
        .get(key)
        .and_then(|t| t.get("value"))
        .filter(|v| !v.is_null())
        .cloned()
}

pub fn format_state(state: &Value) -> String {
    let link = if state["connected"].as_bool() == Some(true) {
        match state["model"].as_str() {
            Some(model) => format!("connected, {model}"),
            None => "connected".to_string(),
        }
    } else {
        "not connected".to_string()
    };
    let battery = tracked(state, "battery")
        .and_then(|b| {
            let level = b.get(0)?.as_u64()?;
            let charging = match b.get(1).and_then(Value::as_str) {
                Some("Charging") => " (charging)",
                Some("Charged") => " (charged)",
                _ => "",
            };
            Some(format!("{level} %{charging}"))
        })
        .unwrap_or_else(|| BLANK.into());
    let noise = tracked(state, "nc")
        .and_then(|nc| {
            Some(match nc["mode"].as_str()? {
                "NoiseCancelling" => "noise cancelling".to_string(),
                "Ambient" => {
                    let voice = if nc["voice_focus"].as_bool() == Some(true) {
                        ", voice focus on"
                    } else {
                        ""
                    };
                    format!("ambient, level {}{voice}", nc["level"])
                }
                "Off" => "off".to_string(),
                other => other.to_string(),
            })
        })
        .unwrap_or_else(|| BLANK.into());
    let ult = tracked(state, "eq_ult")
        .and_then(|eq| {
            Some(
                match eq["ult"].as_str()? {
                    "Off" => "off",
                    "Ult1" => "1",
                    "Ult2" => "2",
                    other => other,
                }
                .to_string(),
            )
        })
        .unwrap_or_else(|| BLANK.into());
    let volume = tracked(state, "volume")
        .and_then(|v| v.as_u64())
        .map(|v| format!("{v} / 30"))
        .unwrap_or_else(|| BLANK.into());
    let codec = tracked(state, "codec")
        .map(|c| match c.as_str() {
            Some("Sbc") => "SBC".to_string(),
            Some("Aac") => "AAC".to_string(),
            Some("Ldac") => "LDAC".to_string(),
            Some("Lc3") => "LC3".to_string(),
            Some("Unsettled") => BLANK.to_string(),
            Some(other) => other.to_string(),
            None => match c.get("Other").and_then(Value::as_u64) {
                Some(id) => format!("other ({id:#04X})"),
                None => BLANK.to_string(),
            },
        })
        .unwrap_or_else(|| BLANK.into());
    [
        ("Headphones", link),
        ("Battery", battery),
        ("Noise", noise),
        ("ULT", ult),
        ("Volume", volume),
        ("Codec", codec),
    ]
    .iter()
    .map(|(k, v)| format!("{k:<11}{v}"))
    .collect::<Vec<_>>()
    .join("\n")
}

pub fn format_battery(battery: &Value) -> String {
    match battery["minutes_left"].as_u64() {
        Some(minutes) if minutes >= 60 => {
            format!("about {} h {} m left", minutes / 60, minutes % 60)
        }
        Some(minutes) => format!("about {minutes} m left"),
        None => "not enough history yet".to_string(),
    }
}

pub fn format_scenes(answer: &Value) -> String {
    let active = answer["active"].as_str();
    let lines: Vec<String> = answer["scenes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|scene| {
            let mark = if scene["id"].as_str().is_some() && scene["id"].as_str() == active {
                "*"
            } else {
                " "
            };
            let off = if scene["enabled"].as_bool() == Some(false) {
                " (off)"
            } else {
                ""
            };
            format!("{mark} {}{off}", scene["name"].as_str().unwrap_or(BLANK))
        })
        .collect();
    if lines.is_empty() {
        "no scenes".to_string()
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(args: &[&str]) -> Value {
        let mut argv = vec!["shdcctl"];
        argv.extend_from_slice(args);
        request(&Cli::try_parse_from(argv).expect("parses").command)
    }

    #[test]
    fn each_subcommand_maps_to_its_request() {
        assert_eq!(req(&["state"]), json!({"cmd": "state"}));
        assert_eq!(
            req(&["nc", "nc"]),
            json!({"cmd": "set", "control": "nc", "value": "nc"})
        );
        assert_eq!(
            req(&["nc", "ambient", "--level", "5", "--voice", "on"]),
            json!({"cmd": "set", "control": "nc", "value": "ambient", "level": 5, "voice": true})
        );
        assert_eq!(
            req(&["nc", "off", "--voice", "off"]),
            json!({"cmd": "set", "control": "nc", "value": "off", "voice": false})
        );
        assert_eq!(
            req(&["ult", "2"]),
            json!({"cmd": "set", "control": "ult", "value": 2})
        );
        assert_eq!(
            req(&["volume", "14"]),
            json!({"cmd": "set", "control": "volume", "value": 14})
        );
        assert_eq!(
            req(&["eq", "bass boost"]),
            json!({"cmd": "set", "control": "eq_preset", "value": "bass boost"})
        );
        assert_eq!(
            req(&["eq", "16"]),
            json!({"cmd": "set", "control": "eq_preset", "value": "16"})
        );
        assert_eq!(
            req(&["scene", "Commute"]),
            json!({"cmd": "scene", "name": "Commute"})
        );
        assert_eq!(req(&["scenes"]), json!({"cmd": "scenes"}));
        assert_eq!(req(&["battery", "--json"]), json!({"cmd": "battery"}));
    }

    #[test]
    fn bad_arguments_are_refused_before_sending() {
        for args in [
            vec!["shdcctl"],
            vec!["shdcctl", "nc", "loud"],
            vec!["shdcctl", "nc", "ambient", "--voice", "maybe"],
            vec!["shdcctl", "volume", "loud"],
            vec!["shdcctl", "raw", "22"],
            vec!["shdcctl", "power"],
        ] {
            assert!(Cli::try_parse_from(&args).is_err(), "{args:?}");
        }
        let cli = Cli::try_parse_from(["shdcctl", "--json", "state"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn socket_path_prefers_the_environment() {
        let home = Path::new("/Users/roy");
        assert_eq!(
            socket_path(Some("/tmp/h.sock"), home),
            PathBuf::from("/tmp/h.sock")
        );
        let default = socket_path(None, home);
        assert!(default.ends_with("com.drake0306.hairline/shdc.sock"));
        if cfg!(target_os = "macos") {
            assert_eq!(
                default,
                PathBuf::from(
                    "/Users/roy/Library/Application Support/com.drake0306.hairline/shdc.sock"
                )
            );
        }
    }

    #[test]
    fn state_prints_one_line_per_control() {
        let state = json!({
            "connected": true,
            "model": "WH-ULT900N",
            "battery": {"value": [80, "Charging"], "status": "Confirmed"},
            "nc": {"value": {"mode": "Ambient", "level": 5, "voice_focus": true}, "status": "Confirmed"},
            "eq_ult": {"value": {"preset": 22, "ult": "Ult2", "bands": []}, "status": "Confirmed"},
            "volume": {"value": 12, "status": "Confirmed"},
            "codec": {"value": "Ldac", "status": "Confirmed"},
        });
        assert_eq!(
            format_state(&state),
            "Headphones connected, WH-ULT900N\n\
             Battery    80 % (charging)\n\
             Noise      ambient, level 5, voice focus on\n\
             ULT        2\n\
             Volume     12 / 30\n\
             Codec      LDAC"
        );
    }

    #[test]
    fn state_blanks_what_is_unknown() {
        let state = json!({
            "connected": false,
            "battery": {"value": null, "status": "Unavailable"},
            "codec": {"value": {"Other": 7}, "status": "Confirmed"},
        });
        assert_eq!(
            format_state(&state),
            "Headphones not connected\n\
             Battery    --\n\
             Noise      --\n\
             ULT        --\n\
             Volume     --\n\
             Codec      other (0x07)"
        );
    }

    #[test]
    fn battery_prints_time_left_or_says_why_not() {
        assert_eq!(
            format_battery(&json!({"minutes_left": 320, "samples": 9, "since": 1})),
            "about 5 h 20 m left"
        );
        assert_eq!(
            format_battery(&json!({"minutes_left": 40, "samples": 9, "since": 1})),
            "about 40 m left"
        );
        assert_eq!(
            format_battery(&json!({"minutes_left": null, "samples": 0, "since": null})),
            "not enough history yet"
        );
    }

    #[test]
    fn scenes_mark_the_active_one() {
        let answer = json!({
            "ok": true,
            "scenes": [
                {"id": "a", "name": "Commute", "enabled": true},
                {"id": "b", "name": "Focus", "enabled": false},
            ],
            "active": "a",
        });
        assert_eq!(format_scenes(&answer), "* Commute\n  Focus (off)");
    }
}
