use std::borrow::Cow;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{de::Deserializer, de::Error, ser::Serializer};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct MyBytesHelper<'a> {
    path: Cow<'a, Path>,
    bytes: Cow<'a, [u8]>,
}

#[derive(Clone, PartialEq, Debug)]
struct MyBytes {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl MyBytes {
    pub fn open<P: AsRef<Path>>(p: P) -> io::Result<MyBytes> {
        println!("opening file in {}", std::process::id());
        let path = p.as_ref().to_path_buf();
        Ok(MyBytes {
            bytes: fs::read(&path)?,
            path,
        })
    }
}

#[cfg(unix)]
fn serialize_ipc_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use std::os::unix::ffi::OsStrExt;

    serializer.serialize_bytes(path.as_os_str().as_bytes())
}

#[cfg(not(unix))]
fn serialize_ipc_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let path = path
        .to_str()
        .ok_or_else(|| <S::Error as serde::ser::Error>::custom("path is not valid UTF-8"))?;
    serializer.serialize_str(path)
}

#[cfg(unix)]
fn deserialize_ipc_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(OsString::from_vec(Vec::deserialize(
        deserializer,
    )?)))
}

#[cfg(not(unix))]
fn deserialize_ipc_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(PathBuf::from(String::deserialize(deserializer)?))
}

impl Serialize for MyBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if procspawn::serde::in_ipc_mode() {
            println!("serialize in ipc mode");
            serialize_ipc_path(&self.path, serializer)
        } else {
            println!("serialize in normal mode");
            MyBytesHelper {
                path: Cow::Borrowed(&self.path),
                bytes: Cow::Borrowed(&self.bytes),
            }
            .serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for MyBytes {
    fn deserialize<D>(deserializer: D) -> Result<MyBytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        if procspawn::serde::in_ipc_mode() {
            println!("deserialize in ipc mode");
            let path = deserialize_ipc_path(deserializer)?;
            MyBytes::open(path).map_err(D::Error::custom)
        } else {
            println!("deserialize in normal mode");
            let helper = MyBytesHelper::deserialize(deserializer)?;
            Ok(MyBytes {
                path: helper.path.into_owned(),
                bytes: helper.bytes.into_owned(),
            })
        }
    }
}

fn main() {
    procspawn::init();

    let bytes = MyBytes::open("Cargo.toml").unwrap();

    let bytes_two = procspawn::spawn!((bytes.clone() => bytes) || {
        println!("length: {}", bytes.bytes.len());
        bytes
    })
    .join()
    .unwrap();

    assert_eq!(bytes, bytes_two);
}
