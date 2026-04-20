use std::{
    collections::HashMap,
    io::Cursor,
    path::Path,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use msbt::{builder::MsbtBuilder, Msbt};
use serde::Deserialize;
use smash_arc::Hash40;
use xml::common::Position;

use crate::{
    hashes,
    modfs::{registry::FileHandler, DiscoveryContext, ModFsError},
};

#[derive(Debug, Deserialize)]
struct Xmsbt {
    #[serde(rename = "entry")]
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    label: String,
    base64: Option<bool>,
    #[serde(rename = "text")]
    text: Text,
}

#[derive(Debug, Deserialize)]
struct Text {
    #[serde(rename = "$value")]
    value: String,
}

enum TextType {
    Text(String),
    Data(Vec<u8>),
}

struct ParsedPatch {
    entries: Vec<(String, TextType)>,
}

#[derive(Default)]
pub struct MsbtHandler {
    patches: HashMap<Hash40, Vec<ParsedPatch>>,
}

fn parse_patch(path: &Path) -> Option<ParsedPatch> {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(e) => {
            warn!("XMSBT file `{}` could not be read: {}", path.display(), e);
            return None;
        },
    };
    let xmsbt: Xmsbt = match serde_xml_rs::from_reader(Cursor::new(raw)) {
        Ok(x) => x,
        Err(err) => {
            match err {
                serde_xml_rs::Error::Syntax { source } => {
                    let position = source.position();
                    warn!(
                        "XMSBT file `{}` could not be read due to syntax error at line {}, column {}: `{}`, skipping.",
                        path.display(),
                        position.row + 1,
                        position.column,
                        source.msg()
                    );
                },
                _ => warn!("XMSBT file `{}` is malformed, skipping.", path.display()),
            }
            return None;
        },
    };

    let mut entries: Vec<(String, TextType)> = Vec::with_capacity(xmsbt.entries.len());
    for entry in xmsbt.entries {
        if entry.base64.unwrap_or(false) {
            match BASE64_STANDARD.decode::<String>(entry.text.value) {
                Ok(mut decoded) => {
                    decoded.push(0);
                    decoded.push(0);
                    entries.push((entry.label, TextType::Data(decoded)));
                },
                Err(err) => error!(
                    "XMSBT file `{}` label could not be base64 decoded. Reason: {}",
                    path.display(),
                    err
                ),
            }
        } else {
            entries.push((entry.label, TextType::Text(entry.text.value)));
        }
    }

    Some(ParsedPatch { entries })
}

impl FileHandler for MsbtHandler {
    fn name(&self) -> &'static str {
        "msbt"
    }

    fn patches_load(&self) -> bool {
        true
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["xmsbt"]
    }

    fn discover(&mut self, _ctx: &mut DiscoveryContext, full_path: &Path, local: &Path, _size: usize) -> Option<Hash40> {
        let base_local = local.with_extension("msbt");
        let (base_local, region) = super::strip_regional(&base_local);
        let is_current_region = match region {
            Some(r) => r == format!("{}", config::region()),
            None => true,
        };

        let hash = super::try_smash_hash(&base_local)?;

        if let Some(parsed) = parse_patch(full_path) {
            self.patches.entry(hash).or_default().push(parsed);
        }

        if let Some(s) = local.to_str() {
            hashes::add(s);
        }
        if is_current_region {
            if let Some(s) = base_local.to_str() {
                hashes::add(s);
            }
        }
        Some(hash)
    }

    fn apply(&self, hash: Hash40, bytes: Vec<u8>) -> Result<Vec<u8>, ModFsError> {
        let Some(patches) = self.patches.get(&hash) else {
            return Ok(bytes);
        };

        let mut labels: HashMap<String, &TextType> = HashMap::new();
        for patch in patches {
            for (label, text) in &patch.entries {
                labels.insert(label.clone(), text);
            }
        }

        let mut msbt = Msbt::from_reader(Cursor::new(&bytes))
            .map_err(|e| ModFsError::Handler(format!("failed to parse base msbt: {:?}", e)))?;

        for lbl in msbt
            .lbl1_mut()
            .ok_or_else(|| ModFsError::Handler("base msbt missing LBL1 section".to_string()))?
            .labels_mut()
        {
            let lbl_name = lbl.name().to_owned();
            if let Some(text_type) = labels.remove(&lbl_name) {
                let text_data = to_text_data(text_type);
                lbl.set_value_raw(text_data)
                    .map_err(|e| ModFsError::Handler(format!("failed to set label {}: {:?}", lbl_name, e)))?;
            }
        }

        let mut builder = MsbtBuilder::from(msbt);
        for (label, text_type) in labels {
            let text_data = to_text_data(text_type);
            builder = builder.add_label(label, text_data);
        }

        let out_msbt = builder.build();
        let mut cursor = Cursor::new(Vec::new());
        out_msbt
            .write_to(&mut cursor)
            .map_err(|e| ModFsError::Handler(format!("failed to write patched msbt: {:?}", e)))?;
        Ok(cursor.into_inner())
    }
}

fn to_text_data(text_type: &TextType) -> Vec<u8> {
    match text_type {
        TextType::Text(text) => {
            let mut str_val: Vec<u16> = text.encode_utf16().collect();
            str_val.push(0);
            let slice: &[u8] = unsafe { std::slice::from_raw_parts(str_val.as_ptr() as *const u8, str_val.len() * 2) };
            slice.to_vec()
        },
        TextType::Data(data) => data.clone(),
    }
}
