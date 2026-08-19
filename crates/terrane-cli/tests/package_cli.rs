use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "terrane-cli-package-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("package.toml"),
            "package = \"cli-package\"\nprelude = false\n[namespaces]\n\"cli/app\" = \"app\"\n\"cli/support\" = \"support\"\n",
        )
        .unwrap();
        fs::create_dir_all(path.join("app")).unwrap();
        fs::create_dir_all(path.join("support")).unwrap();
        fs::write(
            path.join("app/main.trn"),
            concat!(
                "namespace cli/app\n",
                "from /core/output import print\n",
                "function main\n",
                "  print; 'manifest CLI'\n",
            ),
        )
        .unwrap();
        fs::write(
            path.join("support/support.trn"),
            "namespace cli/support\npublic constant value = 1\n",
        )
        .unwrap();
        Self(path)
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn manifest_file_and_package_directory_use_the_shared_cli_pipeline() {
    let package = TempPackage::new();
    let executable = env!("CARGO_BIN_EXE_terrane");

    let rust = Command::new(executable)
        .args(["rust", package.0.join("package.toml").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        rust.status.success(),
        "{}",
        String::from_utf8_lossy(&rust.stderr)
    );
    let generated = String::from_utf8(rust.stdout).unwrap();
    assert!(generated.contains("// Source: main.trn"));
    assert!(generated.contains("// Namespace: cli/app"));

    let run = Command::new(executable)
        .args(["run", package.0.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "manifest CLI\n");

    let build = Command::new(executable)
        .args(["build", package.0.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let executable_path = PathBuf::from(String::from_utf8(build.stdout).unwrap().trim());
    let build_root = executable_path
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == ".trn"))
        .unwrap();
    assert_eq!(build_root.parent(), Some(package.0.as_path()));
    let generated_project = fs::read_dir(build_root.join("build"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let metadata = fs::read_to_string(generated_project.join("terrane-build.toml")).unwrap();
    assert!(metadata.contains("path = \"app/main.trn\""));
    assert!(metadata.contains("path = \"support/support.trn\""));
}
