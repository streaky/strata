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

    /// Loads a package from a `package.toml` file or a directory containing one.
    ///
    /// The compact manifest is line-oriented:
    /// `package <identity>`, `prelude <true|false>`, and one or more
    /// `source <relative-path>` entries. Source units are assembled in sorted path order.
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
            errors.push(manifest_error(
                manifest_path,
                line_index,
                "expected a manifest value",
            ));
            continue;
        };
        let value = value.trim();
        match key {
            "package" if identity.is_none() && !value.is_empty() => {
                identity = Some(value.to_owned());
            }
            "package" => errors.push(manifest_error(
                manifest_path,
                line_index,
                "duplicate or empty package identity",
            )),
            "prelude" if !prelude_seen && matches!(value, "true" | "false") => {
                prelude_seen = true;
                prelude = value == "true";
            }
            "prelude" => errors.push(manifest_error(
                manifest_path,
                line_index,
                "prelude must be `true` or `false` and appear once",
            )),
            "source" if valid_relative_source(value) => {
                if !paths.insert(PathBuf::from(value)) {
                    errors.push(manifest_error(
                        manifest_path,
                        line_index,
                        format!("duplicate source `{value}`"),
                    ));
                }
            }
            "source" => errors.push(manifest_error(
                manifest_path,
                line_index,
                "source must be a relative `.strata` path",
            )),
            _ => errors.push(manifest_error(
                manifest_path,
                line_index,
                format!("unknown manifest field `{key}`"),
            )),
        }
    }
    if identity.is_none() {
        errors.push(PackageLoadError {
            path: manifest_path.to_path_buf(),
            message: "missing `package` identity".to_owned(),
        });
    }
    if paths.is_empty() {
        errors.push(PackageLoadError {
            path: manifest_path.to_path_buf(),
            message: "package must enumerate at least one source".to_owned(),
        });
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

fn manifest_error(path: &Path, line_index: usize, message: impl Into<String>) -> PackageLoadError {
    PackageLoadError {
        path: path.to_path_buf(),
        message: format!("line {}: {}", line_index + 1, message.into()),
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
