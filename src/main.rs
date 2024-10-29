mod utils;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use cairo_lang_project::{AllCratesConfig, ProjectConfigContent};
use cairo_lang_filesystem::db::{CrateSettings, DependencySettings, Edition};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use clap::Parser;
use scarb_ui::args::PackagesFilter;
use crate::utils::{scarb_cfg_set_to_cairo, scarb_package_edition, scarb_package_experimental_features};

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

    #[arg(long)]
    no_deps: bool,
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
                "could not find a compilation unit suitable for ejection for \
                package {}",
                main_package.id
            )
        })?;

    let crate_roots = compilation_unit
        .components
        .iter()
        .filter(|c| c.name != "core")
        .map(|c| (c.name.clone().into(), c.source_root().into()))
        .collect();

    let edition = scarb_package_edition(&Some(&main_package), main_package.name.as_str());
    let version = main_package.version.clone();
    let cfg_set = scarb_cfg_set_to_cairo(&compilation_unit.cfg, main_package.name.as_str());

    let dependencies = if args.no_deps {
        BTreeMap::new()
    } else {
        compilation_unit
            .components
            .iter()
            .filter(|c| c.name != "core")
            .map(|c| {
                (
                    c.name.clone().into(),
                    DependencySettings {
                        discriminator: c.discriminator.clone().map(|d| d.into()),
                    }
                )
            })
            .collect()
    };

    let experimental_features = scarb_package_experimental_features(&Some(&main_package));

    let crate_settings = CrateSettings {
        edition,
        version: Some(version),
        cfg_set,
        dependencies,
        experimental_features,
    };

    let crates_config = AllCratesConfig {
        global: crate_settings,
        override_map: OrderedHashMap::default(),
    };

    let project_config = ProjectConfigContent {
        crate_roots,
        crates_config,
    };

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
