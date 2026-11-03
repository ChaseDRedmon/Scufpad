use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT: AtomicU64 = AtomicU64::new(0);
static FIXTURES: Mutex<()> = Mutex::new(());

pub struct Fixture {
    pub root: PathBuf,
    files: RefCell<Vec<PathBuf>>,
    directories: RefCell<Vec<PathBuf>>,
    _serial: MutexGuard<'static, ()>,
}

impl Fixture {
    pub fn new() -> Self {
        // Endpoint creation sets umask before production threads start. Keep filesystem
        // fixtures serialized so parallel tests preserve that startup invariant.
        let serial = FIXTURES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = PathBuf::from("/tmp/opencode").join(format!(
            "scufpad-pure-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self {
            root,
            files: RefCell::new(Vec::new()),
            directories: RefCell::new(Vec::new()),
            _serial: serial,
        }
    }
    pub fn directory(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        self.ensure_directory(&path);
        path
    }
    fn ensure_directory(&self, path: &Path) {
        if path == self.root || path.exists() {
            return;
        }
        self.ensure_directory(path.parent().unwrap());
        fs::create_dir(path).unwrap();
        self.directories.borrow_mut().push(path.to_path_buf());
    }
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        self.ensure_directory(path.parent().unwrap());
        fs::write(&path, bytes).unwrap();
        self.track(path.clone());
        path
    }
    pub fn track(&self, path: PathBuf) {
        self.files.borrow_mut().push(path);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in self.files.get_mut().iter().rev() {
            let _ = fs::remove_file(path);
        }
        for path in self.directories.get_mut().iter().rev() {
            let _ = fs::remove_dir(path);
        }
        let _ = fs::remove_dir(&self.root);
    }
}
