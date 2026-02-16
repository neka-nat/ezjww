use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use crate::error::JwwError;
use crate::header::parse_header;
use crate::model::{
    Arc, Block, BlockDef, Dimension, Entity, EntityBase, JwwDocument, Line, Point, Solid, Text,
};
use crate::reader::Reader;

pub fn parse_document(data: &[u8]) -> Result<JwwDocument, JwwError> {
    let header = parse_header(data)?;
    let entity_list_offset =
        find_entity_list_offset(data, header.version).ok_or(JwwError::EntityListNotFound)?;
    let mut reader = Reader::new(&data[entity_list_offset..]);
    let entities = parse_entity_list(&mut reader, header.version)?;
    let block_data_start = entity_list_offset + reader.bytes_read();
    let block_defs = if block_data_start < data.len() {
        parse_block_def_list(&data[block_data_start..], header.version)
    } else {
        Vec::new()
    };
    Ok(JwwDocument {
        header,
        entities,
        block_defs,
    })
}

pub fn read_document_from_file(path: impl AsRef<Path>) -> Result<JwwDocument, JwwError> {
    let data = fs::read(path)?;
    parse_document(&data)
}

fn find_entity_list_offset(data: &[u8], version: u32) -> Option<usize> {
    let [schema_low, schema_high, _, _] = version.to_le_bytes();
    if data.len() < 128 {
        return None;
    }

    let mut i = 100usize;
    while i + 20 < data.len() {
        if data[i] == 0xFF
            && data[i + 1] == 0xFF
            && data[i + 2] == schema_low
            && data[i + 3] == schema_high
        {
            let name_len = u16::from_le_bytes([data[i + 4], data[i + 5]]) as usize;
            if (8..=32).contains(&name_len) && i + 6 + name_len <= data.len() {
                let class_name = &data[i + 6..i + 6 + name_len];
                if class_name.starts_with(b"CData") && i >= 2 {
                    return Some(i - 2);
                }
            }
        }
        i += 1;
    }
    None
}

fn parse_entity_list(reader: &mut Reader<'_>, version: u32) -> Result<Vec<Entity>, JwwError> {
    let count = reader.read_u16()? as usize;
    let mut entities = Vec::with_capacity(count);

    let mut pid_to_class_name = HashMap::<u32, String>::new();
    let mut next_pid: u32 = 1;

    for _ in 0..count {
        let (entity, new_pid) =
            parse_entity_with_pid_tracking(reader, version, &mut pid_to_class_name, next_pid)?;
        next_pid = new_pid;
        if let Some(entity) = entity {
            entities.push(entity);
        }
    }

    Ok(entities)
}

fn parse_entity_with_pid_tracking(
    reader: &mut Reader<'_>,
    version: u32,
    pid_to_class_name: &mut HashMap<u32, String>,
    mut next_pid: u32,
) -> Result<(Option<Entity>, u32), JwwError> {
    let class_id = reader.read_u16()?;

    let class_name = if class_id == 0xFFFF {
        let _schema_version = reader.read_u16()?;
        let name_len = reader.read_u16()? as usize;
        let name = String::from_utf8_lossy(&reader.read_bytes(name_len)?).to_string();
        pid_to_class_name.insert(next_pid, name.clone());
        next_pid += 1;
        name
    } else if class_id == 0x8000 {
        return Ok((None, next_pid));
    } else {
        let class_pid = (class_id & 0x7FFF) as u32;
        pid_to_class_name
            .get(&class_pid)
            .cloned()
            .ok_or(JwwError::UnknownClassPid(class_pid))?
    };

    let entity = match class_name.as_str() {
        "CDataSen" => Some(Entity::Line(parse_line(reader, version)?)),
        "CDataEnko" => Some(Entity::Arc(parse_arc(reader, version)?)),
        "CDataTen" => Some(Entity::Point(parse_point(reader, version)?)),
        "CDataMoji" => Some(Entity::Text(parse_text(reader, version)?)),
        "CDataSolid" => Some(Entity::Solid(parse_solid(reader, version)?)),
        "CDataBlock" => Some(Entity::Block(parse_block(reader, version)?)),
        "CDataSunpou" => Some(Entity::Dimension(parse_dimension(reader, version)?)),
        _ => return Err(JwwError::UnknownEntityClass(class_name)),
    };

    next_pid += 1;
    Ok((entity, next_pid))
}

fn parse_entity_base(reader: &mut Reader<'_>, version: u32) -> Result<EntityBase, JwwError> {
    let group = reader.read_u32()?;
    let pen_style = reader.read_u8()?;
    let pen_color = reader.read_u16()?;
    let pen_width = if version >= 351 {
        reader.read_u16()?
    } else {
        0
    };
    let layer = reader.read_u16()?;
    let layer_group = reader.read_u16()?;
    let flag = reader.read_u16()?;

    Ok(EntityBase {
        group,
        pen_style,
        pen_color,
        pen_width,
        layer,
        layer_group,
        flag,
    })
}

fn parse_line(reader: &mut Reader<'_>, version: u32) -> Result<Line, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Line {
        base,
        start_x: reader.read_f64()?,
        start_y: reader.read_f64()?,
        end_x: reader.read_f64()?,
        end_y: reader.read_f64()?,
    })
}

fn parse_arc(reader: &mut Reader<'_>, version: u32) -> Result<Arc, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Arc {
        base,
        center_x: reader.read_f64()?,
        center_y: reader.read_f64()?,
        radius: reader.read_f64()?,
        start_angle: reader.read_f64()?,
        arc_angle: reader.read_f64()?,
        tilt_angle: reader.read_f64()?,
        flatness: reader.read_f64()?,
        is_full_circle: reader.read_u32()? != 0,
    })
}

fn parse_point(reader: &mut Reader<'_>, version: u32) -> Result<Point, JwwError> {
    let base = parse_entity_base(reader, version)?;
    let x = reader.read_f64()?;
    let y = reader.read_f64()?;
    let is_temporary = reader.read_u32()? != 0;

    let (code, angle, scale) = if base.pen_style == 100 {
        (reader.read_u32()?, reader.read_f64()?, reader.read_f64()?)
    } else {
        (0, 0.0, 0.0)
    };

    Ok(Point {
        base,
        x,
        y,
        is_temporary,
        code,
        angle,
        scale,
    })
}

fn parse_text(reader: &mut Reader<'_>, version: u32) -> Result<Text, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Text {
        base,
        start_x: reader.read_f64()?,
        start_y: reader.read_f64()?,
        end_x: reader.read_f64()?,
        end_y: reader.read_f64()?,
        text_type: reader.read_u32()?,
        size_x: reader.read_f64()?,
        size_y: reader.read_f64()?,
        spacing: reader.read_f64()?,
        angle: reader.read_f64()?,
        font_name: reader.read_cstring()?,
        content: reader.read_cstring()?,
    })
}

fn parse_solid(reader: &mut Reader<'_>, version: u32) -> Result<Solid, JwwError> {
    let base = parse_entity_base(reader, version)?;
    let point1_x = reader.read_f64()?;
    let point1_y = reader.read_f64()?;
    let point4_x = reader.read_f64()?;
    let point4_y = reader.read_f64()?;
    let point2_x = reader.read_f64()?;
    let point2_y = reader.read_f64()?;
    let point3_x = reader.read_f64()?;
    let point3_y = reader.read_f64()?;
    let color = if base.pen_color == 10 {
        Some(reader.read_u32()?)
    } else {
        None
    };

    Ok(Solid {
        base,
        point1_x,
        point1_y,
        point2_x,
        point2_y,
        point3_x,
        point3_y,
        point4_x,
        point4_y,
        color,
    })
}

fn parse_block(reader: &mut Reader<'_>, version: u32) -> Result<Block, JwwError> {
    let base = parse_entity_base(reader, version)?;
    Ok(Block {
        base,
        ref_x: reader.read_f64()?,
        ref_y: reader.read_f64()?,
        scale_x: reader.read_f64()?,
        scale_y: reader.read_f64()?,
        rotation: reader.read_f64()?,
        def_number: reader.read_u32()?,
    })
}

fn parse_dimension(reader: &mut Reader<'_>, version: u32) -> Result<Dimension, JwwError> {
    let base = parse_entity_base(reader, version)?;
    let line = parse_line(reader, version)?;
    let text = parse_text(reader, version)?;

    let mut sxf_mode = None;
    let mut aux_lines = Vec::new();
    let mut aux_points = Vec::new();
    if version >= 420 {
        sxf_mode = Some(reader.read_u16()?);
        for _ in 0..2 {
            aux_lines.push(parse_line(reader, version)?);
        }
        for _ in 0..4 {
            aux_points.push(parse_point(reader, version)?);
        }
    }

    Ok(Dimension {
        base,
        line,
        text,
        sxf_mode,
        aux_lines,
        aux_points,
    })
}

fn parse_block_def_list(data: &[u8], version: u32) -> Vec<BlockDef> {
    let mut reader = Reader::new(data);
    let count = match reader.read_u32() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    if count > 10_000 {
        return Vec::new();
    }

    let mut block_defs = Vec::<BlockDef>::with_capacity(count as usize);
    let mut class_map = HashMap::<u16, String>::new();
    let mut next_id = 1u16;

    for _ in 0..count {
        let parsed = parse_block_def_with_tracking(&mut reader, version, &mut class_map, next_id);
        let (block_def, new_next_id) = match parsed {
            Ok(v) => v,
            Err(_) => break,
        };
        next_id = new_next_id;
        if let Some(block_def) = block_def {
            block_defs.push(block_def);
        }
    }

    block_defs
}

fn parse_block_def_with_tracking(
    reader: &mut Reader<'_>,
    version: u32,
    class_map: &mut HashMap<u16, String>,
    mut next_id: u16,
) -> Result<(Option<BlockDef>, u16), JwwError> {
    let class_id = reader.read_u16()?;
    if class_id == 0xFFFF {
        let _schema = reader.read_u16()?;
        let name_len = reader.read_u16()? as usize;
        let class_name = String::from_utf8_lossy(&reader.read_bytes(name_len)?).to_string();
        class_map.insert(next_id, class_name);
        next_id = next_id.saturating_add(1);
    } else if class_id == 0x8000 {
        return Ok((None, next_id));
    }

    let base = parse_entity_base(reader, version)?;
    let number = reader.read_u32()?;
    let is_referenced = reader.read_u32()? != 0;
    reader.skip(4)?; // CTime
    let name = reader.read_cstring()?;

    let entities = parse_entity_list(reader, version).unwrap_or_default();

    Ok((
        Some(BlockDef {
            base,
            number,
            is_referenced,
            name,
            entities,
        }),
        next_id,
    ))
}

pub fn entity_counts(entities: &[Entity]) -> HashMap<&'static str, usize> {
    let mut counts = HashMap::<&'static str, usize>::new();
    for entity in entities {
        *counts.entry(entity.entity_type()).or_insert(0) += 1;
    }
    counts
}

pub fn block_def_name_map(block_defs: &[BlockDef]) -> HashMap<u32, String> {
    let mut map = HashMap::<u32, String>::with_capacity(block_defs.len());
    for block_def in block_defs {
        map.insert(block_def.number, block_def.name.clone());
    }
    map
}

pub fn resolve_block_name<'a>(def_number: u32, block_defs: &'a [BlockDef]) -> Option<&'a str> {
    block_defs
        .iter()
        .find(|def| def.number == def_number)
        .map(|def| def.name.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockReferenceValidation {
    pub total_references: usize,
    pub resolved_references: usize,
    pub unresolved_def_numbers: Vec<u32>,
}

impl BlockReferenceValidation {
    pub fn has_unresolved(&self) -> bool {
        !self.unresolved_def_numbers.is_empty()
    }
}

pub fn validate_block_references(document: &JwwDocument) -> BlockReferenceValidation {
    let mut ref_numbers = Vec::<u32>::new();
    collect_block_ref_numbers(&document.entities, &mut ref_numbers);
    for block_def in &document.block_defs {
        collect_block_ref_numbers(&block_def.entities, &mut ref_numbers);
    }

    let total_references = ref_numbers.len();
    let name_map = block_def_name_map(&document.block_defs);

    let mut resolved_references = 0usize;
    let mut unresolved = BTreeSet::<u32>::new();
    for def_number in ref_numbers {
        if name_map.contains_key(&def_number) {
            resolved_references += 1;
        } else {
            unresolved.insert(def_number);
        }
    }

    BlockReferenceValidation {
        total_references,
        resolved_references,
        unresolved_def_numbers: unresolved.into_iter().collect(),
    }
}

fn collect_block_ref_numbers(entities: &[Entity], out: &mut Vec<u32>) {
    for entity in entities {
        if let Entity::Block(block) = entity {
            out.push(block.def_number);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use crate::model::{BlockDef, Entity, EntityBase};

    use super::{
        block_def_name_map, entity_counts, read_document_from_file, resolve_block_name,
        validate_block_references, JwwError,
    };

    fn jww_samples_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("jww_samples")
    }

    #[test]
    fn parse_all_jww_samples() {
        let dir = jww_samples_dir();
        let mut files = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext == "jww").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();

        for path in files {
            let doc = read_document_from_file(&path)
                .unwrap_or_else(|e| panic!("failed parsing {}: {e}", path.display()));
            assert_eq!(doc.header.version, 600);
            assert!(
                !doc.entities.is_empty(),
                "no entities in {}",
                path.display()
            );
        }
    }

    #[test]
    fn real_data_scan_nested_dimensions_in_block_defs() {
        let dir = jww_samples_dir();
        let mut files = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|ext| ext == "jww").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();

        let mut files_scanned = 0usize;
        let mut files_with_block_defs = 0usize;
        let mut files_with_nested_dimension = 0usize;

        for path in files {
            let doc = read_document_from_file(&path)
                .unwrap_or_else(|e| panic!("failed parsing {}: {e}", path.display()));
            files_scanned += 1;

            if !doc.block_defs.is_empty() {
                files_with_block_defs += 1;
            }

            let mut nested_dim_count = 0usize;
            for block_def in &doc.block_defs {
                for entity in &block_def.entities {
                    if let Entity::Dimension(dim) = entity {
                        nested_dim_count += 1;
                        assert!(
                            dim.text.size_x >= 0.0 && dim.text.size_y >= 0.0,
                            "invalid dimension text size in {}",
                            path.display()
                        );
                    }
                }
            }
            if nested_dim_count > 0 {
                files_with_nested_dimension += 1;
            }
        }

        assert!(files_scanned > 0, "no files scanned");
        eprintln!(
            "real_data_scan: files={}, with_block_defs={}, with_nested_dimension={}",
            files_scanned, files_with_block_defs, files_with_nested_dimension
        );
    }

    #[test]
    fn parse_shikichizu_expected_counts() {
        let path = jww_samples_dir().join("敷地図.jww");
        let doc = read_document_from_file(&path).unwrap();
        let counts = entity_counts(&doc.entities);

        assert_eq!(counts.get("LINE").copied().unwrap_or(0), 9);
        assert_eq!(counts.get("ARC").copied().unwrap_or(0), 0);
        assert_eq!(counts.get("POINT").copied().unwrap_or(0), 0);
        assert_eq!(counts.get("TEXT").copied().unwrap_or(0), 0);
    }

    #[test]
    fn invalid_signature_returns_error() {
        let err = super::parse_document(b"NotJwwData").unwrap_err();
        assert!(matches!(err, JwwError::InvalidSignature));
    }

    #[test]
    fn parse_minimal_with_block_def() {
        let data = build_minimal_jww_with_block_def();
        let doc = super::parse_document(&data).unwrap();
        assert_eq!(doc.entities.len(), 1);
        assert_eq!(doc.block_defs.len(), 1);

        match &doc.entities[0] {
            Entity::Block(block) => assert_eq!(block.def_number, 1),
            other => panic!("expected BLOCK entity, got {:?}", other),
        }

        let def = &doc.block_defs[0];
        assert_eq!(def.number, 1);
        assert!(!def.is_referenced);
        assert_eq!(def.name, "BLK");
        assert_eq!(resolve_block_name(1, &doc.block_defs), Some("BLK"));

        let validation = validate_block_references(&doc);
        assert_eq!(validation.total_references, 1);
        assert_eq!(validation.resolved_references, 1);
        assert!(validation.unresolved_def_numbers.is_empty());
        assert!(!validation.has_unresolved());
    }

    #[test]
    fn block_def_map_works() {
        let defs = vec![
            BlockDef {
                base: EntityBase::default(),
                number: 3,
                is_referenced: false,
                name: "A".to_string(),
                entities: vec![],
            },
            BlockDef {
                base: EntityBase::default(),
                number: 7,
                is_referenced: true,
                name: "B".to_string(),
                entities: vec![],
            },
        ];

        let map = block_def_name_map(&defs);
        assert_eq!(map.get(&3).map(String::as_str), Some("A"));
        assert_eq!(map.get(&7).map(String::as_str), Some("B"));
        assert_eq!(resolve_block_name(10, &defs), None);
    }

    #[test]
    fn parse_minimal_with_dimension_entity() {
        let data = build_minimal_jww_with_dimension();
        let doc = super::parse_document(&data).unwrap();
        assert_eq!(doc.entities.len(), 1);

        match &doc.entities[0] {
            Entity::Dimension(dim) => {
                assert_eq!(dim.line.start_x, 0.0);
                assert_eq!(dim.line.end_x, 10.0);
                assert_eq!(dim.text.content, "1000");
            }
            other => panic!("expected DIMENSION entity, got {:?}", other),
        }
    }

    #[test]
    fn validate_unresolved_block_reference() {
        let data = build_minimal_jww_with_unresolved_block_ref();
        let doc = super::parse_document(&data).unwrap();
        let validation = validate_block_references(&doc);

        assert_eq!(validation.total_references, 1);
        assert_eq!(validation.resolved_references, 0);
        assert_eq!(validation.unresolved_def_numbers, vec![99]);
        assert!(validation.has_unresolved());
    }

    fn build_minimal_jww_with_block_def() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());

        // memo CString: empty
        data.push(0);

        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        // entity list count (WORD)
        data.extend_from_slice(&1u16.to_le_bytes());

        // class definition: CDataBlock
        data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        data.extend_from_slice(&600u16.to_le_bytes());
        let entity_class = b"CDataBlock";
        data.extend_from_slice(&(entity_class.len() as u16).to_le_bytes());
        data.extend_from_slice(entity_class);

        // EntityBase for block insert
        data.extend_from_slice(&0u32.to_le_bytes()); // group
        data.push(1); // pen_style
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_color
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_width
        data.extend_from_slice(&0u16.to_le_bytes()); // layer
        data.extend_from_slice(&0u16.to_le_bytes()); // layer_group
        data.extend_from_slice(&0u16.to_le_bytes()); // flag

        // Block insert payload: ref_x, ref_y, scale_x, scale_y, rotation, def_number
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_y
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_x
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // rotation
        data.extend_from_slice(&1u32.to_le_bytes()); // def_number

        // block def count (DWORD)
        data.extend_from_slice(&1u32.to_le_bytes());

        // class definition: CDataList
        data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        data.extend_from_slice(&600u16.to_le_bytes());
        let class_name = b"CDataList";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        // EntityBase
        data.extend_from_slice(&0u32.to_le_bytes()); // group
        data.push(1); // pen_style
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_color
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_width
        data.extend_from_slice(&0u16.to_le_bytes()); // layer
        data.extend_from_slice(&0u16.to_le_bytes()); // layer_group
        data.extend_from_slice(&0u16.to_le_bytes()); // flag

        data.extend_from_slice(&1u32.to_le_bytes()); // number
        data.extend_from_slice(&0u32.to_le_bytes()); // is_referenced
        data.extend_from_slice(&0u32.to_le_bytes()); // ctime

        // CString "BLK"
        data.push(3);
        data.write_all(b"BLK").unwrap();

        // nested entity list count (WORD)
        data.extend_from_slice(&0u16.to_le_bytes());

        data
    }

    fn build_minimal_jww_with_dimension() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());
        data.push(0); // memo
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // group state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        data.extend_from_slice(&1u16.to_le_bytes()); // entity count
        data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // new class
        data.extend_from_slice(&600u16.to_le_bytes()); // schema
        let class_name = b"CDataSunpou";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        // Dimension base
        append_entity_base(&mut data);
        // Dimension line
        append_entity_base(&mut data);
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_y
        data.extend_from_slice(&10.0f64.to_le_bytes()); // end_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // end_y

        // Dimension text
        append_entity_base(&mut data);
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // start_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // end_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // end_y
        data.extend_from_slice(&0u32.to_le_bytes()); // text_type
        data.extend_from_slice(&1.0f64.to_le_bytes()); // size_x
        data.extend_from_slice(&1.0f64.to_le_bytes()); // size_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // spacing
        data.extend_from_slice(&0.0f64.to_le_bytes()); // angle
        data.push(0); // font_name cstring
        data.push(4); // content cstring len
        data.write_all(b"1000").unwrap();

        // version >= 420 payload
        data.extend_from_slice(&0u16.to_le_bytes()); // sxf mode
        for _ in 0..2 {
            append_entity_base(&mut data);
            data.extend_from_slice(&0.0f64.to_le_bytes());
            data.extend_from_slice(&0.0f64.to_le_bytes());
            data.extend_from_slice(&0.0f64.to_le_bytes());
            data.extend_from_slice(&0.0f64.to_le_bytes());
        }
        for _ in 0..4 {
            append_entity_base(&mut data);
            data.extend_from_slice(&0.0f64.to_le_bytes()); // x
            data.extend_from_slice(&0.0f64.to_le_bytes()); // y
            data.extend_from_slice(&0u32.to_le_bytes()); // is_temporary
        }

        data.extend_from_slice(&0u32.to_le_bytes()); // block def count
        data
    }

    fn append_entity_base(data: &mut Vec<u8>) {
        data.extend_from_slice(&0u32.to_le_bytes()); // group
        data.push(1); // pen_style
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_color
        data.extend_from_slice(&1u16.to_le_bytes()); // pen_width
        data.extend_from_slice(&0u16.to_le_bytes()); // layer
        data.extend_from_slice(&0u16.to_le_bytes()); // layer_group
        data.extend_from_slice(&0u16.to_le_bytes()); // flag
    }

    fn build_minimal_jww_with_unresolved_block_ref() -> Vec<u8> {
        let mut data = Vec::<u8>::new();
        data.extend_from_slice(b"JwwData.");
        data.extend_from_slice(&600u32.to_le_bytes());
        data.push(0); // memo
        data.extend_from_slice(&0u32.to_le_bytes()); // paper size
        data.extend_from_slice(&0u32.to_le_bytes()); // write layer group

        for _ in 0..16 {
            data.extend_from_slice(&0u32.to_le_bytes()); // state
            data.extend_from_slice(&0u32.to_le_bytes()); // write layer
            data.extend_from_slice(&1.0f64.to_le_bytes()); // scale
            data.extend_from_slice(&0u32.to_le_bytes()); // protect
            for _ in 0..16 {
                data.extend_from_slice(&0u32.to_le_bytes()); // layer state
                data.extend_from_slice(&0u32.to_le_bytes()); // layer protect
            }
        }

        data.extend_from_slice(&1u16.to_le_bytes()); // entity count
        data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // new class
        data.extend_from_slice(&600u16.to_le_bytes()); // schema
        let class_name = b"CDataBlock";
        data.extend_from_slice(&(class_name.len() as u16).to_le_bytes());
        data.extend_from_slice(class_name);

        append_entity_base(&mut data);
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_x
        data.extend_from_slice(&0.0f64.to_le_bytes()); // ref_y
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_x
        data.extend_from_slice(&1.0f64.to_le_bytes()); // scale_y
        data.extend_from_slice(&0.0f64.to_le_bytes()); // rotation
        data.extend_from_slice(&99u32.to_le_bytes()); // unresolved def_number

        data.extend_from_slice(&0u32.to_le_bytes()); // block def count
        data
    }
}
