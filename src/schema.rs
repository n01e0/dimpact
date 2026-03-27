use std::fs;
use std::path::PathBuf;

use thiserror::Error;

pub const JSON_SCHEMA_MAJOR_VERSION: u32 = 1;
pub const JSON_SCHEMA_NAMESPACE: &str = "dimpact";
pub const JSON_SCHEMA_FORMAT: &str = "json";
pub const JSON_SCHEMA_ROOT: &str = "resources/schemas/json";
pub const JSON_SCHEMA_DRAFT_URL: &str = "https://json-schema.org/draft/2020-12/schema";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaOutputFormat {
    Json,
    Yaml,
    Dot,
    Html,
}

impl SchemaOutputFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Dot => "dot",
            Self::Html => "html",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaProfile {
    DiffDefault,
    ChangedDefault,
    IdDefault,
    Impact(ImpactSchemaProfile),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpactSchemaProfile {
    pub layout: ImpactSchemaLayout,
    pub edge_detail: ImpactSchemaEdgeDetail,
    pub graph_mode: ImpactSchemaGraphMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactSchemaLayout {
    Default,
    PerSeed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactSchemaEdgeDetail {
    SummaryOnly,
    WithEdges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactSchemaGraphMode {
    CallGraph,
    Pdg,
    Propagation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaCommand {
    Diff,
    Changed,
    Impact {
        per_seed: bool,
        with_edges: bool,
        with_pdg: bool,
        with_propagation: bool,
    },
    Id {
        raw: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaProfileInput {
    pub format: SchemaOutputFormat,
    pub command: SchemaCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSchemaProfile {
    pub profile: SchemaProfile,
    pub profile_slug: String,
    pub schema_id: String,
    pub schema_path: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SchemaProfileResolveError {
    #[error("schema profiles are only defined for json output (got {format})")]
    UnsupportedFormat { format: &'static str },
    #[error("schema profiles are not defined for subcommand '{subcommand}'")]
    UnsupportedCommand { subcommand: &'static str },
    #[error("schema profile is not available for raw id output")]
    RawIdOutput,
}

#[derive(Debug, Error)]
pub enum SchemaRegistryError {
    #[error("unknown schema id '{schema_id}'")]
    UnknownSchemaId { schema_id: String },
    #[error("failed to read schema document at '{schema_path}': {source}")]
    ReadSchemaDocument {
        schema_path: String,
        #[source]
        source: std::io::Error,
    },
}

impl SchemaProfile {
    pub fn profile_slug(self) -> String {
        match self {
            Self::DiffDefault => "diff/default".to_string(),
            Self::ChangedDefault => "changed/default".to_string(),
            Self::IdDefault => "id/default".to_string(),
            Self::Impact(impact) => format!(
                "impact/{}/{}/{}",
                impact.layout.slug(),
                impact.edge_detail.slug(),
                impact.graph_mode.slug()
            ),
        }
    }

    pub fn schema_id(self) -> String {
        format!(
            "{}:{}/v{}/{}",
            JSON_SCHEMA_NAMESPACE,
            JSON_SCHEMA_FORMAT,
            JSON_SCHEMA_MAJOR_VERSION,
            self.profile_slug()
        )
    }

    pub fn schema_path(self) -> String {
        format!(
            "{}/v{}/{}.schema.json",
            JSON_SCHEMA_ROOT,
            JSON_SCHEMA_MAJOR_VERSION,
            self.profile_slug()
        )
    }

    pub fn resolved(self) -> ResolvedSchemaProfile {
        ResolvedSchemaProfile {
            profile: self,
            profile_slug: self.profile_slug(),
            schema_id: self.schema_id(),
            schema_path: self.schema_path(),
        }
    }
}

impl ImpactSchemaLayout {
    fn slug(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::PerSeed => "per_seed",
        }
    }
}

impl ImpactSchemaEdgeDetail {
    fn slug(self) -> &'static str {
        match self {
            Self::SummaryOnly => "summary_only",
            Self::WithEdges => "with_edges",
        }
    }
}

impl ImpactSchemaGraphMode {
    fn slug(self) -> &'static str {
        match self {
            Self::CallGraph => "call_graph",
            Self::Pdg => "pdg",
            Self::Propagation => "propagation",
        }
    }
}

pub fn resolve_schema_profile(
    input: SchemaProfileInput,
) -> Result<ResolvedSchemaProfile, SchemaProfileResolveError> {
    if input.format != SchemaOutputFormat::Json {
        return Err(SchemaProfileResolveError::UnsupportedFormat {
            format: input.format.as_str(),
        });
    }

    let profile = match input.command {
        SchemaCommand::Diff => SchemaProfile::DiffDefault,
        SchemaCommand::Changed => SchemaProfile::ChangedDefault,
        SchemaCommand::Id { raw: true } => return Err(SchemaProfileResolveError::RawIdOutput),
        SchemaCommand::Id { raw: false } => SchemaProfile::IdDefault,
        SchemaCommand::Impact {
            per_seed,
            with_edges,
            with_pdg,
            with_propagation,
        } => SchemaProfile::Impact(ImpactSchemaProfile {
            layout: if per_seed {
                ImpactSchemaLayout::PerSeed
            } else {
                ImpactSchemaLayout::Default
            },
            edge_detail: if with_edges {
                ImpactSchemaEdgeDetail::WithEdges
            } else {
                ImpactSchemaEdgeDetail::SummaryOnly
            },
            graph_mode: if with_propagation {
                ImpactSchemaGraphMode::Propagation
            } else if with_pdg {
                ImpactSchemaGraphMode::Pdg
            } else {
                ImpactSchemaGraphMode::CallGraph
            },
        }),
    };

    Ok(profile.resolved())
}

pub fn registered_schema_profiles() -> Vec<SchemaProfile> {
    let mut profiles = vec![
        SchemaProfile::DiffDefault,
        SchemaProfile::ChangedDefault,
        SchemaProfile::IdDefault,
    ];

    for layout in [ImpactSchemaLayout::Default, ImpactSchemaLayout::PerSeed] {
        for edge_detail in [
            ImpactSchemaEdgeDetail::SummaryOnly,
            ImpactSchemaEdgeDetail::WithEdges,
        ] {
            for graph_mode in [
                ImpactSchemaGraphMode::CallGraph,
                ImpactSchemaGraphMode::Pdg,
                ImpactSchemaGraphMode::Propagation,
            ] {
                profiles.push(SchemaProfile::Impact(ImpactSchemaProfile {
                    layout,
                    edge_detail,
                    graph_mode,
                }));
            }
        }
    }

    profiles
}

pub fn list_registered_schemas() -> Vec<ResolvedSchemaProfile> {
    registered_schema_profiles()
        .into_iter()
        .map(SchemaProfile::resolved)
        .collect()
}

pub fn find_registered_schema(schema_id: &str) -> Option<ResolvedSchemaProfile> {
    list_registered_schemas()
        .into_iter()
        .find(|schema| schema.schema_id == schema_id)
}

fn schema_fs_path(schema: &ResolvedSchemaProfile) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&schema.schema_path)
}

pub fn read_schema_document(schema_id: &str) -> Result<String, SchemaRegistryError> {
    let schema =
        find_registered_schema(schema_id).ok_or_else(|| SchemaRegistryError::UnknownSchemaId {
            schema_id: schema_id.to_string(),
        })?;
    let fs_path = schema_fs_path(&schema);

    fs::read_to_string(&fs_path).map_err(|source| SchemaRegistryError::ReadSchemaDocument {
        schema_path: schema.schema_path,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_profile_slug_id_and_path_are_deterministic() {
        let resolved = resolve_schema_profile(SchemaProfileInput {
            format: SchemaOutputFormat::Json,
            command: SchemaCommand::Impact {
                per_seed: true,
                with_edges: true,
                with_pdg: true,
                with_propagation: true,
            },
        })
        .expect("resolve impact schema profile");

        assert_eq!(
            resolved.profile_slug,
            "impact/per_seed/with_edges/propagation"
        );
        assert_eq!(
            resolved.schema_id,
            "dimpact:json/v1/impact/per_seed/with_edges/propagation"
        );
        assert_eq!(
            resolved.schema_path,
            "resources/schemas/json/v1/impact/per_seed/with_edges/propagation.schema.json"
        );
    }

    #[test]
    fn propagation_mode_wins_over_pdg() {
        let resolved = resolve_schema_profile(SchemaProfileInput {
            format: SchemaOutputFormat::Json,
            command: SchemaCommand::Impact {
                per_seed: false,
                with_edges: false,
                with_pdg: true,
                with_propagation: true,
            },
        })
        .expect("resolve impact propagation profile");

        assert_eq!(
            resolved.profile,
            SchemaProfile::Impact(ImpactSchemaProfile {
                layout: ImpactSchemaLayout::Default,
                edge_detail: ImpactSchemaEdgeDetail::SummaryOnly,
                graph_mode: ImpactSchemaGraphMode::Propagation,
            })
        );
    }

    #[test]
    fn non_impact_profiles_use_default_family() {
        let diff = resolve_schema_profile(SchemaProfileInput {
            format: SchemaOutputFormat::Json,
            command: SchemaCommand::Diff,
        })
        .expect("resolve diff profile");
        assert_eq!(diff.profile, SchemaProfile::DiffDefault);
        assert_eq!(diff.profile_slug, "diff/default");

        let changed = resolve_schema_profile(SchemaProfileInput {
            format: SchemaOutputFormat::Json,
            command: SchemaCommand::Changed,
        })
        .expect("resolve changed profile");
        assert_eq!(changed.profile, SchemaProfile::ChangedDefault);
        assert_eq!(changed.profile_slug, "changed/default");

        let id = resolve_schema_profile(SchemaProfileInput {
            format: SchemaOutputFormat::Json,
            command: SchemaCommand::Id { raw: false },
        })
        .expect("resolve id profile");
        assert_eq!(id.profile, SchemaProfile::IdDefault);
        assert_eq!(id.profile_slug, "id/default");
    }

    #[test]
    fn non_json_format_is_unsupported() {
        let err = resolve_schema_profile(SchemaProfileInput {
            format: SchemaOutputFormat::Yaml,
            command: SchemaCommand::Diff,
        })
        .expect_err("yaml should be unsupported");

        assert_eq!(
            err,
            SchemaProfileResolveError::UnsupportedFormat { format: "yaml" }
        );
    }

    #[test]
    fn raw_id_output_is_unsupported() {
        let err = resolve_schema_profile(SchemaProfileInput {
            format: SchemaOutputFormat::Json,
            command: SchemaCommand::Id { raw: true },
        })
        .expect_err("raw id should be unsupported");

        assert_eq!(err, SchemaProfileResolveError::RawIdOutput);
    }

    #[test]
    fn registry_lists_all_expected_schema_profiles() {
        let schemas = list_registered_schemas();
        assert_eq!(schemas.len(), 15);
        assert_eq!(schemas[0].schema_id, "dimpact:json/v1/diff/default");
        assert_eq!(schemas[1].schema_id, "dimpact:json/v1/changed/default");
        assert_eq!(schemas[2].schema_id, "dimpact:json/v1/id/default");
        assert!(schemas.iter().any(|schema| {
            schema.schema_id == "dimpact:json/v1/impact/per_seed/with_edges/propagation"
        }));
    }

    #[test]
    fn registry_lookup_round_trips_known_schema_id() {
        let schema_id = "dimpact:json/v1/impact/default/summary_only/pdg";
        let resolved = find_registered_schema(schema_id).expect("schema should be registered");

        assert_eq!(resolved.schema_id, schema_id);
        assert_eq!(
            resolved.schema_path,
            "resources/schemas/json/v1/impact/default/summary_only/pdg.schema.json"
        );
    }

    #[test]
    fn read_schema_document_loads_registered_concrete_file() {
        let document =
            read_schema_document("dimpact:json/v1/diff/default").expect("read concrete schema");

        assert!(document.contains("\"$id\": \"dimpact:json/v1/diff/default\""));
        assert!(document.contains("\"status\": \"concrete\""));
        assert!(document.contains("\"change_kind\""));
    }

    #[test]
    fn read_schema_document_rejects_unknown_schema_id() {
        let err = read_schema_document("dimpact:json/v1/nope")
            .expect_err("unknown schema id should fail");

        assert!(matches!(err, SchemaRegistryError::UnknownSchemaId { .. }));
    }
}
