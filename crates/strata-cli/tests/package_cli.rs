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
            "strata-cli-package-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("package.toml"),
            "package = \"cli-package\"\nprelude = false\nsources = [\"support.strata\", \"main.strata\"]\n",
        )
        .unwrap();
        fs::write(
            path.join("main.strata"),
            concat!(
                "namespace cli app\n",
                "from /core output import .print\n",
                "print = .print\n",
                "function main\n",
                "  print; 'manifest CLI'\n",
            ),
        )
        .unwrap();
        fs::write(
            path.join("support.strata"),
            "namespace cli support\npublic .value = 1\n",
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
    let executable = env!("CARGO_BIN_EXE_strata");

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
    assert!(generated.contains("// Source: main.strata"));
    assert!(generated.contains("// Namespace: cli app"));

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
}
