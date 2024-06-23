use std::{env, fs, path::PathBuf, path::Path};
use tempfile::TempDir;

pub struct Fixture {
    path: PathBuf,
    source: PathBuf,
    _tempdir: TempDir,
}

impl Fixture {
    pub fn blank(fixture_filename: &str) -> Self {
        // First, figure out the right file in `tests/fixtures/`:
        let root_dir = &env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
        let mut source = PathBuf::from(root_dir);
        source.push("tests/fixtures");
        source.push(fixture_filename);

        // The "real" path of the file is going to be under a temporary directory:
        let tempdir = tempfile::tempdir().unwrap();
        let mut path = PathBuf::from(&tempdir.path());
        path.push(fixture_filename);

        Fixture { _tempdir: tempdir, source, path }
    }

    pub fn copy(fixture_filename: &str) -> Self {
        let fixture = Fixture::blank(fixture_filename);
        fs::copy(&fixture.source, &fixture.path).unwrap();
        fixture
    }

    pub fn to_str(&self) -> &str
    {
        self.path.to_str().unwrap()
    }

    pub fn to_path(&self) -> &Path
    {
        self.path.as_path()
    }
}
