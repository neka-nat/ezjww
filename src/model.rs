use crate::header::JwwHeader;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EntityBase {
    pub group: u32,
    pub pen_style: u8,
    pub pen_color: u16,
    pub pen_width: u16,
    pub layer: u16,
    pub layer_group: u16,
    pub flag: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Coord2D {
    pub x: f64,
    pub y: f64,
}

impl Coord2D {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

pub fn coordinates_bbox(points: &[Coord2D]) -> Option<(Coord2D, Coord2D)> {
    let first = points.first().copied()?;
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.x;
    let mut max_y = first.y;

    for p in points.iter().skip(1) {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }

    Some((Coord2D::new(min_x, min_y), Coord2D::new(max_x, max_y)))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub base: EntityBase,
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arc {
    pub base: EntityBase,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub start_angle: f64,
    pub arc_angle: f64,
    pub tilt_angle: f64,
    pub flatness: f64,
    pub is_full_circle: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub base: EntityBase,
    pub x: f64,
    pub y: f64,
    pub is_temporary: bool,
    pub code: u32,
    pub angle: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub base: EntityBase,
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub text_type: u32,
    pub size_x: f64,
    pub size_y: f64,
    pub spacing: f64,
    pub angle: f64,
    pub font_name: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Solid {
    pub base: EntityBase,
    pub point1_x: f64,
    pub point1_y: f64,
    pub point2_x: f64,
    pub point2_y: f64,
    pub point3_x: f64,
    pub point3_y: f64,
    pub point4_x: f64,
    pub point4_y: f64,
    pub color: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub base: EntityBase,
    pub ref_x: f64,
    pub ref_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation: f64,
    pub def_number: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    pub base: EntityBase,
    pub line: Line,
    pub text: Text,
    pub sxf_mode: Option<u16>,
    pub aux_lines: Vec<Line>,
    pub aux_points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockDef {
    pub base: EntityBase,
    pub number: u32,
    pub is_referenced: bool,
    pub name: String,
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entity {
    Line(Line),
    Arc(Arc),
    Point(Point),
    Text(Text),
    Solid(Solid),
    Block(Block),
    Dimension(Dimension),
}

impl Entity {
    pub fn entity_type(&self) -> &'static str {
        match self {
            Self::Line(_) => "LINE",
            Self::Arc(arc) => {
                if arc.is_full_circle {
                    "CIRCLE"
                } else {
                    "ARC"
                }
            }
            Self::Point(_) => "POINT",
            Self::Text(_) => "TEXT",
            Self::Solid(_) => "SOLID",
            Self::Block(_) => "BLOCK",
            Self::Dimension(_) => "DIMENSION",
        }
    }

    pub fn base(&self) -> &EntityBase {
        match self {
            Self::Line(v) => &v.base,
            Self::Arc(v) => &v.base,
            Self::Point(v) => &v.base,
            Self::Text(v) => &v.base,
            Self::Solid(v) => &v.base,
            Self::Block(v) => &v.base,
            Self::Dimension(v) => &v.base,
        }
    }

    // Common extraction helper for downstream converters (e.g. DXF writer).
    // Returns control-like points that are explicit in each entity payload.
    pub fn common_coordinates(&self) -> Vec<Coord2D> {
        match self {
            Self::Line(v) => vec![
                Coord2D::new(v.start_x, v.start_y),
                Coord2D::new(v.end_x, v.end_y),
            ],
            Self::Arc(v) => vec![Coord2D::new(v.center_x, v.center_y)],
            Self::Point(v) => vec![Coord2D::new(v.x, v.y)],
            Self::Text(v) => vec![
                Coord2D::new(v.start_x, v.start_y),
                Coord2D::new(v.end_x, v.end_y),
            ],
            Self::Solid(v) => vec![
                Coord2D::new(v.point1_x, v.point1_y),
                Coord2D::new(v.point2_x, v.point2_y),
                Coord2D::new(v.point3_x, v.point3_y),
                Coord2D::new(v.point4_x, v.point4_y),
            ],
            Self::Block(v) => vec![Coord2D::new(v.ref_x, v.ref_y)],
            Self::Dimension(v) => {
                let mut points =
                    Vec::<Coord2D>::with_capacity(4 + v.aux_lines.len() * 2 + v.aux_points.len());
                points.push(Coord2D::new(v.line.start_x, v.line.start_y));
                points.push(Coord2D::new(v.line.end_x, v.line.end_y));
                points.push(Coord2D::new(v.text.start_x, v.text.start_y));
                points.push(Coord2D::new(v.text.end_x, v.text.end_y));
                for line in &v.aux_lines {
                    points.push(Coord2D::new(line.start_x, line.start_y));
                    points.push(Coord2D::new(line.end_x, line.end_y));
                }
                for point in &v.aux_points {
                    points.push(Coord2D::new(point.x, point.y));
                }
                points
            }
        }
    }

    pub fn first_coordinate(&self) -> Option<Coord2D> {
        self.common_coordinates().into_iter().next()
    }

    pub fn common_coordinate_bbox(&self) -> Option<(Coord2D, Coord2D)> {
        coordinates_bbox(&self.common_coordinates())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JwwDocument {
    pub header: JwwHeader,
    pub entities: Vec<Entity>,
    pub block_defs: Vec<BlockDef>,
}

pub fn collect_entity_coordinates(entities: &[Entity]) -> Vec<Coord2D> {
    let mut points = Vec::<Coord2D>::new();
    for entity in entities {
        points.extend(entity.common_coordinates());
    }
    points
}

#[cfg(test)]
mod tests {
    use super::{
        collect_entity_coordinates, coordinates_bbox, Arc, Coord2D, Dimension, Entity, EntityBase,
        Line, Point, Solid, Text,
    };

    #[test]
    fn line_common_coordinates_and_bbox() {
        let line = Entity::Line(Line {
            base: EntityBase::default(),
            start_x: 1.0,
            start_y: 2.0,
            end_x: 4.0,
            end_y: 6.0,
        });

        let coords = line.common_coordinates();
        assert_eq!(coords, vec![Coord2D::new(1.0, 2.0), Coord2D::new(4.0, 6.0)]);

        let (min, max) = line.common_coordinate_bbox().unwrap();
        assert_eq!(min, Coord2D::new(1.0, 2.0));
        assert_eq!(max, Coord2D::new(4.0, 6.0));
    }

    #[test]
    fn dimension_common_coordinates_include_aux() {
        let dim = Entity::Dimension(Dimension {
            base: EntityBase::default(),
            line: Line {
                base: EntityBase::default(),
                start_x: 0.0,
                start_y: 0.0,
                end_x: 10.0,
                end_y: 0.0,
            },
            text: Text {
                base: EntityBase::default(),
                start_x: 5.0,
                start_y: 1.0,
                end_x: 5.5,
                end_y: 1.0,
                text_type: 0,
                size_x: 1.0,
                size_y: 1.0,
                spacing: 0.0,
                angle: 0.0,
                font_name: String::new(),
                content: String::new(),
            },
            sxf_mode: Some(0),
            aux_lines: vec![Line {
                base: EntityBase::default(),
                start_x: 0.0,
                start_y: -1.0,
                end_x: 10.0,
                end_y: -1.0,
            }],
            aux_points: vec![Point {
                base: EntityBase::default(),
                x: 2.0,
                y: 2.0,
                is_temporary: false,
                code: 0,
                angle: 0.0,
                scale: 1.0,
            }],
        });

        let coords = dim.common_coordinates();
        assert_eq!(coords.len(), 7);
        assert!(coords.contains(&Coord2D::new(0.0, 0.0)));
        assert!(coords.contains(&Coord2D::new(10.0, -1.0)));
        assert!(coords.contains(&Coord2D::new(2.0, 2.0)));
    }

    #[test]
    fn collect_entity_coordinates_works() {
        let entities = vec![
            Entity::Point(Point {
                base: EntityBase::default(),
                x: 1.0,
                y: 2.0,
                is_temporary: false,
                code: 0,
                angle: 0.0,
                scale: 1.0,
            }),
            Entity::Arc(Arc {
                base: EntityBase::default(),
                center_x: -1.0,
                center_y: -2.0,
                radius: 3.0,
                start_angle: 0.0,
                arc_angle: 1.0,
                tilt_angle: 0.0,
                flatness: 1.0,
                is_full_circle: false,
            }),
            Entity::Solid(Solid {
                base: EntityBase::default(),
                point1_x: 0.0,
                point1_y: 0.0,
                point2_x: 1.0,
                point2_y: 0.0,
                point3_x: 1.0,
                point3_y: 1.0,
                point4_x: 0.0,
                point4_y: 1.0,
                color: None,
            }),
        ];

        let all = collect_entity_coordinates(&entities);
        assert_eq!(all.len(), 6);
        let (min, max) = coordinates_bbox(&all).unwrap();
        assert_eq!(min, Coord2D::new(-1.0, -2.0));
        assert_eq!(max, Coord2D::new(1.0, 2.0));
    }
}
