//! Per-unit device records: `devices/<normalised address>.json` beside
//! `settings.json`, holding the functions proven to work on that unit and the
//! RFCOMM channel its v2 link last ran on.

use std::io;
use std::path::{Path, PathBuf};

use mdr_proto::Table;
use mdr_session::DeviceState;
use serde::{Deserialize, Serialize};

const DIR_NAME: &str = "devices";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RecordFile", into = "RecordFile")]
pub struct DeviceRecord {
    /// Functions this unit answers without declaring them.
    pub proven: Vec<(Table, u8)>,
    /// RFCOMM channel of the last successful connect. Linux tries it before
    /// scanning (BlueZ has no SDP browsing over D-Bus); elsewhere it is kept
    /// for information only.
    pub rfcomm_channel: Option<u8>,
}

/// The file layout:
/// `{ "proven": [{ "table": "T1", "function": 80 }], "rfcomm_channel": 18 }`.
#[derive(Serialize, Deserialize)]
struct RecordFile {
    #[serde(default)]
    proven: Vec<ProvenEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rfcomm_channel: Option<u8>,
}

#[derive(Serialize, Deserialize)]
struct ProvenEntry {
    table: Table,
    function: u8,
}

impl From<RecordFile> for DeviceRecord {
    fn from(f: RecordFile) -> Self {
        DeviceRecord {
            proven: f
                .proven
                .into_iter()
                .map(|e| (e.table, e.function))
                .collect(),
            rfcomm_channel: f.rfcomm_channel,
        }
    }
}

impl From<DeviceRecord> for RecordFile {
    fn from(r: DeviceRecord) -> Self {
        RecordFile {
            proven: r
                .proven
                .into_iter()
                .map(|(table, function)| ProvenEntry { table, function })
                .collect(),
            rfcomm_channel: r.rfcomm_channel,
        }
    }
}

/// The record's path: the address's hex digits, lowercased, so `AC:80:…` and
/// `ac-80-…` share a file and nothing else can reach the path. `None` when
/// the address has no hex digits.
fn path_for(dir: &Path, address: &str) -> Option<PathBuf> {
    let name: String = address
        .chars()
        .filter(char::is_ascii_hexdigit)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (!name.is_empty()).then(|| dir.join(DIR_NAME).join(format!("{name}.json")))
}

/// The record for `address` in `dir`; empty when missing or unreadable.
pub fn load(dir: &Path, address: &str) -> DeviceRecord {
    path_for(dir, address)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes the record as pretty JSON by way of a `.tmp` file, so a crash
/// cannot leave half a file.
pub fn save(dir: &Path, address: &str, record: &DeviceRecord) -> io::Result<()> {
    let path = path_for(dir, address).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("address {address:?} has no hex digits"),
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(record).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)
}

/// Records the channel the link runs on when it differs from the stored one,
/// keeping the rest of the record. Returns whether the file was written.
pub fn remember_channel(dir: &Path, address: &str, channel: Option<u8>) -> io::Result<bool> {
    let Some(channel) = channel else {
        return Ok(false);
    };
    let mut record = load(dir, address);
    if record.rfcomm_channel == Some(channel) {
        return Ok(false);
    }
    record.rfcomm_channel = Some(channel);
    save(dir, address, &record).map(|()| true)
}

/// One row of the Advanced screen's capability list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityRow {
    pub table: Table,
    pub id: u8,
    /// `None` when the id is not in the name table (shown as `--`).
    pub name: Option<String>,
    pub declared: bool,
    pub proven: bool,
}

/// Every declared or proven function, T1 then T2, by id.
pub fn capability_rows(state: &DeviceState) -> Vec<CapabilityRow> {
    let mut rows = Vec::new();
    for (table, declared) in [
        (Table::T1, &state.capabilities_t1),
        (Table::T2, &state.capabilities_t2),
    ] {
        let proven = state
            .proven
            .iter()
            .filter(|(t, _)| *t == table)
            .map(|&(_, id)| id);
        let mut ids: Vec<u8> = declared.iter().copied().chain(proven).collect();
        ids.sort_unstable();
        ids.dedup();
        rows.extend(ids.into_iter().map(|id| CapabilityRow {
            table,
            id,
            name: mdr_proto::function::name(table, id).map(str::to_owned),
            declared: declared.contains(&id),
            proven: state.proven.contains(&(table, id)),
        }));
    }
    rows
}
