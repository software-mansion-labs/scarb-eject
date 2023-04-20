use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use cairo_lang_project::ProjectConfigContent;
use clap::Parser;
use scarb_metadata::{Metadata, PackageId};

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

#[derive(Parser, Clone, Debug)]
struct PackagesFilter {
    /// Specify package to eject.
    #[arg(short, long, value_name = "SPEC")]
    package: Option<String>,
}

impl PackagesFilter {
    fn match_one(&self, metadata: &Metadata) -> Result<PackageId> {
        match &self.package {
            Some(name) => {
                let package = metadata
                    .packages
                    .iter()
                    .find(|pkg| pkg.name == *name)
                    .ok_or_else(|| anyhow!("package `{name}` not found in workspace"))?;

                if !metadata.workspace.members.contains(&package.id) {
                    bail!("package `{name}` is not a member of this workspace");
                }

                Ok(package.id.clone())
            }
            None => {
                let members = &metadata.workspace.members;
                match members.len() {
                    0 => bail!("workspace has no members"),
                    1 => Ok(members[0].clone()),
                    _ => bail!("workspace has multiple members, please specify package"),
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    let metadata = scarb_metadata::MetadataCommand::new()
        .inherit_stderr()
        .exec()?;

    let main_package_id = args.packages_filter.match_one(&metadata)?;

    let compilation_unit = metadata
        .compilation_units
        .iter()
        .filter(|unit| unit.package == main_package_id)
        .min_by_key(|unit| match unit.target.name.as_str() {
            name @ "starknet-contract" => (0, name),
            name @ "lib" => (1, name),
            name => (2, name),
        })
        .ok_or_else(|| {
            anyhow!(
                "could not find a compilation unit suitable for ejection for \
                package {main_package_id}"
            )
        })?;

    let crate_roots = compilation_unit
        .components
        .iter()
        .filter(|c| c.name != "core")
        .map(|c| (c.name.clone().into(), c.source_root().into()))
        .collect();

    let project_config = ProjectConfigContent { crate_roots };

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
