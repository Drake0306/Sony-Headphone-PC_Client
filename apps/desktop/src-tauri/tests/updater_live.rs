//! A live run of the update path, by hand, against the real feeds:
//!
//!     cargo test -p shdc --test updater_live -- --ignored --nocapture
//!
//! Set `SHDC_UPDATE_FEED` to try one feed. The mock app reports a version
//! every release beats, so the newest release is fetched, downloaded and
//! checked against the public key, exactly as the plugin does it in the app;
//! nothing is installed. Not part of the normal test run (it needs GitHub).
//! On Windows the test binary exits at start-up before the harness runs, even
//! with the test ignored, so it is compiled out there; macOS and Linux run it.

#![cfg(not(windows))]

use tauri_plugin_updater::UpdaterExt;

#[test]
#[ignore = "talks to GitHub; run by hand to prove a release's feed and download"]
fn live_feed_offers_a_downloadable_signed_release() {
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
    let updater_conf = &conf["plugins"]["updater"];
    // The plugin reads its config from the app's tauri.conf.json; the mock
    // context has none, so the real one is copied in.
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context
        .config_mut()
        .plugins
        .0
        .insert("updater".into(), updater_conf.clone());
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .expect("mock app");
    let configured: Vec<tauri::Url> = updater_conf["endpoints"]
        .as_array()
        .expect("endpoints")
        .iter()
        .map(|e| e.as_str().expect("url").parse().expect("valid url"))
        .collect();
    let endpoints = shdc_lib::updater::feed_override()
        .map(|u| vec![u])
        .unwrap_or(configured);
    let pubkey = updater_conf["pubkey"].as_str().expect("pubkey");
    println!("mock app version: {}", app.package_info().version);
    for e in &endpoints {
        println!("feed: {e}");
    }

    tauri::async_runtime::block_on(async {
        let updater = app
            .updater_builder()
            .endpoints(endpoints)
            .expect("endpoints accepted")
            .pubkey(pubkey)
            .build()
            .expect("updater");
        let update = updater
            .check()
            .await
            .expect("a feed answered")
            .expect("the feed names a release newer than the mock app");
        println!(
            "feed offers {} from {}",
            update.version, update.download_url
        );
        let mut seen = 0usize;
        let bytes = update
            .download(
                |chunk, total| {
                    seen += chunk;
                    if let Some(t) = total {
                        let _ = t;
                    }
                },
                || println!("download finished, checking the signature"),
            )
            .await
            .expect("downloaded and verified against the public key");
        println!("verified {} bytes ({seen} streamed)", bytes.len());
        assert!(
            bytes.len() > 1_000_000,
            "a real bundle is several megabytes"
        );
    });
}
