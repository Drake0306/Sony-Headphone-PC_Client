//! The updater's pure parts: version comparison, error mapping, the
//! download → install → relaunch order and the background check's silence.
//! The plugin's real check needs the network and a running app, so it is not
//! exercised here.

use shdc_lib::updater::{
    background_outcome, classify, classify_error, is_newer, next_step, Background, BackgroundLog,
    PercentGate, Phase, Progress, UpdateInfo,
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
    // A pre-release of the next version is newer than this release, but older
    // than its own final release.
    assert!(is_newer("0.2.0", "0.2.1-beta.1"));
    assert!(!is_newer("0.2.1", "0.2.1-beta.1"));
    assert!(!is_newer("0.2.0", "not a version"));
    assert!(!is_newer("garbage", "0.3.0"));
}

#[test]
fn update_without_valid_signature_is_refused() {
    // A real minisign check of tampered bytes against SHDC's key fails...
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
    // A signature for another version (a tampered feed) is refused too.
    assert_eq!(
        classify(&tauri_plugin_updater::Error::SignedVersionMismatch {
            signed: "0.2.1".into(),
            announced: "9.9.9".into(),
        }),
        not_verified()
    );
    assert_eq!(
        classify(&tauri_plugin_updater::Error::MissingSignedVersion),
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
            "Download request failed with status: 503 Service Unavailable".into()
        )),
        unreachable()
    );
    // The feed names a file the release does not have.
    let missing = Phase::Failed {
        error: "The update file is missing from the release".into(),
    };
    assert_eq!(
        classify(&tauri_plugin_updater::Error::Network(
            "Download request failed with status: 404 Not Found".into()
        )),
        missing
    );
    assert_eq!(
        classify_error("Download request failed with status: 404 Not Found"),
        missing
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
    let quiet = Background {
        emit: None,
        log: None,
        forget: false,
    };
    // The first failure logs one line and emits nothing...
    let first = background_outcome("0.2.0", offline(), &mut log);
    assert_eq!(first.emit, None);
    assert!(first.log.is_some());
    assert!(!first.forget);
    // ...later failures in the same streak log nothing more.
    assert_eq!(background_outcome("0.2.0", offline(), &mut log), quiet);
    // A newer release is offered.
    let found = background_outcome("0.2.0", Ok(Some(info("0.2.1"))), &mut log);
    assert_eq!(
        found.emit,
        Some(Phase::Available {
            version: "0.2.1".into(),
            notes: "Fixes".into(),
            date: Some("2026-11-08".into()),
        })
    );
    assert_eq!((found.log, found.forget), (None, false));
    // Up to date, or the same version: nothing is emitted (the background
    // never says "up to date"), and a release remembered earlier is forgotten.
    let gone = Background {
        emit: None,
        log: None,
        forget: true,
    };
    assert_eq!(background_outcome("0.2.0", Ok(None), &mut log), gone);
    assert_eq!(
        background_outcome("0.2.0", Ok(Some(info("0.2.0"))), &mut log),
        gone
    );
    // Failing again after a success logs once more.
    assert!(background_outcome("0.2.0", offline(), &mut log)
        .log
        .is_some());
}

#[test]
fn progress_is_sent_once_per_whole_percent() {
    let mut gate = PercentGate::default();
    let sent: Vec<u64> = [1u64, 5, 9, 10, 15, 20, 1000]
        .into_iter()
        .filter(|&d| gate.pass(d, Some(1000)))
        .collect();
    // 0 % once, then 1 % (10 bytes), 2 % (20), 100 %.
    assert_eq!(sent, [1, 10, 20, 1000]);
    // Without a size, one event per MiB.
    let mut gate = PercentGate::default();
    let mib = 1024 * 1024;
    let sent: Vec<u64> = [1u64, 500, mib, mib + 1, 3 * mib]
        .into_iter()
        .filter(|&d| gate.pass(d, None))
        .collect();
    assert_eq!(sent, [1, mib, 3 * mib]);
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
        Progress::DownloadFinished {
            downloaded: 90,
            total: Some(100),
        },
        Progress::Verified,
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
            // The bar reaches 100 % before the signature check...
            Phase::Downloading {
                downloaded: 100,
                total: Some(100)
            },
            Phase::Verifying,
            // ...and "Installing…" comes only after it passed.
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
        json(&Phase::Verifying),
        serde_json::json!({ "phase": "verifying" })
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
