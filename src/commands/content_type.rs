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
