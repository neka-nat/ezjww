mod dxf;
mod error;
mod header;
mod model;
mod parser;
mod reader;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

pub use dxf::{
    convert_document, convert_document_with_options, document_to_string, write_document_to_file,
    ConvertOptions, DxfArc, DxfBlock, DxfCircle, DxfDocument, DxfEllipse, DxfEntity, DxfInsert,
    DxfLayer, DxfLine, DxfPoint, DxfSolid, DxfText,
};
pub use error::JwwError;
pub use header::{
    is_jww_signature, parse_header, read_header_from_file, JwwHeader, LayerGroupHeader, LayerHeader,
};
pub use model::{
    collect_entity_coordinates, coordinates_bbox, Arc, Block, BlockDef, Coord2D, Dimension, Entity,
    EntityBase, JwwDocument, Line, Point, Solid, Text,
};
pub use parser::{
    block_def_name_map, entity_counts, parse_document, read_document_from_file, resolve_block_name,
    validate_block_references, BlockReferenceValidation,
};

#[pyfunction]
fn hello_from_bin() -> String {
    "Hello from ezjww!".to_string()
}

#[pyfunction]
fn is_jww_file(path: &str) -> PyResult<bool> {
    let mut file = File::open(path).map_err(|e| PyIOError::new_err(e.to_string()))?;
    let mut signature = [0_u8; 8];
    match file.read_exact(&mut signature) {
        Ok(()) => Ok(is_jww_signature(&signature)),
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(err) => Err(PyIOError::new_err(err.to_string())),
    }
}

#[pyfunction]
fn read_header(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    let header = read_header_from_file(path).map_err(to_py_err)?;
    Ok(header_to_pydict(py, &header)?.unbind().into())
}

#[pyfunction]
fn read_document(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    let document = read_document_from_file(path).map_err(to_py_err)?;
    let out = PyDict::new_bound(py);
    let header = header_to_pydict(py, &document.header)?;
    out.set_item("header", header)?;

    let block_name_map = block_def_name_map(&document.block_defs);

    let entities = PyList::empty_bound(py);
    for entity in &document.entities {
        entities.append(entity_to_pydict(py, entity, &block_name_map)?)?;
    }
    out.set_item("entities", entities)?;

    let block_defs = PyList::empty_bound(py);
    for block_def in &document.block_defs {
        block_defs.append(block_def_to_pydict(py, block_def, &block_name_map)?)?;
    }
    out.set_item("block_defs", block_defs)?;
    out.set_item(
        "block_def_names",
        block_def_names_to_pydict(py, &block_name_map)?,
    )?;

    let counts = entity_counts_to_pydict(py, entity_counts(&document.entities))?;
    out.set_item("entity_counts", counts)?;
    let validation = validate_block_references(&document);
    out.set_item(
        "validation",
        block_reference_validation_to_pydict(py, &validation)?,
    )?;

    Ok(out.unbind().into())
}

#[pyfunction(signature = (path, explode_inserts=false, max_block_nesting=32))]
fn read_dxf_document(
    py: Python<'_>,
    path: &str,
    explode_inserts: bool,
    max_block_nesting: usize,
) -> PyResult<PyObject> {
    let document = read_document_from_file(path).map_err(to_py_err)?;
    let options = ConvertOptions {
        explode_inserts,
        max_block_nesting,
    };
    let dxf_document = convert_document_with_options(&document, options);
    Ok(dxf_document_to_pydict(py, &dxf_document)?.unbind().into())
}

#[pyfunction(signature = (path, explode_inserts=false, max_block_nesting=32))]
fn read_dxf_string(
    path: &str,
    explode_inserts: bool,
    max_block_nesting: usize,
) -> PyResult<String> {
    let document = read_document_from_file(path).map_err(to_py_err)?;
    let options = ConvertOptions {
        explode_inserts,
        max_block_nesting,
    };
    let dxf_document = convert_document_with_options(&document, options);
    Ok(document_to_string(&dxf_document))
}

#[pyfunction(signature = (path, output_path, explode_inserts=false, max_block_nesting=32))]
fn write_dxf(
    path: &str,
    output_path: &str,
    explode_inserts: bool,
    max_block_nesting: usize,
) -> PyResult<()> {
    let document = read_document_from_file(path).map_err(to_py_err)?;
    let options = ConvertOptions {
        explode_inserts,
        max_block_nesting,
    };
    let dxf_document = convert_document_with_options(&document, options);
    write_document_to_file(&dxf_document, output_path)
        .map_err(|err| PyIOError::new_err(err.to_string()))?;
    Ok(())
}

fn to_py_err(err: JwwError) -> PyErr {
    match err {
        JwwError::Io(io) => PyIOError::new_err(io.to_string()),
        JwwError::InvalidSignature => PyValueError::new_err("invalid JWW signature"),
        JwwError::UnexpectedEof(ctx) => {
            PyValueError::new_err(format!("unexpected EOF while reading {ctx}"))
        }
        JwwError::EntityListNotFound => PyValueError::new_err("entity list not found"),
        JwwError::UnknownClassPid(pid) => {
            PyValueError::new_err(format!("unknown class PID: {pid}"))
        }
        JwwError::UnknownEntityClass(name) => {
            PyValueError::new_err(format!("unknown entity class: {name}"))
        }
    }
}

fn header_to_pydict<'py>(py: Python<'py>, header: &JwwHeader) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("version", header.version)?;
    out.set_item("memo", &header.memo)?;
    out.set_item("paper_size", header.paper_size)?;
    out.set_item("write_layer_group", header.write_layer_group)?;

    let layer_groups = PyList::empty_bound(py);
    for group in &header.layer_groups {
        let group_dict = PyDict::new_bound(py);
        group_dict.set_item("state", group.state)?;
        group_dict.set_item("write_layer", group.write_layer)?;
        group_dict.set_item("scale", group.scale)?;
        group_dict.set_item("protect", group.protect)?;
        group_dict.set_item("name", &group.name)?;

        let layers = PyList::empty_bound(py);
        for layer in &group.layers {
            let layer_dict = PyDict::new_bound(py);
            layer_dict.set_item("state", layer.state)?;
            layer_dict.set_item("protect", layer.protect)?;
            layer_dict.set_item("name", &layer.name)?;
            layers.append(layer_dict)?;
        }
        group_dict.set_item("layers", layers)?;
        layer_groups.append(group_dict)?;
    }

    out.set_item("layer_groups", layer_groups)?;
    Ok(out)
}

fn entity_to_pydict<'py>(
    py: Python<'py>,
    entity: &Entity,
    block_name_map: &HashMap<u32, String>,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("type", entity.entity_type())?;

    let base = entity.base();
    let base_dict = PyDict::new_bound(py);
    base_dict.set_item("group", base.group)?;
    base_dict.set_item("pen_style", base.pen_style)?;
    base_dict.set_item("pen_color", base.pen_color)?;
    base_dict.set_item("pen_width", base.pen_width)?;
    base_dict.set_item("layer", base.layer)?;
    base_dict.set_item("layer_group", base.layer_group)?;
    base_dict.set_item("flag", base.flag)?;
    out.set_item("base", base_dict)?;

    match entity {
        Entity::Line(v) => {
            out.set_item("start_x", v.start_x)?;
            out.set_item("start_y", v.start_y)?;
            out.set_item("end_x", v.end_x)?;
            out.set_item("end_y", v.end_y)?;
        }
        Entity::Arc(v) => {
            out.set_item("center_x", v.center_x)?;
            out.set_item("center_y", v.center_y)?;
            out.set_item("radius", v.radius)?;
            out.set_item("start_angle", v.start_angle)?;
            out.set_item("arc_angle", v.arc_angle)?;
            out.set_item("tilt_angle", v.tilt_angle)?;
            out.set_item("flatness", v.flatness)?;
            out.set_item("is_full_circle", v.is_full_circle)?;
        }
        Entity::Point(v) => {
            out.set_item("x", v.x)?;
            out.set_item("y", v.y)?;
            out.set_item("is_temporary", v.is_temporary)?;
            out.set_item("code", v.code)?;
            out.set_item("angle", v.angle)?;
            out.set_item("scale", v.scale)?;
        }
        Entity::Text(v) => {
            out.set_item("start_x", v.start_x)?;
            out.set_item("start_y", v.start_y)?;
            out.set_item("end_x", v.end_x)?;
            out.set_item("end_y", v.end_y)?;
            out.set_item("text_type", v.text_type)?;
            out.set_item("size_x", v.size_x)?;
            out.set_item("size_y", v.size_y)?;
            out.set_item("spacing", v.spacing)?;
            out.set_item("angle", v.angle)?;
            out.set_item("font_name", &v.font_name)?;
            out.set_item("content", &v.content)?;
        }
        Entity::Solid(v) => {
            out.set_item("point1_x", v.point1_x)?;
            out.set_item("point1_y", v.point1_y)?;
            out.set_item("point2_x", v.point2_x)?;
            out.set_item("point2_y", v.point2_y)?;
            out.set_item("point3_x", v.point3_x)?;
            out.set_item("point3_y", v.point3_y)?;
            out.set_item("point4_x", v.point4_x)?;
            out.set_item("point4_y", v.point4_y)?;
            out.set_item("color", v.color)?;
        }
        Entity::Block(v) => {
            out.set_item("ref_x", v.ref_x)?;
            out.set_item("ref_y", v.ref_y)?;
            out.set_item("scale_x", v.scale_x)?;
            out.set_item("scale_y", v.scale_y)?;
            out.set_item("rotation", v.rotation)?;
            out.set_item("def_number", v.def_number)?;
            out.set_item("block_name", block_name_map.get(&v.def_number).cloned())?;
        }
        Entity::Dimension(v) => {
            out.set_item("line", line_to_pydict(py, &v.line)?)?;
            out.set_item("text", text_to_pydict(py, &v.text)?)?;
            out.set_item("sxf_mode", v.sxf_mode)?;

            let aux_lines = PyList::empty_bound(py);
            for line in &v.aux_lines {
                aux_lines.append(line_to_pydict(py, line)?)?;
            }
            out.set_item("aux_lines", aux_lines)?;

            let aux_points = PyList::empty_bound(py);
            for point in &v.aux_points {
                aux_points.append(point_to_pydict(py, point)?)?;
            }
            out.set_item("aux_points", aux_points)?;
        }
    }

    Ok(out)
}

fn line_to_pydict<'py>(py: Python<'py>, line: &Line) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("start_x", line.start_x)?;
    out.set_item("start_y", line.start_y)?;
    out.set_item("end_x", line.end_x)?;
    out.set_item("end_y", line.end_y)?;
    Ok(out)
}

fn point_to_pydict<'py>(py: Python<'py>, point: &Point) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("x", point.x)?;
    out.set_item("y", point.y)?;
    out.set_item("is_temporary", point.is_temporary)?;
    out.set_item("code", point.code)?;
    out.set_item("angle", point.angle)?;
    out.set_item("scale", point.scale)?;
    Ok(out)
}

fn text_to_pydict<'py>(py: Python<'py>, text: &Text) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("start_x", text.start_x)?;
    out.set_item("start_y", text.start_y)?;
    out.set_item("end_x", text.end_x)?;
    out.set_item("end_y", text.end_y)?;
    out.set_item("text_type", text.text_type)?;
    out.set_item("size_x", text.size_x)?;
    out.set_item("size_y", text.size_y)?;
    out.set_item("spacing", text.spacing)?;
    out.set_item("angle", text.angle)?;
    out.set_item("font_name", &text.font_name)?;
    out.set_item("content", &text.content)?;
    Ok(out)
}

fn dxf_document_to_pydict<'py>(
    py: Python<'py>,
    dxf_document: &DxfDocument,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);

    let layers = PyList::empty_bound(py);
    for layer in &dxf_document.layers {
        layers.append(dxf_layer_to_pydict(py, layer)?)?;
    }
    out.set_item("layers", layers)?;

    let entities = PyList::empty_bound(py);
    for entity in &dxf_document.entities {
        entities.append(dxf_entity_to_pydict(py, entity)?)?;
    }
    out.set_item("entities", entities)?;

    let blocks = PyList::empty_bound(py);
    for block in &dxf_document.blocks {
        blocks.append(dxf_block_to_pydict(py, block)?)?;
    }
    out.set_item("blocks", blocks)?;
    out.set_item("unsupported_entities", &dxf_document.unsupported_entities)?;

    Ok(out)
}

fn dxf_layer_to_pydict<'py>(py: Python<'py>, layer: &DxfLayer) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("name", &layer.name)?;
    out.set_item("color", layer.color)?;
    out.set_item("line_type", &layer.line_type)?;
    out.set_item("frozen", layer.frozen)?;
    out.set_item("locked", layer.locked)?;
    Ok(out)
}

fn dxf_block_to_pydict<'py>(py: Python<'py>, block: &DxfBlock) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("name", &block.name)?;
    out.set_item("base_x", block.base_x)?;
    out.set_item("base_y", block.base_y)?;

    let entities = PyList::empty_bound(py);
    for entity in &block.entities {
        entities.append(dxf_entity_to_pydict(py, entity)?)?;
    }
    out.set_item("entities", entities)?;
    Ok(out)
}

fn dxf_entity_to_pydict<'py>(py: Python<'py>, entity: &DxfEntity) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("type", entity.entity_type())?;

    match entity {
        DxfEntity::Line(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("x1", v.x1)?;
            out.set_item("y1", v.y1)?;
            out.set_item("x2", v.x2)?;
            out.set_item("y2", v.y2)?;
        }
        DxfEntity::Circle(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("center_x", v.center_x)?;
            out.set_item("center_y", v.center_y)?;
            out.set_item("radius", v.radius)?;
        }
        DxfEntity::Arc(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("center_x", v.center_x)?;
            out.set_item("center_y", v.center_y)?;
            out.set_item("radius", v.radius)?;
            out.set_item("start_angle", v.start_angle)?;
            out.set_item("end_angle", v.end_angle)?;
        }
        DxfEntity::Ellipse(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("center_x", v.center_x)?;
            out.set_item("center_y", v.center_y)?;
            out.set_item("major_axis_x", v.major_axis_x)?;
            out.set_item("major_axis_y", v.major_axis_y)?;
            out.set_item("minor_ratio", v.minor_ratio)?;
            out.set_item("start_param", v.start_param)?;
            out.set_item("end_param", v.end_param)?;
        }
        DxfEntity::Point(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("x", v.x)?;
            out.set_item("y", v.y)?;
        }
        DxfEntity::Text(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("x", v.x)?;
            out.set_item("y", v.y)?;
            out.set_item("height", v.height)?;
            out.set_item("rotation", v.rotation)?;
            out.set_item("content", &v.content)?;
            out.set_item("style", &v.style)?;
        }
        DxfEntity::Solid(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("x1", v.x1)?;
            out.set_item("y1", v.y1)?;
            out.set_item("x2", v.x2)?;
            out.set_item("y2", v.y2)?;
            out.set_item("x3", v.x3)?;
            out.set_item("y3", v.y3)?;
            out.set_item("x4", v.x4)?;
            out.set_item("y4", v.y4)?;
        }
        DxfEntity::Insert(v) => {
            out.set_item("layer", &v.layer)?;
            out.set_item("color", v.color)?;
            out.set_item("line_type", &v.line_type)?;
            out.set_item("block_name", &v.block_name)?;
            out.set_item("x", v.x)?;
            out.set_item("y", v.y)?;
            out.set_item("scale_x", v.scale_x)?;
            out.set_item("scale_y", v.scale_y)?;
            out.set_item("rotation", v.rotation)?;
        }
    }

    Ok(out)
}

fn entity_counts_to_pydict<'py>(
    py: Python<'py>,
    counts: HashMap<&'static str, usize>,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    for (k, v) in counts {
        out.set_item(k, v)?;
    }
    Ok(out)
}

fn block_def_to_pydict<'py>(
    py: Python<'py>,
    block_def: &BlockDef,
    block_name_map: &HashMap<u32, String>,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("number", block_def.number)?;
    out.set_item("is_referenced", block_def.is_referenced)?;
    out.set_item("name", &block_def.name)?;

    let base = &block_def.base;
    let base_dict = PyDict::new_bound(py);
    base_dict.set_item("group", base.group)?;
    base_dict.set_item("pen_style", base.pen_style)?;
    base_dict.set_item("pen_color", base.pen_color)?;
    base_dict.set_item("pen_width", base.pen_width)?;
    base_dict.set_item("layer", base.layer)?;
    base_dict.set_item("layer_group", base.layer_group)?;
    base_dict.set_item("flag", base.flag)?;
    out.set_item("base", base_dict)?;

    let entities = PyList::empty_bound(py);
    for entity in &block_def.entities {
        entities.append(entity_to_pydict(py, entity, block_name_map)?)?;
    }
    out.set_item("entities", entities)?;
    Ok(out)
}

fn block_def_names_to_pydict<'py>(
    py: Python<'py>,
    block_name_map: &HashMap<u32, String>,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    for (k, v) in block_name_map {
        out.set_item(*k, v)?;
    }
    Ok(out)
}

fn block_reference_validation_to_pydict<'py>(
    py: Python<'py>,
    validation: &BlockReferenceValidation,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new_bound(py);
    out.set_item("total_references", validation.total_references)?;
    out.set_item("resolved_references", validation.resolved_references)?;
    out.set_item("unresolved_def_numbers", &validation.unresolved_def_numbers)?;
    out.set_item("has_unresolved", validation.has_unresolved())?;
    Ok(out)
}

/// A Python module implemented in Rust. The name of this function must match
/// the `lib.name` setting in the `Cargo.toml`, else Python will not be able to
/// import the module.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hello_from_bin, m)?)?;
    m.add_function(wrap_pyfunction!(is_jww_file, m)?)?;
    m.add_function(wrap_pyfunction!(read_header, m)?)?;
    m.add_function(wrap_pyfunction!(read_document, m)?)?;
    m.add_function(wrap_pyfunction!(read_dxf_document, m)?)?;
    m.add_function(wrap_pyfunction!(read_dxf_string, m)?)?;
    m.add_function(wrap_pyfunction!(write_dxf, m)?)?;
    Ok(())
}
