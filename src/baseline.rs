// 2026-09-24: Grandfathered comments, fingerprinted by path plus a hash of
// the normalised comment text, so a baseline survives reformatting but not
// a change to what the comment says.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BASELINE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub path: String,
    pub fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineFile {
    pub version: u32,
    pub entries: Vec<BaselineEntry>,
}

pub struct Baseline {
    keys: HashSet<(String, String)>,
}

pub fn fingerprint(path: &str, text: &str) -> String {
    let normalised = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut h = Sha256::new();
    h.update(path.as_bytes());
    h.update([0u8]);
    h.update(normalised.as_bytes());
    format!("{:x}", h.finalize())
}

impl Baseline {
    pub fn parse(json: &str) -> Result<Self, String> {
        let file: BaselineFile =
            serde_json::from_str(json).map_err(|e| format!("baseline: invalid JSON: {e}"))?;
        if file.version != BASELINE_VERSION {
            return Err(format!(
                "baseline: unsupported version {} (expected {BASELINE_VERSION})",
                file.version
            ));
        }
        Ok(Baseline {
            keys: file
                .entries
                .into_iter()
                .map(|e| (e.path, e.fingerprint))
                .collect(),
        })
    }

    pub fn contains(&self, path: &str, text: &str) -> bool {
        self.keys
            .contains(&(path.to_string(), fingerprint(path, text)))
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

pub fn render(entries: Vec<BaselineEntry>) -> String {
    let file = BaselineFile {
        version: BASELINE_VERSION,
        entries,
    };
    serde_json::to_string_pretty(&file).expect("baseline serialises") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_ignores_whitespace_but_not_words_or_path() {
        let a = fingerprint("a.rs", "hello   world\n  again");
        assert_eq!(a, fingerprint("a.rs", "hello world again"));
        assert_ne!(a, fingerprint("a.rs", "hello world again!"));
        assert_ne!(a, fingerprint("b.rs", "hello world again"));
    }

    #[test]
    fn round_trip_and_version_check() {
        let json = render(vec![BaselineEntry {
            path: "x.py".into(),
            fingerprint: fingerprint("x.py", "old note"),
            line: Some(3),
            excerpt: Some("old note".into()),
        }]);
        let b = Baseline::parse(&json).unwrap();
        assert!(b.contains("x.py", "old  note"));
        assert!(!b.contains("x.py", "new note"));
        assert!(!b.contains("y.py", "old note"));
        assert_eq!(b.len(), 1);
        assert!(Baseline::parse(r#"{"version": 2, "entries": []}"#).is_err());
        assert!(Baseline::parse("nope").is_err());
        assert!(
            Baseline::parse(r#"{"version": 1, "entries": []}"#)
                .unwrap()
                .is_empty()
        );
    }
}
