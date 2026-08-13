use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Diagnostic, SourceFile, Span};

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

#[derive(Clone, Debug)]
pub struct PackageLoadError {
    pub source: SourceFile,
    pub diagnostic: Diagnostic,
}

impl PackageLoadError {
    fn new(path: PathBuf, text: String, message: impl Into<String>, span: Option<Span>) -> Self {
        let source = SourceFile::new(0, path, text);
        let mut diagnostic = Diagnostic::unlocated_error("S2001", message);
        diagnostic.primary = span;
        Self { source, diagnostic }
    }

    fn unreadable(path: PathBuf, message: impl Into<String>) -> Self {
        Self::new(path, String::new(), message, None)
    }
}

impl Package {
    #[must_use]
    pub fn implicit(path: impl Into<PathBuf>, text: String) -> Self {
        let path = path.into();
        let root = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
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
            vec![PackageLoadError::unreadable(
                manifest_path.clone(),
                format!("cannot read package manifest: {error}"),
            )]
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
        let span = error
            .span()
            .map(|range| Span::new(0, range.start, range.end));
        vec![PackageLoadError::new(
            manifest_path.to_path_buf(),
            text.to_owned(),
            format!("invalid TOML: {error}"),
            span,
        )]
    })?;
    let mut errors = Vec::new();
    for key in table.keys() {
        if !matches!(key.as_str(), "package" | "prelude" | "sources") {
            errors.push(manifest_error(
                manifest_path,
                text,
                format!("unknown manifest field `{key}`"),
                Some(key),
            ));
        }
    }
    let identity = match table.get("package") {
        Some(toml::Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(_) => {
            errors.push(manifest_error(
                manifest_path,
                text,
                "`package` must be a non-empty string",
                Some("package"),
            ));
            None
        }
        None => {
            errors.push(manifest_error(
                manifest_path,
                text,
                "missing `package` identity",
                None,
            ));
            None
        }
    };
    let prelude = match table.get("prelude") {
        Some(toml::Value::Boolean(value)) => *value,
        Some(_) => {
            errors.push(manifest_error(
                manifest_path,
                text,
                "`prelude` must be a boolean",
                Some("prelude"),
            ));
            true
        }
        None => true,
    };
    let paths = parse_source_paths(manifest_path, text, &table, &mut errors);
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

fn parse_source_paths(
    manifest_path: &Path,
    text: &str,
    table: &toml::Table,
    errors: &mut Vec<PackageLoadError>,
) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    match table.get("sources") {
        Some(toml::Value::Array(values)) if !values.is_empty() => {
            for value in values {
                let Some(value) = value.as_str() else {
                    errors.push(manifest_error(
                        manifest_path,
                        text,
                        "every `sources` entry must be a string",
                        Some("sources"),
                    ));
                    continue;
                };
                if !valid_relative_source(value) {
                    errors.push(manifest_error(
                        manifest_path,
                        text,
                        format!("source `{value}` must be a relative `.strata` path"),
                        Some(value),
                    ));
                } else if !paths.insert(PathBuf::from(value)) {
                    errors.push(manifest_error(
                        manifest_path,
                        text,
                        format!("duplicate source `{value}`"),
                        Some(value),
                    ));
                }
            }
        }
        Some(_) => errors.push(manifest_error(
            manifest_path,
            text,
            "`sources` must be a non-empty array",
            Some("sources"),
        )),
        None => errors.push(manifest_error(
            manifest_path,
            text,
            "package must enumerate at least one source",
            None,
        )),
    }
    paths
}

fn manifest_error(
    path: &Path,
    text: &str,
    message: impl Into<String>,
    needle: Option<&str>,
) -> PackageLoadError {
    let span = needle.and_then(|needle| {
        text.find(needle)
            .map(|start| Span::new(0, start, start + needle.len()))
    });
    PackageLoadError::new(path.to_path_buf(), text.to_owned(), message, span)
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
            errors.push(PackageLoadError::unreadable(
                source_path,
                "package has too many source units",
            ));
            continue;
        };
        match fs::read_to_string(&source_path) {
            Ok(source_text) => units.push(SourceUnit {
                relative_path,
                source: SourceFile::new(source_id, source_path, source_text),
            }),
            Err(error) => errors.push(PackageLoadError::unreadable(
                source_path,
                format!("cannot read package source: {error}"),
            )),
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
