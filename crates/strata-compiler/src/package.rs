use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::SourceFile;

pub const MANIFEST_FILE_NAME: &str = "strata.package";
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

    /// Loads a package from a `strata.package` file or a directory containing one.
    ///
    /// The compact manifest is line-oriented:
    /// `package <identity>`, `prelude <true|false>`, and one or more
    /// `source <relative-path>` entries. Source units are assembled in sorted path order.
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

        let mut identity = None;
        let mut prelude = true;
        let mut prelude_seen = false;
        let mut paths = BTreeSet::new();
        let mut errors = Vec::new();
        for (line_index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(char::is_whitespace) else {
                errors.push(PackageLoadError {
                    path: manifest_path.clone(),
                    message: format!("line {}: expected a manifest value", line_index + 1),
                });
                continue;
            };
            let value = value.trim();
            match key {
                "package" if identity.is_none() && !value.is_empty() => {
                    identity = Some(value.to_owned())
                }
                "package" => errors.push(PackageLoadError {
                    path: manifest_path.clone(),
                    message: format!(
                        "line {}: duplicate or empty package identity",
                        line_index + 1
                    ),
                }),
                "prelude" if !prelude_seen && matches!(value, "true" | "false") => {
                    prelude_seen = true;
                    prelude = value == "true";
                }
                "prelude" => errors.push(PackageLoadError {
                    path: manifest_path.clone(),
                    message: format!(
                        "line {}: prelude must be `true` or `false` and appear once",
                        line_index + 1
                    ),
                }),
                "source" if valid_relative_source(value) => {
                    if !paths.insert(PathBuf::from(value)) {
                        errors.push(PackageLoadError {
                            path: manifest_path.clone(),
                            message: format!("line {}: duplicate source `{value}`", line_index + 1),
                        });
                    }
                }
                "source" => errors.push(PackageLoadError {
                    path: manifest_path.clone(),
                    message: format!(
                        "line {}: source must be a relative `.strata` path",
                        line_index + 1
                    ),
                }),
                _ => errors.push(PackageLoadError {
                    path: manifest_path.clone(),
                    message: format!("line {}: unknown manifest field `{key}`", line_index + 1),
                }),
            }
        }
        if identity.is_none() {
            errors.push(PackageLoadError {
                path: manifest_path.clone(),
                message: "missing `package` identity".to_owned(),
            });
        }
        if paths.is_empty() {
            errors.push(PackageLoadError {
                path: manifest_path.clone(),
                message: "package must enumerate at least one source".to_owned(),
            });
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let mut units = Vec::with_capacity(paths.len());
        for (id, relative_path) in paths.into_iter().enumerate() {
            let source_path = root.join(&relative_path);
            match fs::read_to_string(&source_path) {
                Ok(source_text) => units.push(SourceUnit {
                    relative_path,
                    source: SourceFile::new(id as u32, source_path, source_text),
                }),
                Err(error) => errors.push(PackageLoadError {
                    path: source_path,
                    message: format!("cannot read package source: {error}"),
                }),
            }
        }
        if errors.is_empty() {
            Ok(Self {
                identity: identity.unwrap(),
                root,
                prelude,
                units,
            })
        } else {
            Err(errors)
        }
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
