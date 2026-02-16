use std::fs;
use std::path::Path;

use crate::error::JwwError;
use crate::reader::Reader;

pub const JWW_SIGNATURE: &[u8; 8] = b"JwwData.";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerHeader {
    pub state: u32,
    pub protect: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerGroupHeader {
    pub state: u32,
    pub write_layer: u32,
    pub scale: f64,
    pub protect: u32,
    pub layers: [LayerHeader; 16],
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JwwHeader {
    pub version: u32,
    pub memo: String,
    pub paper_size: u32,
    pub write_layer_group: u32,
    pub layer_groups: [LayerGroupHeader; 16],
}

pub fn is_jww_signature(data: &[u8]) -> bool {
    data.len() >= JWW_SIGNATURE.len() && &data[..JWW_SIGNATURE.len()] == JWW_SIGNATURE
}

pub fn parse_header(data: &[u8]) -> Result<JwwHeader, JwwError> {
    if !is_jww_signature(data) {
        return Err(JwwError::InvalidSignature);
    }

    let mut reader = Reader::new(data);
    reader.skip(JWW_SIGNATURE.len())?;

    let version = reader.read_u32()?;
    let memo = reader.read_cstring()?;
    let paper_size = reader.read_u32()?;
    let write_layer_group = reader.read_u32()?;

    let mut layer_groups = std::array::from_fn(|_| LayerGroupHeader {
        layers: std::array::from_fn(|_| LayerHeader::default()),
        ..LayerGroupHeader::default()
    });
    for group in &mut layer_groups {
        group.state = reader.read_u32()?;
        group.write_layer = reader.read_u32()?;
        group.scale = reader.read_f64()?;
        group.protect = reader.read_u32()?;

        for layer in &mut group.layers {
            layer.state = reader.read_u32()?;
            layer.protect = reader.read_u32()?;
        }
    }

    // Layer names and group names are stored later in the header block.
    // If this optional extraction fails, keep deterministic default names.
    if parse_layer_names(&mut reader, version, &mut layer_groups).is_err() {
        apply_default_layer_names(&mut layer_groups);
    } else {
        apply_default_layer_names_for_blanks(&mut layer_groups);
    }

    Ok(JwwHeader {
        version,
        memo,
        paper_size,
        write_layer_group,
        layer_groups,
    })
}

fn parse_layer_names(
    reader: &mut Reader<'_>,
    version: u32,
    layer_groups: &mut [LayerGroupHeader; 16],
) -> Result<(), JwwError> {
    // Only version >= 300 layout is currently supported for this section.
    if version < 300 {
        return Err(JwwError::UnexpectedEof("layer names"));
    }

    // Skip fields defined before layer names in jwdatafmt:
    // 14 dummy DWORD + 5 dimension DWORD + 1 dummy DWORD + max-draw-width DWORD.
    reader.skip((14 + 5 + 1 + 1) * 4)?;

    // Printer/memory settings before names:
    // printer origin(x,y) [16]
    // printer scale [8]
    // printer set [4]
    // memori mode [4]
    // memori min [8]
    // memori x/y [16]
    // memori origin x/y [16]
    reader.skip(16 + 8 + 4 + 4 + 8 + 16 + 16)?;

    for g in 0..16 {
        for l in 0..16 {
            layer_groups[g].layers[l].name = reader.read_cstring()?;
        }
    }

    for (g, group) in layer_groups.iter_mut().enumerate() {
        let _ = g;
        group.name = reader.read_cstring()?;
    }

    Ok(())
}

fn apply_default_layer_names(layer_groups: &mut [LayerGroupHeader; 16]) {
    for (g_idx, group) in layer_groups.iter_mut().enumerate() {
        group.name = format!("Group{:X}", g_idx);
        for (l_idx, layer) in group.layers.iter_mut().enumerate() {
            layer.name = format!("{:X}-{:X}", g_idx, l_idx);
        }
    }
}

fn apply_default_layer_names_for_blanks(layer_groups: &mut [LayerGroupHeader; 16]) {
    for (g_idx, group) in layer_groups.iter_mut().enumerate() {
        if group.name.is_empty() {
            group.name = format!("Group{:X}", g_idx);
        }
        for (l_idx, layer) in group.layers.iter_mut().enumerate() {
            if layer.name.is_empty() {
                layer.name = format!("{:X}-{:X}", g_idx, l_idx);
            }
        }
    }
}

pub fn read_header_from_file(path: impl AsRef<Path>) -> Result<JwwHeader, JwwError> {
    let data = fs::read(path)?;
    parse_header(&data)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{is_jww_signature, parse_header, read_header_from_file, JwwError};

    fn jww_samples_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("jww_samples")
    }

    #[test]
    fn signature_check() {
        assert!(is_jww_signature(b"JwwData.\x00\x00"));
        assert!(!is_jww_signature(b"NotJwwData"));
    }

    #[test]
    fn invalid_signature_is_rejected() {
        let err = parse_header(b"NotJwwData").unwrap_err();
        assert!(matches!(err, JwwError::InvalidSignature));
    }

    #[test]
    fn parse_all_jww_sample_headers() {
        let dir = jww_samples_dir();
        assert!(
            dir.exists(),
            "jww_samples directory is required for this test: {}",
            dir.display()
        );

        let mut files = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext == "jww").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();

        assert!(
            !files.is_empty(),
            "no .jww files found in {}",
            dir.display()
        );

        for path in files {
            let header = read_header_from_file(&path)
                .unwrap_or_else(|e| panic!("failed parsing {}: {e}", path.display()));
            assert_eq!(
                header.version,
                600,
                "unexpected version in {}",
                path.display()
            );
            assert_eq!(header.layer_groups.len(), 16);
            for group in &header.layer_groups {
                assert_eq!(group.layers.len(), 16);
                assert!(
                    !group.name.is_empty(),
                    "group name should not be empty in {}",
                    path.display()
                );
                for layer in &group.layers {
                    assert!(
                        !layer.name.is_empty(),
                        "layer name should not be empty in {}",
                        path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn extracts_non_default_layer_names_when_present() {
        let path = jww_samples_dir().join("Ａマンション平面例.jww");
        if !path.exists() {
            return;
        }

        let header = read_header_from_file(&path).unwrap();
        let group0 = &header.layer_groups[0];
        let layer0 = &group0.layers[0];

        assert_ne!(group0.name, "Group0");
        assert_ne!(layer0.name, "0-0");
    }
}
