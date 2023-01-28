use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

enum mode {
  Json,
  Binary,
}

// Debug only
const FS_MODE: mode = mode::Binary;

fn deserialize<T: for<'de> Deserialize<'de>>(c: &Vec<u8>) -> Result<T, String> {
  match FS_MODE {
    mode::Json => serde_json::from_slice(c).map_err(|e| e.to_string()),
    mode::Binary => bincode::deserialize(&c).map_err(|e| e.to_string()),
  }
}

fn serialize(data: impl Serialize) -> Result<Vec<u8>, String> {
  match FS_MODE {
    mode::Json => serde_json::to_vec(&data).map_err(|e| e.to_string()),
    mode::Binary => bincode::serialize(&data).map_err(|e| e.to_string()),
  }
}

fn serialize_into(
  file: impl std::io::Write,
  append_data: impl Serialize,
) -> Result<(), String> {
  match FS_MODE {
    mode::Json => {
      serde_json::to_writer(file, &append_data).map_err(|e| e.to_string())
    }
    mode::Binary => {
      bincode::serialize_into(file, &append_data).map_err(|e| e.to_string())
    }
  }
}

fn deserialize_from<T: for<'de> Deserialize<'de>>(
  f: impl std::io::Read,
) -> Result<T, String> {
  match FS_MODE {
    mode::Json => serde_json::from_reader(f).map_err(|e| e.to_string()),
    mode::Binary => bincode::deserialize_from(f).map_err(|e| e.to_string()),
  }
}

pub fn binary_read<T: for<'de> Deserialize<'de>>(
  path: PathBuf,
) -> Result<T, String> {
  // Try open staging
  let mut file = OpenOptions::new()
    .read(true)
    .open(&path)
    .map_err(|_| format!("No binary file found: {:?}", &path))?;
  let mut contents = vec![];
  file.read_to_end(&mut contents).map_err(|e| e.to_string())?;
  deserialize(&contents)
}

pub fn binary_continuous_read<T: for<'de> Deserialize<'de>>(
  path: PathBuf,
) -> Result<Vec<T>, String> {
  // Try open staging
  let mut res: Vec<T> = Vec::new();
  let mut f = std::fs::File::open(&path)
    .map_err(|_| format!("No binary file found: {:?}", path))?;
  f.seek(SeekFrom::Current(0)).unwrap();
  let mut i = 0;
  loop {
    println!("{}", i);
    i += 1;
    match deserialize_from(&f) {
      Ok(r) => res.push(r),
      Err(_) => {
        break;
      }
    }
  }
  Ok(res)
}

pub fn binary_update<T: Serialize + core::fmt::Debug>(
  path: PathBuf,
  data: T,
) -> Result<(), String> {
  let mut file = OpenOptions::new()
    .write(true)
    .open(&path)
    .map_err(|_| format!("No bin file found to update: {:?}", &path))?;
  file
    .write_all(&serialize(data)?)
    .map_err(|e| e.to_string())?;
  file.flush().map_err(|e| e.to_string())?;
  Ok(())
}

pub fn binary_continuous_append<T: Serialize>(
  path: PathBuf,
  append_data: T,
) -> Result<(), String> {
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .append(true)
    .open(&path)
    .map_err(|_| format!("No continuous file found to append: {:?}", &path))?;
  serialize_into(&file, &append_data)?;
  file.flush().map_err(|e| e.to_string())?;
  Ok(())
}

pub fn binary_init<
  T: Serialize + for<'de> Deserialize<'de> + core::fmt::Debug,
>(
  path: PathBuf,
  init_data: T,
) -> Result<T, String> {
  // Get file parent folder
  let parent = path.parent().unwrap();
  // Create parent dirs
  std::fs::create_dir_all(parent)
    .map_err(|_| format!("Error creating file parent folder: {:?}", &path))?;
  std::fs::File::create(&path)
    .map_err(|_| format!("Error creating file with path: {:?}", &path))?;
  binary_update(path.clone(), init_data)?;
  let res = binary_read(path)?;
  Ok(res)
}

pub fn binary_init_empty(path: PathBuf) -> Result<(), String> {
  // Get file parent folder
  let parent = path.parent().unwrap();
  // Create parent dirs
  std::fs::create_dir_all(parent)
    .map_err(|_| format!("Error creating file parent folder: {:?}", &path))?;
  std::fs::File::create(&path)
    .map_err(|_| format!("Error creating file with path: {:?}", &path))?;
  Ok(())
}
