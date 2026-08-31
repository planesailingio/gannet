use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use gannet::archive::{Format, detect_format, discover, extract};
use gannet::platform::Os;

#[cfg(unix)]
const HOST: Os = Os::Macos;
#[cfg(windows)]
const HOST: Os = Os::Windows;

fn make_targz(path: &Path, entries: &[(&str, &[u8], u32)]) {
    let gz = GzEncoder::new(File::create(path).unwrap(), Compression::default());
    let mut tar = tar::Builder::new(gz);
    for (name, data, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        tar.append_data(&mut header, name, *data).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap();
}

fn make_zip(path: &Path, entries: &[(&str, &[u8], Option<u32>)]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, data, mode) in entries {
        let mut opts = zip::write::SimpleFileOptions::default();
        if let Some(mode) = mode {
            opts = opts.unix_permissions(*mode);
        }
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn targz_with_docs_and_nested_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("tool-1.0-linux.tar.gz");
    make_targz(
        &archive,
        &[
            ("tool-1.0/README.md", b"docs", 0o644),
            ("tool-1.0/LICENSE", b"mit", 0o644),
            ("tool-1.0/completions/tool.bash", b"complete", 0o644),
            ("tool-1.0/tool", b"#!binary", 0o755),
        ],
    );
    assert_eq!(
        detect_format("tool-1.0-linux.tar.gz", &archive).unwrap(),
        Format::TarGz
    );
    let staging = tmp.path().join("out");
    fs::create_dir(&staging).unwrap();
    extract(&archive, Format::TarGz, &staging, "tool-1.0-linux.tar.gz").unwrap();
    let found = discover(&staging, HOST, "tool", None).unwrap();
    assert!(found.ends_with("tool-1.0/tool"));
}

#[test]
fn zip_slip_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("evil.zip");
    make_zip(&archive, &[("../evil", b"boom", None)]);
    let staging = tmp.path().join("out");
    fs::create_dir(&staging).unwrap();
    let err = extract(&archive, Format::Zip, &staging, "evil.zip").unwrap_err();
    assert!(err.to_string().contains("unsafe path"), "got: {err}");
    assert!(!tmp.path().join("evil").exists());
}

#[cfg(unix)]
#[test]
fn zip_preserves_exec_bit() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("tool.zip");
    make_zip(
        &archive,
        &[
            ("README.md", b"docs", None),
            ("tool", b"#!binary", Some(0o755)),
        ],
    );
    let staging = tmp.path().join("out");
    fs::create_dir(&staging).unwrap();
    extract(&archive, Format::Zip, &staging, "tool.zip").unwrap();
    let mode = fs::metadata(staging.join("tool"))
        .unwrap()
        .permissions()
        .mode();
    assert_ne!(mode & 0o111, 0);
    let found = discover(&staging, HOST, "tool", None).unwrap();
    assert!(found.ends_with("tool"));
}

#[test]
fn gz_single_file() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("tool-linux-amd64.gz");
    let mut gz = GzEncoder::new(File::create(&archive).unwrap(), Compression::default());
    gz.write_all(b"binary bytes").unwrap();
    gz.finish().unwrap();
    assert_eq!(
        detect_format("tool-linux-amd64.gz", &archive).unwrap(),
        Format::Gz
    );
    let staging = tmp.path().join("out");
    fs::create_dir(&staging).unwrap();
    extract(&archive, Format::Gz, &staging, "tool-linux-amd64.gz").unwrap();
    assert_eq!(
        fs::read(staging.join("tool-linux-amd64")).unwrap(),
        b"binary bytes"
    );
}

#[test]
fn bare_binary_passthrough() {
    let tmp = tempfile::tempdir().unwrap();
    let bare = tmp.path().join("jq-macos-arm64");
    fs::write(&bare, b"\x00binary").unwrap();
    assert_eq!(
        detect_format("jq-macos-arm64", &bare).unwrap(),
        Format::Bare
    );
    let staging = tmp.path().join("out");
    fs::create_dir(&staging).unwrap();
    extract(&bare, Format::Bare, &staging, "jq-macos-arm64").unwrap();
    let found = discover(&staging, HOST, "jq", None).unwrap();
    assert!(found.ends_with("jq-macos-arm64"));
}

#[test]
fn extensionless_gzipped_tarball_is_sniffed() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("mystery");
    make_targz(&archive, &[("tool", b"#!binary", 0o755)]);
    assert_eq!(detect_format("mystery", &archive).unwrap(), Format::TarGz);
}

#[cfg(unix)]
#[test]
fn multiple_executables_require_bin_flag() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("out");
    fs::create_dir(&staging).unwrap();
    for name in ["alpha", "beta"] {
        let p = staging.join(name);
        fs::write(&p, b"bin").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let err = discover(&staging, HOST, "tool", None).unwrap_err();
    assert!(err.to_string().contains("--bin"), "got: {err}");
    let found = discover(&staging, HOST, "tool", Some("beta")).unwrap();
    assert!(found.ends_with("beta"));
}
