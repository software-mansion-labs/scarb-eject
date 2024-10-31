use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use cairo_lang_filesystem::cfg::CfgSet;
use cairo_lang_filesystem::db::{
    CrateIdentifier, CrateSettings, DependencySettings, Edition, ExperimentalFeaturesConfig,
};
use cairo_lang_project::{AllCratesConfig, ProjectConfigContent};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use clap::Parser;
use scarb_metadata::{
    CompilationUnitComponentDependencyMetadata, CompilationUnitComponentMetadata,
    CompilationUnitMetadata, Metadata, PackageMetadata,
};
use scarb_ui::args::PackagesFilter;
use tracing::warn;

#[derive(Parser, Clone, Debug)]
#[command(about, author, version)]
struct Args {
    /// Path to `cairo_project.toml` file to overwrite.
    /// Defaults to next to `Scarb.toml` for this workspace.
    /// Use `-` to write to standard output.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    #[command(flatten)]
    packages_filter: PackagesFilter,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    let metadata = scarb_metadata::MetadataCommand::new()
        .inherit_stderr()
        .exec()?;

    let main_package = args.packages_filter.match_one(&metadata)?;

    let compilation_unit = metadata
        .compilation_units
        .iter()
        .filter(|unit| unit.package == main_package.id)
        .min_by_key(|unit| match unit.target.name.as_str() {
            name @ "starknet-contract" => (0, name),
            name @ "lib" => (1, name),
            name => (2, name),
        })
        .ok_or_else(|| {
            anyhow!(
                "could not find a compilation unit suitable for ejection for package {}",
                main_package.id
            )
        })?;

    let project_config = get_project_config(&metadata, compilation_unit, &main_package);

    let mut cairo_project_toml = toml::to_string_pretty(&project_config)?;
    cairo_project_toml.push('\n');

    let output = args.output.unwrap_or_else(|| {
        metadata
            .workspace
            .root
            .clone()
            .into_std_path_buf()
            .join("cairo_project.toml")
    });
    if output == Path::new("-") {
        println!("{cairo_project_toml}");
    } else {
        fs::write(output, cairo_project_toml)?;
    }

    Ok(())
}

pub fn get_project_config(
    metadata: &Metadata,
    compilation_unit: &CompilationUnitMetadata,
    main_package: &PackageMetadata,
) -> ProjectConfigContent {
    let crate_roots = get_crate_roots(compilation_unit);
    let crates_config = get_crates_config(metadata, compilation_unit, main_package);

    ProjectConfigContent {
        crate_roots,
        crates_config,
    }
}

fn get_crate_roots(
    compilation_unit_metadata: &CompilationUnitMetadata,
) -> OrderedHashMap<CrateIdentifier, PathBuf> {
    compilation_unit_metadata
        .components
        .iter()
        .filter(|c| c.name != "core")
        .map(|c| (c.name.clone().into(), c.source_root().into()))
        .collect()
}

fn get_crates_config(
    metadata: &Metadata,
    compilation_unit: &CompilationUnitMetadata,
    main_package: &PackageMetadata,
) -> AllCratesConfig {
    let global_crate_settings = get_global_crate_settings(compilation_unit, main_package);

    let override_map = compilation_unit
        .components
        .iter()
        .filter(|c| c.name != "core")
        .map(|component| {
            (
                component.name.clone().into(),
                get_crate_settings_for_component(component, compilation_unit, metadata),
            )
        })
        .collect();

    AllCratesConfig {
        global: global_crate_settings,
        override_map,
    }
}

fn get_global_crate_settings(
    compilation_unit: &CompilationUnitMetadata,
    package: &PackageMetadata,
) -> CrateSettings {
    let edition = get_edition(&Some(&package), package.name.as_str());

    let version = package.version.clone();

    let cfg_set = scarb_cfg_set_to_cairo(&compilation_unit.cfg, package.name.as_str());

    let dependencies = compilation_unit
        .components
        .iter()
        .filter(|c| c.name != "core")
        .map(|c| {
            (
                c.name.clone().into(),
                DependencySettings {
                    discriminator: c.discriminator.clone().map(|d| d.into()),
                },
            )
        })
        .collect();

    let experimental_features = get_experimental_features(&Some(&package));

    CrateSettings {
        name: None,
        edition,
        version: Some(version),
        cfg_set,
        dependencies,
        experimental_features,
    }
}

fn get_crate_settings_for_component(
    component: &CompilationUnitComponentMetadata,
    unit: &CompilationUnitMetadata,
    metadata: &Metadata,
) -> CrateSettings {
    let package = metadata
        .packages
        .iter()
        .find(|package| package.id == component.package);

    let edition = get_edition(&package, component.name.as_str());

    let version = package.map(|p| p.version.clone());

    let cfg_set = component
        .cfg
        .clone()
        .and_then(|cfg| scarb_cfg_set_to_cairo(&cfg, component.name.as_str()));

    let dependencies = component
        .dependencies
        .as_ref()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|CompilationUnitComponentDependencyMetadata { id, .. }| {
            unit.components
                .iter()
                .filter(|c| c.name != "core")
                .find_map(|c| {
                    c.id.as_ref().and_then(|component_id| {
                        if component_id == id {
                            Some((
                                c.name.clone(),
                                DependencySettings {
                                    discriminator: c.discriminator.clone().map(|d| d.into()),
                                },
                            ))
                        } else {
                            None
                        }
                    })
                })
        })
        .collect();

    let experimental_features = get_experimental_features(&package);

    CrateSettings {
        name: Some(component.name.clone().into()),
        edition,
        version,
        cfg_set,
        dependencies,
        experimental_features,
    }
}

/// Get the [`Edition`] from [`PackageMetadata`], or assume the default edition.
pub fn get_edition(package: &Option<&PackageMetadata>, crate_name: &str) -> Edition {
    package
        .and_then(|p| p.edition.clone())
        .and_then(|e| {
            serde_json::from_value(e.into())
                .with_context(|| format!("failed to parse edition of package: {crate_name}"))
                .inspect_err(|e| warn!("{e:?}"))
                .ok()
        })
        .unwrap_or_default()
}

/// Convert a slice of [`scarb_metadata::Cfg`]s to a [`cairo_lang_filesystem::cfg::CfgSet`].
///
/// The conversion is done the same way as in Scarb (except no panicking):
/// <https://github.com/software-mansion/scarb/blob/9fe97c8eb8620a1e2103e7f5251c5a9189e75716/scarb/src/ops/metadata.rs#L295-L302>
pub fn scarb_cfg_set_to_cairo(cfg_set: &[scarb_metadata::Cfg], crate_name: &str) -> Option<CfgSet> {
    serde_json::to_value(cfg_set)
        .and_then(serde_json::from_value)
        .with_context(|| {
            format!(
                "scarb metadata cfg did not convert identically to cairo one for crate: {crate_name}"
            )
        })
        .inspect_err(|e| warn!("{e:?}"))
        .ok()
}

/// Get [`ExperimentalFeaturesConfig`] from [`PackageMetadata`] fields.
pub fn get_experimental_features(package: &Option<&PackageMetadata>) -> ExperimentalFeaturesConfig {
    let contains = |feature: &str| -> bool {
        let Some(package) = package else { return false };
        package.experimental_features.contains(&feature.into())
    };

    ExperimentalFeaturesConfig {
        negative_impls: contains("negative_impls"),
        coupons: contains("coupons"),
    }
}
