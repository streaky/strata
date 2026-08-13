use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::SourceFile;

pub const MANIFEST_FILE_NAME: &str = "package.toml";
pub const IMPLICIT_PACKAGE_ID: &str = "single-file";

#[derive(Clone, Debug)]
pub struct SourceUnit {
    pub relative_path: PathBuf,
    pub source: SourceFile,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub identity: String,
    pub root: PathBuf,
    pub prelude: bool,
    pub units: Vec<SourceUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageLoadError {
    pub path: PathBuf,
    pub message: String,
}

impl Package {
    #[must_use]
    pub fn implicit(path: impl Into<PathBuf>, text: String) -> Self {
        let path = path.into();
        let root = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let relative_path = path
            .strip_prefix(&root)
            .map_or_else(|_| path.clone(), Path::to_path_buf);
        Self {
            identity: IMPLICIT_PACKAGE_ID.to_owned(),
            root,
            prelude: true,
            units: vec![SourceUnit {
                relative_path,
                source: SourceFile::new(0, path, text),
            }],
        }
    }

    /// The manifest is TOML with required `package` and `sources` fields and
    /// an optional `prelude` boolean. Source units are assembled in sorted path order.
    ///
    /// # Errors
    ///
    /// Returns every manifest validation error, or every source file read error.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Vec<PackageLoadError>> {
        let requested = path.as_ref();
        let manifest_path = if requested.is_dir() {
            requested.join(MANIFEST_FILE_NAME)
        } else {
            requested.to_path_buf()
        };
        let root = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let text = fs::read_to_string(&manifest_path).map_err(|error| {
            vec![PackageLoadError {
                path: manifest_path.clone(),
                message: format!("cannot read package manifest: {error}"),
            }]
        })?;
        let manifest = parse_manifest(&manifest_path, &text)?;
        let units = load_source_units(&root, manifest.paths)?;
        Ok(Self {
            identity: manifest.identity,
            root,
            prelude: manifest.prelude,
            units,
        })
    }
}

struct ParsedManifest {
    identity: String,
    prelude: bool,
    paths: BTreeSet<PathBuf>,
}

fn parse_manifest(
    manifest_path: &Path,
    text: &str,
) -> Result<ParsedManifest, Vec<PackageLoadError>> {
    let table = text.parse::<toml::Table>().map_err(|error| {
        vec![PackageLoadError {
            path: manifest_path.to_path_buf(),
            message: format!("invalid TOML: {error}"),
        }]
    })?;
    let mut errors = Vec::new();
    for key in table.keys() {
        if !matches!(key.as_str(), "package" | "prelude" | "sources") {
            errors.push(PackageLoadError {
                path: manifest_path.to_path_buf(),
                message: format!("unknown manifest field `{key}`"),
            });
        }
    }
    let identity = match table.get("package") {
        Some(toml::Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(_) => {
            errors.push(PackageLoadError {
                path: manifest_path.to_path_buf(),
                message: "`package` must be a non-empty string".to_owned(),
            });
            None
        }
        None => {
            errors.push(PackageLoadError {
                path: manifest_path.to_path_buf(),
                message: "missing `package` identity".to_owned(),
            });
            None
        }
    };
    let prelude = match table.get("prelude") {
        Some(toml::Value::Boolean(value)) => *value,
        Some(_) => {
            errors.push(PackageLoadError {
                path: manifest_path.to_path_buf(),
                message: "`prelude` must be a boolean".to_owned(),
            });
            true
        }
        None => true,
    };
    let mut paths = BTreeSet::new();
    match table.get("sources") {
        Some(toml::Value::Array(values)) if !values.is_empty() => {
            for value in values {
                let Some(value) = value.as_str() else {
                    errors.push(PackageLoadError {
                        path: manifest_path.to_path_buf(),
                        message: "every `sources` entry must be a string".to_owned(),
                    });
                    continue;
                };
                if !valid_relative_source(value) {
                    errors.push(PackageLoadError {
                        path: manifest_path.to_path_buf(),
                        message: format!("source `{value}` must be a relative `.strata` path"),
                    });
                } else if !paths.insert(PathBuf::from(value)) {
                    errors.push(PackageLoadError {
                        path: manifest_path.to_path_buf(),
                        message: format!("duplicate source `{value}`"),
                    });
                }
            }
        }
        Some(_) => errors.push(PackageLoadError {
            path: manifest_path.to_path_buf(),
            message: "`sources` must be a non-empty array".to_owned(),
        }),
        None => errors.push(PackageLoadError {
            path: manifest_path.to_path_buf(),
            message: "package must enumerate at least one source".to_owned(),
        }),
    }
    if errors.is_empty() {
        Ok(ParsedManifest {
            identity: identity.expect("validated package identity"),
            prelude,
            paths,
        })
    } else {
        Err(errors)
    }
}

fn load_source_units(
    root: &Path,
    paths: BTreeSet<PathBuf>,
) -> Result<Vec<SourceUnit>, Vec<PackageLoadError>> {
    let mut units = Vec::with_capacity(paths.len());
    let mut errors = Vec::new();
    for (id, relative_path) in paths.into_iter().enumerate() {
        let source_path = root.join(&relative_path);
        let Ok(source_id) = u32::try_from(id) else {
            errors.push(PackageLoadError {
                path: source_path,
                message: "package has too many source units".to_owned(),
            });
            continue;
        };
        match fs::read_to_string(&source_path) {
            Ok(source_text) => units.push(SourceUnit {
                relative_path,
                source: SourceFile::new(source_id, source_path, source_text),
            }),
            Err(error) => errors.push(PackageLoadError {
                path: source_path,
                message: format!("cannot read package source: {error}"),
            }),
        }
    }
    if errors.is_empty() {
        Ok(units)
    } else {
        Err(errors)
    }
}

fn valid_relative_source(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && path
            .extension()
            .is_some_and(|extension| extension == "strata")
        && !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
}
