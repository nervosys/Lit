//! Content type system — extends Lit from SWE-only VCS to a universal
//! versioning system for CAD, EDA, manuscripts, databases, scientific data,
//! media assets, and arbitrary domain content.
//!
//! Each content type carries diff/merge strategy hints, metadata schemas,
//! and size/storage policies so that Lit can handle domain-specific files
//! without requiring external plugins.

use crate::core::find_repo_root;
use crate::errors::LitError;
use crate::response::ContentTypeResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── Data types ──────────────────────────────────────────────────────────────

/// How a content type should be diffed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiffStrategy {
    /// Line-based textual diff (source code, markdown, config)
    Text,
    /// Binary diff (delta compression)
    Binary,
    /// Structural diff (JSON/XML/AST tree comparison)
    Structural,
    /// Semantic diff (schema-aware: databases, CAD feature trees)
    Semantic,
    /// No diff — treat as opaque blob, show only size/hash changes
    Opaque,
}

/// How a content type should be merged
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Standard 3-way text merge
    TextThreeWay,
    /// Take ours or theirs — no automatic merge (binary, CAD)
    ManualResolve,
    /// Schema-aware merge (databases, structured data)
    SchemaAware,
    /// Component-level merge (EDA: merge at schematic block level)
    ComponentLevel,
    /// Append-only merge (logs, audit trails)
    AppendOnly,
    /// Last-writer-wins (media assets, compiled outputs)
    LastWriterWins,
}

/// Storage tier hint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StorageTier {
    /// Normal object store (small text files)
    Standard,
    /// LFS — large file storage (binary assets, datasets)
    Lfs,
    /// Chunked — content-defined chunking for large structured files
    Chunked,
    /// External — reference to external storage (S3, GCS, NFS)
    External,
}

/// Domain classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentDomain {
    Software,
    Cad,
    Eda,
    Cam,
    Simulation,
    MlModel,
    Manuscript,
    Database,
    Scientific,
    Media,
    Geospatial,
    Legal,
    Financial,
    Config,
    Documentation,
    Custom(String),
}

impl std::fmt::Display for ContentDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentDomain::Software => write!(f, "software"),
            ContentDomain::Cad => write!(f, "cad"),
            ContentDomain::Eda => write!(f, "eda"),
            ContentDomain::Cam => write!(f, "cam"),
            ContentDomain::Simulation => write!(f, "simulation"),
            ContentDomain::MlModel => write!(f, "ml-model"),
            ContentDomain::Manuscript => write!(f, "manuscript"),
            ContentDomain::Database => write!(f, "database"),
            ContentDomain::Scientific => write!(f, "scientific"),
            ContentDomain::Media => write!(f, "media"),
            ContentDomain::Geospatial => write!(f, "geospatial"),
            ContentDomain::Legal => write!(f, "legal"),
            ContentDomain::Financial => write!(f, "financial"),
            ContentDomain::Config => write!(f, "config"),
            ContentDomain::Documentation => write!(f, "documentation"),
            ContentDomain::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

/// A registered content type with domain-specific handling policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentType {
    /// Unique identifier (e.g. "cad/step", "eda/kicad-pcb", "db/sqlite")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Domain classification
    pub domain: ContentDomain,
    /// MIME type(s) associated with this content type
    pub mime_types: Vec<String>,
    /// File extensions (without dot) that map to this type
    pub extensions: Vec<String>,
    /// Magic bytes for binary detection (hex-encoded prefixes)
    #[serde(default)]
    pub magic_bytes: Vec<String>,
    /// Recommended diff strategy
    pub diff_strategy: DiffStrategy,
    /// Recommended merge strategy
    pub merge_strategy: MergeStrategy,
    /// Storage tier
    pub storage_tier: StorageTier,
    /// Maximum inline size (bytes) before promoting to LFS
    pub lfs_threshold: Option<u64>,
    /// Metadata schema — JSON Schema fragment describing domain-specific fields
    #[serde(default)]
    pub metadata_schema: Option<serde_json::Value>,
    /// Whether this type supports structural diffing natively
    pub structural_diff: bool,
    /// Whether this type supports component-level locking
    pub component_locking: bool,
    /// Description
    pub description: String,
}

// ── Built-in content types ──────────────────────────────────────────────────

fn builtin_types() -> Vec<ContentType> {
    vec![
        // ── CAD ──
        ContentType {
            id: "cad/step".into(),
            name: "STEP CAD Model".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/step".into()],
            extensions: vec!["step".into(), "stp".into(), "p21".into()],
            magic_bytes: vec!["49534F2D".into()], // "ISO-"
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "units": {"type": "string", "enum": ["mm", "in", "m"]},
                    "assembly_count": {"type": "integer"},
                    "bounding_box": {"type": "array", "items": {"type": "number"}}
                }
            })),
            structural_diff: true,
            component_locking: true,
            description: "ISO 10303 STEP geometry exchange format".into(),
        },
        ContentType {
            id: "cad/stl".into(),
            name: "STL Mesh".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/stl".into()],
            extensions: vec!["stl".into()],
            magic_bytes: vec!["736F6C6964".into()], // "solid" (ASCII STL)
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(512 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "triangle_count": {"type": "integer"},
                    "format": {"type": "string", "enum": ["ascii", "binary"]}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "Stereolithography mesh format for 3D printing".into(),
        },
        ContentType {
            id: "cad/iges".into(),
            name: "IGES CAD Model".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/iges".into()],
            extensions: vec!["igs".into(), "iges".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: false,
            description: "Initial Graphics Exchange Specification".into(),
        },
        ContentType {
            id: "cad/3mf".into(),
            name: "3MF Model".into(),
            domain: ContentDomain::Cad,
            mime_types: vec![
                "model/3mf".into(),
                "application/vnd.ms-package.3dmanufacturing-3dmodel+xml".into(),
            ],
            extensions: vec!["3mf".into()],
            magic_bytes: vec!["504B0304".into()], // ZIP header
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: true,
            description: "3D Manufacturing Format (ZIP-based XML)".into(),
        },
        // ── CAD (native mechanical formats) ──
        ContentType {
            id: "cad/dwg".into(),
            name: "AutoCAD DWG".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["image/vnd.dwg".into()],
            extensions: vec!["dwg".into()],
            magic_bytes: vec!["4143".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Autodesk AutoCAD native drawing".into(),
        },
        ContentType {
            id: "cad/dxf".into(),
            name: "AutoCAD DXF".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["image/vnd.dxf".into()],
            extensions: vec!["dxf".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(20 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: false,
            description: "Drawing Exchange Format (ASCII/binary CAD interchange)".into(),
        },
        ContentType {
            id: "cad/solidworks".into(),
            name: "SolidWorks Document".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-solidworks".into()],
            extensions: vec!["sldprt".into(), "sldasm".into(), "slddrw".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Dassault SolidWorks part/assembly/drawing".into(),
        },
        ContentType {
            id: "cad/catia".into(),
            name: "CATIA Document".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-catia".into()],
            extensions: vec![
                "catpart".into(),
                "catproduct".into(),
                "catdrawing".into(),
                "cgr".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Dassault CATIA V5 part/product/drawing".into(),
        },
        ContentType {
            id: "cad/inventor".into(),
            name: "Autodesk Inventor".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-inventor".into()],
            extensions: vec!["ipt".into(), "iam".into(), "idw".into(), "ipn".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Autodesk Inventor part/assembly/drawing/presentation".into(),
        },
        ContentType {
            id: "cad/fusion360".into(),
            name: "Fusion 360 Archive".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-fusion360".into()],
            extensions: vec!["f3d".into(), "f3z".into()],
            magic_bytes: vec!["504B0304".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Autodesk Fusion 360 design archive".into(),
        },
        ContentType {
            id: "cad/creo".into(),
            name: "PTC Creo / Pro-ENGINEER".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-creo".into()],
            extensions: vec!["prt".into(), "asm".into(), "drw".into(), "frm".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "PTC Creo/Pro-E part/assembly/drawing".into(),
        },
        ContentType {
            id: "cad/siemens-nx".into(),
            name: "Siemens NX".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-siemens-nx".into()],
            // NX part/assembly/drawing all use .prt (shared with Creo .prt)
            extensions: vec!["prt".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Siemens NX (Unigraphics) part/assembly/drawing".into(),
        },
        ContentType {
            id: "cad/solid-edge".into(),
            name: "Solid Edge".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-solid-edge".into()],
            extensions: vec![
                "par".into(),
                "psm".into(),
                "pwd".into(),
                "asm".into(),
                "dft".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Siemens Solid Edge part/sheet-metal/weldment/assembly/draft".into(),
        },
        ContentType {
            id: "cad/rhino".into(),
            name: "Rhino 3DM".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/vnd.rhino".into()],
            extensions: vec!["3dm".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Rhinoceros 3D NURBS model (openNURBS)".into(),
        },
        ContentType {
            id: "cad/sketchup".into(),
            name: "SketchUp Model".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/vnd.sketchup.skp".into()],
            extensions: vec!["skp".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Trimble SketchUp model".into(),
        },
        ContentType {
            id: "cad/freecad".into(),
            name: "FreeCAD Document".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-extension-fcstd".into()],
            extensions: vec!["fcstd".into(), "fcstd1".into()],
            magic_bytes: vec!["504B0304".into()],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: true,
            description: "FreeCAD parametric document (ZIP-based)".into(),
        },
        ContentType {
            id: "cad/openscad".into(),
            name: "OpenSCAD Script".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-openscad".into()],
            extensions: vec!["scad".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "OpenSCAD programmatic solid model source".into(),
        },
        ContentType {
            id: "cad/parasolid".into(),
            name: "Parasolid Model".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-parasolid".into()],
            extensions: vec![
                "x_t".into(),
                "x_b".into(),
                "xmt_txt".into(),
                "xmt_bin".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Siemens Parasolid B-rep geometry kernel format".into(),
        },
        ContentType {
            id: "cad/acis".into(),
            name: "ACIS SAT".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-acis".into()],
            extensions: vec!["sat".into(), "sab".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Spatial ACIS solid model (SAT/SAB)".into(),
        },
        ContentType {
            id: "cad/jt".into(),
            name: "JT Visualization".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/jt".into()],
            extensions: vec!["jt".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "ISO 14306 JT lightweight 3D visualization".into(),
        },
        // ── 3D modeling / mesh interchange ──
        ContentType {
            id: "cad/obj".into(),
            name: "Wavefront OBJ".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/obj".into()],
            extensions: vec!["obj".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Wavefront OBJ geometry mesh".into(),
        },
        ContentType {
            id: "cad/fbx".into(),
            name: "Autodesk FBX".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/octet-stream".into()],
            extensions: vec!["fbx".into()],
            magic_bytes: vec!["4B6179646172612046425820".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Autodesk FBX scene/asset interchange".into(),
        },
        ContentType {
            id: "cad/gltf".into(),
            name: "glTF / GLB".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/gltf+json".into(), "model/gltf-binary".into()],
            extensions: vec!["gltf".into(), "glb".into()],
            magic_bytes: vec!["676C5446".into()],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "mesh_count": {"type": "integer"},
                    "material_count": {"type": "integer"},
                    "animation_count": {"type": "integer"},
                    "generator": {"type": "string"}
                }
            })),
            structural_diff: true,
            component_locking: false,
            description: "Khronos glTF 2.0 runtime 3D asset (text/binary)".into(),
        },
        ContentType {
            id: "cad/collada".into(),
            name: "COLLADA".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/vnd.collada+xml".into()],
            extensions: vec!["dae".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: false,
            description: "COLLADA (.dae) XML 3D asset interchange".into(),
        },
        ContentType {
            id: "cad/usd".into(),
            name: "Universal Scene Description".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["model/vnd.usd".into()],
            extensions: vec!["usd".into(), "usda".into(), "usdc".into(), "usdz".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: true,
            description: "Pixar OpenUSD scene description (ascii/crate/zip)".into(),
        },
        ContentType {
            id: "cad/ply".into(),
            name: "Polygon File Format".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-ply".into()],
            extensions: vec!["ply".into()],
            magic_bytes: vec!["706C79".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Stanford PLY polygon/point-cloud mesh".into(),
        },
        ContentType {
            id: "cad/blender".into(),
            name: "Blender Scene".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-blender".into()],
            extensions: vec!["blend".into()],
            magic_bytes: vec!["424C454E444552".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Blender .blend scene file".into(),
        },
        ContentType {
            id: "cad/alembic".into(),
            name: "Alembic Cache".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-alembic".into()],
            extensions: vec!["abc".into()],
            magic_bytes: vec!["4F6761776100".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Alembic baked geometry/animation cache".into(),
        },
        ContentType {
            id: "cad/3ds".into(),
            name: "Autodesk 3DS".into(),
            domain: ContentDomain::Cad,
            mime_types: vec!["application/x-3ds".into()],
            extensions: vec!["3ds".into()],
            magic_bytes: vec!["4D4D".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(512 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Autodesk 3D Studio legacy mesh".into(),
        },
        // ── EDA ──
        ContentType {
            id: "eda/kicad-pcb".into(),
            name: "KiCad PCB Layout".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-kicad-pcb".into()],
            extensions: vec!["kicad_pcb".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "layers": {"type": "integer"},
                    "component_count": {"type": "integer"},
                    "net_count": {"type": "integer"},
                    "board_dimensions": {"type": "object", "properties": {
                        "width_mm": {"type": "number"},
                        "height_mm": {"type": "number"}
                    }}
                }
            })),
            structural_diff: true,
            component_locking: true,
            description: "KiCad PCB layout (S-expression format)".into(),
        },
        ContentType {
            id: "eda/kicad-sch".into(),
            name: "KiCad Schematic".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-kicad-schematic".into()],
            extensions: vec!["kicad_sch".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: true,
            component_locking: true,
            description: "KiCad schematic (S-expression format)".into(),
        },
        ContentType {
            id: "eda/gerber".into(),
            name: "Gerber PCB Fabrication".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-gerber".into()],
            extensions: vec![
                "gbr".into(),
                "ger".into(),
                "gtl".into(),
                "gbl".into(),
                "gts".into(),
                "gbs".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Gerber RS-274X PCB fabrication data".into(),
        },
        ContentType {
            id: "eda/spice".into(),
            name: "SPICE Netlist".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["text/x-spice".into()],
            extensions: vec!["spice".into(), "sp".into(), "cir".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "SPICE circuit simulation netlist".into(),
        },
        // ── EDA (native tools, HDL, layout) ──
        ContentType {
            id: "eda/altium-sch".into(),
            name: "Altium Schematic".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-altium-schdoc".into()],
            extensions: vec!["schdoc".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Altium Designer schematic document".into(),
        },
        ContentType {
            id: "eda/altium-pcb".into(),
            name: "Altium PCB".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-altium-pcbdoc".into()],
            extensions: vec!["pcbdoc".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Altium Designer PCB layout document".into(),
        },
        ContentType {
            id: "eda/altium-project".into(),
            name: "Altium Project".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-altium-project".into()],
            extensions: vec!["prjpcb".into(), "prjfpg".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Altium Designer project file".into(),
        },
        ContentType {
            id: "eda/eagle".into(),
            name: "EAGLE Design".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-eagle".into()],
            extensions: vec!["brd".into(), "lbr".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(20 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: true,
            description: "Autodesk EAGLE board/library (XML)".into(),
        },
        ContentType {
            id: "eda/orcad".into(),
            name: "OrCAD Design".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-orcad".into()],
            extensions: vec!["dsn".into(), "opj".into(), "olb".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Cadence OrCAD schematic/project/library".into(),
        },
        ContentType {
            id: "eda/verilog".into(),
            name: "Verilog / SystemVerilog".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["text/x-verilog".into()],
            extensions: vec!["v".into(), "sv".into(), "svh".into(), "vh".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Verilog/SystemVerilog HDL source".into(),
        },
        ContentType {
            id: "eda/vhdl".into(),
            name: "VHDL".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["text/x-vhdl".into()],
            extensions: vec!["vhd".into(), "vhdl".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "VHDL hardware description language source".into(),
        },
        ContentType {
            id: "eda/excellon".into(),
            name: "Excellon Drill".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-excellon".into()],
            extensions: vec!["drl".into(), "xln".into(), "exc".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Excellon NC drill/route data for PCB fabrication".into(),
        },
        ContentType {
            id: "eda/gdsii".into(),
            name: "GDSII Layout".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-gdsii".into()],
            extensions: vec!["gds".into(), "gds2".into(), "gdsii".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Calma GDSII IC mask layout stream".into(),
        },
        ContentType {
            id: "eda/oasis".into(),
            name: "OASIS Layout".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/x-oasis".into()],
            extensions: vec!["oas".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "SEMI OASIS IC mask layout (GDSII successor)".into(),
        },
        ContentType {
            id: "eda/ipc2581".into(),
            name: "IPC-2581".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["application/xml".into()],
            extensions: vec!["cvg".into(), "xml2581".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: true,
            description: "IPC-2581 open PCB manufacturing data (XML)".into(),
        },
        ContentType {
            id: "eda/touchstone".into(),
            name: "Touchstone S-Parameters".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["text/x-touchstone".into()],
            extensions: vec![
                "s1p".into(),
                "s2p".into(),
                "s3p".into(),
                "s4p".into(),
                "snp".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Touchstone RF/microwave network parameter data".into(),
        },
        ContentType {
            id: "eda/lef-def".into(),
            name: "LEF / DEF".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["text/x-lefdef".into()],
            extensions: vec!["lef".into(), "def".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Library/Design Exchange Format for IC place-and-route".into(),
        },
        ContentType {
            id: "eda/spef".into(),
            name: "SPEF Parasitics".into(),
            domain: ContentDomain::Eda,
            mime_types: vec!["text/x-spef".into()],
            extensions: vec!["spef".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Standard Parasitic Exchange Format (IC timing)".into(),
        },
        // ── CAM (toolpaths / NC machining) ──
        ContentType {
            id: "cam/gcode".into(),
            name: "G-code Toolpath".into(),
            domain: ContentDomain::Cam,
            mime_types: vec!["text/x-gcode".into()],
            extensions: vec![
                "gcode".into(),
                "gco".into(),
                "nc".into(),
                "tap".into(),
                "cnc".into(),
                "ngc".into(),
                "mpf".into(),
                "g".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "line_count": {"type": "integer"},
                    "flavor": {"type": "string"},
                    "machine": {"type": "string"},
                    "estimated_time_s": {"type": "number"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "RS-274 G-code CNC/3D-printer toolpath".into(),
        },
        ContentType {
            id: "cam/step-nc".into(),
            name: "STEP-NC".into(),
            domain: ContentDomain::Cam,
            mime_types: vec!["model/step-nc".into()],
            extensions: vec!["stpnc".into(), "238".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: true,
            component_locking: true,
            description: "ISO 14649 STEP-NC machining data".into(),
        },
        ContentType {
            id: "cam/apt".into(),
            name: "APT CL Data".into(),
            domain: ContentDomain::Cam,
            mime_types: vec!["text/x-apt".into()],
            extensions: vec!["apt".into(), "cls".into(), "cl".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "APT cutter-location source / CL data".into(),
        },
        ContentType {
            id: "cam/mastercam".into(),
            name: "Mastercam".into(),
            domain: ContentDomain::Cam,
            mime_types: vec!["application/x-mastercam".into()],
            extensions: vec![
                "mcam".into(),
                "mcx".into(),
                "mcx-7".into(),
                "mcx-8".into(),
                "mcx-9".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "Mastercam part/toolpath document".into(),
        },
        // ── Manuscripts ──
        ContentType {
            id: "manuscript/latex".into(),
            name: "LaTeX Document".into(),
            domain: ContentDomain::Manuscript,
            mime_types: vec!["application/x-latex".into(), "text/x-tex".into()],
            extensions: vec!["tex".into(), "latex".into(), "ltx".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "document_class": {"type": "string"},
                    "word_count": {"type": "integer"},
                    "bibliography_entries": {"type": "integer"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "LaTeX typesetting source".into(),
        },
        ContentType {
            id: "manuscript/docx".into(),
            name: "Word Document".into(),
            domain: ContentDomain::Manuscript,
            mime_types: vec![
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
            ],
            extensions: vec!["docx".into()],
            magic_bytes: vec!["504B0304".into()], // ZIP
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(5 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "page_count": {"type": "integer"},
                    "word_count": {"type": "integer"},
                    "author": {"type": "string"},
                    "revision": {"type": "integer"}
                }
            })),
            structural_diff: true,
            component_locking: false,
            description: "Microsoft Word OOXML document".into(),
        },
        ContentType {
            id: "manuscript/typst".into(),
            name: "Typst Document".into(),
            domain: ContentDomain::Manuscript,
            mime_types: vec!["text/x-typst".into()],
            extensions: vec!["typ".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Typst markup language source".into(),
        },
        ContentType {
            id: "manuscript/asciidoc".into(),
            name: "AsciiDoc".into(),
            domain: ContentDomain::Manuscript,
            mime_types: vec!["text/asciidoc".into()],
            extensions: vec!["adoc".into(), "asciidoc".into(), "asc".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "AsciiDoc markup language".into(),
        },
        // ── Databases ──
        ContentType {
            id: "db/sqlite".into(),
            name: "SQLite Database".into(),
            domain: ContentDomain::Database,
            mime_types: vec![
                "application/vnd.sqlite3".into(),
                "application/x-sqlite3".into(),
            ],
            extensions: vec!["sqlite".into(), "sqlite3".into(), "db".into()],
            magic_bytes: vec!["53514C69746520666F726D6174".into()], // "SQLite format"
            diff_strategy: DiffStrategy::Semantic,
            merge_strategy: MergeStrategy::SchemaAware,
            storage_tier: StorageTier::Chunked,
            lfs_threshold: Some(10 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "table_count": {"type": "integer"},
                    "row_count": {"type": "integer"},
                    "schema_version": {"type": "string"},
                    "page_size": {"type": "integer"}
                }
            })),
            structural_diff: true,
            component_locking: true,
            description: "SQLite embedded database file".into(),
        },
        ContentType {
            id: "db/csv".into(),
            name: "CSV Data".into(),
            domain: ContentDomain::Database,
            mime_types: vec!["text/csv".into()],
            extensions: vec!["csv".into(), "tsv".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::SchemaAware,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "column_count": {"type": "integer"},
                    "row_count": {"type": "integer"},
                    "delimiter": {"type": "string"},
                    "has_header": {"type": "boolean"}
                }
            })),
            structural_diff: true,
            component_locking: false,
            description: "Comma/tab-separated values".into(),
        },
        ContentType {
            id: "db/parquet".into(),
            name: "Apache Parquet".into(),
            domain: ContentDomain::Database,
            mime_types: vec!["application/x-parquet".into()],
            extensions: vec!["parquet".into()],
            magic_bytes: vec!["50415231".into()], // "PAR1"
            diff_strategy: DiffStrategy::Semantic,
            merge_strategy: MergeStrategy::SchemaAware,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(10 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "row_groups": {"type": "integer"},
                    "row_count": {"type": "integer"},
                    "column_count": {"type": "integer"},
                    "compression": {"type": "string"}
                }
            })),
            structural_diff: true,
            component_locking: false,
            description: "Apache Parquet columnar storage".into(),
        },
        ContentType {
            id: "db/sql-migration".into(),
            name: "SQL Migration".into(),
            domain: ContentDomain::Database,
            mime_types: vec!["application/sql".into()],
            extensions: vec!["sql".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::AppendOnly,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "direction": {"type": "string", "enum": ["up", "down"]},
                    "version": {"type": "string"},
                    "idempotent": {"type": "boolean"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "SQL database migration script".into(),
        },
        // ── Scientific ──
        ContentType {
            id: "scientific/hdf5".into(),
            name: "HDF5 Dataset".into(),
            domain: ContentDomain::Scientific,
            mime_types: vec!["application/x-hdf5".into()],
            extensions: vec!["h5".into(), "hdf5".into(), "he5".into()],
            magic_bytes: vec!["894844460D0A1A0A".into()],
            diff_strategy: DiffStrategy::Semantic,
            merge_strategy: MergeStrategy::SchemaAware,
            storage_tier: StorageTier::Chunked,
            lfs_threshold: Some(10 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "dataset_count": {"type": "integer"},
                    "total_size": {"type": "integer"},
                    "compression": {"type": "string"}
                }
            })),
            structural_diff: true,
            component_locking: true,
            description: "Hierarchical Data Format 5 for scientific datasets".into(),
        },
        ContentType {
            id: "scientific/fits".into(),
            name: "FITS Astronomical Data".into(),
            domain: ContentDomain::Scientific,
            mime_types: vec!["application/fits".into()],
            extensions: vec!["fits".into(), "fit".into()],
            magic_bytes: vec!["53494D504C45".into()], // "SIMPLE"
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(5 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Flexible Image Transport System (astronomy)".into(),
        },
        ContentType {
            id: "scientific/jupyter".into(),
            name: "Jupyter Notebook".into(),
            domain: ContentDomain::Scientific,
            mime_types: vec!["application/x-ipynb+json".into()],
            extensions: vec!["ipynb".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "cell_count": {"type": "integer"},
                    "kernel": {"type": "string"},
                    "language": {"type": "string"}
                }
            })),
            structural_diff: true,
            component_locking: true,
            description: "Jupyter/IPython notebook (cell-level versioning)".into(),
        },
        // ── Media ──
        ContentType {
            id: "media/image".into(),
            name: "Image Asset".into(),
            domain: ContentDomain::Media,
            mime_types: vec![
                "image/png".into(),
                "image/jpeg".into(),
                "image/webp".into(),
                "image/tiff".into(),
            ],
            extensions: vec![
                "png".into(),
                "jpg".into(),
                "jpeg".into(),
                "webp".into(),
                "tiff".into(),
                "tif".into(),
                "bmp".into(),
            ],
            magic_bytes: vec!["89504E47".into(), "FFD8FF".into()], // PNG, JPEG
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(256 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "width": {"type": "integer"},
                    "height": {"type": "integer"},
                    "format": {"type": "string"},
                    "color_space": {"type": "string"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "Raster image asset".into(),
        },
        ContentType {
            id: "media/video".into(),
            name: "Video Asset".into(),
            domain: ContentDomain::Media,
            mime_types: vec![
                "video/mp4".into(),
                "video/webm".into(),
                "video/quicktime".into(),
            ],
            extensions: vec![
                "mp4".into(),
                "webm".into(),
                "mov".into(),
                "mkv".into(),
                "avi".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::External,
            lfs_threshold: Some(10 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "duration_seconds": {"type": "number"},
                    "resolution": {"type": "string"},
                    "codec": {"type": "string"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "Video media asset".into(),
        },
        ContentType {
            id: "media/audio".into(),
            name: "Audio Asset".into(),
            domain: ContentDomain::Media,
            mime_types: vec![
                "audio/mpeg".into(),
                "audio/wav".into(),
                "audio/flac".into(),
                "audio/ogg".into(),
            ],
            extensions: vec![
                "mp3".into(),
                "wav".into(),
                "flac".into(),
                "ogg".into(),
                "aac".into(),
            ],
            magic_bytes: vec!["494433".into(), "52494646".into()], // "ID3", "RIFF"
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Audio media asset".into(),
        },
        // ── Geospatial ──
        ContentType {
            id: "geo/geojson".into(),
            name: "GeoJSON".into(),
            domain: ContentDomain::Geospatial,
            mime_types: vec!["application/geo+json".into()],
            extensions: vec!["geojson".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::ComponentLevel,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "feature_count": {"type": "integer"},
                    "geometry_types": {"type": "array", "items": {"type": "string"}},
                    "crs": {"type": "string"}
                }
            })),
            structural_diff: true,
            component_locking: false,
            description: "RFC 7946 GeoJSON geographic data".into(),
        },
        ContentType {
            id: "geo/shapefile".into(),
            name: "Shapefile".into(),
            domain: ContentDomain::Geospatial,
            mime_types: vec!["application/x-shapefile".into()],
            extensions: vec!["shp".into(), "shx".into(), "dbf".into(), "prj".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(5 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "ESRI Shapefile geospatial vector data".into(),
        },
        // ── Legal / Financial ──
        ContentType {
            id: "legal/pdf".into(),
            name: "PDF Document".into(),
            domain: ContentDomain::Legal,
            mime_types: vec!["application/pdf".into()],
            extensions: vec!["pdf".into()],
            magic_bytes: vec!["25504446".into()], // "%PDF"
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "page_count": {"type": "integer"},
                    "signed": {"type": "boolean"},
                    "version": {"type": "string"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "Portable Document Format".into(),
        },
        ContentType {
            id: "financial/xlsx".into(),
            name: "Excel Spreadsheet".into(),
            domain: ContentDomain::Financial,
            mime_types: vec![
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
            ],
            extensions: vec!["xlsx".into()],
            magic_bytes: vec!["504B0304".into()], // ZIP
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::SchemaAware,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(5 * 1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "sheet_count": {"type": "integer"},
                    "row_count": {"type": "integer"},
                    "has_macros": {"type": "boolean"}
                }
            })),
            structural_diff: true,
            component_locking: true,
            description: "Microsoft Excel OOXML spreadsheet".into(),
        },
        // ── Config / Infrastructure ──
        ContentType {
            id: "config/terraform".into(),
            name: "Terraform HCL".into(),
            domain: ContentDomain::Config,
            mime_types: vec!["text/x-hcl".into()],
            extensions: vec!["tf".into(), "tfvars".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "HashiCorp Terraform infrastructure-as-code".into(),
        },
        ContentType {
            id: "config/kubernetes".into(),
            name: "Kubernetes Manifest".into(),
            domain: ContentDomain::Config,
            mime_types: vec!["application/x-yaml".into()],
            extensions: vec!["yaml".into(), "yml".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Structural,
            merge_strategy: MergeStrategy::SchemaAware,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: true,
            component_locking: false,
            description: "Kubernetes resource manifests (YAML)".into(),
        },
        // ── Simulation (FEA / CFD / multiphysics) ──
        ContentType {
            id: "sim/nastran".into(),
            name: "Nastran Bulk Data".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["text/x-nastran".into()],
            extensions: vec!["bdf".into(), "nas".into(), "dat".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "MSC/NX Nastran bulk-data input deck".into(),
        },
        ContentType {
            id: "sim/nastran-op2".into(),
            name: "Nastran OP2 Results".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-nastran-op2".into()],
            extensions: vec!["op2".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Nastran OUTPUT2 binary results database".into(),
        },
        ContentType {
            id: "sim/abaqus".into(),
            name: "Abaqus Input Deck".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["text/x-abaqus".into()],
            extensions: vec!["inp".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Abaqus/Standard keyword input deck".into(),
        },
        ContentType {
            id: "sim/abaqus-odb".into(),
            name: "Abaqus ODB Results".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-abaqus-odb".into()],
            extensions: vec!["odb".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Abaqus output database (binary results)".into(),
        },
        ContentType {
            id: "sim/ansys-cdb".into(),
            name: "ANSYS CDB Archive".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["text/x-ansys-cdb".into()],
            extensions: vec!["cdb".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "ANSYS APDL CDB model archive".into(),
        },
        ContentType {
            id: "sim/ansys-db".into(),
            name: "ANSYS Database / Results".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-ansys".into()],
            extensions: vec!["db".into(), "rst".into(), "rth".into(), "rmg".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "ANSYS binary database/results files".into(),
        },
        ContentType {
            id: "sim/lsdyna".into(),
            name: "LS-DYNA Keyword".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["text/x-lsdyna".into()],
            extensions: vec!["k".into(), "key".into(), "dyn".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "LS-DYNA keyword input deck".into(),
        },
        ContentType {
            id: "sim/openfoam".into(),
            name: "OpenFOAM Case".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["text/x-openfoam".into()],
            extensions: vec!["foam".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "OpenFOAM case dictionary / field data".into(),
        },
        ContentType {
            id: "sim/comsol".into(),
            name: "COMSOL Model".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-comsol".into()],
            extensions: vec!["mph".into()],
            magic_bytes: vec!["504B0304".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "COMSOL Multiphysics model file".into(),
        },
        ContentType {
            id: "sim/gmsh".into(),
            name: "Gmsh Mesh / Geometry".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["text/x-gmsh".into()],
            extensions: vec!["msh".into(), "geo".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Standard,
            lfs_threshold: Some(50 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Gmsh mesh (.msh) and geometry (.geo)".into(),
        },
        ContentType {
            id: "sim/vtk".into(),
            name: "VTK Visualization".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-vtk".into()],
            extensions: vec![
                "vtk".into(),
                "vtu".into(),
                "vtp".into(),
                "vti".into(),
                "vtr".into(),
                "vts".into(),
                "pvd".into(),
                "pvtu".into(),
            ],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(5 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "VTK/ParaView mesh & field visualization data".into(),
        },
        ContentType {
            id: "sim/cgns".into(),
            name: "CGNS".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-cgns".into()],
            extensions: vec!["cgns".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(5 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "CFD General Notation System (HDF5-based)".into(),
        },
        ContentType {
            id: "sim/exodus".into(),
            name: "Exodus II".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-exodus".into()],
            extensions: vec!["exo".into(), "exoii".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(5 * 1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Exodus II finite-element results (netCDF-based)".into(),
        },
        ContentType {
            id: "sim/modelica".into(),
            name: "Modelica Model".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["text/x-modelica".into()],
            extensions: vec!["mo".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Text,
            merge_strategy: MergeStrategy::TextThreeWay,
            storage_tier: StorageTier::Standard,
            lfs_threshold: None,
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Modelica equation-based system model source".into(),
        },
        ContentType {
            id: "sim/simulink".into(),
            name: "Simulink Model".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-simulink".into()],
            extensions: vec!["slx".into(), "mdl".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "MathWorks Simulink block-diagram model".into(),
        },
        ContentType {
            id: "sim/fmu".into(),
            name: "Functional Mock-up Unit".into(),
            domain: ContentDomain::Simulation,
            mime_types: vec!["application/x-fmu".into()],
            extensions: vec!["fmu".into()],
            magic_bytes: vec!["504B0304".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: true,
            description: "FMI Functional Mock-up Unit (co-simulation)".into(),
        },
        // ── AI / ML model formats ──
        ContentType {
            id: "ml/onnx".into(),
            name: "ONNX Model".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-onnx".into()],
            extensions: vec!["onnx".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "opset_version": {"type": "integer"},
                    "producer": {"type": "string"},
                    "input_count": {"type": "integer"},
                    "output_count": {"type": "integer"},
                    "parameter_count": {"type": "integer"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "Open Neural Network Exchange model (protobuf)".into(),
        },
        ContentType {
            id: "ml/safetensors".into(),
            name: "SafeTensors Weights".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-safetensors".into()],
            extensions: vec!["safetensors".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "tensor_count": {"type": "integer"},
                    "dtype": {"type": "string"},
                    "total_parameters": {"type": "integer"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "SafeTensors safe zero-copy tensor weights".into(),
        },
        ContentType {
            id: "ml/pytorch".into(),
            name: "PyTorch Checkpoint".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-pytorch".into()],
            extensions: vec!["pt".into(), "pth".into(), "bin".into()],
            magic_bytes: vec!["504B0304".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "PyTorch serialized model/state-dict (ZIP/pickle)".into(),
        },
        ContentType {
            id: "ml/tensorflow".into(),
            name: "TensorFlow SavedModel".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-tensorflow".into()],
            extensions: vec!["pb".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "TensorFlow GraphDef / SavedModel protobuf".into(),
        },
        ContentType {
            id: "ml/keras".into(),
            name: "Keras Model".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-keras".into()],
            extensions: vec!["keras".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Keras v3 model archive".into(),
        },
        ContentType {
            id: "ml/gguf".into(),
            name: "GGUF / GGML Model".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-gguf".into()],
            extensions: vec!["gguf".into(), "ggml".into()],
            magic_bytes: vec!["47475546".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::External,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "architecture": {"type": "string"},
                    "quantization": {"type": "string"},
                    "parameter_count": {"type": "integer"},
                    "context_length": {"type": "integer"}
                }
            })),
            structural_diff: false,
            component_locking: false,
            description: "GGUF/GGML quantized LLM weights (llama.cpp)".into(),
        },
        ContentType {
            id: "ml/tensorrt".into(),
            name: "TensorRT Engine".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-tensorrt".into()],
            extensions: vec!["engine".into(), "plan".into(), "trt".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "NVIDIA TensorRT serialized inference engine".into(),
        },
        ContentType {
            id: "ml/coreml".into(),
            name: "Core ML Model".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-coreml".into()],
            extensions: vec!["mlmodel".into(), "mlpackage".into(), "mlmodelc".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::ManualResolve,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Apple Core ML model package".into(),
        },
        ContentType {
            id: "ml/tflite".into(),
            name: "TensorFlow Lite".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-tflite".into()],
            extensions: vec!["tflite".into(), "lite".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "TensorFlow Lite flatbuffer model (edge/mobile)".into(),
        },
        ContentType {
            id: "ml/pickle".into(),
            name: "Python Pickle".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-python-pickle".into()],
            extensions: vec!["pkl".into(), "pickle".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Python pickle serialized object (untrusted: arbitrary code on load)"
                .into(),
        },
        ContentType {
            id: "ml/numpy".into(),
            name: "NumPy Array".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-numpy".into()],
            extensions: vec!["npy".into(), "npz".into()],
            magic_bytes: vec!["934E554D5059".into()],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "NumPy .npy/.npz array data".into(),
        },
        ContentType {
            id: "ml/checkpoint".into(),
            name: "Model Checkpoint".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-checkpoint".into()],
            extensions: vec!["ckpt".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "Generic training checkpoint (Lightning/TF/Diffusers)".into(),
        },
        ContentType {
            id: "ml/joblib".into(),
            name: "Joblib Model".into(),
            domain: ContentDomain::MlModel,
            mime_types: vec!["application/x-joblib".into()],
            extensions: vec!["joblib".into()],
            magic_bytes: vec![],
            diff_strategy: DiffStrategy::Opaque,
            merge_strategy: MergeStrategy::LastWriterWins,
            storage_tier: StorageTier::Lfs,
            lfs_threshold: Some(1024 * 1024),
            metadata_schema: None,
            structural_diff: false,
            component_locking: false,
            description: "scikit-learn / joblib serialized estimator".into(),
        },
    ]
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn types_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("content-types")
}

fn save_type(repo_root: &Path, ct: &ContentType) -> Result<(), LitError> {
    let dir = types_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Create content-types dir: {}", e)))?;
    let safe_id: String = ct.id.replace('/', "_");
    let path = dir.join(format!("{}.json", safe_id));
    let json = serde_json::to_string_pretty(ct)
        .map_err(|e| LitError::general(format!("Serialize content type: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write content type: {}", e)))?;
    Ok(())
}

fn load_all_types(repo_root: &Path) -> Result<Vec<ContentType>, LitError> {
    let dir = types_dir(repo_root);
    let mut types = builtin_types();

    // Overlay custom types from repo
    if dir.exists() {
        for entry in fs::read_dir(&dir).map_err(|e| LitError::io(e.to_string()))? {
            let entry = entry.map_err(|e| LitError::io(e.to_string()))?;
            if entry
                .path()
                .extension()
                .map(|e| e == "json")
                .unwrap_or(false)
            {
                let json =
                    fs::read_to_string(entry.path()).map_err(|e| LitError::io(e.to_string()))?;
                if let Ok(ct) = serde_json::from_str::<ContentType>(&json) {
                    // Custom types override builtins with the same id
                    types.retain(|t| t.id != ct.id);
                    types.push(ct);
                }
            }
        }
    }
    Ok(types)
}

/// Detect content type for a file by extension, then magic bytes
pub fn detect(file_path: &str, first_bytes: Option<&[u8]>) -> Option<ContentType> {
    let ext = file_path
        .rsplit('.')
        .next()
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // Try builtins first (avoid needing repo root for detection)
    let all = builtin_types();

    // Extension match
    if let Some(ct) = all.iter().find(|t| t.extensions.contains(&ext)) {
        return Some(ct.clone());
    }

    // Magic bytes match
    if let Some(bytes) = first_bytes {
        let hex: String = bytes
            .iter()
            .take(16)
            .map(|b| format!("{:02X}", b))
            .collect();
        if let Some(ct) = all
            .iter()
            .find(|t| t.magic_bytes.iter().any(|mb| hex.starts_with(mb)))
        {
            return Some(ct.clone());
        }
    }

    None
}

// ── Public API ──────────────────────────────────────────────────────────────

/// List all registered content types, optionally filtered by domain
pub fn execute_list(domain_filter: Option<String>) -> Result<ContentTypeResponse, LitError> {
    let repo_root = find_repo_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut types = load_all_types(&repo_root)?;

    if let Some(ref domain) = domain_filter {
        types.retain(|t| t.domain.to_string() == *domain);
    }

    let count = types.len();
    Ok(ContentTypeResponse {
        action: "list".into(),
        content_type_id: None,
        message: format!("{} content type(s)", count),
        details: Some(serde_json::to_value(&types).unwrap_or_default()),
    })
}

/// Show a specific content type
pub fn execute_show(type_id: String) -> Result<ContentTypeResponse, LitError> {
    let repo_root = find_repo_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let types = load_all_types(&repo_root)?;

    let ct = types
        .iter()
        .find(|t| t.id == type_id)
        .ok_or_else(|| LitError::general(format!("Content type not found: {}", type_id)))?;

    Ok(ContentTypeResponse {
        action: "show".into(),
        content_type_id: Some(ct.id.clone()),
        message: format!("{} ({})", ct.name, ct.domain),
        details: Some(serde_json::to_value(ct).unwrap_or_default()),
    })
}

/// Register a custom content type
pub fn execute_register(
    id: String,
    name: String,
    domain: String,
    extensions: Vec<String>,
    diff_strategy: Option<String>,
    merge_strategy: Option<String>,
    storage_tier: Option<String>,
) -> Result<ContentTypeResponse, LitError> {
    let repo_root = find_repo_root()?;

    let domain_enum = match domain.as_str() {
        "software" => ContentDomain::Software,
        "cad" => ContentDomain::Cad,
        "eda" => ContentDomain::Eda,
        "cam" => ContentDomain::Cam,
        "simulation" | "sim" | "fea" | "cfd" => ContentDomain::Simulation,
        "ml-model" | "ml" | "ai" | "model" => ContentDomain::MlModel,
        "manuscript" => ContentDomain::Manuscript,
        "database" => ContentDomain::Database,
        "scientific" => ContentDomain::Scientific,
        "media" => ContentDomain::Media,
        "geospatial" => ContentDomain::Geospatial,
        "legal" => ContentDomain::Legal,
        "financial" => ContentDomain::Financial,
        "config" => ContentDomain::Config,
        "documentation" => ContentDomain::Documentation,
        other => ContentDomain::Custom(other.to_string()),
    };

    let diff = match diff_strategy.as_deref() {
        Some("text") => DiffStrategy::Text,
        Some("binary") => DiffStrategy::Binary,
        Some("structural") => DiffStrategy::Structural,
        Some("semantic") => DiffStrategy::Semantic,
        Some("opaque") => DiffStrategy::Opaque,
        _ => DiffStrategy::Binary,
    };

    let merge = match merge_strategy.as_deref() {
        Some("text-three-way") => MergeStrategy::TextThreeWay,
        Some("manual-resolve") => MergeStrategy::ManualResolve,
        Some("schema-aware") => MergeStrategy::SchemaAware,
        Some("component-level") => MergeStrategy::ComponentLevel,
        Some("append-only") => MergeStrategy::AppendOnly,
        Some("last-writer-wins") => MergeStrategy::LastWriterWins,
        _ => MergeStrategy::ManualResolve,
    };

    let tier = match storage_tier.as_deref() {
        Some("standard") => StorageTier::Standard,
        Some("lfs") => StorageTier::Lfs,
        Some("chunked") => StorageTier::Chunked,
        Some("external") => StorageTier::External,
        _ => StorageTier::Lfs,
    };

    let ct = ContentType {
        id: id.clone(),
        name: name.clone(),
        domain: domain_enum,
        mime_types: vec![],
        extensions,
        magic_bytes: vec![],
        diff_strategy: diff,
        merge_strategy: merge,
        storage_tier: tier,
        lfs_threshold: None,
        metadata_schema: None,
        structural_diff: false,
        component_locking: false,
        description: format!("Custom content type: {}", name),
    };

    save_type(&repo_root, &ct)?;

    Ok(ContentTypeResponse {
        action: "register".into(),
        content_type_id: Some(id),
        message: format!("Content type '{}' registered", name),
        details: Some(serde_json::to_value(&ct).unwrap_or_default()),
    })
}

/// Detect the content type(s) of one or more files
pub fn execute_detect(paths: Vec<String>) -> Result<ContentTypeResponse, LitError> {
    let mut results: HashMap<String, serde_json::Value> = HashMap::new();

    for path in &paths {
        let first_bytes = fs::read(path).ok().map(|b| b[..b.len().min(16)].to_vec());
        let detected = detect(path, first_bytes.as_deref());
        results.insert(
            path.clone(),
            match detected {
                Some(ct) => serde_json::to_value(&ct).unwrap_or_default(),
                None => serde_json::json!({"detected": false}),
            },
        );
    }

    Ok(ContentTypeResponse {
        action: "detect".into(),
        content_type_id: None,
        message: format!("Detected types for {} file(s)", paths.len()),
        details: Some(serde_json::to_value(&results).unwrap_or_default()),
    })
}
