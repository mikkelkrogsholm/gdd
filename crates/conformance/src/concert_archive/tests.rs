use super::*;
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use tar::{Builder, EntryType, Header};

const DATA: &[u8] = b"test content\n";
const BLOB: &str = "d670460b4b4aece5915caf5c68d12f560a9fe3e4";

fn member(tar: &mut Builder<Vec<u8>>, name: &str, kind: EntryType, mode: u32, link: bool) {
    let mut header = Header::new_ustar();
    // Assign the bytes directly so lexical aliases reach the reader verbatim.
    header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
    header.set_entry_type(kind);
    header.set_size(DATA.len() as u64);
    header.set_mode(mode);
    if link {
        header.set_link_name("target").unwrap();
    }
    header.set_cksum();
    tar.append(&header, DATA).unwrap();
}

fn compress(bytes: &[u8]) -> Vec<u8> {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(bytes).unwrap();
    gzip.finish().unwrap()
}

struct Fixture {
    directory: tempfile::TempDir,
    archive: PathBuf,
    index: PathBuf,
    document: Value,
}

impl Fixture {
    fn new(names: &[&str], build: impl FnOnce(&mut Builder<Vec<u8>>)) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let archive = directory.path().join("bundle.tar.gz");
        let index = directory.path().join("index.json");
        let mut tar = Builder::new(Vec::new());
        build(&mut tar);
        let raw = compress(&tar.into_inner().unwrap());
        fs::write(&archive, &raw).unwrap();
        let files: Vec<_> = names
            .iter()
            .map(|name| {
                json!({
                    "original_path": name, "retrieval_path": name, "size": DATA.len(),
                    "git_blob": BLOB, "git_mode": "100755", "sha256": gdd::sha256(DATA),
                })
            })
            .collect();
        let document = json!({
            "format": FORMAT, "source": {"git_commit": "fixture"},
            "archive": {"sha256": gdd::sha256(&raw), "size": raw.len()}, "files": files
        });
        let fixture = Self {
            directory,
            archive,
            index,
            document,
        };
        fixture.save_index();
        fixture
    }

    fn simple() -> Self {
        Self::new(&["file"], |tar| {
            member(tar, "file", EntryType::Regular, 0o755, false)
        })
    }

    fn save_index(&self) {
        fs::write(&self.index, serde_json::to_vec(&self.document).unwrap()).unwrap();
    }

    fn output(&self) -> PathBuf {
        self.directory
            .path()
            .canonicalize()
            .unwrap()
            .join("restored")
    }

    fn assert_refused(&self) {
        assert!(restore(&self.archive, &self.index, &self.output()).is_err());
        assert!(!self.output().exists());
    }
}

#[test]
fn historical_receipt_and_all_restored_files_match_the_index() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../archive/experiments");
    let archive = directory.join("concert-7e27757.tar.gz");
    let index = directory.join("concert-7e27757.index.json");
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().canonicalize().unwrap().join("recovered");
    let receipt = restore(&archive, &index, &output).unwrap();
    assert_eq!(
        serde_json::to_value(receipt).unwrap(),
        json!({
            "format": FORMAT, "verified": true, "git_commit": "7e27757c34b6fa57ecc69660e263c99e1420649b",
            "archive_sha256": "4888e79870a157e9521e2a5ef226bcbd076fda73c45e2c3c7ee9291d34b50d2d",
            "files": 197, "recovered_bytes": 7443413, "output": output
        })
    );
    let index: Index = serde_json::from_slice(&fs::read(index).unwrap()).unwrap();
    for entry in index.files {
        let file = output.join(entry.original_path);
        let bytes = fs::read(&file).unwrap();
        assert_eq!(bytes.len() as u64, entry.size);
        assert_eq!(gdd::sha256(&bytes), entry.sha256);
        #[cfg(unix)]
        assert_eq!(
            file.metadata().unwrap().permissions().mode() & 0o7777,
            entry.git_mode.permissions()
        );
    }
}

#[test]
fn pax_name_mode_and_receipt_survive_input_replacement() {
    let name = format!("{}/exécuter", "directory/".repeat(12).trim_end_matches('/'));
    let fixture = Fixture::new(&[&name], |tar| {
        tar.append_pax_extensions([("path", name.as_bytes())])
            .unwrap();
        member(tar, "placeholder", EntryType::Regular, 0o755, false);
    });
    let verified = verified_files(&fixture.archive, &fixture.index).unwrap();
    fs::write(&fixture.archive, []).unwrap();
    fs::write(&fixture.index, []).unwrap();
    let output = fixture.output();
    let receipt = verified.restore(&output).unwrap();
    assert_eq!(
        receipt.archive_sha256,
        fixture.document["archive"]["sha256"]
    );
    assert_eq!(receipt.files, 1);
    let file = output.join(name);
    assert_eq!(fs::read(&file).unwrap(), DATA);
    #[cfg(unix)]
    assert_eq!(
        file.metadata().unwrap().permissions().mode() & 0o7777,
        0o755
    );
}

#[test]
fn applies_pax_size_override_and_preserves_gnu_long_name() {
    let name = "long-name".repeat(13);
    let fixture = Fixture::new(&[&name], |tar| {
        tar.append_pax_extensions([("size", DATA.len().to_string().as_bytes())])
            .unwrap();
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::GNULongName);
        header.set_size((name.len() + 1) as u64);
        header.set_cksum();
        tar.append(&header, format!("{name}\0").as_bytes()).unwrap();
        header.set_entry_type(EntryType::Regular);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append(&header, DATA).unwrap();
    });
    let verified = verified_files(&fixture.archive, &fixture.index).unwrap();
    assert_eq!(verified.files[0].0.original_path, name);
    assert_eq!(verified.files[0].1, DATA);
}

#[test]
fn rejects_malformed_linked_duplicate_and_sparse_pax_metadata() {
    for records in [
        b"13 path=file\n13 path=file\n".as_slice(),
        b"19 linkpath=target\n",
        b"26 SCHILY.filetype=sparse\n",
        b"21 GNU.sparse.size=0\n",
        b"999 path=file\n",
        b"13 path=file!",
    ] {
        let fixture = Fixture::new(&["file"], |tar| {
            let mut header = Header::new_ustar();
            header.set_entry_type(EntryType::XHeader);
            header.set_size(records.len() as u64);
            header.set_cksum();
            tar.append(&header, records).unwrap();
            member(tar, "file", EntryType::Regular, 0o755, false);
        });
        fixture.assert_refused();
    }
}

#[test]
fn check_and_restore_accept_opaque_descriptive_index_fields() {
    let mut fixture = Fixture::simple();
    fixture.document["source"]["git_tree"] = Value::Null;
    fixture.document["source"]["git_path"] = json!({"note": "legacy metadata"});
    fixture.document["archive"]["path"] = Value::Null;
    fixture.save_index();
    assert_eq!(
        verified_files(&fixture.archive, &fixture.index)
            .unwrap()
            .receipt
            .files,
        1
    );
    assert_eq!(
        restore(&fixture.archive, &fixture.index, &fixture.output())
            .unwrap()
            .files,
        1
    );
}

#[test]
fn rejects_changed_hashes_sizes_paths_and_index_sets() {
    for (field, invalid) in [
        ("sha256", json!("0".repeat(64))),
        ("git_blob", json!("0".repeat(40))),
        ("git_mode", json!("100644")),
        ("git_mode", json!("120000")),
        ("git_mode", json!({"100755": null})),
        ("size", json!(DATA.len() - 1)),
        ("retrieval_path", json!("alias")),
    ] {
        let mut fixture = Fixture::simple();
        fixture.document["files"][0][field] = invalid;
        fixture.save_index();
        fixture.assert_refused();
    }
    for (field, invalid) in [("sha256", json!("0".repeat(64))), ("size", json!(0))] {
        let mut fixture = Fixture::simple();
        fixture.document["archive"][field] = invalid;
        fixture.save_index();
        fixture.assert_refused();
    }
    for second in ["file", "file/child", "missing"] {
        Fixture::new(&["file", second], |tar| {
            member(tar, "file", EntryType::Regular, 0o755, false)
        })
        .assert_refused();
    }
}

#[test]
fn canonical_paths_are_checked_in_both_index_and_tar() {
    for name in [
        "",
        ".",
        "./file",
        "/file",
        "a/../file",
        "..",
        "a//b",
        "a/",
        "a\\b",
        "a\0b",
    ] {
        assert!(check_path(name).is_err(), "{name:?}");
        let fixture = Fixture::new(&[name], |tar| {
            member(tar, "file", EntryType::Regular, 0o755, false)
        });
        fixture.assert_refused();
    }
    for name in ["./file", "/file", "a/../file", "a\\b", "file/"] {
        Fixture::new(&["file"], |tar| {
            member(tar, name, EntryType::Regular, 0o755, false)
        })
        .assert_refused();
    }
    #[cfg(unix)]
    {
        let fixture = Fixture::new(&["a:book"], |tar| {
            member(tar, "a:book", EntryType::Regular, 0o755, false)
        });
        restore(&fixture.archive, &fixture.index, &fixture.output()).unwrap();
        assert_eq!(fs::read(fixture.output().join("a:book")).unwrap(), DATA);
    }
}

#[test]
fn rejects_links_nonregular_members_duplicates_and_special_mode_bits() {
    for (kind, mode, link) in [
        (EntryType::Symlink, 0o755, true),
        (EntryType::Link, 0o755, true),
        (EntryType::Directory, 0o755, false),
        (EntryType::Fifo, 0o755, false),
        (EntryType::Regular, 0o755, true),
        (EntryType::Regular, 0o4755, false),
    ] {
        Fixture::new(&["file"], |tar| member(tar, "file", kind, mode, link)).assert_refused();
    }
    for extra in ["file", "extra"] {
        Fixture::new(&["file"], |tar| {
            member(tar, "file", EntryType::Regular, 0o755, false);
            member(tar, extra, EntryType::Regular, 0o755, false);
        })
        .assert_refused();
    }
}

#[test]
fn rejects_corrupt_gzip_trailer_even_with_updated_index() {
    let mut fixture = Fixture::simple();
    let mut raw = fs::read(&fixture.archive).unwrap();
    let crc_offset = raw.len() - 8;
    raw[crc_offset] ^= 1;
    fixture.document["archive"]["sha256"] = gdd::sha256(&raw).into();
    fs::write(&fixture.archive, raw).unwrap();
    fixture.save_index();
    fixture.assert_refused();
}

#[test]
fn refuses_existing_output_and_missing_or_linked_parents() {
    let fixture = Fixture::simple();
    let root = fixture.directory.path().canonicalize().unwrap();
    let existing = root.join("existing");
    fs::create_dir(&existing).unwrap();
    let sentinel = existing.join("sentinel");
    fs::write(&sentinel, b"keep").unwrap();
    for output in [&existing, &sentinel, &root.join("missing/child")] {
        assert!(restore(&fixture.archive, &fixture.index, output).is_err());
    }
    #[cfg(unix)]
    {
        let link = root.join("link");
        let broken = root.join("broken");
        std::os::unix::fs::symlink(&existing, &link).unwrap();
        std::os::unix::fs::symlink(root.join("absent"), &broken).unwrap();
        for output in [&link, &broken, &link.join("child"), &link.join("../new")] {
            assert!(restore(&fixture.archive, &fixture.index, output).is_err());
        }
        assert!(!existing.join("child").exists());
    }
    assert_eq!(fs::read(sentinel).unwrap(), b"keep");
}
