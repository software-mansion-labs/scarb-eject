use anyhow::Context;
use cairo_lang_filesystem::db::{Edition, ExperimentalFeaturesConfig};
use scarb_metadata::PackageMetadata;
use tracing::warn;

/// Get the [`Edition`] from [`PackageMetadata`], or assume the default edition.
pub fn scarb_package_edition(package: &Option<&PackageMetadata>, crate_name: &str) -> Edition {
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
pub fn scarb_cfg_set_to_cairo(
    cfg_set: &[scarb_metadata::Cfg],
    crate_name: &str,
) -> Option<cairo_lang_filesystem::cfg::CfgSet> {
    serde_json::to_value(cfg_set)
        .and_then(serde_json::from_value)
        .with_context(|| {
            format!(
                "scarb metadata cfg did not convert identically to cairo one for crate: \
                 {crate_name}"
            )
        })
        .inspect_err(|e| warn!("{e:?}"))
        .ok()
}

/// Get [`ExperimentalFeaturesConfig`] from [`PackageMetadata`] fields.
pub fn scarb_package_experimental_features(
    package: &Option<&PackageMetadata>,
) -> ExperimentalFeaturesConfig {
    let contains = |feature: &str| -> bool {
        let Some(package) = package else { return false };
        package.experimental_features.iter().any(|f| f == feature)
    };

    ExperimentalFeaturesConfig {
        negative_impls: contains("negative_impls"),
        coupons: contains("coupons"),
    }
}
