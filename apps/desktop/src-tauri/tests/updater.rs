//! The updater's pure parts: version comparison, error mapping, the
//! download → install → relaunch order and the background check's silence.
//! The plugin's real check needs the network and a running app, so it is not
//! exercised here.

use hairline_lib::updater::{
    background_outcome, classify, classify_error, is_newer, next_step, BackgroundLog, Phase,
    Progress, UpdateInfo,
};

/// The public key built into the app (tauri.conf.json `plugins.updater.pubkey`).
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFCMDk2QTVGMzJDQ0VENkUKUldSdTdjd3lYMm9KR3dQY3ZFUWVUelpkZDdpQ3FwRk44MHFjcDMvd25ZMkZMcmVuK2N4SVZNbUQK";

fn not_verified() -> Phase {
    Phase::Failed {
        error: "Update could not be verified".into(),
    }
}

fn unreachable() -> Phase {
    Phase::Failed {
        error: "Could not reach GitHub".into(),
    }
}

fn info(version: &str) -> UpdateInfo {
    UpdateInfo {
        version: version.into(),
        notes: "Fixes".into(),
        date: Some("2026-11-08".into()),
    }
}

#[test]
fn newer_is_a_semver_comparison() {
    assert!(is_newer("0.2.0", "0.2.1"));
    assert!(is_newer("0.2.0", "0.10.0"));
    assert!(is_newer("0.2.0", "v1.0.0"));
    assert!(!is_newer("0.2.0", "0.2.0"));
    assert!(!is_newer("0.2.1", "0.2.0"));
    assert!(!is_newer("0.2.0", "not a version"));
    assert!(!is_newer("garbage", "0.3.0"));
}

#[test]
fn update_without_valid_signature_is_refused() {
    // A real minisign check of tampered bytes against Hairline's key fails...
    let key_text = String::from_utf8(base64_decode(PUBKEY)).unwrap();
    let key = minisign_verify::PublicKey::decode(&key_text).unwrap();
    // A well-formed signature made by some other key for other bytes.
    let other_sig = "untrusted comment: signature from minisign secret key\n\
        RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\n\
        trusted comment: timestamp:1633700835\tfile:test\tprehashed\n\
        wLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==";
    let sig = minisign_verify::Signature::decode(other_sig).unwrap();
    let err = key.verify(b"tampered bundle", &sig, true).unwrap_err();
    // ...and the plugin's error for it maps to "not verified", never to an install.
    let phase = classify(&tauri_plugin_updater::Error::Minisign(err));
    assert_eq!(phase, not_verified());
    let step = next_step(Progress::Installed(Err(
        tauri_plugin_updater::Error::Minisign(minisign_verify::Error::InvalidSignature).to_string(),
    )));
    assert_eq!(step.emit, [not_verified()]);
    assert!(!step.restart);

    // An unsigned or garbled signature is refused the same way.
    assert_eq!(
        classify(&tauri_plugin_updater::Error::SignatureUtf8("x".into())),
        not_verified()
    );
    assert_eq!(
        classify_error("The signature verification failed"),
        not_verified()
    );
    assert_eq!(
        classify_error("The signature was created with a different key than the one provided"),
        not_verified()
    );
}

#[test]
fn network_errors_read_as_github_unreachable() {
    assert_eq!(
        classify(&tauri_plugin_updater::Error::ReleaseNotFound),
        unreachable()
    );
    assert_eq!(
        classify(&tauri_plugin_updater::Error::Network(
            "Download request failed with status: 404".into()
        )),
        unreachable()
    );
    assert_eq!(
        classify_error("error sending request for url (https://github.com/x): dns error"),
        unreachable()
    );
    // Anything else keeps its own text.
    assert_eq!(
        classify_error("temp directory is not on the same mount point as the app"),
        Phase::Failed {
            error: "Update failed: temp directory is not on the same mount point as the app".into()
        }
    );
}

#[test]
fn offline_check_is_silent() {
    let mut log = BackgroundLog::default();
    let offline = || Err("error sending request for url (https://github.com)".to_string());
    // The first failure logs one line and emits nothing...
    let (emit, line) = background_outcome("0.2.0", offline(), &mut log);
    assert_eq!(emit, None);
    assert!(line.is_some());
    // ...later failures in the same streak log nothing more.
    let (emit, line) = background_outcome("0.2.0", offline(), &mut log);
    assert_eq!((emit, line), (None, None));
    // Up to date: still nothing (the background never says "up to date").
    assert_eq!(
        background_outcome("0.2.0", Ok(None), &mut log),
        (None, None)
    );
    // A newer release is offered.
    let (emit, line) = background_outcome("0.2.0", Ok(Some(info("0.2.1"))), &mut log);
    assert_eq!(
        emit,
        Some(Phase::Available {
            version: "0.2.1".into(),
            notes: "Fixes".into(),
            date: Some("2026-11-08".into()),
        })
    );
    assert_eq!(line, None);
    // The same or an older version is not.
    assert_eq!(
        background_outcome("0.2.0", Ok(Some(info("0.2.0"))), &mut log),
        (None, None)
    );
    // Failing again after a success logs once more.
    assert!(background_outcome("0.2.0", offline(), &mut log).1.is_some());
}

#[test]
fn install_then_relaunch_order() {
    let mut emitted = Vec::new();
    let mut restarts = Vec::new();
    for p in [
        Progress::Chunk {
            downloaded: 10,
            total: Some(100),
        },
        Progress::Chunk {
            downloaded: 60,
            total: Some(100),
        },
        Progress::DownloadFinished {
            downloaded: 90,
            total: Some(100),
        },
        Progress::Installed(Ok(())),
    ] {
        let step = next_step(p);
        restarts.push(step.restart);
        emitted.extend(step.emit);
    }
    assert_eq!(
        emitted,
        [
            Phase::Downloading {
                downloaded: 10,
                total: Some(100)
            },
            Phase::Downloading {
                downloaded: 60,
                total: Some(100)
            },
            // The bar reaches 100 % before "Installing…".
            Phase::Downloading {
                downloaded: 100,
                total: Some(100)
            },
            Phase::Installing,
            Phase::Restarting,
        ]
    );
    // Only a successful install relaunches.
    assert_eq!(restarts, [false, false, false, true]);
    let failed = next_step(Progress::Installed(Err("disk full".into())));
    assert!(!failed.restart);
    assert!(!failed.emit.contains(&Phase::Restarting));
    // A chunk never reports more than the total.
    assert_eq!(
        next_step(Progress::Chunk {
            downloaded: 120,
            total: Some(100)
        })
        .emit,
        [Phase::Downloading {
            downloaded: 100,
            total: Some(100)
        }]
    );
}

#[test]
fn phases_serialise_with_a_phase_tag() {
    let json = |p: &Phase| serde_json::to_value(p).unwrap();
    assert_eq!(
        json(&Phase::Downloading {
            downloaded: 5,
            total: None
        }),
        serde_json::json!({ "phase": "downloading", "downloaded": 5, "total": null })
    );
    assert_eq!(
        json(&Phase::UpToDate),
        serde_json::json!({ "phase": "up_to_date" })
    );
    assert_eq!(
        json(&Phase::Installing),
        serde_json::json!({ "phase": "installing" })
    );
    assert_eq!(
        json(&not_verified()),
        serde_json::json!({ "phase": "failed", "error": "Update could not be verified" })
    );
    assert_eq!(
        json(&Phase::Available {
            version: "0.2.1".into(),
            notes: "n".into(),
            date: None
        }),
        serde_json::json!({ "phase": "available", "version": "0.2.1", "notes": "n", "date": null })
    );
}

fn base64_decode(s: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|&c| c != b'=') {
        acc = (acc << 6) | ALPHABET.iter().position(|&a| a == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}
