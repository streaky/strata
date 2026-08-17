use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Diagnostic, SourceFile, Span};

pub const MANIFEST_FILE_NAME: &str = "package.toml";
pub const IMPLICIT_PACKAGE_ID: &str = "single-file";

#[derive(Clone, Debug)]
pub struct SourceUnit {
    pub relative_path: PathBuf,
    pub source: SourceFile,
    pub expected_namespace: Option<String>,
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
                expected_namespace: None,
            }],
        }
    }

    /// The manifest is TOML with required `package` and `namespaces` fields and
    /// an optional `prelude` boolean. Source units are discovered in sorted path order.
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
        let units = discover_source_units(&root, &manifest.namespace_roots)?;
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
    namespace_roots: Vec<NamespaceRoot>,
}

#[derive(Clone, Debug)]
struct NamespaceRoot {
    namespace: String,
    directory: PathBuf,
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
        if !matches!(key.as_str(), "package" | "prelude" | "namespaces") {
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
    let namespace_roots = parse_namespace_roots(manifest_path, text, &table, &mut errors);
    if errors.is_empty() {
        Ok(ParsedManifest {
            identity: identity.expect("validated package identity"),
            prelude,
            namespace_roots,
        })
    } else {
        Err(errors)
    }
}

fn parse_namespace_roots(
    manifest_path: &Path,
    text: &str,
    table: &toml::Table,
    errors: &mut Vec<PackageLoadError>,
) -> Vec<NamespaceRoot> {
    let Some(toml::Value::Table(mappings)) = table.get("namespaces") else {
        errors.push(manifest_error(
            manifest_path,
            text,
            "package must declare a non-empty `namespaces` mapping table",
            Some("namespaces"),
        ));
        return Vec::new();
    };
    if mappings.is_empty() {
        errors.push(manifest_error(
            manifest_path,
            text,
            "`namespaces` must be a non-empty mapping table",
            Some("namespaces"),
        ));
    }
    let mut directories = BTreeMap::<PathBuf, String>::new();
    let mut roots = Vec::new();
    for (namespace, value) in mappings {
        if namespace == "/" {
            errors.push(manifest_error(
                manifest_path,
                text,
                "namespace root `/` cannot be declared by a source file",
                Some(namespace),
            ));
            continue;
        }
        let path = namespace.trim_start_matches('/');
        if let Some(segment) = path
            .split('/')
            .find(|segment| !valid_namespace_segment(segment))
        {
            errors.push(manifest_error(
                manifest_path,
                text,
                format!("namespace segment `{segment}` must match `[a-z]([a-z0-9]|-[a-z0-9])*`"),
                Some(namespace),
            ));
            continue;
        }
        if let Some(segment) = path
            .split('/')
            .find(|segment| reserved_namespace_segment(segment))
        {
            errors.push(manifest_error(
                manifest_path,
                text,
                format!("namespace segment `{segment}` is reserved"),
                Some(namespace),
            ));
            continue;
        }
        let canonical = format!("/{path}");
        let Some(directory) = value.as_str().and_then(normalized_relative_directory) else {
            errors.push(manifest_error(
                manifest_path,
                text,
                format!("directory for namespace root `{namespace}` must be a relative path"),
                Some(namespace),
            ));
            continue;
        };
        if let Some(existing) = directories.insert(directory.clone(), namespace.clone()) {
            errors.push(manifest_error(
                manifest_path,
                text,
                format!(
                    "namespace roots `{existing}` and `{namespace}` map to the same directory `{}`",
                    directory.display()
                ),
                Some(namespace),
            ));
            continue;
        }
        roots.push(NamespaceRoot {
            namespace: canonical,
            directory,
        });
    }
    roots.sort_by(|left, right| left.namespace.cmp(&right.namespace));
    roots
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

fn discover_source_units(
    root: &Path,
    namespace_roots: &[NamespaceRoot],
) -> Result<Vec<SourceUnit>, Vec<PackageLoadError>> {
    let mut discovered = BTreeMap::<PathBuf, (usize, String)>::new();
    let mut errors = Vec::new();
    for mapping in namespace_roots {
        let directory = root.join(&mapping.directory);
        let mut paths = BTreeSet::new();
        let errors_before_discovery = errors.len();
        discover_trn_files(&directory, root, &mut paths, &mut errors);
        if paths.is_empty() && errors.len() == errors_before_discovery {
            errors.push(PackageLoadError::unreadable(
                directory,
                format!(
                    "namespace root `{}` contains no `.trn` source files",
                    mapping.namespace
                ),
            ));
        }
        let depth = mapping.directory.components().count();
        for relative_path in paths {
            let suffix = relative_path
                .parent()
                .expect("discovered source has a parent")
                .strip_prefix(&mapping.directory)
                .expect("discovered source is beneath its normalized namespace root");
            let expected = match expected_namespace(&mapping.namespace, suffix) {
                Ok(expected) => expected,
                Err(message) => {
                    errors.push(PackageLoadError::unreadable(
                        root.join(&relative_path),
                        message,
                    ));
                    continue;
                }
            };
            match discovered.get(&relative_path) {
                Some((existing_depth, _)) if *existing_depth >= depth => {}
                _ => {
                    discovered.insert(relative_path, (depth, expected));
                }
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut units = Vec::with_capacity(discovered.len());
    for (id, (relative_path, (_, expected_namespace))) in discovered.into_iter().enumerate() {
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
                expected_namespace: Some(expected_namespace),
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

fn discover_trn_files(
    directory: &Path,
    root: &Path,
    paths: &mut BTreeSet<PathBuf>,
    errors: &mut Vec<PackageLoadError>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(PackageLoadError::unreadable(
                directory.to_path_buf(),
                format!("cannot read namespace directory: {error}"),
            ));
            return;
        }
    };
    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) if file_type.is_symlink() => match fs::metadata(&path) {
                Ok(metadata) => metadata.file_type(),
                Err(error) => {
                    errors.push(PackageLoadError::unreadable(
                        path,
                        format!("cannot inspect symlinked namespace source entry: {error}"),
                    ));
                    continue;
                }
            },
            Ok(file_type) => file_type,
            Err(_) => {
                errors.push(PackageLoadError::unreadable(
                    path,
                    "cannot inspect namespace source entry",
                ));
                continue;
            }
        };
        if file_type.is_dir() {
            discover_trn_files(&path, root, paths, errors);
        } else if file_type.is_file()
            && path.extension().is_some_and(|extension| extension == "trn")
        {
            match path.strip_prefix(root) {
                Ok(relative) => {
                    paths.insert(relative.to_path_buf());
                }
                Err(_) => errors.push(PackageLoadError::unreadable(
                    path,
                    "discovered source escapes the package root",
                )),
            }
        }
    }
}

fn expected_namespace(root: &str, suffix: &Path) -> Result<String, String> {
    let mut segments = Vec::new();
    for component in suffix.components() {
        let std::path::Component::Normal(value) = component else {
            return Err("source directory is not a normalized relative path".to_owned());
        };
        let Some(segment) = value.to_str() else {
            return Err("source directory contains a non-UTF-8 namespace segment".to_owned());
        };
        if !valid_namespace_segment(segment) {
            return Err(format!(
                "source directory segment `{segment}` must match `[a-z]([a-z0-9]|-[a-z0-9])*`"
            ));
        }
        if reserved_namespace_segment(segment) {
            return Err(format!("source directory segment `{segment}` is reserved"));
        }
        segments.push(segment);
    }
    if segments.is_empty() {
        Ok(root.to_owned())
    } else {
        Ok(format!("{root}/{}", segments.join("/")))
    }
}

fn valid_namespace_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    let mut previous_hyphen = false;
    for byte in bytes {
        if byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_hyphen = false;
        } else {
            return false;
        }
    }
    !previous_hyphen
}

fn reserved_namespace_segment(segment: &str) -> bool {
    matches!(segment, "con" | "prn" | "aux" | "nul")
        || segment
            .strip_prefix("com")
            .or_else(|| segment.strip_prefix("lpt"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

fn normalized_relative_directory(value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(component) => normalized.push(component),
            _ => return None,
        }
    }
    Some(normalized)
}
