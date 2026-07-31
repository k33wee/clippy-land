use bytes::Bytes;

#[derive(Debug, Clone)]
pub enum ClipboardEntry {
    Text(String),
    Image {
        mime: String,
        bytes: Bytes,
        hash: u64,
        thumbnail_png: Option<Bytes>,
    },
}

#[derive(Debug, Clone)]
pub struct ClipboardThumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Bytes,
    pub png: Bytes,
}

impl ClipboardEntry {
    pub fn fingerprint(&self) -> ClipboardFingerprint {
        match self {
            ClipboardEntry::Text(text) => ClipboardFingerprint::Text(text.clone()),
            ClipboardEntry::Image {
                mime,
                bytes,
                hash,
                thumbnail_png: _,
            } => ClipboardFingerprint::Image {
                mime: mime.clone(),
                bytes_len: bytes.len(),
                hash: *hash,
            },
        }
    }
}

impl PartialEq for ClipboardEntry {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ClipboardEntry::Text(a), ClipboardEntry::Text(b)) => a == b,
            (
                ClipboardEntry::Image {
                    mime: am,
                    bytes: ab,
                    hash: ah,
                    ..
                },
                ClipboardEntry::Image {
                    mime: bm,
                    bytes: bb,
                    hash: bh,
                    ..
                },
            ) => ah == bh && am == bm && ab.len() == bb.len(),
            _ => false,
        }
    }
}

impl Eq for ClipboardEntry {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardFingerprint {
    Text(String),
    Image {
        mime: String,
        bytes_len: usize,
        hash: u64,
    },
}
