use async_fs::File;
use futures_lite::future::{block_on, zip};
use futures_lite::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use std::io::{ErrorKind, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "async-fs-create-new-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn create_new_is_readable_and_writable() {
    block_on(async {
        let dir = TempDir::new();
        let mut file = File::create_new(dir.0.join("new")).await.unwrap();
        let mut contents = String::new();
        assert_eq!(file.read_to_string(&mut contents).await.unwrap(), 0);
        file.write_all(b"created").await.unwrap();
        file.flush().await.unwrap();
        file.seek(SeekFrom::Start(0)).await.unwrap();
        file.read_to_string(&mut contents).await.unwrap();
        assert_eq!(contents, "created");
    });
}

#[test]
fn create_new_preserves_existing_paths() {
    block_on(async {
        let dir = TempDir::new();
        let path = dir.0.join("existing");
        std::fs::write(&path, b"preserved").unwrap();
        assert_eq!(
            File::create_new(&path).await.unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(path).unwrap(), b"preserved");
        assert!(File::create_new(&dir.0).await.is_err());
    });
}

#[test]
fn concurrent_create_new_has_one_winner() {
    block_on(async {
        let dir = TempDir::new();
        let path = dir.0.join("concurrent");
        let (first, second) = zip(File::create_new(&path), File::create_new(&path)).await;
        match (first, second) {
            (Ok(_), Err(error)) | (Err(error), Ok(_)) => {
                assert_eq!(error.kind(), ErrorKind::AlreadyExists);
            }
            _ => panic!("exactly one creation must succeed"),
        }
    });
}

#[cfg(unix)]
#[test]
fn create_new_does_not_follow_symlinks() {
    block_on(async {
        let dir = TempDir::new();
        let target = dir.0.join("target");
        let link = dir.0.join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            File::create_new(&link).await.unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
        assert!(!target.exists());
        std::fs::write(&target, b"preserved").unwrap();
        assert_eq!(
            File::create_new(&link).await.unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(target).unwrap(), b"preserved");
    });
}
